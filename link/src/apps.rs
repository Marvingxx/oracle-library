//! The Oracle apps and where they are on this computer.
//!
//! An app is looked for, in order: the `ORACLE_<APP>_EXE` / `ORACLE_<APP>_CLI` environment variable
//! (tests, unusual installs); its installer's folder (`Program Files\<Name>`, the launcher's own
//! `%LOCALAPPDATA%\Oracle Suite`); then a development build in the suite's folder
//! (`<suite>/<repo>/target/…/release`), found from this program's location or `ORACLE_SUITE_DIR`.

use std::path::{Path, PathBuf};
use std::process::Command;

/// An Oracle app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum App {
    Edit,
    Vector,
    Guide,
    BrandKit,
    MockupStudio,
    Suite,
}

impl App {
    pub const ALL: [App; 6] = [App::Edit, App::Vector, App::Guide, App::BrandKit, App::MockupStudio, App::Suite];

    /// The app's id, as the launcher's catalog names it.
    pub fn id(self) -> &'static str {
        match self {
            App::Edit => "oracle-edit",
            App::Vector => "oracle-vector",
            App::Guide => "oracle-guide",
            App::BrandKit => "oracle-brand-kit",
            App::MockupStudio => "oracle-mockup-studio",
            App::Suite => "oracle-suite",
        }
    }

    /// The name people see (and its installer's folder name).
    pub fn name(self) -> &'static str {
        match self {
            App::Edit => "Oracle Edit",
            App::Vector => "Oracle Vector",
            App::Guide => "Oracle Brand Guide",
            App::BrandKit => "Oracle Brand Kit",
            App::MockupStudio => "Oracle Mockup Studio",
            App::Suite => "Oracle Suite",
        }
    }

    /// The app whose id is `id`.
    pub fn from_id(id: &str) -> Option<App> {
        App::ALL.into_iter().find(|a| a.id() == id)
    }

    /// The repository (folder of the suite) that builds the app.
    fn repo(self) -> &'static str {
        match self {
            App::Edit => "oracle-edit",
            App::Vector => "oracle-vector",
            App::Guide => "oracle-guide",
            App::BrandKit | App::MockupStudio => "oracle-brand",
            App::Suite => "oracle-launcher",
        }
    }

    /// The command-line engine's program name (without `.exe`), if the app has one.
    fn cli_stem(self) -> Option<&'static str> {
        match self {
            App::Edit => Some("oracle-edit-cli"),
            App::Vector => Some("oracle-vector-cli"),
            App::Guide => Some("oracle-guide-cli"),
            App::BrandKit => Some("oracle-brand"),
            App::MockupStudio | App::Suite => None,
        }
    }

    fn env_key(self, what: &str) -> String {
        format!("{}_{what}", self.id().to_ascii_uppercase().replace('-', "_"))
    }

    /// The app itself (the program that opens a window), if it can be found.
    pub fn exe(self) -> Option<PathBuf> {
        find(self, self.id(), &self.env_key("EXE"))
    }

    /// The app's command-line engine, if it has one and it can be found.
    pub fn cli(self) -> Option<PathBuf> {
        find(self, self.cli_stem()?, &self.env_key("CLI"))
    }
}

/// Where the program `stem` of `app` is: the environment variable `env`, the installer's folders,
/// then development builds.
fn find(app: App, stem: &str, env: &str) -> Option<PathBuf> {
    if let Some(p) = std::env::var_os(env).map(PathBuf::from) {
        // Set but wrong is a mistake to report, not a reason to pick another copy silently.
        if p.is_file() {
            return Some(p);
        }
        log::warn!("{env} = {} is not a file", p.display());
        return None;
    }
    let exe = format!("{stem}{}", std::env::consts::EXE_SUFFIX);
    candidates(app).into_iter().map(|dir| dir.join(&exe)).find(|p| p.is_file())
}

/// The folders an app's programs may be in, most likely first.
fn candidates(app: App) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for var in ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(root) = std::env::var_os(var) {
            dirs.push(PathBuf::from(root).join(app.name()));
        }
    }
    if let Some(local) = dirs::data_local_dir() {
        dirs.push(local.join(app.name()));
        dirs.push(local.join("Programs").join(app.name()));
    }
    // Development builds, in the suite's folder.
    for suite in suite_dirs() {
        let target = suite.join(app.repo()).join("target");
        for sub in ["windows-package/x64", "x86_64-pc-windows-msvc/release", "release", "debug"] {
            dirs.push(target.join(sub));
        }
    }
    dirs
}

/// The suite's folder (the one holding `oracle-edit`, `oracle-vector`, …): `ORACLE_SUITE_DIR`, or an
/// ancestor of this program or of this crate's sources that has an `oracle-library` folder.
fn suite_dirs() -> Vec<PathBuf> {
    let mut found = Vec::new();
    if let Some(dir) = std::env::var_os("ORACLE_SUITE_DIR") {
        found.push(PathBuf::from(dir));
    }
    let starts = [std::env::current_exe().ok(), Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")))];
    for start in starts.into_iter().flatten() {
        if let Some(suite) = start.ancestors().find(|a| a.join("oracle-library").join("link").is_dir())
            && !found.iter().any(|f| f == suite)
        {
            found.push(suite.to_path_buf());
        }
    }
    found
}

/// Opens `file` in `app` (a running app receives it: the Oracle apps forward a second launch to the
/// window already open). `Err` says why it couldn't, in French, for the interface.
pub fn open_in_app(app: App, file: &Path) -> Result<(), String> {
    let exe = app.exe().ok_or_else(|| format!("{} est introuvable sur cet ordinateur", app.name()))?;
    Command::new(&exe).arg(file).spawn().map(|_| ()).map_err(|e| format!("impossible de lancer {} : {e}", app.name()))
}

/// Edit Original for an Oracle document placed in another app: opens `file` in the app that made it
/// (an `.oracle-vector` in Oracle Vector). Saving it there updates it where it is placed (the link
/// watch). Returns that app; `Err` (French, for the interface) when `file` isn't an Oracle document
/// or the app can't be started.
pub fn edit_original(file: &Path) -> Result<App, String> {
    let format = crate::oracle_format(file).ok_or_else(|| format!("{} n'est pas un document Oracle", file.display()))?;
    open_in_app(format.app, file)?;
    Ok(format.app)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_names_and_programs() {
        for app in App::ALL {
            assert_eq!(App::from_id(app.id()), Some(app));
            assert!(app.name().starts_with("Oracle "));
        }
        assert_eq!(App::Vector.cli_stem(), Some("oracle-vector-cli"));
        assert_eq!(App::Edit.env_key("CLI"), "ORACLE_EDIT_CLI");
        assert_eq!(App::MockupStudio.cli_stem(), None);
    }

    #[test]
    fn the_suite_folder_is_found_from_the_sources() {
        assert!(suite_dirs().iter().any(|d| d.join("oracle-library").is_dir()));
    }

    #[test]
    fn an_environment_override_wins_and_a_wrong_one_finds_nothing() {
        let me = std::env::current_exe().unwrap();
        // Unique variable names: tests run in parallel and the environment is process-wide.
        unsafe { std::env::set_var("ORACLE_TEST_LINK_A", &me) };
        assert_eq!(find(App::Edit, "nope", "ORACLE_TEST_LINK_A"), Some(me));
        unsafe { std::env::set_var("ORACLE_TEST_LINK_B", "Z:/no/such/program.exe") };
        assert_eq!(find(App::Edit, "oracle-edit-cli", "ORACLE_TEST_LINK_B"), None);
    }
}
