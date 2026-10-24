//! The brands of Oracle Brand Kit, for every app: their logos, palette and type, read from each
//! brand project's own `brand.json`, so what an app shows is always the brand as it is now.
//!
//! Brand Kit (and Mockup Studio) note each brand project they open in a small registry,
//! `%APPDATA%\Oracle\brands.json`; nothing else is copied. The library shows each brand as a
//! collection, Oracle Edit and Oracle Vector its colours as swatches. An app that wants to follow
//! changes compares [`Registry::stamp`] from time to time (a few file times, nothing parsed).
//! Archived brands are left out.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// The registry's file name, next to the library's settings.
const FILE: &str = "brands.json";
/// The brand project file inside a project folder.
pub const PROJECT_FILE: &str = "brand.json";
/// The largest `brand.json` read (a real one is a few kilobytes).
const MAX_PROJECT_BYTES: u64 = 16 << 20;

/// One brand, as its project describes it.
#[derive(Clone, Debug, PartialEq)]
pub struct Brand {
    /// The project's stable id (`projectId`).
    pub id: String,
    pub name: String,
    /// The project folder.
    pub dir: PathBuf,
    pub colors: Vec<BrandColor>,
    pub logos: Vec<BrandLogo>,
    pub fonts: Vec<BrandFont>,
}

/// A palette colour: its name and sRGB value, plus the CMYK values the designer typed in.
#[derive(Clone, Debug, PartialEq)]
pub struct BrandColor {
    pub id: String,
    pub name: String,
    /// `#RRGGBB`, upper case.
    pub hex: String,
    pub rgb: [u8; 3],
    /// Cyan, magenta, yellow, black in percent, when given.
    pub cmyk: Option<[f32; 4]>,
    pub pantone: String,
}

/// A logo master: the designer's own file (an `.oracle-vector`, an SVG, a PSD…), never a copy.
#[derive(Clone, Debug, PartialEq)]
pub struct BrandLogo {
    pub id: String,
    /// The designer's name for it, or its role and variant ("Logo principal — Noir").
    pub label: String,
    /// "Logo principal", "Symbole"…
    pub role: String,
    /// "Couleur", "Noir"…
    pub variant: String,
    pub file: PathBuf,
}

/// A typeface and the font files the designer keeps for it.
#[derive(Clone, Debug, PartialEq)]
pub struct BrandFont {
    pub id: String,
    pub family: String,
    pub usage: String,
    pub files: Vec<PathBuf>,
}

/// The list of brand projects.
#[derive(Clone, Debug)]
pub struct Registry {
    file: PathBuf,
}

impl Registry {
    /// The user's registry (`%APPDATA%\Oracle\brands.json`).
    pub fn user() -> Option<Registry> {
        Some(Registry::at(dirs::config_dir()?.join("Oracle").join(FILE)))
    }

    /// A registry in `file` (tests, or another place).
    pub fn at(file: impl Into<PathBuf>) -> Registry {
        Registry { file: file.into() }
    }

    /// The project folders listed, in the order they were first noted.
    pub fn projects(&self) -> Vec<PathBuf> {
        let Ok(bytes) = std::fs::read(&self.file) else { return Vec::new() };
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            log::warn!("{} is unreadable", self.file.display());
            return Vec::new();
        };
        v.get("projects").and_then(|p| p.as_array()).into_iter().flatten().filter_map(|p| p.as_str()).map(PathBuf::from).collect()
    }

    /// Notes the brand project in `dir` (a folder holding `brand.json`). Nothing is written when it
    /// is already listed.
    pub fn register(&self, dir: &Path) -> Result<(), String> {
        let dir = std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf());
        let mut projects = self.projects();
        if projects.iter().any(|p| same_dir(p, &dir)) {
            return Ok(());
        }
        projects.push(dir);
        self.write(&projects)
    }

    /// Takes the project in `dir` off the list (its files are not touched).
    pub fn forget(&self, dir: &Path) -> Result<(), String> {
        let dir = std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf());
        let mut projects = self.projects();
        let before = projects.len();
        projects.retain(|p| !same_dir(p, &dir));
        if projects.len() == before { Ok(()) } else { self.write(&projects) }
    }

    fn write(&self, projects: &[PathBuf]) -> Result<(), String> {
        if let Some(parent) = self.file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{} : {e}", parent.display()))?;
        }
        let list: Vec<String> = projects.iter().map(|p| p.display().to_string()).collect();
        let json = serde_json::to_vec_pretty(&serde_json::json!({ "projects": list })).map_err(|e| e.to_string())?;
        // Written whole: another app may be reading it.
        let tmp = self.file.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| format!("{} : {e}", tmp.display()))?;
        std::fs::rename(&tmp, &self.file).map_err(|e| format!("{} : {e}", self.file.display()))
    }

    /// Every listed brand that can be read and isn't archived, in registry order. A project that
    /// moved or can't be read is skipped (and stays listed: its drive may only be unplugged).
    pub fn brands(&self) -> Vec<Brand> {
        let mut seen = std::collections::HashSet::new();
        self.projects().iter().filter_map(|dir| read_project(dir).map_err(|e| log::info!("brand project skipped: {e}")).ok()).filter(|b| seen.insert(b.id.clone())).collect()
    }

    /// Changes whenever the list or one of its projects changes: the file times and sizes of the
    /// registry and of each `brand.json`. Cheap enough to call every few seconds.
    pub fn stamp(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let mut file_stamp = |p: &Path| {
            let meta = std::fs::metadata(p).ok();
            meta.as_ref().map(|m| m.len()).hash(&mut h);
            meta.and_then(|m| m.modified().ok()).unwrap_or(SystemTime::UNIX_EPOCH).hash(&mut h);
        };
        file_stamp(&self.file);
        for dir in self.projects() {
            file_stamp(&dir.join(PROJECT_FILE));
        }
        h.finish()
    }
}

