//! Saved network locations ("favourites"). Only a display name and the UNC path are stored,
//! in `network.json` in Nexdir's settings folder: never a user name or password. Favourites stay
//! visible when the server is offline, and loading them never contacts the network.

use super::unc::{self, UncKind};
use crate::error::{AppError, AppResult, ErrorCode};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const MAX_FAVORITES: usize = 200;
const MAX_LABEL: usize = 120;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Favorite {
    /// UNC path of a share or a folder in a share (canonical form).
    pub path: String,
    /// Name shown in Nexdir; empty means "use the share name".
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSettings {
    #[serde(default)]
    pub favorites: Vec<Favorite>,
}

/// Validates and normalises a favourite. A bare server is not a folder and is refused.
pub fn normalize(f: &Favorite) -> AppResult<Favorite> {
    let parsed = unc::parse(&f.path)?;
    if parsed.kind() == UncKind::Server {
        return Err(AppError::new(
            ErrorCode::InvalidPath,
            "Choose a share, for example \\\\NAS\\Media. A server alone cannot be saved.",
        )
        .with_path(&f.path));
    }
    let label: String = f.label.trim().chars().filter(|c| !c.is_control()).take(MAX_LABEL).collect();
    Ok(Favorite {
        path: parsed.display(),
        label,
    })
}

/// Validates a whole list: normalised paths, no duplicates (case-insensitive), bounded size.
pub fn normalize_all(list: &[Favorite]) -> AppResult<Vec<Favorite>> {
    if list.len() > MAX_FAVORITES {
        return Err(AppError::new(
            ErrorCode::Unsupported,
            format!("At most {MAX_FAVORITES} network favourites can be saved."),
        ));
    }
    let mut out: Vec<Favorite> = Vec::with_capacity(list.len());
    for f in list {
        let n = normalize(f)?;
        if !out
            .iter()
            .any(|o| o.path.eq_ignore_ascii_case(&n.path) || o.path.to_lowercase() == n.path.to_lowercase())
        {
            out.push(n);
        }
    }
    Ok(out)
}

/// Loads favourites; entries that are no longer valid are dropped, never "repaired".
pub fn load(file: &Path) -> Vec<Favorite> {
    let settings: NetworkSettings = crate::store::load(file);
    settings
        .favorites
        .iter()
        .filter_map(|f| normalize(f).ok())
        .take(MAX_FAVORITES)
        .collect()
}

pub fn save(file: &Path, list: &[Favorite]) -> AppResult<Vec<Favorite>> {
    let favorites = normalize_all(list)?;
    crate::store::save(
        file,
        &NetworkSettings {
            favorites: favorites.clone(),
        },
    )
    .map_err(|e| AppError::new(ErrorCode::Io, e))?;
    Ok(favorites)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fav(path: &str, label: &str) -> Favorite {
        Favorite {
            path: path.into(),
            label: label.into(),
        }
    }

    #[test]
    fn stores_only_path_and_label() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("network.json");
        let saved = save(&file, &[fav("//NAS/Media/", "  Home NAS  "), fav(r"\\nas\backups", "")]).unwrap();
        assert_eq!(saved, vec![fav(r"\\NAS\Media", "Home NAS"), fav(r"\\nas\backups", "")]);
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(!text.to_lowercase().contains("password") && !text.to_lowercase().contains("user"));
        assert_eq!(load(&file), saved);
    }

    #[test]
    fn removes_duplicates_and_refuses_bad_entries() {
        let list = normalize_all(&[fav(r"\\NAS\Media", "A"), fav(r"\\nas\media\", "B")]).unwrap();
        assert_eq!(list.len(), 1);
        assert!(normalize(&fav(r"\\NAS", "x")).is_err());
        assert!(normalize(&fav(r"C:\Users", "x")).is_err());
        assert!(normalize(&fav(r"\\.\pipe\x", "x")).is_err());
        assert!(normalize(&fav(r"\\nas\share\..\x", "x")).is_err());
        let many: Vec<Favorite> = (0..=MAX_FAVORITES).map(|i| fav(&format!(r"\\nas\s{i}"), "")).collect();
        assert!(normalize_all(&many).is_err());
    }

    #[test]
    fn invalid_saved_entries_are_dropped_on_load() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("network.json");
        std::fs::write(
            &file,
            r#"{"favorites":[{"path":"\\\\nas\\ok","label":"ok"},{"path":"\\\\.\\pipe\\x","label":"bad"}]}"#,
        )
        .unwrap();
        assert_eq!(load(&file), vec![fav(r"\\nas\ok", "ok")]);
        // A missing or broken file means "no favourites", not an error.
        std::fs::write(&file, "not json").unwrap();
        assert!(load(&file).is_empty());
    }

    #[test]
    fn labels_are_trimmed_and_bounded() {
        let f = normalize(&fav(r"\\nas\s", &format!("a\u{7}{}", "b".repeat(300)))).unwrap();
        assert_eq!(f.label.chars().count(), MAX_LABEL);
        assert!(!f.label.contains('\u{7}'));
    }
}
