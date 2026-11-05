//! Parsing and validation of network (UNC) paths typed by the user or stored as favourites.
//!
//! A UNC path names a server, optionally a share on it, and optionally folders inside the
//! share: `\\server`, `\\server\share`, `\\server\share\folder`. Only these three forms are
//! accepted. Everything that reaches Windows' device namespace (`\\.\…`, `\\?\…` other than
//! `\\?\UNC\…`), relative segments and characters that are invalid in server, share or file
//! names are rejected, so a "network path" can never address a local device or pipe.

use crate::error::{AppError, AppResult, ErrorCode};
use serde::Serialize;

/// Longest server name Windows accepts in a UNC path (DNS name limit).
const MAX_SERVER: usize = 253;
/// Longest share name supported by SMB servers (Windows limit).
const MAX_SHARE: usize = 80;
/// Longest folder or file name (UTF-16 code units).
const MAX_SEGMENT: usize = 255;
/// Overall length limit (extended-length paths).
const MAX_TOTAL: usize = 32_000;

/// What a UNC path points to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UncKind {
    /// Only a server (`\\server`). It is not a folder and cannot be opened like one.
    Server,
    /// A share (`\\server\share`).
    Share,
    /// A folder or file inside a share (`\\server\share\folder`).
    Folder,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UncPath {
    pub server: String,
    pub share: Option<String>,
    /// Folders below the share.
    pub segments: Vec<String>,
}

impl UncPath {
    pub fn kind(&self) -> UncKind {
        match (&self.share, self.segments.is_empty()) {
            (None, _) => UncKind::Server,
            (Some(_), true) => UncKind::Share,
            (Some(_), false) => UncKind::Folder,
        }
    }

    /// Canonical display form: `\\server\share\folder` (no trailing backslash).
    pub fn display(&self) -> String {
        let mut s = format!(r"\\{}", self.server);
        if let Some(share) = &self.share {
            s.push('\\');
            s.push_str(share);
        }
        for seg in &self.segments {
            s.push('\\');
            s.push_str(seg);
        }
        s
    }

    /// `\\server\share` of this path (None for a bare server).
    pub fn share_root(&self) -> Option<String> {
        self.share.as_ref().map(|share| format!(r"\\{}\{}", self.server, share))
    }

    /// Lower-case key used to recognise the same location in different spellings.
    pub fn key(&self) -> String {
        self.display().to_lowercase()
    }
}

fn invalid(msg: impl Into<String>, raw: &str) -> AppError {
    AppError::new(ErrorCode::InvalidPath, msg).with_path(raw)
}

/// Characters that are never valid in server, share or file names.
fn has_forbidden_char(s: &str) -> bool {
    s.chars()
        .any(|c| matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || (c as u32) < 32 || c == '\u{7f}')
}

/// Validates the server part: a NetBIOS or DNS name (Unicode allowed), an IPv4 address or
/// an IPv6 address in the `…ipv6-literal.net` form that Windows uses in UNC paths.
pub fn validate_server(server: &str, raw: &str) -> AppResult<()> {
    if server.is_empty() {
        return Err(invalid("The server name is missing. Example: \\\\NAS\\Share", raw));
    }
    if server.chars().count() > MAX_SERVER {
        return Err(invalid("The server name is too long.", raw));
    }
    if server.starts_with('[') || server.contains(':') {
        return Err(invalid(
            "IPv6 addresses must be written in the Windows form, e.g. \\\\fe80--1.ipv6-literal.net\\Share.",
            raw,
        ));
    }
    if server == "." || server == "?" || server.chars().all(|c| c == '.') {
        return Err(invalid("Device paths are not supported.", raw));
    }
    if has_forbidden_char(server)
        || server.chars().any(|c| {
            c.is_whitespace()
                || matches!(
                    c,
                    '@' | '%' | ',' | ';' | '=' | '+' | '[' | ']' | '{' | '}' | '^' | '`' | '~' | '!' | '#' | '&' | '\'' | '(' | ')'
                )
        })
    {
        return Err(invalid("The server name contains characters that are not allowed.", raw));
    }
    if server.starts_with('-') || server.starts_with('.') || server.ends_with('-') {
        return Err(invalid("The server name is not valid.", raw));
    }
    Ok(())
}

