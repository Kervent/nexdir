//! Read-only analysis of the file system: storage usage and duplicate files.

pub mod duplicates;
pub mod storage;

/// Coarse file category by extension, used for type distributions and search filters.
pub fn category_of(ext: &str) -> &'static str {
    match ext {
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "tif" | "tiff" | "svg" | "ico" | "heic" | "heif" | "avif" | "raw" | "cr2"
        | "nef" | "psd" => "image",
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm" | "flv" | "m4v" | "mpg" | "mpeg" | "3gp" | "m2ts" => "video",
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "wma" | "opus" | "aiff" | "mid" | "midi" => "audio",
        "pdf" | "doc" | "docx" | "odt" | "rtf" | "txt" | "md" | "xls" | "xlsx" | "ods" | "csv" | "ppt" | "pptx" | "odp" => "document",
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "zst" | "cab" | "iso" | "tgz" => "archive",
        "exe" | "msi" | "dll" | "sys" | "com" | "msix" | "appx" => "program",
        "js" | "jsx" | "ts" | "tsx" | "rs" | "py" | "java" | "c" | "h" | "cpp" | "hpp" | "cs" | "go" | "rb" | "php" | "html" | "css"
        | "json" | "xml" | "yaml" | "yml" | "toml" | "sql" | "sh" | "ps1" => "code",
        _ => "other",
    }
}
