//! What the user adds to the library: favourites, their own tags, recently used files, and where
//! the library folder is. Shared by every Oracle app through one file in the user's data folder
//! (`%APPDATA%\Oracle\library.json` on Windows), so a tag added in Oracle Edit shows in Oracle Suite.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// How many recently used files are remembered.
const MAX_RECENT: usize = 60;
/// The largest settings file read back.
const MAX_FILE_BYTES: u64 = 32 << 20;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserData {
    pub version: u32,
    /// The library folder; empty until chosen (or found on the Desktop).
    pub root: String,
    pub favorites: BTreeSet<String>,
    /// Asset id → the user's own tags, as typed.
    pub tags: BTreeMap<String, Vec<String>>,
    /// Most recent first.
    pub recent: Vec<String>,
}

impl UserData {
    /// The shared file: `<config dir>/Oracle/library.json`.
    pub fn default_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("Oracle").join("library.json"))
    }

    /// Reads `path`; a missing or damaged file gives empty data (never an error that blocks the
    /// library), and a damaged one is kept aside as `library.json.bad` so nothing is lost.
    pub fn load(path: &Path) -> UserData {
        let Ok(meta) = std::fs::metadata(path) else { return UserData::default() };
        if meta.len() > MAX_FILE_BYTES {
            return UserData::default();
        }
        match std::fs::read(path).map_err(|e| e.to_string()).and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string())) {
            Ok(data) => data,
            Err(e) => {
                log::warn!("library settings unreadable ({e}); starting afresh");
                let _ = std::fs::rename(path, path.with_extension("json.bad"));
                UserData::default()
            }
        }
    }

    /// Writes `path` atomically (a temporary file, then a rename).
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let mut data = self.clone();
        data.version = 1;
        let json = serde_json::to_vec_pretty(&data).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())
    }

    /// The library folder to use: the chosen one, else `library` on the Desktop when it exists.
    pub fn library_root(&self) -> Option<PathBuf> {
        if !self.root.is_empty() {
            return Some(PathBuf::from(&self.root));
        }
        let desktop = dirs::desktop_dir()?;
        ["library", "Library", "Bibliothèque", "Bibliotheque"].iter().map(|n| desktop.join(n)).find(|p| p.is_dir())
    }

    pub fn is_favorite(&self, id: &str) -> bool {
        self.favorites.contains(id)
    }

    pub fn toggle_favorite(&mut self, id: &str) {
        if !self.favorites.remove(id) {
            self.favorites.insert(id.to_string());
        }
    }

    pub fn tags_of(&self, id: &str) -> Vec<String> {
        self.tags.get(id).cloned().unwrap_or_default()
    }

    /// Adds `tag` (trimmed; ignored when empty or already there, whatever its case and accents).
    pub fn add_tag(&mut self, id: &str, tag: &str) {
        let tag = tag.trim();
        if tag.is_empty() {
            return;
        }
        let list = self.tags.entry(id.to_string()).or_default();
        let folded = crate::fold::fold(tag);
        if !list.iter().any(|t| crate::fold::fold(t) == folded) {
            list.push(tag.to_string());
        }
    }

    pub fn remove_tag(&mut self, id: &str, tag: &str) {
        if let Some(list) = self.tags.get_mut(id) {
            list.retain(|t| t != tag);
            if list.is_empty() {
                self.tags.remove(id);
            }
        }
    }

    /// Every tag the user has used, with how many files carry it, most used first.
    pub fn all_tags(&self) -> Vec<(String, usize)> {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for list in self.tags.values() {
            for t in list {
                *counts.entry(t.clone()).or_insert(0) += 1;
            }
        }
        let mut out: Vec<_> = counts.into_iter().collect();
        out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        out
    }

    /// Notes that `id` was just used (opened or dragged).
    pub fn touch(&mut self, id: &str) {
        self.recent.retain(|r| r != id);
        self.recent.insert(0, id.to_string());
        self.recent.truncate(MAX_RECENT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_dedupe_whatever_the_accents() {
        let mut u = UserData::default();
        u.add_tag("a", "Métal brossé");
        u.add_tag("a", "metal brosse");
        u.add_tag("a", "  ");
        assert_eq!(u.tags_of("a"), ["Métal brossé"]);
        u.remove_tag("a", "Métal brossé");
        assert!(u.tags.is_empty());
    }

    #[test]
    fn saves_and_loads_and_survives_a_damaged_file() {
        let dir = std::env::temp_dir().join(format!("oracle-library-user-{}", std::process::id()));
        let path = dir.join("library.json");
        let mut u = UserData::default();
        u.toggle_favorite("x");
        u.touch("y");
        u.save(&path).unwrap();
        let back = UserData::load(&path);
        assert!(back.is_favorite("x") && back.recent == ["y"]);
        std::fs::write(&path, b"{not json").unwrap();
        assert_eq!(UserData::load(&path), UserData::default());
        assert!(dir.join("library.json.bad").exists());
    }
}