/// Validates a share name. `IPC$` is the named-pipe share, not a file share, and is refused.
pub fn validate_share(share: &str, raw: &str) -> AppResult<()> {
    if share.is_empty() {
        return Err(invalid("The share name is missing.", raw));
    }
    if share == "." || share == ".." {
        return Err(invalid("Relative segments (..) are not allowed.", raw));
    }
    if share.chars().count() > MAX_SHARE {
        return Err(invalid("The share name is too long (maximum 80 characters).", raw));
    }
    // Windows share names additionally exclude [ ] + = ; , (file names may contain them).
    if has_forbidden_char(share) || share.chars().any(|c| matches!(c, '[' | ']' | '+' | '=' | ';' | ',')) {
        return Err(invalid("The share name contains characters that are not allowed.", raw));
    }
    if share.eq_ignore_ascii_case("IPC$") {
        return Err(invalid("IPC$ is not a file share.", raw));
    }
    Ok(())
}

fn validate_segment(seg: &str, raw: &str) -> AppResult<()> {
    if seg == ".." {
        return Err(invalid("Relative segments (..) are not allowed.", raw));
    }
    if has_forbidden_char(seg) {
        return Err(invalid("The path contains characters that are not allowed in names.", raw));
    }
    if seg.encode_utf16().count() > MAX_SEGMENT {
        return Err(invalid("A folder name in the path is too long.", raw));
    }
    Ok(())
}

/// Parses user input into a UNC path. Accepts `\\server[\share[\folder…]]`, the same with
/// forward slashes, the extended form `\\?\UNC\server\share…` and `smb://server/share/…`
/// (as NAS web interfaces show it). Surrounding quotes and whitespace are ignored.
pub fn parse(input: &str) -> AppResult<UncPath> {
    let raw = input.trim().trim_matches('"').trim();
    if raw.is_empty() {
        return Err(invalid("Enter a network path, for example \\\\NAS\\Share.", input));
    }
    if raw.contains('\0') {
        return Err(invalid("The path contains invalid characters.", input));
    }
    if raw.len() > MAX_TOTAL {
        return Err(invalid("The path is too long.", input));
    }
    let lower = raw.to_ascii_lowercase();
    let body: String = if let Some(rest) = lower.strip_prefix("smb://").map(|_| &raw[6..]) {
        rest.replace('/', "\\")
    } else {
        let s = raw.replace('/', "\\");
        if let Some(rest) = s.strip_prefix(r"\\?\UNC\").or_else(|| s.strip_prefix(r"\\?\unc\")) {
            rest.to_string()
        } else if s.starts_with(r"\\?\") || s.starts_with(r"\\.\") || s.starts_with(r"\??\") {
            return Err(invalid("Device paths are not supported.", input));
        } else if let Some(rest) = s.strip_prefix(r"\\") {
            rest.to_string()
        } else {
            return Err(invalid(
                "A network path starts with two backslashes, for example \\\\NAS\\Share.",
                input,
            ));
        }
    };
    // A third leading backslash ("\\\server") is not a valid network path.
    if body.starts_with('\\') {
        return Err(invalid("The network path is not valid (too many backslashes at the start).", input));
    }
    // Empty segments ("\\server\\share", trailing "\") and "." are ignored; ".." is refused.
    let mut parts = body.split('\\').filter(|p| !p.is_empty() && *p != ".");
    let server = parts.next().unwrap_or("").to_string();
    validate_server(&server, input)?;
    let share = parts.next().map(str::to_string);
    if let Some(share) = &share {
        validate_share(share, input)?;
    }
    let segments: Vec<String> = parts.map(str::to_string).collect();
    for seg in &segments {
        validate_segment(seg, input)?;
    }
    Ok(UncPath { server, share, segments })
}

