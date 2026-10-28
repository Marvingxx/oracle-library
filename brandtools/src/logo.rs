//! A logo read from SVG: its outlines in absolute coordinates, split into components.
//!
//! The designer's grouping decides the components, as in Illustrator: the top-level groups (or
//! shapes) of the file, after stepping through a single wrapping group. A logomark and a logotype
//! are therefore two groups. Islands are finer: each outer outline with the holes inside it (a
//! letter, a dot), found from the geometry whatever the grouping.

use crate::geom::{Contour, Rect};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Shape {
    pub contours: Vec<Contour>,
    /// `#rrggbb`, when the shape is filled with a flat colour.
    pub fill: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Component {
    /// The SVG id of the group, when it has one.
    pub name: String,
    pub shapes: Vec<Shape>,
}

impl Component {
    pub fn contours(&self) -> impl Iterator<Item = &Contour> {
        self.shapes.iter().flat_map(|s| s.contours.iter())
    }
    pub fn bbox(&self) -> Rect {
        self.contours().fold(Rect::empty(), |r, c| r.union(&c.bbox()))
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Logo {
    pub components: Vec<Component>,
    /// The document size of the SVG (its artboard).
    pub width: f64,
    pub height: f64,
}

#[derive(Debug)]
pub struct LoadError(pub String);

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for LoadError {}

impl Logo {
    #[cfg(feature = "svg")]
    /// Reads a logo. Text that is still live is drawn with the fonts installed on this computer.
    pub fn from_svg(src: &str) -> Result<Logo, LoadError> {
        let mut opt = usvg::Options::default();
        if src.contains("<text") {
            opt.fontdb_mut().load_system_fonts();
        }
        let tree = usvg::Tree::from_str(src, &opt).map_err(|e| LoadError(format!("SVG illisible : {e}")))?;
        let mut root = tree.root();
        // Step through wrapping groups: a single group around everything is not a component.
        while let [usvg::Node::Group(g)] = root.children() {
            root = g.as_ref();
        }
        let mut components = Vec::new();
        for node in root.children() {
            let mut c = Component { name: node.id().to_string(), shapes: Vec::new() };
            collect(node, &mut c.shapes);
            if c.contours().next().is_some() {
                components.push(c);
            }
        }
        if components.is_empty() {
            return Err(LoadError("Le fichier ne contient aucun tracé.".into()));
        }
        let size = tree.size();
        Ok(Logo { components, width: size.width() as f64, height: size.height() as f64 })
    }

    pub fn bbox(&self) -> Rect {
        self.components.iter().fold(Rect::empty(), |r, c| r.union(&c.bbox()))
    }

    pub fn contours(&self) -> impl Iterator<Item = &Contour> {
        self.components.iter().flat_map(|c| c.contours())
    }

    /// A size every tolerance is measured against.
    pub fn scale(&self) -> f64 {
        self.bbox().max_dim().max(1e-6)
    }

    /// Each outer outline with the outlines inside it.
    pub fn islands(&self) -> Vec<Rect> {
        let mut boxes: Vec<Rect> = self.contours().map(|c| c.bbox()).filter(|b| !b.is_empty()).collect();
        boxes.sort_by(|a, b| b.area().total_cmp(&a.area()));
        let eps = self.scale() * 1e-4;
        let mut islands: Vec<Rect> = Vec::new();
        for b in boxes {
            if !islands.iter().any(|i| i.contains_rect(&b, eps)) {
                islands.push(b);
            }
        }
        islands
    }
}

#[cfg(feature = "svg")]
fn collect(node: &usvg::Node, out: &mut Vec<Shape>) {
    match node {
        usvg::Node::Group(g) => {
            for child in g.children() {
                collect(child, out);
            }
        }
        usvg::Node::Path(p) => {
            let fill = p.fill().and_then(|f| match f.paint() {
                usvg::Paint::Color(c) => Some(format!("#{:02x}{:02x}{:02x}", c.red, c.green, c.blue)),
                _ => None,
            });
            let Some(data) = p.data().clone().transform(p.abs_transform()) else { return };
            let contours = contours_of(&data);
            if !contours.is_empty() {
                out.push(Shape { contours, fill: fill.or_else(|| Some("#000000".into())) });
            }
        }
        usvg::Node::Text(t) => {
            for child in t.flattened().children() {
                collect(child, out);
            }
        }
        usvg::Node::Image(_) => {}
    }
}

#[cfg(feature = "svg")]
fn contours_of(path: &usvg::tiny_skia_path::Path) -> Vec<Contour> {
    use crate::geom::{Pt, Seg, pt};
    use usvg::tiny_skia_path::PathSegment as S;
    let p = |q: usvg::tiny_skia_path::Point| pt(q.x as f64, q.y as f64);
    let mut out = Vec::new();
    let mut cur = Contour::default();
    let (mut start, mut last) = (Pt::default(), Pt::default());
    fn flush(cur: &mut Contour, out: &mut Vec<Contour>) {
        if !cur.segs.is_empty() {
            out.push(std::mem::take(cur));
        }
    }
    for s in path.segments() {
        match s {
            S::MoveTo(a) => {
                flush(&mut cur, &mut out);
                start = p(a);
                last = start;
            }
            S::LineTo(a) => {
                let a = p(a);
                if a.dist(last) > 1e-9 {
                    cur.segs.push(Seg::Line(last, a));
                }
                last = a;
            }
            S::QuadTo(c, a) => {
                let (c, a) = (p(c), p(a));
                cur.segs.push(Seg::Cubic(last, last + (c - last) * (2.0 / 3.0), a + (c - a) * (2.0 / 3.0), a));
                last = a;
            }
            S::CubicTo(c1, c2, a) => {
                let a = p(a);
                cur.segs.push(Seg::Cubic(last, p(c1), p(c2), a));
                last = a;
            }
            S::Close => {
                if last.dist(start) > 1e-9 {
                    cur.segs.push(Seg::Line(last, start));
                }
                cur.closed = true;
                last = start;
                flush(&mut cur, &mut out);
            }
        }
    }
    flush(&mut cur, &mut out);
    out
}

#[cfg(all(test, feature = "svg"))]
pub(crate) mod tests {
    use super::*;

    /// A test logo: a circle mark and two letter-like bars, grouped as mark + type.
    pub const MARK_AND_TYPE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="300" height="100" viewBox="0 0 300 100">
      <g id="mark"><circle cx="50" cy="50" r="40" fill="#d31c3b"/></g>
      <g id="type" transform="translate(110 20)">
        <rect x="0" y="0" width="40" height="60"/>
        <rect x="60" y="0" width="40" height="60"/>
        <path d="M120 60 L150 0 L180 60 Z"/>
      </g>
    </svg>"##;

    #[test]
    fn groups_become_components_and_transforms_are_applied() {
        let logo = Logo::from_svg(MARK_AND_TYPE).unwrap();
        assert_eq!(logo.components.len(), 2);
        assert_eq!(logo.components[0].name, "mark");
        let mark = logo.components[0].bbox();
        assert!((mark.x0 - 10.0).abs() < 0.01 && (mark.x1 - 90.0).abs() < 0.01);
        let t = logo.components[1].bbox();
        assert!((t.x0 - 110.0).abs() < 0.01 && (t.y0 - 20.0).abs() < 0.01 && (t.x1 - 290.0).abs() < 0.01);
        assert_eq!(logo.components[0].shapes[0].fill.as_deref(), Some("#d31c3b"));
        assert_eq!(logo.islands().len(), 4);
    }

    #[test]
    fn a_single_wrapping_group_is_stepped_through() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><g><g><rect width="2" height="2"/></g><rect x="5" width="2" height="2"/></g></svg>"#;
        assert_eq!(Logo::from_svg(svg).unwrap().components.len(), 2);
    }

    #[test]
    fn a_file_without_outlines_is_refused() {
        assert!(Logo::from_svg(r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"/>"#).is_err());
    }
}
