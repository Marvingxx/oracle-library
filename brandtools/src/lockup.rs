//! Lockups: the mark and the logotype arranged side by side, stacked, condensed, or next to a
//! partner's logo, all with one spacing rule measured on the mark.

use crate::drawing::{Drawing, Item, ORACLE_RED, Style};
use crate::geom::{Rect, pt};
use crate::logo::{Component, Logo};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LockupKind {
    #[default]
    Horizontal,
    Vertical,
    /// The mark brought to the height of the logotype, closer to it.
    Condensed,
    /// The whole logo, a divider, and a partner's logo at the same height.
    Partner,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LockupParams {
    pub kind: LockupKind,
    /// The gap, as a multiple of the unit (the mark's smaller side).
    pub spacing: f64,
    /// The second group is the mark.
    pub invert: bool,
    pub color: String,
}

impl Default for LockupParams {
    fn default() -> Self {
        LockupParams { kind: LockupKind::Horizontal, spacing: 0.5, invert: false, color: ORACLE_RED.into() }
    }
}

/// Where one part ends up: `transform` maps its original coordinates into the lockup.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    /// "mark", "type", "logo" or "partner".
    pub part: String,
    pub transform: [f64; 6],
    pub rect: Rect,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lockup {
    pub drawing: Drawing,
    pub placements: Vec<Placement>,
    pub unit: f64,
    pub gap: f64,
}

struct Part<'a> {
    name: &'static str,
    comps: Vec<&'a Component>,
    bbox: Rect,
}

fn part<'a>(name: &'static str, comps: Vec<&'a Component>) -> Part<'a> {
    let bbox = comps.iter().fold(Rect::empty(), |r, c| r.union(&c.bbox()));
    Part { name, comps, bbox }
}

/// Places `p` scaled by `k` with its top-left corner at (x, y).
fn place(p: &Part, k: f64, x: f64, y: f64) -> Placement {
    let t = [k, 0.0, 0.0, k, x - k * p.bbox.x0, y - k * p.bbox.y0];
    Placement {
        part: p.name.into(),
        transform: t,
        rect: Rect::from_size(x, y, p.bbox.width() * k, p.bbox.height() * k),
    }
}

pub fn lockup(logo: &Logo, partner: Option<&Logo>, p: &LockupParams) -> Result<Lockup, String> {
    let all = part("logo", logo.components.iter().collect());
    let (mark, word) = if logo.components.len() >= 2 {
        let m = usize::from(p.invert);
        let rest = logo.components.iter().enumerate().filter(|(i, _)| *i != m).map(|(_, c)| c).collect();
        (Some(part("mark", vec![&logo.components[m]])), Some(part("type", rest)))
    } else {
        (None, None)
    };
    let unit = mark.as_ref().map_or(all.bbox.min_dim() * 0.25, |m| m.bbox.min_dim());
    let gap = p.spacing.max(0.0) * unit;

    let needs_two = || "Le logo doit avoir deux groupes : le symbole et le logotype.".to_string();
    let mut placements = Vec::new();
    let mut extra: Vec<Item> = Vec::new();
    let mut gaps: Vec<Rect> = Vec::new();
    match p.kind {
        LockupKind::Horizontal | LockupKind::Condensed => {
            let (m, w) = (mark.as_ref().ok_or_else(needs_two)?, word.as_ref().ok_or_else(needs_two)?);
            let k = if p.kind == LockupKind::Condensed { w.bbox.height() / m.bbox.height().max(1e-9) } else { 1.0 };
            let g = if p.kind == LockupKind::Condensed { gap * k } else { gap };
            let (mw, mh) = (m.bbox.width() * k, m.bbox.height() * k);
            let h = mh.max(w.bbox.height());
            placements.push(place(m, k, 0.0, (h - mh) / 2.0));
            placements.push(place(w, 1.0, mw + g, (h - w.bbox.height()) / 2.0));
            gaps.push(Rect::from_size(mw, (h - g) / 2.0, g, g));
        }
        LockupKind::Vertical => {
            let (m, w) = (mark.as_ref().ok_or_else(needs_two)?, word.as_ref().ok_or_else(needs_two)?);
            let width = m.bbox.width().max(w.bbox.width());
            placements.push(place(m, 1.0, (width - m.bbox.width()) / 2.0, 0.0));
            placements.push(place(w, 1.0, (width - w.bbox.width()) / 2.0, m.bbox.height() + gap));
            gaps.push(Rect::from_size((width - gap) / 2.0, m.bbox.height(), gap, gap));
        }
        LockupKind::Partner => {
            let h = all.bbox.height();
            placements.push(place(&all, 1.0, 0.0, 0.0));
            let divider_x = all.bbox.width() + gap;
            extra.push(Item::Line { a: pt(divider_x, 0.0), b: pt(divider_x, h) });
            gaps.push(Rect::from_size(all.bbox.width(), (h - gap) / 2.0, gap, gap));
            gaps.push(Rect::from_size(divider_x, (h - gap) / 2.0, gap, gap));
            let px = divider_x + gap;
            match partner {
                Some(pl) => {
                    let pp = part("partner", pl.components.iter().collect());
                    let k = h / pp.bbox.height().max(1e-9);
                    placements.push(place(&pp, k, px, 0.0));
                }
                None => placements.push(Placement {
                    part: "partner".into(),
                    transform: [1.0, 0.0, 0.0, 1.0, px, 0.0],
                    rect: Rect::from_size(px, 0.0, h * 1.6, h),
                }),
            }
        }
    }

    let bounds = placements.iter().fold(Rect::empty(), |r, pl| r.union(&pl.rect));
    let mut d = Drawing::new(bounds.inflate(gap.max(bounds.max_dim() * 0.05)));
    let mut art = Vec::new();
    let mut placeholder = Vec::new();
    for pl in &placements {
        let comps: Vec<&Component> = match pl.part.as_str() {
            "mark" => mark.as_ref().map(|m| m.comps.clone()).unwrap_or_default(),
            "type" => word.as_ref().map(|w| w.comps.clone()).unwrap_or_default(),
            "logo" => all.comps.clone(),
            _ => partner.map(|l| l.components.iter().collect()).unwrap_or_default(),
        };
        if pl.part == "partner" && partner.is_none() {
            placeholder.push(Item::Rect { rect: pl.rect, radius: 0.0 });
        }
        for c in comps {
            for s in &c.shapes {
                let path: String = s.contours.iter().map(|ct| ct.to_path_data()).collect();
                art.push(Item::Path { d: path, fill: s.fill.clone(), transform: Some(pl.transform) });
            }
        }
    }
    d.layer("Logo", Style { stroke: None, fill: None, ..Style::default() }, art);
    d.layer("Partenaire", Style::stroke("#8a8a8a", 0.75).dashed(&[4.0, 3.0]), placeholder);
    d.layer("Séparateur", Style::stroke("#1b1a17", 1.0), extra);
    let gap_boxes = gaps.iter().map(|r| Item::Rect { rect: *r, radius: 0.0 }).collect();
    d.layer("Espacement", Style::stroke(&p.color, 0.5).with_fill(&p.color).with_opacity(0.3), gap_boxes);
    // Guides at each edge of the gaps, across the whole lockup.
    let mut guides = Vec::new();
    for r in &gaps {
        if p.kind == LockupKind::Vertical {
            for y in [r.y0, r.y1] {
                guides.push(Item::Line { a: pt(d.bounds.x0, y), b: pt(d.bounds.x1, y) });
            }
        } else {
            for x in [r.x0, r.x1] {
                guides.push(Item::Line { a: pt(x, d.bounds.y0), b: pt(x, d.bounds.y1) });
            }
        }
    }
    d.layer("Repères d'espacement", Style::stroke(&p.color, 0.5).dashed(&[2.0, 2.0]), guides);
    Ok(Lockup { drawing: d, placements, unit, gap })
}

