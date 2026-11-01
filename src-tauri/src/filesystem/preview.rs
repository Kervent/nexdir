//! Safe file previews. Content is only ever read, never executed or rendered as HTML:
//! images go to an `<img>` element (scripts in SVG do not run there), text is shown as
//! plain text. Sizes are capped so a preview cannot exhaust memory.

use super::{entry_from_metadata, FileEntry};
use crate::error::{AppError, AppResult};
use base64::Engine;
use serde::Serialize;
use std::fs;
use std::io::Read;
use std::path::Path;

const MAX_IMAGE_BYTES: u64 = 25 * 1024 * 1024;
const MAX_TEXT_BYTES: usize = 128 * 1024;
/// Files without a known text extension are sniffed for binary content up to this size.
const SNIFF_BYTES: usize = 8 * 1024;
const MAX_FOLDER_COUNT: usize = 100_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum PreviewContent {
    Image { mime: String, data: String },
    Text { text: String, truncated: bool },
    Folder { items: usize, truncated: bool },
    None { reason: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub entry: FileEntry,
    pub content: PreviewContent,
}

fn image_mime(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "png" => "image/png",
        "jpg" | "jpeg" | "jfif" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "avif" => "image/avif",
        "svg" => "image/svg+xml",
        _ => return None,
    })
}

const TEXT_EXTENSIONS: &[&str] = &[
    "txt",
    "md",
    "markdown",
    "log",
    "ini",
    "cfg",
    "conf",
    "csv",
    "tsv",
    "json",
    "jsonc",
    "xml",
    "yaml",
    "yml",
    "toml",
    "html",
    "htm",
    "css",
    "scss",
    "js",
    "mjs",
    "cjs",
    "jsx",
    "ts",
    "tsx",
    "rs",
    "py",
    "java",
    "c",
    "h",
    "cpp",
    "hpp",
    "cs",
    "go",
    "rb",
    "php",
    "swift",
    "kt",
    "sh",
    "ps1",
    "psm1",
    "bat",
    "cmd",
    "sql",
    "lua",
    "gitignore",
    "editorconfig",
    "env",
    "properties",
    "reg",
    "srt",
    "vtt",
    "tex",
];

fn read_prefix(path: &Path, limit: usize) -> std::io::Result<(Vec<u8>, bool)> {
    let file = fs::File::open(path)?;
    let mut buf = Vec::with_capacity(limit.min(64 * 1024));
    file.take(limit as u64 + 1).read_to_end(&mut buf)?;
    let truncated = buf.len() > limit;
    buf.truncate(limit);
    Ok((buf, truncated))
}

pub(crate) fn decode_text(bytes: &[u8]) -> String {
    // UTF-16 LE/BE with BOM (common for Windows-generated files), otherwise UTF-8.
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let units: Vec<u16> = rest.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
        return String::from_utf16_lossy(&units);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        let units: Vec<u16> = rest.as_chunks::<2>().0.iter().map(|c| u16::from_be_bytes(*c)).collect();
        return String::from_utf16_lossy(&units);
    }
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    String::from_utf8_lossy(bytes).into_owned()
}

fn looks_textual(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) || !bytes.contains(&0)
}

