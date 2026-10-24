//! The library index: every creative file under the library folder, with what search needs.
//!
//! The folder may carry a catalogue made beforehand (`00_Catalogue/Integration/library.manifest.json`,
//! schema v1): its ids, tags, sizes, licences and preview pictures are used as they are. Files the
//! catalogue doesn't list yet (added since it was made) are indexed from their names and folders.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::fold::words;

/// Where the prepared catalogue lives, relative to the library folder.
pub const MANIFEST: &str = "00_Catalogue/Integration/library.manifest.json";
/// The catalogue's own folder: previews and reports, not assets.
pub const CATALOGUE_DIR: &str = "00_Catalogue";
/// The largest manifest we read (the real one is ~25 MB).
const MAX_MANIFEST_BYTES: u64 = 512 << 20;
/// How deep folders are followed.
const MAX_DEPTH: usize = 24;

/// What kind of file an asset is, for the format filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FormatGroup {
    /// Documents of the Oracle apps (`.oracle-vector`, `.pcraft`, `.oracle-guide`).
    Oracle,
    Image,
    Vector,
    Font,
    Photoshop,
    Layout,
    Video,
    Other,
}

impl FormatGroup {
    pub const ALL: [FormatGroup; 8] =
        [FormatGroup::Oracle, FormatGroup::Image, FormatGroup::Vector, FormatGroup::Font, FormatGroup::Photoshop, FormatGroup::Layout, FormatGroup::Video, FormatGroup::Other];

    /// The group of a file extension (any case, without the dot).
    pub fn of(ext: &str) -> FormatGroup {
        match ext.to_ascii_lowercase().as_str() {
            "oracle-vector" | "pcraft" | "oracle-guide" => FormatGroup::Oracle,
            "jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp" | "tif" | "tiff" | "heic" | "avif" | "exr" | "hdr" | "tga" => FormatGroup::Image,
            "svg" | "svgz" | "ai" | "eps" | "pdf" | "emf" | "wmf" | "dxf" => FormatGroup::Vector,
            "ttf" | "otf" | "woff" | "woff2" | "ttc" => FormatGroup::Font,
            "psd" | "psb" | "abr" | "pat" | "grd" | "asl" | "atn" | "jsx" | "aco" | "ase" => FormatGroup::Photoshop,
            "indd" | "idml" | "indt" => FormatGroup::Layout,
            "mp4" | "mov" | "webm" | "mogrt" | "fcpxml" | "setting" => FormatGroup::Video,
            _ => FormatGroup::Other,
        }
    }

    /// Whether files of this extension belong in the library at all (documents and notes don't).
    pub fn is_creative(ext: &str) -> bool {
        !matches!(ext.to_ascii_lowercase().as_str(), "txt" | "md" | "json" | "jsonl" | "csv" | "xml" | "html" | "htm" | "js" | "mjs" | "py" | "ini" | "db" | "ds_store" | "url" | "lnk" | "log")
    }
}

/// One file of the library.
#[derive(Clone, Debug)]
pub struct Asset {
    /// Stable id: the catalogue's uuid, or `path:<relative path>` for a file it doesn't list.
    pub id: String,
    /// Display name, without extension.
    pub name: String,
    /// Path relative to the library folder, with `/`.
    pub path: String,
    /// Upper-case extension (`JPG`, `SVG`).
    pub format: String,
    pub group: FormatGroup,
    /// The top-level section, without its number (`Textures photo`).
    pub kind: String,
    /// The second-level folder (`Metal`), if any.
    pub category: String,
    pub bytes: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Folded words to search: the catalogue's tags plus the words of the name and folders.
    pub tags: Vec<String>,
    pub license: String,
    pub source: String,
    /// A small picture to show for it, relative to the library folder.
    pub preview: Option<String>,
    /// Folded name words, matched first.
    pub(crate) name_words: Vec<String>,
    /// A file of a collection outside the library folder (a brand's logo): `path` and `preview`
    /// are then absolute, and the file is the collection's own, never a copy.
    pub outside: bool,
}

impl Asset {
    /// The file on disk.
    pub fn file(&self, root: &Path) -> PathBuf {
        if self.outside { PathBuf::from(&self.path) } else { resolve(root, &self.path) }
    }

    /// Its preview picture on disk, if the catalogue made one.
    pub fn preview_file(&self, root: &Path) -> Option<PathBuf> {
        self.preview.as_deref().map(|p| if self.outside { PathBuf::from(p) } else { resolve(root, p) })
    }