#[cfg(all(test, feature = "svg"))]
mod tests {
    use super::*;
    use crate::logo::tests::MARK_AND_TYPE;

    fn logo() -> Logo {
        Logo::from_svg(MARK_AND_TYPE).unwrap()
    }

    #[test]
    fn horizontal_puts_the_type_one_gap_after_the_mark_centred() {
        let l = lockup(&logo(), None, &LockupParams::default()).unwrap();
        assert!((l.unit - 80.0).abs() < 0.01 && (l.gap - 40.0).abs() < 0.01);
        let (m, t) = (&l.placements[0].rect, &l.placements[1].rect);
        assert!((t.x0 - m.x1 - 40.0).abs() < 0.01);
        assert!((t.center().y - m.center().y).abs() < 0.01);
    }

    #[test]
    fn vertical_stacks_with_the_same_gap() {
        let l = lockup(&logo(), None, &LockupParams { kind: LockupKind::Vertical, ..Default::default() }).unwrap();
        let (m, t) = (&l.placements[0].rect, &l.placements[1].rect);
        assert!((t.y0 - m.y1 - 40.0).abs() < 0.01);
        assert!((t.center().x - m.center().x).abs() < 0.01);
    }

    #[test]
    fn condensed_brings_the_mark_to_the_type_height() {
        let l = lockup(&logo(), None, &LockupParams { kind: LockupKind::Condensed, ..Default::default() }).unwrap();
        let (m, t) = (&l.placements[0].rect, &l.placements[1].rect);
        assert!((m.height() - t.height()).abs() < 0.01);
        // The gap scales with the mark: 0.5 × 60.
        assert!((t.x0 - m.x1 - 30.0).abs() < 0.01);
    }

    #[test]
    fn partner_matches_heights_and_leaves_room_for_a_missing_logo() {
        let partner = Logo::from_svg(r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="20" height="10"/></svg>"#).unwrap();
        let l = lockup(&logo(), Some(&partner), &LockupParams { kind: LockupKind::Partner, ..Default::default() }).unwrap();
        let (main, other) = (&l.placements[0].rect, &l.placements[1].rect);
        assert!((main.height() - other.height()).abs() < 0.01);
        assert_eq!(l.drawing.count("Séparateur"), 1);
        let empty = lockup(&logo(), None, &LockupParams { kind: LockupKind::Partner, ..Default::default() }).unwrap();
        assert_eq!(empty.drawing.count("Partenaire"), 1);
    }

    #[test]
    fn a_single_group_cannot_be_split_into_mark_and_type() {
        let one = Logo::from_svg(r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="2" height="2"/></svg>"#).unwrap();
        assert!(lockup(&one, None, &LockupParams::default()).is_err());
        assert!(lockup(&one, None, &LockupParams { kind: LockupKind::Partner, ..Default::default() }).is_ok());
    }
}