/// The brands of the user's registry.
pub fn brands() -> Vec<Brand> {
    Registry::user().map(|r| r.brands()).unwrap_or_default()
}

/// Notes the brand project in `dir` in the user's registry (Brand Kit does when it opens one).
pub fn register(dir: &Path) {
    if let Some(r) = Registry::user()
        && let Err(e) = r.register(dir)
    {
        log::warn!("couldn't note the brand project {}: {e}", dir.display());
    }
}

/// The user's [`Registry::stamp`] (0 without a user folder).
pub fn stamp() -> u64 {
    Registry::user().map(|r| r.stamp()).unwrap_or(0)
}

fn same_dir(a: &Path, b: &Path) -> bool {
    if cfg!(windows) { a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy()) } else { a == b }
}

/// Reads the brand project in `dir`.
pub fn read_project(dir: &Path) -> Result<Brand, String> {
    let file = dir.join(PROJECT_FILE);
    let meta = std::fs::metadata(&file).map_err(|e| format!("{} : {e}", file.display()))?;
    if meta.len() > MAX_PROJECT_BYTES {
        return Err(format!("{} : fichier trop gros", file.display()));
    }
    let bytes = std::fs::read(&file).map_err(|e| format!("{} : {e}", file.display()))?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| format!("{} : {e}", file.display()))?;
    if v.get("status").and_then(|s| s.as_str()) == Some("archived") {
        return Err(format!("{} : marque archivée", dir.display()));
    }
    let name = text(&v, "brandName");
    let name = if name.is_empty() { text(&v, "name") } else { name };
    let id = text(&v, "projectId");
    let id = if id.is_empty() { dir.display().to_string() } else { id };
    let list = |key: &str| v.get(key).and_then(|l| l.as_array()).cloned().unwrap_or_default();

    let colors = list("colors")
        .iter()
        .filter_map(|c| {
            let rgb = parse_hex(&text(c, "hex"))?;
            let cmyk = c.get("cmyk").filter(|k| k.is_object()).map(|k| ["c", "m", "y", "k"].map(|n| k.get(n).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32));
            Some(BrandColor { id: text(c, "id"), name: text(c, "name"), hex: format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]), rgb, cmyk, pantone: text(c, "pantone") })
        })
        .collect();
    let logos = list("assets")
        .iter()
        .filter_map(|a| {
            let source = text(a, "source");
            if source.is_empty() {
                return None;
            }
            let role = role_fr(&text(a, "role"));
            let variant = variant_fr(&text(a, "variant"));
            let label = text(a, "label");
            let label = if label.trim().is_empty() { format!("{role} — {variant}") } else { label };
            Some(BrandLogo { id: text(a, "id"), label, role: role.into(), variant: variant.into(), file: resolve(dir, &source) })
        })
        .collect();
    let fonts = list("fonts")
        .iter()
        .map(|f| BrandFont {
            id: text(f, "id"),
            family: text(f, "family"),
            usage: text(f, "usage"),
            files: f.get("localFiles").and_then(|l| l.as_array()).into_iter().flatten().filter_map(|p| p.as_str()).map(|p| resolve(dir, p)).collect(),
        })
        .collect();
    Ok(Brand { id, name, dir: dir.to_path_buf(), colors, logos, fonts })
}

fn text(v: &serde_json::Value, key: &str) -> String {
    v.get(key).and_then(|s| s.as_str()).unwrap_or_default().to_string()
}

/// `#RRGGBB` (or `RRGGBB`) → (r, g, b).
fn parse_hex(hex: &str) -> Option<[u8; 3]> {
    let h = hex.trim();
    let h = h.strip_prefix('#').unwrap_or(h);
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// A path as Brand Kit stores it: relative to the project folder (with `/`) or absolute.
fn resolve(dir: &Path, stored: &str) -> PathBuf {
    let p = Path::new(stored);
    if p.is_absolute() {
        return p.to_path_buf();
    }
    let mut out = dir.to_path_buf();
    for part in stored.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            p => out.push(p),
        }
    }
    out
}