    /// "6120 × 4080" when known.
    pub fn dimensions(&self) -> Option<String> {
        Some(format!("{} × {}", self.width?, self.height?))
    }
}

/// `root` joined with a `/`-separated relative path, never leaving `root`.
pub fn resolve(root: &Path, rel: &str) -> PathBuf {
    let mut out = root.to_path_buf();
    for part in rel.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.contains(['\\', ':']) {
            continue;
        }
        out.push(part);
    }
    out
}

/// The whole library.
#[derive(Debug, Default)]
pub struct Library {
    pub root: PathBuf,
    /// The catalogue's id, when there is one.
    pub library_id: Option<String>,
    pub assets: Vec<Asset>,
    /// Top-level sections in folder order, with their asset counts.
    pub kinds: Vec<(String, usize)>,
    /// How many files the catalogue listed, and how many were found beyond it.
    pub from_catalogue: usize,
    pub found_since: usize,
    /// Collections of files that live elsewhere (each brand of Oracle Brand Kit), after the
    /// sections: their assets are in `assets` with [`Asset::outside`] set.
    pub collections: Vec<Collection>,
}

/// A set of files outside the library folder shown as one section (a brand's logos and fonts).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collection {
    /// Its assets' [`Asset::kind`] (`Marque · Maison Été`), unique among sections and collections.
    pub kind: String,
    /// What the sidebar shows (`Maison Été`).
    pub name: String,
    pub count: usize,
}

/// One file of a [`Collection`].
#[derive(Clone, Debug)]
pub struct CollectionFile {
    /// Stable id, unique in the library (`brand:<project>:<logo>`).
    pub id: String,
    pub name: String,
    pub file: PathBuf,
    /// Shown as the asset's category (`Logo principal`).
    pub category: String,
    /// Words to find it by, beyond its name and the collection's.
    pub tags: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    library_id: Option<String>,
    #[serde(default)]
    assets: Vec<ManifestAsset>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestAsset {
    id: String,
    name: String,
    path: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    bytes: u64,
    width: Option<u32>,
    height: Option<u32>,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    source: String,
    #[serde(default)]
    license: String,
    preview: Option<String>,
}

impl Library {
    /// Indexes the library at `root`: the catalogue if there is one, then every file it misses.
    pub fn open(root: &Path) -> Result<Library, String> {
        if !root.is_dir() {
            return Err(format!("{} n'est pas un dossier", root.display()));
        }
        let mut by_path: HashMap<String, ManifestAsset> = HashMap::new();
        let mut library_id = None;
        let manifest_path = resolve(root, MANIFEST);
        if let Ok(meta) = std::fs::metadata(&manifest_path)
            && meta.len() <= MAX_MANIFEST_BYTES
        {
            match std::fs::read(&manifest_path).map_err(|e| e.to_string()).and_then(|b| serde_json::from_slice::<Manifest>(&b).map_err(|e| e.to_string())) {
                Ok(m) => {
                    library_id = m.library_id;
                    by_path = m.assets.into_iter().map(|a| (a.path.clone(), a)).collect();
                }
                Err(e) => log::warn!("library catalogue unreadable, indexing from the folders: {e}"),
            }
        }

        let mut files = Vec::new();
        walk(root, root, 0, &mut files);
        let mut assets = Vec::with_capacity(files.len());
        let (mut from_catalogue, mut found_since) = (0, 0);
        for (rel, bytes) in files {
            let asset = match by_path.remove(&rel) {
                Some(m) => {
                    from_catalogue += 1;
                    from_manifest(m)
                }
                None => {
                    found_since += 1;
                    from_path(&rel, bytes)
                }
            };
            assets.push(asset);
        }
        assets.sort_by(|a, b| a.path.cmp(&b.path));

        // Sections in folder order (their numbers sort them), named without the number.
        let mut kinds: BTreeMap<String, (String, usize)> = BTreeMap::new();
        for a in &assets {
            let top = a.path.split('/').next().unwrap_or_default().to_string();
            kinds.entry(top).or_insert_with(|| (a.kind.clone(), 0)).1 += 1;
        }
        Ok(Library {
            root: root.to_path_buf(),
            library_id,
            assets,
            kinds: kinds.into_values().collect(),
            from_catalogue,
            found_since,
            collections: Vec::new(),
        })
    }

