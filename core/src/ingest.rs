//! Adding files to the library: what the user drops on the library, or sends to it from an app.
//!
//! Files are copied into the library folder's [`OWN_FOLDER`], never over another file (`logo.svg`
//! arriving twice becomes `logo (2).svg`); a file already inside the library folder stays where it is.
//! Nothing else in the library folder is touched.

use std::path::{Path, PathBuf};

/// The library's folder for what the user adds from Oracle.
pub const OWN_FOLDER: &str = "Mes créations";

/// The largest file copied in (a video or a huge PSD is fine; a disk image dropped by mistake isn't).
const MAX_BYTES: u64 = 8 << 30;

/// Copies `files` into `root`'s [`OWN_FOLDER`]. Returns where each one is now in the library, in
/// order; `Err` names the first file that couldn't be added (those before it are added).
pub fn add(root: &Path, files: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let into = root.join(OWN_FOLDER);
    let root_abs = std::path::absolute(root).unwrap_or_else(|_| root.to_path_buf());
    let mut added = Vec::new();
    for file in files {
        let name = file.file_name().ok_or_else(|| format!("{} : ce n'est pas un fichier", file.display()))?;
        let meta = std::fs::metadata(file).map_err(|e| format!("{} : {e}", file.display()))?;
        if !meta.is_file() {
            return Err(format!("{} : seuls les fichiers s'ajoutent, pas les dossiers", file.display()));
        }
        if meta.len() > MAX_BYTES {
            return Err(format!("{} : fichier trop gros pour la bibliothèque", file.display()));
        }
        // Already in the library: nothing to copy.
        if std::path::absolute(file).is_ok_and(|f| f.starts_with(&root_abs)) {
            added.push(file.clone());
            continue;
        }
        std::fs::create_dir_all(&into).map_err(|e| format!("{} : {e}", into.display()))?;
        let target = free_name(&into, Path::new(name));
        std::fs::copy(file, &target).map_err(|e| format!("{} : {e}", file.display()))?;
        added.push(target);
    }
    Ok(added)
}

/// `name` in `dir`, or `stem (2).ext`, `stem (3).ext`… when it is taken.
fn free_name(dir: &Path, name: &Path) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let stem = name.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    let ext = name.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .unwrap_or(first)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("oracle-ingest-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn files_are_copied_into_the_own_folder_without_overwriting() {
        let root = temp("root");
        let outside = temp("outside");
        let logo = outside.join("logo.svg");
        std::fs::write(&logo, "<svg/>").unwrap();
        let first = add(&root, std::slice::from_ref(&logo)).unwrap();
        assert_eq!(first, [root.join(OWN_FOLDER).join("logo.svg")]);
        let second = add(&root, std::slice::from_ref(&logo)).unwrap();
        assert_eq!(second, [root.join(OWN_FOLDER).join("logo (2).svg")]);
        assert!(logo.exists(), "the original stays where it was");
        assert_eq!(std::fs::read_to_string(&second[0]).unwrap(), "<svg/>");
    }

    #[test]
    fn a_file_already_in_the_library_stays_put_and_folders_are_refused() {
        let root = temp("inside");
        let own = root.join("01_Textures").join("metal.jpg");
        std::fs::create_dir_all(own.parent().unwrap()).unwrap();
        std::fs::write(&own, "x").unwrap();
        assert_eq!(add(&root, std::slice::from_ref(&own)).unwrap(), std::slice::from_ref(&own));
        assert!(!root.join(OWN_FOLDER).exists(), "nothing was copied");
        assert!(add(&root, &[root.join("01_Textures")]).unwrap_err().contains("dossiers"));
    }
}
