//! Clearspace: the zone kept free around the logo, measured with a unit taken from the logo
//! itself (its mark) or set in pixels, centimetres or inches.

use crate::drawing::{Anchor, Drawing, Item, ORACLE_RED, Style};
use crate::geom::{Rect, n, pt};
use crate::logo::Logo;
use serde::{Deserialize, Serialize};

/// SVG and CSS units: 96 px to the inch.
pub const PX_PER_IN: f64 = 96.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClearUnit {
    /// The smaller side of the logomark, times `size`.
    #[default]
    Logomark,
    Px,
    Cm,
    In,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClearspaceParams {
    pub unit: ClearUnit,
    /// A multiple of the mark (Logomark), or a length in the chosen unit.
    pub size: f64,
    /// Measure with the other component (the logotype instead of the mark).
    pub invert: bool,
    /// Fill the unit boxes instead of only outlining them.
    pub fill: bool,
    /// Minimum sizes to state with the zone; 0 leaves them out.
    pub min_print_mm: f64,
    pub min_screen_px: f64,
    pub color: String,
}

impl Default for ClearspaceParams {
    fn default() -> Self {
        ClearspaceParams {
            unit: ClearUnit::Logomark,
            size: 1.0,
            invert: false,
            fill: false,
            min_print_mm: 0.0,
            min_screen_px: 0.0,
            color: ORACLE_RED.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clearspace {
    /// The unit in document units (px).
    pub unit: f64,
    pub zone: Rect,
    /// The box of the part the unit was measured on, with Logomark.
    pub measured_on: Option<Rect>,
}

/// The part the unit is measured on: the first or second group, or the first or second island
/// from the left when the logo is a single group.
fn mark_box(logo: &Logo, invert: bool) -> Rect {
    if logo.components.len() >= 2 {
        logo.components[usize::from(invert)].bbox()
    } else {
        let mut islands = logo.islands();
        islands.sort_by(|a, b| a.x0.total_cmp(&b.x0));
        islands.get(usize::from(invert)).or(islands.first()).copied().unwrap_or_else(|| logo.bbox())
    }
}

pub fn measure(logo: &Logo, p: &ClearspaceParams) -> Clearspace {
    let (unit, measured_on) = match p.unit {
        ClearUnit::Logomark => {
            let m = mark_box(logo, p.invert);
            (m.min_dim() * p.size, Some(m))
        }
        ClearUnit::Px => (p.size, None),
        ClearUnit::Cm => (p.size * PX_PER_IN / 2.54, None),
        ClearUnit::In => (p.size * PX_PER_IN, None),
    };
    let unit = unit.max(0.0);
    Clearspace { unit, zone: logo.bbox().inflate(unit), measured_on }
}

pub fn clearspace(logo: &Logo, p: &ClearspaceParams) -> (Drawing, Clearspace) {
    let cs = measure(logo, p);
    let (u, b, z) = (cs.unit, logo.bbox(), cs.zone);
    let c = b.center();
    let text = (logo.scale() * 0.03).max(4.0);
    let mut d = Drawing::new(z.inflate(u.max(text) * 0.6 + text * 3.0));

    d.layer(
        "Zone de protection",
        Style::stroke(&p.color, 0.75).dashed(&[4.0, 3.0]),
        vec![Item::Rect { rect: z, radius: 0.0 }, Item::Rect { rect: b, radius: 0.0 }],
    );

    // One unit on each side, and in each corner.
    let sides = [
        Rect::from_size(z.x0, c.y - u / 2.0, u, u),
        Rect::from_size(b.x1, c.y - u / 2.0, u, u),
        Rect::from_size(c.x - u / 2.0, z.y0, u, u),
        Rect::from_size(c.x - u / 2.0, b.y1, u, u),
    ];
    let corners = [
        Rect::from_size(z.x0, z.y0, u, u),
        Rect::from_size(b.x1, z.y0, u, u),
        Rect::from_size(z.x0, b.y1, u, u),
        Rect::from_size(b.x1, b.y1, u, u),
    ];
    let mut box_style = Style::stroke(&p.color, 0.5);
    if p.fill {
        box_style = box_style.with_fill(&p.color).with_opacity(0.3);
    }
    let boxes = sides.iter().chain(corners.iter()).map(|r| Item::Rect { rect: *r, radius: 0.0 }).collect();
    d.layer("Unités", box_style, boxes);

    // With Logomark, the mark itself shown in each side box: the unit is visibly the logo's own.
    if let Some(m) = cs.measured_on {
        let k = u / m.max_dim().max(1e-9);
        let mut ghosts = Vec::new();
        for r in &sides {
            let t = [k, 0.0, 0.0, k, r.center().x - k * m.center().x, r.center().y - k * m.center().y];
            for comp in &logo.components {
                for shape in &comp.shapes {
                    let inside: String = shape
                        .contours
                        .iter()
                        .filter(|ct| m.contains_rect(&ct.bbox(), logo.scale() * 1e-4))
                        .map(|ct| ct.to_path_data())
                        .collect();
                    if !inside.is_empty() {
                        ghosts.push(Item::Path { d: inside, fill: Some(p.color.clone()), transform: Some(t) });
                    }
                }
            }
        }
        d.layer("Repères", Style { stroke: None, fill: None, opacity: 0.35, ..Style::default() }, ghosts);
    }

    let mut labels = vec![Item::Text {
        at: pt(z.center().x, z.y1 + u * 0.0 + text * 1.6),
        text: format!("x = {} px", n(u)),
        size: text,
        anchor: Anchor::Middle,
    }];
    let mut line = 2.9;
    if p.min_print_mm > 0.0 {
        labels.push(Item::Text {
            at: pt(z.center().x, z.y1 + text * line),
            text: format!("Taille minimale impression : {} mm", n(p.min_print_mm)),
            size: text * 0.85,
            anchor: Anchor::Middle,
        });
        line += 1.2;
    }
    if p.min_screen_px > 0.0 {
        labels.push(Item::Text {
            at: pt(z.center().x, z.y1 + text * line),
            text: format!("Taille minimale écran : {} px", n(p.min_screen_px)),
            size: text * 0.85,
            anchor: Anchor::Middle,
        });
    }
    d.layer("Cotes", Style::fill(&p.color), labels);
    (d, cs)
}

#[cfg(all(test, feature = "svg"))]
mod tests {
    use super::*;
    use crate::logo::tests::MARK_AND_TYPE;

    #[test]
    fn the_logomark_unit_is_the_marks_smaller_side() {
        let l = Logo::from_svg(MARK_AND_TYPE).unwrap();
        let cs = measure(&l, &ClearspaceParams::default());
        assert!((cs.unit - 80.0).abs() < 0.01, "{}", cs.unit);
        let b = l.bbox();
        assert!((cs.zone.x0 - (b.x0 - 80.0)).abs() < 0.01);
        // Inverted: measured on the logotype (60 high).
        let inv = measure(&l, &ClearspaceParams { invert: true, size: 0.5, ..Default::default() });
        assert!((inv.unit - 30.0).abs() < 0.01);
    }

    #[test]
    fn fixed_units_convert_at_96_px_per_inch() {
        let l = Logo::from_svg(MARK_AND_TYPE).unwrap();
        let cm = measure(&l, &ClearspaceParams { unit: ClearUnit::Cm, size: 2.54, ..Default::default() });
        assert!((cm.unit - 96.0).abs() < 1e-9 && cm.measured_on.is_none());
        let px = measure(&l, &ClearspaceParams { unit: ClearUnit::Px, size: 12.0, ..Default::default() });
        assert_eq!(px.unit, 12.0);
    }

    #[test]
    fn the_drawing_has_eight_units_and_the_mark_in_four() {
        let l = Logo::from_svg(MARK_AND_TYPE).unwrap();
        let (d, _) = clearspace(&l, &ClearspaceParams { fill: true, min_print_mm: 20.0, ..Default::default() });
        assert_eq!(d.count("Unités"), 8);
        assert_eq!(d.count("Repères"), 4);
        assert_eq!(d.count("Cotes"), 2);
        assert_eq!(d.find("Unités").unwrap().style.fill.as_deref(), Some(ORACLE_RED));
    }
}
