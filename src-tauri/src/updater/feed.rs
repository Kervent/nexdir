//! Update feeds: one small JSON file per release channel, generated from the published GitHub
//! releases by `tools/release/update_feed.py` and served from the `update-feed` branch.
//!
//! The feed is fetched over HTTPS but is not signed itself, so everything in it is treated as
//! untrusted: the version must be newer than the running one, download addresses must point at
//! this repository's release files for exactly that version, and the installer's own signature
//! (checked by the Tauri updater with `requireSignedVersion`) must name the same version.
//! A tampered feed can at worst announce a release that then fails verification.

use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Feed format understood by this version.
pub const SCHEMA: u32 = 1;
/// Larger answers are rejected before parsing (a real feed is a few KB).
pub const MAX_FEED_BYTES: usize = 256 * 1024;
/// A minisign signature as written by the Tauri CLI is well below 1 KB (base64 of ~400 bytes).
const MAX_SIGNATURE_LEN: usize = 4 * 1024;
const MAX_NOTES_LEN: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Channel {
    /// Finished releases only (no pre-release identifier, not marked pre-release on GitHub).
    Stable,
    /// Alpha and beta releases, plus stable releases when they are newer.
    Preview,
}

impl Channel {
    pub fn file_name(self) -> &'static str {
        match self {
            Channel::Stable => "stable.json",
            Channel::Preview => "preview.json",
        }
    }

    /// The channel a fresh installation starts on: pre-release builds follow the preview
    /// channel (otherwise their users would never hear of the next alpha), others stable.
    pub fn default_for(version: &Version) -> Channel {
        if version.pre.is_empty() {
            Channel::Stable
        } else {
            Channel::Preview
        }
    }
}

/// How this copy of Nexdir was installed; decides which installer an update uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallKind {
    /// Per-user NSIS setup (`Nexdir-<v>-Setup.exe`).
    Nsis,
    /// Windows Installer package (`Nexdir-<v>.msi`).
    Msi,
    /// Unpacked ZIP or a development build: never updated in place.
    Portable,
}

impl InstallKind {
    /// Key of this installer in the feed's `platforms` object (the Tauri updater's
    /// `{os}-{arch}-{bundle_type}` target). Portable copies have none on purpose.
    pub fn platform_key(self) -> Option<&'static str> {
        match self {
            InstallKind::Nsis => Some("windows-x86_64-nsis"),
            InstallKind::Msi => Some("windows-x86_64-msi"),
            InstallKind::Portable => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeedPlatform {
    pub url: String,
    pub signature: String,
    #[serde(default)]
    pub size: Option<u64>,
}

/// The raw feed document. Unknown fields are ignored so later feed versions can add some.
#[derive(Debug, Clone, Deserialize)]
pub struct Feed {
    pub schema: u32,
    pub channel: Channel,
    /// `null` when the channel has no release yet (the stable channel until 1.0).
    pub version: Option<String>,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub pub_date: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub release_url: Option<String>,
    #[serde(default)]
    pub platforms: HashMap<String, FeedPlatform>,
}

/// A release from the feed that passed every check and is newer than the running version.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Offer {
    pub version: String,
    pub prerelease: bool,
    pub pub_date: Option<String>,
    pub notes: String,
    pub release_url: String,
    /// Download size of the installer this copy would use, when the feed states it.
    pub size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// The channel has no release at all yet.
    NoRelease,
    /// The newest release in the channel is not newer than the running version. `latest` is
    /// that release's version (it can be older, e.g. stable 1.0 while running 1.1-beta).
    UpToDate {
        latest: String,
    },
    Available(Offer),
}

#[derive(Debug, Clone, PartialEq)]
pub enum FeedError {
    /// Not JSON, wrong shape, too large or a wrong/unknown schema.
    Malformed(String),
    /// The document claims to be another channel's feed.
    WrongChannel,
    InvalidVersion(String),
    /// A pre-release in the stable feed.
    PrereleaseInStable,
    /// A download address, release page or signature that does not belong to this release.
    Untrusted(String),
    /// The release has no installer for this kind of installation.
    MissingInstaller(&'static str),
}

impl std::fmt::Display for FeedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeedError::Malformed(m) => write!(f, "The update information is malformed ({m})."),
            FeedError::WrongChannel => write!(f, "The update information belongs to another release channel."),
            FeedError::InvalidVersion(v) => write!(f, "The update information contains an invalid version number ({v})."),
            FeedError::PrereleaseInStable => write!(f, "The stable channel announced a pre-release; it was ignored."),
            FeedError::Untrusted(m) => write!(f, "The update information was rejected: {m}."),
            FeedError::MissingInstaller(k) => write!(f, "The new release has no update package for this installation ({k})."),
        }
    }
}