pub fn preview(path: &Path) -> AppResult<Preview> {
    let meta = fs::symlink_metadata(path).map_err(|e| AppError::from_io(&e, path))?;
    let entry = entry_from_metadata(path, &meta);
    let io_err = |e: std::io::Error| AppError::from_io(&e, path);

    let content = if entry.kind.is_navigable() {
        match fs::read_dir(path) {
            Ok(reader) => {
                let items = reader.take(MAX_FOLDER_COUNT + 1).count();
                PreviewContent::Folder {
                    items: items.min(MAX_FOLDER_COUNT),
                    truncated: items > MAX_FOLDER_COUNT,
                }
            }
            Err(e) => PreviewContent::None {
                reason: AppError::from_io(&e, path).message,
            },
        }
    } else if entry.kind != super::EntryKind::File {
        PreviewContent::None {
            reason: "Links are not previewed.".into(),
        }
    } else {
        let ext = entry.extension.clone().unwrap_or_default();
        if let Some(mime) = image_mime(&ext) {
            if meta.len() > MAX_IMAGE_BYTES {
                PreviewContent::None {
                    reason: "This image is too large to preview.".into(),
                }
            } else {
                let bytes = fs::read(path).map_err(io_err)?;
                PreviewContent::Image {
                    mime: mime.into(),
                    data: base64::engine::general_purpose::STANDARD.encode(bytes),
                }
            }
        } else if TEXT_EXTENSIONS.contains(&ext.as_str()) || ext.is_empty() || meta.len() <= SNIFF_BYTES as u64 * 64 {
            let (bytes, truncated) = read_prefix(path, MAX_TEXT_BYTES).map_err(io_err)?;
            let known_text = TEXT_EXTENSIONS.contains(&ext.as_str());
            if known_text || looks_textual(&bytes[..bytes.len().min(SNIFF_BYTES)]) {
                PreviewContent::Text {
                    text: decode_text(&bytes),
                    truncated,
                }
            } else {
                PreviewContent::None {
                    reason: "No preview is available for this file type.".into(),
                }
            }
        } else {
            PreviewContent::None {
                reason: "No preview is available for this file type.".into(),
            }
        }
    };
    Ok(Preview { entry, content })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_text_with_limits_and_encodings() {
        let dir = tempfile::tempdir().unwrap();
        let small = dir.path().join("notes.md");
        fs::write(&small, "# Hello <script>alert(1)</script>").unwrap();
        match preview(&small).unwrap().content {
            PreviewContent::Text { text, truncated } => {
                assert_eq!(text, "# Hello <script>alert(1)</script>");
                assert!(!truncated);
            }
            other => panic!("unexpected {other:?}"),
        }

        let big = dir.path().join("big.log");
        fs::write(&big, "x".repeat(MAX_TEXT_BYTES + 10)).unwrap();
        match preview(&big).unwrap().content {
            PreviewContent::Text { text, truncated } => {
                assert_eq!(text.len(), MAX_TEXT_BYTES);
                assert!(truncated);
            }
            other => panic!("unexpected {other:?}"),
        }

        let utf16 = dir.path().join("win.txt");
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend("Grüße".encode_utf16().flat_map(|u| u.to_le_bytes()));
        fs::write(&utf16, bytes).unwrap();
        assert!(matches!(preview(&utf16).unwrap().content, PreviewContent::Text { text, .. } if text == "Grüße"));
    }

    #[test]
    fn previews_images_as_base64_and_skips_binaries() {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("a.PNG");
        fs::write(&png, [0x89, b'P', b'N', b'G']).unwrap();
        match preview(&png).unwrap().content {
            PreviewContent::Image { mime, data } => {
                assert_eq!(mime, "image/png");
                assert_eq!(data, "iVBORw==");
            }
            other => panic!("unexpected {other:?}"),
        }

        let bin = dir.path().join("data.bin");
        fs::write(&bin, [0u8, 1, 2, 3, 0, 5]).unwrap();
        assert!(matches!(preview(&bin).unwrap().content, PreviewContent::None { .. }));

        let exe = dir.path().join("setup.exe");
        fs::write(&exe, vec![0u8; 1024 * 1024]).unwrap();
        assert!(matches!(preview(&exe).unwrap().content, PreviewContent::None { .. }));
    }

    #[test]
    fn previews_folders_with_item_count() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a"), "").unwrap();
        fs::write(dir.path().join("b"), "").unwrap();
        let p = preview(dir.path()).unwrap();
        assert!(matches!(
            p.content,
            PreviewContent::Folder {
                items: 2,
                truncated: false
            }
        ));
    }
}