    /// Adds a collection of files outside the library folder, named `name` in the sidebar, its
    /// assets of kind `kind`. Files that don't exist (an unplugged drive) are left out; a kind
    /// already used is replaced.
    pub fn add_collection(&mut self, kind: &str, name: &str, files: Vec<CollectionFile>) {
        self.remove_collection(kind);
        let mut count = 0;
        for f in files {
            let Ok(meta) = std::fs::metadata(&f.file) else { continue };
            if !meta.is_file() {
                continue;
            }
            let path = f.file.display().to_string();
            let ext = f.file.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
            let group = FormatGroup::of(&ext);
            let mut tags: Vec<String> = f.tags.iter().chain([&f.category, &name.to_string()]).flat_map(|t| words(t)).filter(|w| w.len() > 1).collect();
            tags.sort();
            tags.dedup();
            let picture = group == FormatGroup::Image || ext.eq_ignore_ascii_case("svg");
            self.assets.push(Asset {
                id: f.id,
                name_words: words(&f.name),
                name: f.name,
                preview: picture.then(|| path.clone()),
                path,
                format: ext.to_ascii_uppercase(),
                group,
                kind: kind.to_string(),
                category: f.category,
                bytes: meta.len(),
                width: None,
                height: None,
                tags,
                license: String::new(),
                source: String::new(),
                outside: true,
            });
            count += 1;
        }
        self.collections.push(Collection { kind: kind.to_string(), name: name.to_string(), count });
    }

    /// Takes the collection of kind `kind` (and its assets) out of the library.
    pub fn remove_collection(&mut self, kind: &str) {
        self.collections.retain(|c| c.kind != kind);
        self.assets.retain(|a| !(a.outside && a.kind == kind));
    }

    /// The library without any collection: what the folder itself holds.
    pub fn folder_only(&self) -> Library {
        Library {
            root: self.root.clone(),
            library_id: self.library_id.clone(),
            assets: self.assets.iter().filter(|a| !a.outside).cloned().collect(),
            kinds: self.kinds.clone(),
            from_catalogue: self.from_catalogue,
            found_since: self.found_since,
            collections: Vec::new(),
        }
    }

    pub fn by_id(&self, id: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.id == id)
    }
}

/// Every file under `dir` (skipping the catalogue's folder and hidden files): relative path, size.
fn walk(root: &Path, dir: &Path, depth: usize, out: &mut Vec<(String, u64)>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || (depth == 0 && name == CATALOGUE_DIR) {
            continue;
        }
        let Ok(kind) = entry.file_type() else { continue };
        let path = entry.path();
        if kind.is_dir() {
            walk(root, &path, depth + 1, out);
        } else if kind.is_file() {
            let ext = path.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
            if ext.is_empty() || !FormatGroup::is_creative(&ext) {
                continue;
            }
            let Ok(rel) = path.strip_prefix(root) else { continue };
            let rel = rel.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>().join("/");
            let bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
            out.push((rel, bytes));
        }
    }
}

/// Section names lose their ordering number: `01_Textures photo` → `Textures photo`.
fn section_name(folder: &str) -> String {
    let trimmed = folder.trim_start_matches(|c: char| c.is_ascii_digit());
    let trimmed = trimmed.trim_start_matches(['_', '-', ' ', '.']);
    if trimmed.is_empty() { folder.to_string() } else { trimmed.to_string() }
}

fn from_manifest(m: ManifestAsset) -> Asset {
    let ext = m.path.rsplit('.').next().unwrap_or_default().to_string();
    let format = if m.format.is_empty() { ext.to_ascii_uppercase() } else { m.format.to_ascii_uppercase() };
    // The catalogue knows what a file really is ("Icônes SVG" inside "Branding logos et
    // mockups"), so its own kind replaces the section's words: icons don't answer "mockup".
    let mut tags: Vec<String> = m.tags.iter().flat_map(|t| words(t)).collect();
    tags.extend(words(&m.kind).into_iter().filter(|w| w.len() > 1));
    tags.extend(folder_words(&m.path, 1));
    tags.sort();
    tags.dedup();
    let kind = section_name(m.path.split('/').next().unwrap_or_default());
    Asset {
        name_words: words(&m.name),
        id: m.id,
        name: m.name,
        group: FormatGroup::of(&ext),
        format,
        kind,
        category: m.category,
        bytes: m.bytes,
        width: m.width,
        height: m.height,
        tags,
        license: m.license,
        source: m.source,
        preview: m.preview,
        path: m.path,
        outside: false,
    }
}

