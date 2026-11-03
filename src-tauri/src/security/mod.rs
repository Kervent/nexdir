//! Validation of every path and file name that arrives from the WebView.
//!
//! The frontend is treated as untrusted input: each command validates its
//! arguments here before any filesystem call is made.

use crate::error::{AppError, AppResult, ErrorCode};
use std::path::{Component, Path, PathBuf, Prefix};

/// Maximum length of a single path component on NTFS (UTF-16 code units).
const MAX_NAME_UTF16: usize = 255;

const RESERVED_DEVICE_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$", "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
    "COM¹", "COM²", "COM³", "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9", "LPT¹", "LPT²", "LPT³",
];

/// Extensions that run code when opened through the shell. Opening these requires
/// an explicit confirmation from the user (see `open_path`).
const EXECUTABLE_EXTENSIONS: &[&str] = &[
    "exe",
    "com",
    "bat",
    "cmd",
    "msi",
    "msp",
    "msix",
    "msixbundle",
    "appx",
    "appxbundle",
    "ps1",
    "psm1",
    "vbs",
    "vbe",
    "js",
    "jse",
    "wsf",
    "wsh",
    "hta",
    "scr",
    "cpl",
    "pif",
    "reg",
    "lnk",
    "url",
    "jar",
    "application",
    "gadget",
    "msc",
    "inf",
    "scf",
    "appref-ms",
];

/// Validates a path received from the frontend and returns it as an owned, lexically
/// normalised absolute path. `..` components are rejected rather than resolved so that
/// a path can never silently point somewhere other than what the user saw.
pub fn validate_path(raw: &str) -> AppResult<PathBuf> {
    let invalid = |msg: &str| AppError::new(ErrorCode::InvalidPath, msg.to_string()).with_path(raw);
    if raw.is_empty() {
        return Err(invalid("The path is empty."));
    }
    if raw.contains('\0') {
        return Err(invalid("The path contains invalid characters."));
    }
    let path = Path::new(raw);
    if !path.is_absolute() {
        return Err(invalid("Only absolute paths are supported."));
    }
    let mut normalised = PathBuf::new();
    for component in path.components() {
        match component {
            // Decided by the kind of prefix, not by text, so "//./x", "\\?\globalroot\…" and
            // other spellings of the device namespace cannot slip through.
            Component::Prefix(prefix) => match prefix.kind() {
                Prefix::Disk(_) | Prefix::VerbatimDisk(_) | Prefix::UNC(..) | Prefix::VerbatimUNC(..) => {
                    normalised.push(component.as_os_str())
                }
                Prefix::DeviceNS(_) | Prefix::Verbatim(_) => return Err(invalid("Device paths are not supported.")),
            },
            Component::ParentDir => return Err(invalid("Relative segments (..) are not allowed.")),
            Component::CurDir => {}
            Component::Normal(part) => {
                // ":" would address an NTFS alternate data stream ("file.txt:hidden");
                // wildcards and control characters never occur in real names.
                let bad = part
                    .to_string_lossy()
                    .chars()
                    .any(|c| matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*') || (c as u32) < 32);
                if bad {
                    return Err(invalid("The path contains characters that are not allowed in names."));
                }
                normalised.push(part)
            }
            Component::RootDir => normalised.push(component.as_os_str()),
        }
    }
    Ok(normalised)
}

/// Validates a single file or folder name for creation or renaming on Windows.
pub fn validate_name(name: &str) -> AppResult<()> {
    let invalid = |msg: &str| AppError::new(ErrorCode::InvalidName, msg.to_string());
    if name.trim().is_empty() {
        return Err(invalid("The name cannot be empty."));
    }
    if name == "." || name == ".." {
        return Err(invalid("This name is reserved."));
    }
    if let Some(c) = name
        .chars()
        .find(|c| matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || (*c as u32) < 32)
    {
        let shown = if (c as u32) < 32 {
            "control characters".to_string()
        } else {
            format!("\"{c}\"")
        };
        return Err(invalid(&format!("Names cannot contain {shown}. Not allowed: < > : \" / \\ | ? *")));
    }
    if name.ends_with(' ') || name.ends_with('.') {
        return Err(invalid("Names cannot end with a space or a period."));
    }
    if name.encode_utf16().count() > MAX_NAME_UTF16 {
        return Err(invalid("The name is too long (maximum 255 characters)."));
    }
    let stem = name.split('.').next().unwrap_or(name).trim_end().to_uppercase();
    if RESERVED_DEVICE_NAMES.iter().any(|r| *r == stem) {
        return Err(invalid("This name is reserved by Windows."));
    }
    Ok(())
}

pub fn is_executable(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| EXECUTABLE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Case-insensitive check whether `inner` equals `outer` or lies beneath it. Both paths
/// should already be canonicalised so that links and 8.3 names cannot defeat the check.
pub fn is_same_or_inside(inner: &Path, outer: &Path) -> bool {
    let inner: Vec<String> = inner.components().map(|c| c.as_os_str().to_string_lossy().to_lowercase()).collect();
    let outer: Vec<String> = outer.components().map(|c| c.as_os_str().to_string_lossy().to_lowercase()).collect();
    inner.len() >= outer.len() && inner[..outer.len()] == outer[..]
}

/// The only web addresses Nexdir opens: the official repository (issues, releases, docs,
/// prefilled issue forms) and Discord invite links. Opening happens only after a click.
pub const REPOSITORY_URL: &str = "https://github.com/Kervent/nexdir";
const DISCORD_INVITE_PREFIXES: &[&str] = &["https://discord.gg/", "https://discord.com/invite/"];
/// Longer addresses are cut off by browsers or rejected by GitHub.
pub const MAX_URL_LEN: usize = 8_000;

pub fn validate_external_url(url: &str) -> AppResult<()> {
    let invalid = || {
        AppError::new(
            ErrorCode::Unsupported,
            "Nexdir only opens links to its own GitHub repository and Discord server.",
        )
    };
    if url.len() > MAX_URL_LEN {
        return Err(AppError::new(ErrorCode::Unsupported, "The link is too long to open."));
    }
    // Printable ASCII only: no spaces, quotes or control characters that could change how
    // the shell interprets the address. Callers percent-encode everything else.
    if !url.bytes().all(|b| b.is_ascii_graphic() && b != b'"') {
        return Err(invalid());
    }
    if let Some(rest) = url.strip_prefix(REPOSITORY_URL) {
        // The path part may not climb out of the repository ("..", also percent-encoded).
        let path = rest.split(['?', '#']).next().unwrap_or("").to_ascii_lowercase();
        if (rest.is_empty() || rest.starts_with(['/', '?', '#'])) && !path.contains("..") && !path.contains("%2e") && !path.contains('\\') {
            return Ok(());
        }
    }
    for prefix in DISCORD_INVITE_PREFIXES {
        if let Some(code) = url.strip_prefix(prefix) {
            if (2..=32).contains(&code.len()) && code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
                return Ok(());
            }
        }
    }
    Err(invalid())
}

/// The license index shown in About Nexdir, embedded at build time.
const LICENSE_INDEX: &str = include_str!("../../../src/generated/licenses.json");

fn project_urls() -> &'static std::collections::HashSet<String> {
    static URLS: std::sync::OnceLock<std::collections::HashSet<String>> = std::sync::OnceLock::new();
    URLS.get_or_init(|| {
        let index: serde_json::Value = serde_json::from_str(LICENSE_INDEX).unwrap_or_default();
        index["components"]
            .as_array()
            .map(|list| list.iter().filter_map(|c| c["source"].as_str().map(str::to_owned)).collect())
            .unwrap_or_default()
    })
}

