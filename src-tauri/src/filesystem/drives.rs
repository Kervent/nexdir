//! Drive enumeration. Querying a disconnected network drive can block for a long time,
//! so each drive is queried on its own thread with a timeout.

use crate::platform::{self, DriveType};
use serde::Serialize;
use std::sync::mpsc;
use std::time::Duration;

const QUERY_TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DriveKind {
    Fixed,
    Removable,
    Network,
    Optical,
    Ram,
    Unknown,
}

impl From<DriveType> for DriveKind {
    fn from(t: DriveType) -> Self {
        match t {
            DriveType::Fixed => DriveKind::Fixed,
            DriveType::Removable => DriveKind::Removable,
            DriveType::Remote => DriveKind::Network,
            DriveType::CdRom => DriveKind::Optical,
            DriveType::RamDisk => DriveKind::Ram,
            DriveType::Unknown => DriveKind::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveInfo {
    /// Root path, e.g. `C:\`.
    pub path: String,
    pub letter: String,
    pub label: String,
    pub kind: DriveKind,
    pub file_system: Option<String>,
    pub total_bytes: Option<u64>,
    pub free_bytes: Option<u64>,
    /// False when the drive has no media, is disconnected or did not answer in time.
    pub available: bool,
}

pub fn list_drives() -> Vec<DriveInfo> {
    let handles: Vec<_> = platform::logical_drives()
        .into_iter()
        .map(|raw| {
            let (tx, rx) = mpsc::channel();
            let root = raw.root.clone();
            // Detached on purpose: a hung network query must not block the listing.
            std::thread::spawn(move || {
                let _ = tx.send(platform::volume_info(&root));
            });
            (raw, rx)
        })
        .collect();

    handles
        .into_iter()
        .map(|(raw, rx)| {
            let letter = raw.root[..1].to_string();
            let kind = DriveKind::from(raw.kind);
            match rx.recv_timeout(QUERY_TIMEOUT) {
                Ok(Ok(info)) => DriveInfo {
                    label: if info.label.is_empty() {
                        default_label(kind).into()
                    } else {
                        info.label
                    },
                    file_system: Some(info.file_system).filter(|s| !s.is_empty()),
                    total_bytes: Some(info.total_bytes),
                    free_bytes: Some(info.free_bytes),
                    available: true,
                    path: raw.root,
                    letter,
                    kind,
                },
                _ => DriveInfo {
                    label: default_label(kind).into(),
                    file_system: None,
                    total_bytes: None,
                    free_bytes: None,
                    available: false,
                    path: raw.root,
                    letter,
                    kind,
                },
            }
        })
        .collect()
}

fn default_label(kind: DriveKind) -> &'static str {
    match kind {
        DriveKind::Fixed => "Local Disk",
        DriveKind::Removable => "Removable Disk",
        DriveKind::Network => "Network Drive",
        DriveKind::Optical => "Optical Drive",
        DriveKind::Ram => "RAM Disk",
        DriveKind::Unknown => "Drive",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_at_least_the_system_drive() {
        let drives = list_drives();
        let system = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        let d = drives
            .iter()
            .find(|d| d.path.eq_ignore_ascii_case(&format!("{system}\\")))
            .expect("system drive listed");
        assert!(d.available);
        assert_eq!(d.kind, DriveKind::Fixed);
        assert!(d.total_bytes.unwrap() > 0);
    }
}