fn from_path(rel: &str, bytes: u64) -> Asset {
    let file = rel.rsplit('/').next().unwrap_or(rel);
    let (stem, ext) = match file.rsplit_once('.') {
        Some((s, e)) => (s.to_string(), e.to_string()),
        None => (file.to_string(), String::new()),
    };
    let mut folders = rel.split('/').collect::<Vec<_>>();
    folders.pop();
    let mut tags = folder_words(rel, 0);
    tags.sort();
    tags.dedup();
    let group = FormatGroup::of(&ext);
    Asset {
        id: format!("path:{rel}"),
        name_words: words(&stem),
        name: stem,
        path: rel.to_string(),
        format: ext.to_ascii_uppercase(),
        group,
        kind: section_name(folders.first().copied().unwrap_or_default()),
        category: folders.get(1).map(|f| f.to_string()).unwrap_or_default(),
        bytes,
        width: None,
        height: None,
        tags,
        license: String::new(),
        source: String::new(),
        // Small raster and vector files are their own preview.
        preview: matches!(group, FormatGroup::Image | FormatGroup::Vector).then(|| rel.to_string()).filter(|_| ext.eq_ignore_ascii_case("svg") || group == FormatGroup::Image),
        outside: false,
    }
}

/// The folded words of the folders above the file, from depth `skip` on, without section numbers.
fn folder_words(rel: &str, skip: usize) -> Vec<String> {
    let mut parts: Vec<&str> = rel.split('/').collect();
    parts.pop();
    parts.iter().skip(skip).flat_map(|p| words(&section_name(p))).filter(|w| w.len() > 1).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_lose_their_number() {
        assert_eq!(section_name("01_Textures photo"), "Textures photo");
        assert_eq!(section_name("09_Bonus 2D"), "Bonus 2D");
        assert_eq!(section_name("Divers"), "Divers");
    }

    #[test]
    fn a_new_file_is_tagged_from_its_folders() {
        let a = from_path("01_Textures photo/Metal/Texturelabs - Metal/Rouille_02.jpg", 10);
        assert_eq!(a.kind, "Textures photo");
        assert_eq!(a.category, "Metal");
        assert_eq!(a.format, "JPG");
        assert_eq!(a.group, FormatGroup::Image);
        assert!(a.tags.contains(&"metal".to_string()) && a.tags.contains(&"texturelabs".to_string()));
        assert_eq!(a.name_words, ["rouille", "02"]);
        assert_eq!(a.preview.as_deref(), Some("01_Textures photo/Metal/Texturelabs - Metal/Rouille_02.jpg"));
    }

    #[test]
    fn a_collection_lists_files_outside_the_folder_as_they_are() {
        let dir = std::env::temp_dir().join(format!("oracle-collection-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("lib")).unwrap();
        std::fs::write(dir.join("lib").join("metal.jpg"), "x").unwrap();
        let logo = dir.join("Marque").join("logo.svg");
        std::fs::create_dir_all(logo.parent().unwrap()).unwrap();
        std::fs::write(&logo, "<svg/>").unwrap();
        let mut lib = Library::open(&dir.join("lib")).unwrap();
        let file = |id: &str, f: &Path| CollectionFile { id: id.into(), name: "Logo".into(), file: f.to_path_buf(), category: "Logo principal".into(), tags: vec!["logo".into()] };
        lib.add_collection("Marque · Été", "Été", vec![file("b:1", &logo), file("b:2", &dir.join("absent.svg"))]);
        assert_eq!(lib.collections, [Collection { kind: "Marque · Été".into(), name: "Été".into(), count: 1 }]);
        let a = lib.by_id("b:1").unwrap();
        assert_eq!(a.file(&lib.root), logo, "the brand's own file, not a copy");
        assert_eq!(a.preview_file(&lib.root).as_deref(), Some(logo.as_path()));
        assert!(a.tags.contains(&"ete".to_string()) && a.tags.contains(&"principal".to_string()));
        assert_eq!(lib.folder_only().assets.len(), 1);
        lib.add_collection("Marque · Été", "Été", Vec::new());
        assert_eq!(lib.assets.len(), 1, "a collection added again replaces the old one");
    }

    #[test]
    fn resolve_never_leaves_the_root() {
        let root = Path::new("C:/lib");
        assert_eq!(resolve(root, "a/../../b.png"), Path::new("C:/lib").join("a").join("b.png"));
        assert_eq!(resolve(root, "C:/x.png"), Path::new("C:/lib").join("x.png"));
    }
}
