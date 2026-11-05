//! Small JSON files in Nexdir's own configuration folder (rules, index settings).
//! Writes go to an exclusively created temporary file first, which is then renamed through
//! its handle, so a crash never leaves a half-written configuration behind and no other file
//! can be overwritten or deleted by accident.

use crate::platform;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
}

impl AppPaths {
    pub fn rules_file(&self) -> PathBuf {
        self.config_dir.join("rules.json")
    }
    pub fn index_settings_file(&self) -> PathBuf {
        self.config_dir.join("index-settings.json")
    }
    pub fn update_settings_file(&self) -> PathBuf {
        self.config_dir.join("update-settings.json")
    }
    /// Saved network locations (paths and names only, never credentials).
    pub fn network_file(&self) -> PathBuf {
        self.config_dir.join("network.json")
    }
    pub fn index_db(&self) -> PathBuf {
        self.data_dir.join("index").join("index.sqlite")
    }
}

/// Reads a JSON file; a missing or unreadable file yields the default value.
pub fn load<T: DeserializeOwned + Default>(path: &Path) -> T {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn save<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    use std::io::Write;
    use std::os::windows::fs::OpenOptionsExt;
    let dir = path.parent().ok_or("Invalid settings path.")?;
    fs::create_dir_all(dir).map_err(|e| format!("Cannot create the settings folder: {e}"))?;
    let json = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;

    // The temporary file is created exclusively under a fresh name (never an existing file),
    // renamed and, on failure, removed through its handle — never by path.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = dir.join(format!(".{name}.{:x}-{nanos:x}.tmp", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .access_mode(platform::ACCESS_GENERIC_WRITE | platform::ACCESS_DELETE)
        .share_mode(0)
        .open(&tmp)
        .map_err(|e| format!("Cannot save settings: {e}"))?;
    let written = file
        .write_all(&json)
        .and_then(|()| file.sync_all())
        // Replacing is intended here: `path` is Nexdir's own settings file.
        .and_then(|()| platform::rename_by_handle(&file, path, true));
    if let Err(e) = written {
        let _ = platform::delete_by_handle(&file);
        return Err(format!("Cannot save settings: {e}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("nested/settings.json");
        assert_eq!(load::<Vec<String>>(&file), Vec::<String>::new());
        save(&file, &vec!["a".to_string()]).unwrap();
        assert_eq!(load::<Vec<String>>(&file), vec!["a".to_string()]);
        fs::write(&file, "{ not json").unwrap();
        assert_eq!(load::<Vec<String>>(&file), Vec::<String>::new());
    }

    #[test]
    fn save_never_touches_a_foreign_file_with_the_old_temp_name() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("rules.json");
        // The previous implementation wrote to "rules.tmp" and truncated whatever was there.
        fs::write(dir.path().join("rules.tmp"), "foreign").unwrap();
        save(&file, &vec!["a".to_string()]).unwrap();
        save(&file, &vec!["b".to_string()]).unwrap();
        assert_eq!(load::<Vec<String>>(&file), vec!["b".to_string()]);
        assert_eq!(fs::read_to_string(dir.path().join("rules.tmp")).unwrap(), "foreign");
        let names: Vec<_> = fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names.len(), 2, "no temporary files remain: {names:?}");
    }
}