/// Where release files may be downloaded from, and which page describes a release.
#[derive(Debug, Clone)]
pub struct Trust {
    /// e.g. `https://github.com/Kervent/nexdir/releases/download/`
    pub download_prefix: String,
    /// e.g. `https://github.com/Kervent/nexdir/releases/tag/`
    pub release_page_prefix: String,
}

pub fn parse_version(raw: &str) -> Result<Version, FeedError> {
    let v = raw.trim();
    // Exactly what the release workflow produces: no "v" prefix, no build metadata.
    match Version::parse(v) {
        Ok(version) if version.build.is_empty() && v.len() <= 64 => Ok(version),
        _ => Err(FeedError::InvalidVersion(raw.chars().take(64).collect())),
    }
}

/// Parses and checks a feed document for `channel`, compared with the running `current`
/// version, for an installation of `kind`. Pure function: no I/O.
pub fn evaluate(bytes: &[u8], channel: Channel, current: &Version, kind: InstallKind, trust: &Trust) -> Result<Outcome, FeedError> {
    if bytes.len() > MAX_FEED_BYTES {
        return Err(FeedError::Malformed("too large".into()));
    }
    let feed: Feed = serde_json::from_slice(bytes).map_err(|e| FeedError::Malformed(e.classify_name()))?;
    if feed.schema != SCHEMA {
        return Err(FeedError::Malformed(format!("unsupported format {}", feed.schema)));
    }
    if feed.channel != channel {
        return Err(FeedError::WrongChannel);
    }
    let Some(raw_version) = feed.version.as_deref() else {
        return Ok(Outcome::NoRelease);
    };
    let version = parse_version(raw_version)?;
    if channel == Channel::Stable && (feed.prerelease || !version.pre.is_empty()) {
        return Err(FeedError::PrereleaseInStable);
    }
    // Never offer a version that is not strictly newer: no downgrades, no reinstalling.
    if version <= *current {
        return Ok(Outcome::UpToDate {
            latest: version.to_string(),
        });
    }

    let tag = format!("v{version}");
    let release_url = feed.release_url.clone().unwrap_or_default();
    if release_url != format!("{}{tag}", trust.release_page_prefix) {
        return Err(FeedError::Untrusted("unexpected release page".into()));
    }
    if feed.pub_date.as_deref().is_some_and(|d| d.len() > 64) {
        return Err(FeedError::Malformed("date".into()));
    }
    let notes = feed.notes.clone().unwrap_or_default();
    if notes.len() > MAX_NOTES_LEN {
        return Err(FeedError::Malformed("release notes too long".into()));
    }

    // Every installer listed must be one of ours for exactly this version, even the ones this
    // copy does not use: a feed with any foreign address is not trusted at all.
    let download_prefix = format!("{}{tag}/", trust.download_prefix);
    for (key, platform) in &feed.platforms {
        if !matches!(key.as_str(), "windows-x86_64-nsis" | "windows-x86_64-msi") {
            return Err(FeedError::Untrusted(format!(
                "unknown package type {}",
                key.chars().take(40).collect::<String>()
            )));
        }
        check_platform(platform, &download_prefix)?;
    }

    let size = match kind.platform_key() {
        Some(key) => Some(feed.platforms.get(key).ok_or(FeedError::MissingInstaller(key))?.size),
        // Portable copies only need to know that a release exists.
        None => None,
    }
    .flatten();

    Ok(Outcome::Available(Offer {
        version: version.to_string(),
        prerelease: feed.prerelease || !version.pre.is_empty(),
        pub_date: feed.pub_date,
        notes,
        release_url,
        size,
    }))
}

