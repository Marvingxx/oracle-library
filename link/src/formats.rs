//! Oracle's own file formats: which app makes each one, and what it can be drawn as for the
//! others ([`crate::render`]).

use std::path::Path;

use crate::App;

/// What a native file can be drawn as for another app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Preview {
    /// Vector, resolution-free: placed it stays sharp at any size.
    Svg,
    /// Pixels, with transparency.
    Png,
    /// Vector pages (print).
    Pdf,
}

impl Preview {
    pub fn ext(self) -> &'static str {
        match self {
            Preview::Svg => "svg",
            Preview::Png => "png",
            Preview::Pdf => "pdf",
        }
    }
}

/// One of Oracle's own file formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OracleFormat {
    /// The extension, lower case, without the dot.
    pub ext: &'static str,
    /// The app that makes it, and draws it for the others.
    pub app: App,
    /// What the format is, in French and in English.
    pub label_fr: &'static str,
    pub label_en: &'static str,
    /// What it can be drawn as, best first: what another app places when it has the choice.
    pub previews: &'static [Preview],
}

/// Every Oracle format.
pub const FORMATS: [OracleFormat; 3] = [
    OracleFormat {
        ext: "oracle-vector",
        app: App::Vector,
        label_fr: "Document Oracle Vector",
        label_en: "Oracle Vector document",
        previews: &[Preview::Svg, Preview::Pdf, Preview::Png],
    },
    OracleFormat { ext: "pcraft", app: App::Edit, label_fr: "Document Oracle Edit", label_en: "Oracle Edit document", previews: &[Preview::Png] },
    OracleFormat {
        ext: "oracle-guide",
        app: App::Guide,
        label_fr: "Document Oracle Brand Guide",
        label_en: "Oracle Brand Guide document",
        previews: &[Preview::Pdf, Preview::Png],
    },
];

/// The Oracle format of `path`, by its extension (any case).
pub fn oracle_format(path: &Path) -> Option<&'static OracleFormat> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    FORMATS.iter().find(|f| f.ext == ext)
}

impl OracleFormat {
    /// Whether this format can be drawn as `preview`.
    pub fn can(&self, preview: Preview) -> bool {
        self.previews.contains(&preview)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_by_extension_any_case() {
        assert_eq!(oracle_format(Path::new("Logo.ORACLE-VECTOR")).map(|f| f.app), Some(App::Vector));
        assert_eq!(oracle_format(Path::new("C:/a/b/photo.pcraft")).map(|f| f.app), Some(App::Edit));
        assert_eq!(oracle_format(Path::new("charte.oracle-guide")).map(|f| f.app), Some(App::Guide));
        assert_eq!(oracle_format(Path::new("logo.svg")), None);
        assert_eq!(oracle_format(Path::new("sans-extension")), None);
        assert!(oracle_format(Path::new("x.oracle-vector")).unwrap().can(Preview::Svg));
        assert!(!oracle_format(Path::new("x.pcraft")).unwrap().can(Preview::Svg));
    }
}
