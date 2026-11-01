//! Error type shared by every backend module and serialised to the frontend as
//! `{ code, message, path }`.

use serde::Serialize;
use std::io;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    NotFound,
    PermissionDenied,
    AlreadyExists,
    InvalidName,
    InvalidPath,
    NotADirectory,
    InUse,
    DeviceNotReady,
    Unsupported,
    Cancelled,
    Io,
}

#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{message}")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub path: Option<String>,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            path: None,
        }
    }

    pub fn with_path(mut self, path: impl AsRef<Path>) -> Self {
        self.path = Some(path.as_ref().display().to_string());
        self
    }

    /// Converts an I/O error into a user-readable error tied to `path`.
    pub fn from_io(err: &io::Error, path: impl AsRef<Path>) -> Self {
        let (code, message) = classify_io(err);
        Self::new(code, message).with_path(path)
    }
}

// Windows error codes that std::io::ErrorKind does not distinguish well.
const ERROR_SHARING_VIOLATION: i32 = 32;
const ERROR_LOCK_VIOLATION: i32 = 33;
const ERROR_NOT_READY: i32 = 21;
const ERROR_BAD_NETPATH: i32 = 53;
const ERROR_BAD_NET_NAME: i32 = 67;
const ERROR_FILE_EXISTS: i32 = 80;
const ERROR_INVALID_NAME: i32 = 123;
const ERROR_DIRECTORY: i32 = 267;
const ERROR_ALREADY_EXISTS: i32 = 183;
const ERROR_WRITE_PROTECT: i32 = 19;
const ERROR_DISK_FULL: i32 = 112;
const ERROR_REQUEST_ABORTED: i32 = 1235;

fn classify_io(err: &io::Error) -> (ErrorCode, String) {
    if let Some(raw) = err.raw_os_error() {
        match raw {
            ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION => return (ErrorCode::InUse, "The file is in use by another program.".into()),
            ERROR_NOT_READY => {
                return (
                    ErrorCode::DeviceNotReady,
                    "The drive is not ready. Insert media or reconnect the drive.".into(),
                )
            }
            ERROR_BAD_NETPATH | ERROR_BAD_NET_NAME => return (ErrorCode::NotFound, "The network location could not be reached.".into()),
            ERROR_FILE_EXISTS | ERROR_ALREADY_EXISTS => return (ErrorCode::AlreadyExists, "An item with this name already exists.".into()),
            ERROR_INVALID_NAME => return (ErrorCode::InvalidName, "The name or path syntax is invalid.".into()),
            ERROR_DIRECTORY => return (ErrorCode::NotADirectory, "The path is not a folder.".into()),
            ERROR_WRITE_PROTECT => return (ErrorCode::PermissionDenied, "The media is write-protected.".into()),
            ERROR_DISK_FULL => return (ErrorCode::Io, "There is not enough space on the disk.".into()),
            ERROR_REQUEST_ABORTED => return (ErrorCode::Cancelled, "The operation was cancelled.".into()),
            _ => {}
        }
    }
    match err.kind() {
        io::ErrorKind::NotFound => (ErrorCode::NotFound, "The item no longer exists or the path is wrong.".into()),
        io::ErrorKind::PermissionDenied => (
            ErrorCode::PermissionDenied,
            "Access denied. You may not have permission for this item.".into(),
        ),
        io::ErrorKind::AlreadyExists => (ErrorCode::AlreadyExists, "An item with this name already exists.".into()),
        io::ErrorKind::NotADirectory => (ErrorCode::NotADirectory, "The path is not a folder.".into()),
        io::ErrorKind::InvalidFilename | io::ErrorKind::InvalidInput => {
            (ErrorCode::InvalidName, "The name or path syntax is invalid.".into())
        }
        io::ErrorKind::Interrupted => (ErrorCode::Cancelled, "The operation was interrupted.".into()),
        _ => (ErrorCode::Io, err.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_common_io_errors() {
        let e = AppError::from_io(&io::Error::from(io::ErrorKind::NotFound), "C:\\x");
        assert_eq!(e.code, ErrorCode::NotFound);
        assert_eq!(e.path.as_deref(), Some("C:\\x"));

        let e = AppError::from_io(&io::Error::from_raw_os_error(32), "C:\\y");
        assert_eq!(e.code, ErrorCode::InUse);

        let e = AppError::from_io(&io::Error::from_raw_os_error(80), "C:\\z");
        assert_eq!(e.code, ErrorCode::AlreadyExists);
    }

    #[test]
    fn serialises_camel_case() {
        let e = AppError::new(ErrorCode::PermissionDenied, "nope").with_path("C:\\a");
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(json, r#"{"code":"permissionDenied","message":"nope","path":"C:\\a"}"#);
    }
}