fn role_fr(role: &str) -> &'static str {
    match role {
        "primaryLogo" => "Logo principal",
        "horizontalLogo" => "Logo horizontal",
        "verticalLogo" => "Logo vertical",
        "symbol" => "Symbole",
        "favicon" => "Favicon",
        "signature" => "Signature",
        "secondaryLogo" => "Version secondaire",
        _ => "Autre",
    }
}

fn variant_fr(variant: &str) -> &'static str {
    match variant {
        "black" => "Noir",
        "white" => "Blanc",
        "monochrome" => "Monochrome",
        "inverted" => "Inversée",
        "simplified" => "Simplifiée",
        _ => "Couleur",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("oracle-brands-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn project(dir: &Path, json: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(PROJECT_FILE), json).unwrap();
    }

    const MAISON: &str = r##"{
        "schemaVersion": 1, "projectId": "p-1", "brandName": "Maison Été", "status": "inProgress",
        "assets": [
            {"id": "a-1", "role": "primaryLogo", "variant": "color", "source": "Sources/logo.oracle-vector"},
            {"id": "a-2", "role": "symbol", "label": "Étoile", "variant": "black", "source": "C:/Ailleurs/étoile.svg"},
            {"id": "a-3", "role": "other", "source": ""}
        ],
        "colors": [
            {"id": "c-1", "name": "Rouge Été", "hex": "#d31c3b", "cmyk": {"c": 0, "m": 95, "y": 75, "k": 5, "profile": ""}, "pantone": "186 C"},
            {"id": "c-2", "name": "Nuit", "hex": "#101820"},
            {"id": "c-3", "name": "Faux", "hex": "rouge"}
        ],
        "fonts": [{"id": "f-1", "family": "Lexend", "usage": "Titres", "localFiles": ["Polices/Lexend.ttf"]}]
    }"##;

    #[test]
    fn a_project_is_read_with_its_paths_resolved() {
        let root = temp("read");
        let dir = root.join("Maison Été");
        project(&dir, MAISON);
        let b = read_project(&dir).unwrap();
        assert_eq!((b.id.as_str(), b.name.as_str()), ("p-1", "Maison Été"));
        assert_eq!(b.colors.len(), 2, "a colour without a hex value is left out");
        assert_eq!(b.colors[0].hex, "#D31C3B");
        assert_eq!(b.colors[0].rgb, [0xd3, 0x1c, 0x3b]);
        assert_eq!(b.colors[0].cmyk, Some([0.0, 95.0, 75.0, 5.0]));
        assert_eq!(b.colors[1].cmyk, None);
        assert_eq!(b.logos.len(), 2, "a logo without a file is left out");
        assert_eq!(b.logos[0].label, "Logo principal — Couleur");
        assert_eq!(b.logos[0].file, dir.join("Sources").join("logo.oracle-vector"));
        assert_eq!(b.logos[1].label, "Étoile");
        assert_eq!(b.logos[1].variant, "Noir");
        assert_eq!(b.fonts[0].files, [dir.join("Polices").join("Lexend.ttf")]);
    }

    #[test]
    fn the_registry_lists_each_project_once_and_skips_what_it_cant_read() {
        let root = temp("registry");
        let reg = Registry::at(root.join("Oracle").join(FILE));
        assert!(reg.brands().is_empty(), "no file yet: no brands");
        let a = root.join("a");
        project(&a, MAISON);
        let archived = root.join("archived");
        project(&archived, r#"{"projectId": "p-2", "brandName": "Ancienne", "status": "archived"}"#);
        reg.register(&a).unwrap();
        reg.register(&a).unwrap();
        reg.register(&archived).unwrap();
        reg.register(&root.join("gone")).unwrap();
        assert_eq!(reg.projects().len(), 3);
        let brands = reg.brands();
        assert_eq!(brands.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), ["Maison Été"]);
        reg.forget(&root.join("gone")).unwrap();
        assert_eq!(reg.projects().len(), 2);
    }

    #[test]
    fn the_stamp_follows_a_project_change() {
        let root = temp("stamp");
        let reg = Registry::at(root.join(FILE));
        let a = root.join("a");
        project(&a, MAISON);
        reg.register(&a).unwrap();
        let before = reg.stamp();
        assert_eq!(before, reg.stamp(), "nothing changed");
        // A different size is enough even on file systems with coarse times.
        project(&a, &MAISON.replace("#101820", "#202020").replace("\"Nuit\"", "\"Nuit profonde\""));
        assert_ne!(before, reg.stamp());
    }
}