fn check_platform(platform: &FeedPlatform, download_prefix: &str) -> Result<(), FeedError> {
    let url = platform.url.as_str();
    let file = url
        .strip_prefix(download_prefix)
        .ok_or_else(|| FeedError::Untrusted("download address outside the release".into()))?;
    let plain = !file.is_empty() && file.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'));
    if !plain || file.contains("..") || !(file.ends_with(".exe") || file.ends_with(".msi")) {
        return Err(FeedError::Untrusted("unexpected file name".into()));
    }
    let sig = platform.signature.trim();
    if sig.is_empty() {
        return Err(FeedError::Untrusted("missing signature".into()));
    }
    if sig.len() > MAX_SIGNATURE_LEN || !sig.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=')) {
        return Err(FeedError::Untrusted("malformed signature".into()));
    }
    Ok(())
}

/// The feed rewritten into the Tauri updater's static manifest format (version, notes,
/// pub_date, platforms with url and signature), for `Updater::restore_update`.
pub fn updater_manifest(bytes: &[u8]) -> Result<serde_json::Value, FeedError> {
    let feed: Feed = serde_json::from_slice(bytes).map_err(|e| FeedError::Malformed(e.classify_name()))?;
    let platforms: serde_json::Map<String, serde_json::Value> = feed
        .platforms
        .iter()
        .map(|(k, p)| (k.clone(), serde_json::json!({ "url": p.url, "signature": p.signature.trim() })))
        .collect();
    let mut manifest = serde_json::json!({
        "version": feed.version,
        "notes": feed.notes,
        "platforms": platforms,
    });
    if let Some(date) = feed.pub_date {
        manifest["pub_date"] = serde_json::Value::String(date);
    }
    Ok(manifest)
}

trait ClassifyName {
    fn classify_name(&self) -> String;
}