/// Project pages of bundled third-party components (Open Source Licenses). Only addresses
/// listed in the license index Nexdir was built with are opened, and only over HTTPS.
pub fn validate_project_url(url: &str) -> AppResult<()> {
    let ok = url.len() <= MAX_URL_LEN
        && url.starts_with("https://")
        && url.bytes().all(|b| b.is_ascii_graphic() && b != b'"')
        && project_urls().contains(url);
    if ok {
        Ok(())
    } else {
        Err(AppError::new(
            ErrorCode::Unsupported,
            "Nexdir only opens the project pages listed in its license information.",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_urls_come_from_the_license_index() {
        assert!(project_urls().len() > 100, "index embedded");
        assert!(validate_project_url("https://github.com/tauri-apps/tauri").is_ok());
        assert!(validate_project_url("https://sqlite.org/copyright.html").is_ok());
        for bad in [
            "https://evil.example/",
            "http://github.com/tauri-apps/tauri",
            "https://github.com/tauri-apps/tauri/../x",
            "https://github.com/tauri-apps/tauri\"",
            "",
        ] {
            assert!(validate_project_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn accepts_normal_absolute_paths() {
        assert_eq!(validate_path(r"C:\Users\me").unwrap(), PathBuf::from(r"C:\Users\me"));
        assert_eq!(validate_path(r"C:\Users\.\me").unwrap(), PathBuf::from(r"C:\Users\me"));
        assert!(validate_path(r"\\server\share\folder").is_ok());
        assert!(validate_path(r"C:\Ünïcödé\日本語\😀").is_ok());
    }

    #[test]
    fn rejects_dangerous_paths() {
        assert!(validate_path("").is_err());
        assert!(validate_path("relative\\path").is_err());
        assert!(validate_path(r"C:\Users\..\Windows").is_err());
        assert!(validate_path("C:\\a\0b").is_err());
        assert!(validate_path(r"\\.\PhysicalDrive0").is_err());
    }

    #[test]
    fn rejects_device_paths_in_every_spelling() {
        for bad in [
            r"\\.\PhysicalDrive0",
            "//./PhysicalDrive0",
            r"\\.\C:\Windows",
            r"\\?\GLOBALROOT\Device\HarddiskVolume1",
            r"\\?\globalroot\Device\HarddiskVolume1",
            r"\\?\Volume{12345678-0000-0000-0000-000000000000}\x",
            r"\\?\pipe\x",
        ] {
            assert!(validate_path(bad).is_err(), "{bad} should be rejected");
        }
        // Long-path spellings of normal drives and shares stay allowed.
        assert!(validate_path(r"\\?\C:\very\long\path").is_ok());
        assert!(validate_path(r"\\?\UNC\server\share\x").is_ok());
        assert!(validate_path("C:/Users/me").is_ok());
    }

    #[test]
    fn rejects_streams_wildcards_and_control_characters() {
        for bad in [
            r"C:\docs\file.txt:hidden",
            r"C:\docs\file.txt::$DATA",
            r"C:\docs:x\file.txt",
            r"C:\docs\*.txt",
            r"C:\docs\what?",
            "C:\\docs\\tab\there",
            r"C:\docs\<x>",
            r"\\server\share\a|b",
        ] {
            assert!(validate_path(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn validates_names() {
        for ok in [
            "report.pdf",
            "New folder",
            ".gitignore",
            "日本語",
            "a b c",
            "CONSOLE.txt",
            "file.tar.gz",
        ] {
            assert!(validate_name(ok).is_ok(), "{ok} should be valid");
        }
        for bad in [
            "",
            "   ",
            ".",
            "..",
            "a/b",
            "a\\b",
            "a:b",
            "what?",
            "star*",
            "pipe|",
            "<x>",
            "q\"",
            "trail.",
            "trail ",
            "CON",
            "con.txt",
            "Lpt1.log",
            "NUL",
            "COM¹",
            "tab\tname",
        ] {
            assert!(validate_name(bad).is_err(), "{bad:?} should be invalid");
        }
        assert!(validate_name(&"a".repeat(255)).is_ok());
        assert!(validate_name(&"a".repeat(256)).is_err());
    }

    #[test]
    fn detects_executables() {
        assert!(is_executable(Path::new(r"C:\x\setup.EXE")));
        assert!(is_executable(Path::new(r"C:\x\script.ps1")));
        assert!(!is_executable(Path::new(r"C:\x\photo.jpg")));
        assert!(!is_executable(Path::new(r"C:\x\noext")));
    }

    #[test]
    fn containment_is_component_based_and_case_insensitive() {
        assert!(is_same_or_inside(Path::new(r"C:\Data\Sub"), Path::new(r"c:\data")));
        assert!(is_same_or_inside(Path::new(r"C:\Data"), Path::new(r"C:\Data")));
        assert!(!is_same_or_inside(Path::new(r"C:\Database"), Path::new(r"C:\Data")));
        assert!(!is_same_or_inside(Path::new(r"C:\"), Path::new(r"C:\Data")));
    }

    #[test]
    fn opens_only_project_links() {
        for ok in [
            "https://github.com/Kervent/nexdir",
            "https://github.com/Kervent/nexdir/issues",
            "https://github.com/Kervent/nexdir/issues/new?template=bug_report.yml&title=%5BBug%5D%20x",
            "https://github.com/Kervent/nexdir/tree/main/docs",
            "https://github.com/Kervent/nexdir/issues/new?body=see..%2E%20and%20..",
            "https://discord.gg/abc-DEF12",
            "https://discord.com/invite/abcdef",
        ] {
            assert!(validate_external_url(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "http://github.com/Kervent/nexdir",
            "https://github.com/Kervent/nexdir-evil",
            "https://github.com/Kervent/nexdir.evil.com",
            "https://github.com/Kervent/nexdir/../../evil",
            "https://github.com/Kervent/nexdir/%2E%2E/evil",
            "https://github.com/Kervent/nexdir/..%2Fevil",
            "https://github.com/Kervent/nexdir/ x",
            "https://github.com/Kervent/nexdir/\"x",
            "https://github.com/Kervent/nexdir/\u{0}",
            "https://github.com.evil.com/Kervent/nexdir",
            "https://discord.gg/",
            "https://discord.gg/abc/../x",
            "https://discord.gg.evil.com/abc",
            "file:///C:/Windows/System32/calc.exe",
            r"C:\Windows\System32\calc.exe",
            "javascript:alert(1)",
        ] {
            assert!(validate_external_url(bad).is_err(), "{bad}");
        }
        assert!(validate_external_url(&format!("{REPOSITORY_URL}/issues?{}", "a".repeat(MAX_URL_LEN))).is_err());
    }
}
