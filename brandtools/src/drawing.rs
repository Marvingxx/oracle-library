//! What every tool produces: named layers of plain shapes, plus guides. An app turns them into its
//! own objects (Vector: a locked layer of ordinary paths; Layout: page items), or writes the SVG.

use crate::geom::{Pt, Rect, n};
use crate::logo::Logo;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Item {
    Line { a: Pt, b: Pt },
    Polygon { points: Vec<Pt> },
    Circle { c: Pt, r: f64 },
    /// A quarter (or any) arc from `a` to `b` around `c`, turning clockwise on screen when `clockwise`.
    Arc { c: Pt, r: f64, a: Pt, b: Pt, clockwise: bool },
    Rect { rect: Rect, radius: f64 },
    /// SVG path data, drawn through `transform` (a b c d e f) when there is one.
    Path { d: String, fill: Option<String>, transform: Option<[f64; 6]> },
    Text { at: Pt, text: String, size: f64, anchor: Anchor },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Anchor {
    Start,
    Middle,
    End,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Style {
    pub stroke: Option<String>,
    pub fill: Option<String>,
    pub width: f64,
    pub dash: Option<Vec<f64>>,
    pub opacity: f64,
}

impl Default for Style {
    fn default() -> Self {
        Style { stroke: Some(ORACLE_RED.into()), fill: None, width: 0.5, dash: None, opacity: 1.0 }
    }
}

impl Style {
    pub fn stroke(color: &str, width: f64) -> Style {
        Style { stroke: Some(color.into()), width, ..Style::default() }
    }
    pub fn fill(color: &str) -> Style {
        Style { stroke: None, fill: Some(color.into()), ..Style::default() }
    }
    pub fn dashed(mut self, dash: &[f64]) -> Style {
        self.dash = Some(dash.to_vec());
        self
    }
    pub fn with_fill(mut self, color: &str) -> Style {
        self.fill = Some(color.into());
        self
    }
    pub fn with_opacity(mut self, o: f64) -> Style {
        self.opacity = o;
        self
    }
}

pub const ORACLE_RED: &str = "#d31c3b";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    /// Shown to the designer (French, like the rest of the interface).
    pub name: String,
    pub style: Style,
    pub items: Vec<Item>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "axis", content = "at", rename_all = "camelCase")]
pub enum Guide {
    Vertical(f64),
    Horizontal(f64),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Drawing {
    /// The area the drawing covers (the artboard, or the logo with room around it).
    pub bounds: Rect,
    /// Shapes are cut to this rectangle when it is set.
    pub clip: Option<Rect>,
    pub layers: Vec<Layer>,
    pub guides: Vec<Guide>,
}

impl Drawing {
    pub fn new(bounds: Rect) -> Drawing {
        Drawing { bounds, clip: None, layers: Vec::new(), guides: Vec::new() }
    }

    pub fn layer(&mut self, name: &str, style: Style, items: Vec<Item>) {
        if !items.is_empty() {
            self.layers.push(Layer { name: name.into(), style, items });
        }
    }

    pub fn find(&self, name: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.name == name)
    }

    pub fn count(&self, name: &str) -> usize {
        self.find(name).map_or(0, |l| l.items.len())
    }