/// True for paths that start like a UNC path (`\\x` or `//x`, not a device path).
pub fn looks_like_unc(path: &str) -> bool {
    let s = path.replace('/', "\\");
    s.starts_with(r"\\") && !s.starts_with(r"\\.\") && !(s.starts_with(r"\\?\") && !s.to_ascii_uppercase().starts_with(r"\\?\UNC\"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(s: &str) -> UncPath {
        parse(s).unwrap_or_else(|e| panic!("{s}: {}", e.message))
    }
    fn err(s: &str) -> String {
        parse(s).expect_err(s).message
    }

    #[test]
    fn distinguishes_server_share_and_folder() {
        assert_eq!(ok(r"\\NAS").kind(), UncKind::Server);
        assert_eq!(ok(r"\\NAS\").kind(), UncKind::Server);
        assert_eq!(ok(r"\\NAS\Medien").kind(), UncKind::Share);
        assert_eq!(ok(r"\\NAS\Medien\").kind(), UncKind::Share);
        let p = ok(r"\\NAS\Medien\Filme\2026");
        assert_eq!(p.kind(), UncKind::Folder);
        assert_eq!(p.segments, vec!["Filme", "2026"]);
        assert_eq!(p.share_root().as_deref(), Some(r"\\NAS\Medien"));
    }

    #[test]
    fn accepts_common_spellings() {
        assert_eq!(ok(r"\\192.168.178.10\Medien").display(), r"\\192.168.178.10\Medien");
        assert_eq!(ok("//NAS-SERVER/Backups").display(), r"\\NAS-SERVER\Backups");
        assert_eq!(ok(r#"  "\\homeserver\Documents\"  "#).display(), r"\\homeserver\Documents");
        assert_eq!(ok(r"\\?\UNC\nas\share\x").display(), r"\\nas\share\x");
        assert_eq!(ok("smb://nas.local/Media/Fotos").display(), r"\\nas.local\Media\Fotos");
        assert_eq!(ok(r"\\nas\\share\.\x").display(), r"\\nas\share\x");
        assert_eq!(ok(r"\\fe80--1.ipv6-literal.net\share").server, "fe80--1.ipv6-literal.net");
        assert_eq!(ok(r"\\NAS\C$").share.as_deref(), Some("C$"));
    }

    #[test]
    fn accepts_unicode_names() {
        let p = ok(r"\\NAS-WOHNZIMMER\Médias\Ürlaub 日本 😀");
        assert_eq!(p.share.as_deref(), Some("Médias"));
        assert_eq!(p.segments, vec!["Ürlaub 日本 😀"]);
        assert_eq!(ok(r"\\Büro-PC\Ablage").server, "Büro-PC");
    }

    #[test]
    fn refuses_device_namespaces_and_traversal() {
        for bad in [
            r"\\.\pipe\x",
            r"\\?\C:\Windows",
            r"\\?\GLOBALROOT\Device\x",
            r"\??\C:\x",
            r"\\.\UNC\nas\share",
            r"\\.\PhysicalDrive0",
        ] {
            assert!(err(bad).contains("Device"), "{bad}");
        }
        assert!(err(r"\\nas\share\..\other").contains(".."));
        assert!(err(r"\\nas\..").contains(".."));
        assert!(err(r"\\..\share").contains("Device") || err(r"\\..\share").contains("not"));
        assert!(err(r"\\nas\IPC$").contains("IPC$"));
        assert!(err(r"\\nas\ipc$").contains("IPC$"));
    }

    #[test]
    fn refuses_invalid_input() {
        assert!(err("").contains("Enter"));
        assert!(err(r"\\\.\pipe").contains("too many backslashes"));
        assert!(err(r"\\\server\share").contains("too many backslashes"));
        assert!(err(r"C:\Users").contains("two backslashes"));
        assert!(err("nas\\share").contains("two backslashes"));
        assert!(err(r"\\").contains("missing"));
        assert!(err(r"\\[fe80::1]\share").contains("IPv6"));
        assert!(err(r"\\nas:445\share").contains("IPv6"));
        assert!(err(r"\\na s\share").contains("not allowed"));
        assert!(err(r"\\user@nas\share").contains("not allowed"));
        assert!(err(r"\\nas\sha*re").contains("share name"));
        assert!(err(r"\\nas\share\a<b").contains("not allowed"));
        assert!(err("\\\\nas\\share\\a\u{1}b").contains("not allowed"));
        assert!(err(&format!(r"\\nas\{}", "s".repeat(81))).contains("too long"));
        assert!(err(&format!(r"\\{}\share", "a".repeat(254))).contains("too long"));
        assert!(err(&format!(r"\\nas\share\{}", "x".repeat(256))).contains("too long"));
        assert!(err("\\\\nas\\share\0").contains("invalid"));
    }

    #[test]
    fn keys_ignore_case_and_spelling() {
        assert_eq!(ok(r"\\NAS\Medien\").key(), ok("//nas/medien").key());
    }

    #[test]
    fn recognises_unc_like_paths() {
        assert!(looks_like_unc(r"\\nas\share"));
        assert!(looks_like_unc("//nas/share"));
        assert!(looks_like_unc(r"\\?\UNC\nas\share"));
        assert!(!looks_like_unc(r"\\?\C:\x"));
        assert!(!looks_like_unc(r"\\.\pipe\x"));
        assert!(!looks_like_unc(r"C:\x"));
    }
}