impl ClassifyName for serde_json::Error {
    /// Short reason without echoing feed content back into the UI.
    fn classify_name(&self) -> String {
        use serde_json::error::Category;
        match self.classify() {
            Category::Io => "read error".into(),
            Category::Syntax | Category::Eof => "not valid JSON".into(),
            Category::Data => "unexpected content".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIG: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkK";

    fn trust() -> Trust {
        Trust {
            download_prefix: "https://github.com/Kervent/nexdir/releases/download/".into(),
            release_page_prefix: "https://github.com/Kervent/nexdir/releases/tag/".into(),
        }
    }

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    fn feed(channel: &str, version: &str, prerelease: bool) -> String {
        let base = "https://github.com/Kervent/nexdir/releases/download";
        serde_json::json!({
            "schema": 1,
            "channel": channel,
            "version": version,
            "prerelease": prerelease,
            "pub_date": "2026-11-20T12:00:00Z",
            "notes": "Fixes",
            "release_url": format!("https://github.com/Kervent/nexdir/releases/tag/v{version}"),
            "platforms": {
                "windows-x86_64-nsis": { "url": format!("{base}/v{version}/Nexdir-{version}-Setup.exe"), "signature": SIG, "size": 2_600_000 },
                "windows-x86_64-msi": { "url": format!("{base}/v{version}/Nexdir-{version}.msi"), "signature": SIG, "size": 3_500_000 }
            }
        })
        .to_string()
    }

    fn eval(doc: &str, channel: Channel, current: &str, kind: InstallKind) -> Result<Outcome, FeedError> {
        evaluate(doc.as_bytes(), channel, &v(current), kind, &trust())
    }

    #[test]
    fn offers_a_newer_preview_release() {
        let out = eval(
            &feed("preview", "0.5.0-alpha.2", true),
            Channel::Preview,
            "0.5.0-alpha.1",
            InstallKind::Nsis,
        )
        .unwrap();
        let Outcome::Available(offer) = out else {
            panic!("expected an offer: {out:?}")
        };
        assert_eq!(offer.version, "0.5.0-alpha.2");
        assert!(offer.prerelease);
        assert_eq!(offer.size, Some(2_600_000));
        assert_eq!(offer.release_url, "https://github.com/Kervent/nexdir/releases/tag/v0.5.0-alpha.2");
    }

    #[test]
    fn picks_the_size_of_the_matching_installer() {
        let Outcome::Available(offer) = eval(&feed("stable", "1.0.0", false), Channel::Stable, "0.9.0", InstallKind::Msi).unwrap() else {
            panic!()
        };
        assert_eq!(offer.size, Some(3_500_000));
        let Outcome::Available(offer) = eval(&feed("stable", "1.0.0", false), Channel::Stable, "0.9.0", InstallKind::Portable).unwrap()
        else {
            panic!()
        };
        assert_eq!(offer.size, None);
    }

    #[test]
    fn same_or_older_versions_are_never_offered() {
        for current in ["0.5.0-alpha.2", "0.5.0-beta.1", "0.5.0", "1.0.0"] {
            let out = eval(
                &feed("preview", "0.5.0-alpha.2", true),
                Channel::Preview,
                current,
                InstallKind::Nsis,
            )
            .unwrap();
            assert_eq!(
                out,
                Outcome::UpToDate {
                    latest: "0.5.0-alpha.2".into()
                },
                "running {current}"
            );
        }
    }

    #[test]
    fn semver_precedence_including_prerelease_identifiers() {
        let order = [
            "0.4.3",
            "0.5.0-alpha.1",
            "0.5.0-alpha.2",
            "0.5.0-alpha.10",
            "0.5.0-beta",
            "0.5.0-beta.2",
            "0.5.0-rc.1",
            "0.5.0",
            "0.5.1",
            "1.0.0-alpha.1",
            "1.0.0",
        ];
        for pair in order.windows(2) {
            assert!(v(pair[0]) < v(pair[1]), "{} < {}", pair[0], pair[1]);
            let out = eval(&feed("preview", pair[1], true), Channel::Preview, pair[0], InstallKind::Nsis).unwrap();
            assert!(matches!(out, Outcome::Available(_)), "{} -> {}", pair[0], pair[1]);
            let out = eval(&feed("preview", pair[0], true), Channel::Preview, pair[1], InstallKind::Nsis).unwrap();
            assert!(
                matches!(out, Outcome::UpToDate { .. }),
                "{} must not go back to {}",
                pair[1],
                pair[0]
            );
        }
    }

    #[test]
    fn empty_stable_channel_reports_no_release() {
        let doc = r#"{"schema":1,"channel":"stable","version":null}"#;
        assert_eq!(
            eval(doc, Channel::Stable, "0.5.0-alpha.1", InstallKind::Nsis).unwrap(),
            Outcome::NoRelease
        );
    }

    #[test]
    fn stable_feed_with_a_prerelease_is_rejected() {
        assert_eq!(
            eval(&feed("stable", "0.6.0-beta.1", false), Channel::Stable, "0.5.0", InstallKind::Nsis),
            Err(FeedError::PrereleaseInStable)
        );
        assert_eq!(
            eval(&feed("stable", "0.6.0", true), Channel::Stable, "0.5.0", InstallKind::Nsis),
            Err(FeedError::PrereleaseInStable)
        );
    }

    #[test]
    fn feed_of_another_channel_is_rejected() {
        assert_eq!(
            eval(&feed("preview", "0.6.0", false), Channel::Stable, "0.5.0", InstallKind::Nsis),
            Err(FeedError::WrongChannel)
        );
    }

    #[test]
    fn malformed_documents_are_rejected() {
        for doc in [
            "",
            "not json",
            "[]",
            "{}",
            r#"{"schema":2,"channel":"stable","version":null}"#,
            r#"{"schema":1,"channel":"nightly","version":null}"#,
            r#"{"schema":1,"channel":"stable","version":5}"#,
        ] {
            assert!(
                matches!(eval(doc, Channel::Stable, "0.5.0", InstallKind::Nsis), Err(FeedError::Malformed(_))),
                "{doc:?}"
            );
        }
        let huge = format!(
            r#"{{"schema":1,"channel":"stable","version":null,"x":"{}"}}"#,
            "a".repeat(MAX_FEED_BYTES)
        );
        assert!(matches!(
            eval(&huge, Channel::Stable, "0.5.0", InstallKind::Nsis),
            Err(FeedError::Malformed(_))
        ));
    }

    #[test]
    fn invalid_versions_are_rejected() {
        for bad in ["v0.6.0", "0.6", "latest", "0.6.0+build.5", "", "1.0.0-"] {
            let doc = feed("preview", "0.6.0", true).replace("\"version\":\"0.6.0\"", &format!("\"version\":{}", serde_json::json!(bad)));
            assert!(
                matches!(
                    eval(&doc, Channel::Preview, "0.5.0", InstallKind::Nsis),
                    Err(FeedError::InvalidVersion(_))
                ),
                "{bad}"
            );
        }
    }

    #[test]
    fn foreign_download_addresses_are_rejected() {
        let good = feed("preview", "0.6.0", true);
        for (from, to) in [
            (
                "https://github.com/Kervent/nexdir/releases/download/v0.6.0/Nexdir-0.6.0-Setup.exe",
                "https://evil.example/Nexdir-0.6.0-Setup.exe",
            ),
            (
                "https://github.com/Kervent/nexdir/releases/download/v0.6.0/Nexdir-0.6.0-Setup.exe",
                "http://github.com/Kervent/nexdir/releases/download/v0.6.0/Nexdir-0.6.0-Setup.exe",
            ),
            (
                "https://github.com/Kervent/nexdir/releases/download/v0.6.0/Nexdir-0.6.0-Setup.exe",
                "https://github.com/Kervent/nexdir/releases/download/v0.5.0/Nexdir-0.5.0-Setup.exe",
            ),
            (
                "https://github.com/Kervent/nexdir/releases/download/v0.6.0/Nexdir-0.6.0-Setup.exe",
                "https://github.com/Kervent/nexdir/releases/download/v0.6.0/../v0.4.3/Nexdir-0.4.3-Setup.exe",
            ),
            (
                "https://github.com/Kervent/nexdir/releases/download/v0.6.0/Nexdir-0.6.0.msi",
                "https://github.com/Kervent/nexdir/releases/download/v0.6.0/Nexdir-0.6.0.zip",
            ),
            (
                "https://github.com/Kervent/nexdir/releases/download/v0.6.0/Nexdir-0.6.0.msi",
                "https://github.com/Kervent/nexdir/releases/download/v0.6.0/a%2fb.msi",
            ),
            (
                "https://github.com/Kervent/nexdir/releases/tag/v0.6.0",
                "https://github.com/Kervent/nexdir/releases/tag/v0.4.3",
            ),
            ("windows-x86_64-msi", "windows-x86_64"),
        ] {
            let doc = good.replacen(from, to, 1);
            assert_ne!(doc, good, "{from} not in fixture");
            assert!(
                matches!(
                    eval(&doc, Channel::Preview, "0.5.0", InstallKind::Nsis),
                    Err(FeedError::Untrusted(_))
                ),
                "{to}"
            );
        }
    }

    #[test]
    fn missing_or_malformed_signatures_are_rejected() {
        let good = feed("preview", "0.6.0", true);
        for bad in ["", "   ", "not base64!", "<script>"] {
            let doc = good.replacen(
                &format!("\"signature\":\"{SIG}\""),
                &format!("\"signature\":{}", serde_json::json!(bad)),
                1,
            );
            assert!(
                matches!(
                    eval(&doc, Channel::Preview, "0.5.0", InstallKind::Nsis),
                    Err(FeedError::Untrusted(_))
                ),
                "{bad:?}"
            );
        }
        let long = good.replacen(SIG, &"A".repeat(MAX_SIGNATURE_LEN + 1), 1);
        assert!(matches!(
            eval(&long, Channel::Preview, "0.5.0", InstallKind::Nsis),
            Err(FeedError::Untrusted(_))
        ));
    }

    #[test]
    fn missing_installer_for_this_installation_is_reported() {
        let doc = r#"{"schema":1,"channel":"preview","version":"0.6.0","prerelease":true,"release_url":"https://github.com/Kervent/nexdir/releases/tag/v0.6.0","platforms":{}}"#;
        assert_eq!(
            eval(doc, Channel::Preview, "0.5.0", InstallKind::Msi),
            Err(FeedError::MissingInstaller("windows-x86_64-msi"))
        );
        assert!(matches!(
            eval(doc, Channel::Preview, "0.5.0", InstallKind::Portable),
            Ok(Outcome::Available(_))
        ));
    }

    #[test]
    fn default_channel_follows_the_running_version() {
        assert_eq!(Channel::default_for(&v("0.5.0-alpha.1")), Channel::Preview);
        assert_eq!(Channel::default_for(&v("1.0.0")), Channel::Stable);
    }

    #[test]
    fn updater_manifest_keeps_only_what_the_updater_reads() {
        let m = updater_manifest(feed("preview", "0.6.0", true).as_bytes()).unwrap();
        assert_eq!(m["version"], "0.6.0");
        assert_eq!(m["pub_date"], "2026-11-20T12:00:00Z");
        assert_eq!(m["platforms"]["windows-x86_64-nsis"]["signature"], SIG);
        assert!(m["platforms"]["windows-x86_64-nsis"].get("size").is_none());
    }
}