    /// A standalone SVG. With `logo`, the logo is drawn first, under the grid.
    pub fn to_svg(&self, logo: Option<&Logo>, background: Option<&str>) -> String {
        let b = self.bounds;
        let mut s = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="{x} {y} {w} {h}">"#,
            x = n(b.x0),
            y = n(b.y0),
            w = n(b.width()),
            h = n(b.height())
        );
        s.push('\n');
        if let Some(bg) = background {
            s.push_str(&format!(
                r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{bg}"/>"#,
                n(b.x0),
                n(b.y0),
                n(b.width()),
                n(b.height())
            ));
            s.push('\n');
        }
        if let Some(c) = self.clip {
            s.push_str(&format!(
                r#"<defs><clipPath id="oracle-clip"><rect x="{}" y="{}" width="{}" height="{}"/></clipPath></defs>"#,
                n(c.x0),
                n(c.y0),
                n(c.width()),
                n(c.height())
            ));
            s.push('\n');
        }
        if let Some(logo) = logo {
            s.push_str(r#"<g id="logo">"#);
            for comp in &logo.components {
                for shape in &comp.shapes {
                    let d: String = shape.contours.iter().map(|c| c.to_path_data()).collect();
                    s.push_str(&format!(r#"<path d="{d}" fill="{}"/>"#, shape.fill.as_deref().unwrap_or("#000")));
                }
            }
            s.push_str("</g>\n");
        }
        for layer in &self.layers {
            let st = &layer.style;
            let mut attrs = format!(
                r#" id="{}" data-layer="{}" fill="{}" stroke="{}" stroke-width="{}""#,
                slug(&layer.name),
                xml(&layer.name),
                st.fill.as_deref().unwrap_or("none"),
                st.stroke.as_deref().unwrap_or("none"),
                n(st.width)
            );
            if let Some(d) = &st.dash {
                attrs.push_str(&format!(r#" stroke-dasharray="{}""#, d.iter().map(|v| n(*v)).collect::<Vec<_>>().join(" ")));
            }
            if st.opacity < 1.0 {
                attrs.push_str(&format!(r#" opacity="{}""#, n(st.opacity)));
            }
            if self.clip.is_some() {
                attrs.push_str(r#" clip-path="url(#oracle-clip)""#);
            }
            s.push_str(&format!("<g{attrs}>\n"));
            for item in &layer.items {
                s.push_str(&item_svg(item, st));
                s.push('\n');
            }
            s.push_str("</g>\n");
        }
        s.push_str("</svg>\n");
        s
    }
}

fn item_svg(item: &Item, style: &Style) -> String {
    match item {
        Item::Line { a, b } => format!(r#"<line x1="{}" y1="{}" x2="{}" y2="{}"/>"#, n(a.x), n(a.y), n(b.x), n(b.y)),
        Item::Polygon { points } => {
            let p: Vec<String> = points.iter().map(|p| format!("{},{}", n(p.x), n(p.y))).collect();
            format!(r#"<polygon points="{}"/>"#, p.join(" "))
        }
        Item::Circle { c, r } => format!(r#"<circle cx="{}" cy="{}" r="{}"/>"#, n(c.x), n(c.y), n(*r)),
        Item::Arc { r, a, b, clockwise, .. } => format!(
            r#"<path fill="none" d="M{} {}A{r} {r} 0 0 {sweep} {} {}"/>"#,
            n(a.x),
            n(a.y),
            n(b.x),
            n(b.y),
            r = n(*r),
            sweep = u8::from(*clockwise)
        ),
        Item::Rect { rect, radius } => {
            let rx = if *radius > 0.0 { format!(r#" rx="{}""#, n(*radius)) } else { String::new() };
            format!(
                r#"<rect x="{}" y="{}" width="{}" height="{}"{rx}/>"#,
                n(rect.x0),
                n(rect.y0),
                n(rect.width()),
                n(rect.height())
            )
        }
        Item::Path { d, fill, transform } => {
            let f = fill.as_ref().map(|f| format!(r#" fill="{f}" stroke="none""#)).unwrap_or_default();
            let t = transform
                .map(|m| format!(r#" transform="matrix({})""#, m.iter().map(|v| n(*v)).collect::<Vec<_>>().join(" ")))
                .unwrap_or_default();
            format!(r#"<path d="{d}"{f}{t}/>"#)
        }
        Item::Text { at, text, size, anchor } => {
            let color = style.stroke.as_deref().or(style.fill.as_deref()).unwrap_or("#000");
            let a = match anchor {
                Anchor::Start => "start",
                Anchor::Middle => "middle",
                Anchor::End => "end",
            };
            format!(
                r#"<text x="{}" y="{}" font-family="Lexend, Inter, Arial, sans-serif" font-size="{}" text-anchor="{a}" fill="{color}" stroke="none">{}</text>"#,
                n(at.x),
                n(at.y),
                n(*size),
                xml(text)
            )
        }
    }
}

/// An SVG id from a layer name: "Zone de protection" → "zone-de-protection".
fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        let c = match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c => c.to_ascii_lowercase(),
        };
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}
