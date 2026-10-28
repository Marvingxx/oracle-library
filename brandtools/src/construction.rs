//! Construction grids: what the logo's own outlines are built on.
//!
//! Every element is read from the vector geometry, nothing is drawn freehand:
//! - horizontals and verticals pass where an outline is flat or turns back (straight edges,
//!   curve extrema), so two letters whose tops differ by a hair show two lines, not one;
//! - diagonals extend the straight edges that are neither horizontal nor vertical;
//! - circles are the arcs the curves actually follow, gathered when several segments share one;
//! - anchors, handles and outlines are the points and curves of the paths themselves;
//! - spacing markers measure the gaps between islands (letters, mark and type).

use crate::drawing::{Anchor, Drawing, Item, ORACLE_RED, Style};
use crate::geom::{Pt, Rect, Seg, circle_through, clip_line, cluster_1d, n, pt};use crate::logo::Logo;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CircleFilter {
    #[default]
    All,
    Big,
    Small,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Placement {
    #[default]
    Inside,
    Outside,
}

/// How far gridlines run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Extent {
    /// Across the logo, with a little overshoot.
    #[default]
    Logo,
    /// Across the whole artboard.
    Artboard,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ElementStyles {
    pub horizontals: Style,
    pub verticals: Style,
    pub diagonals: Style,
    pub circles: Style,
    pub anchors: Style,
    pub handles: Style,
    pub outlines: Style,
    pub spacing: Style,
    pub measurements: Style,
}

impl Default for ElementStyles {
    fn default() -> Self {
        let line = Style::stroke(ORACLE_RED, 0.5);
        ElementStyles {
            horizontals: line.clone(),
            verticals: line.clone(),
            diagonals: line.clone(),
            circles: line.clone(),
            anchors: Style::stroke(ORACLE_RED, 0.75).with_fill("#ffffff"),
            handles: Style::stroke("#1b1a17", 0.5).with_fill("#1b1a17"),
            outlines: Style::stroke("#1b1a17", 0.75),
            spacing: Style::stroke(ORACLE_RED, 0.5).with_fill(ORACLE_RED).with_opacity(0.25),
            measurements: Style::stroke("#1b1a17", 0.5),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ConstructionParams {
    pub horizontals: bool,
    pub verticals: bool,
    pub diagonals: bool,
    pub circles: bool,
    pub anchors: bool,
    pub handles: bool,
    pub outlines: bool,
    pub spacing: bool,
    pub measurements: bool,
    /// An edge within this angle of horizontal or vertical counts as one; diagonals within it of
    /// each other and on the same line are one diagonal.
    pub angle_tolerance_deg: f64,
    /// Gridline sensitivity: values closer than this share a line, as a fraction of the logo size.
    /// Raise it to merge near-misses, lower it to expose them.
    pub sensitivity: f64,
    pub circle_filter: CircleFilter,
    /// Where big circles start, as a fraction of the logo size (radius).
    pub circle_threshold: f64,
    pub spacing_placement: Placement,
    pub extent: Extent,
    /// Overshoot past the logo, as a fraction of its size.
    pub overshoot: f64,
    /// Anchor and handle marker sizes, in document units.
    pub anchor_size: f64,
    pub handle_size: f64,
    /// Hide what overlaps: duplicated circles, handles inside their anchor, crowded anchors,
    /// gridlines lying on top of each other.
    pub clean: bool,
    pub styles: ElementStyles,
}

impl Default for ConstructionParams {
    fn default() -> Self {
        ConstructionParams {
            horizontals: true,
            verticals: true,
            diagonals: true,
            circles: true,
            anchors: true,
            handles: true,
            outlines: true,
            spacing: false,
            measurements: false,
            angle_tolerance_deg: 2.0,
            sensitivity: 0.004,
            circle_filter: CircleFilter::All,
            circle_threshold: 0.2,
            spacing_placement: Placement::Inside,
            extent: Extent::Logo,
            overshoot: 0.08,
            anchor_size: 0.0,
            handle_size: 0.0,
            clean: false,
            styles: ElementStyles::default(),
        }
    }
}

/// A circle the outlines follow, with how many segments run along it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FoundCircle {
    pub c: Pt,
    pub r: f64,
    pub support: usize,
}

/// A gap between two islands.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gap {
    pub horizontal: bool,
    pub size: f64,
    /// Where the gap starts (x for a horizontal gap, y for a vertical one) and its cross-range.
    pub at: f64,
    pub across: (f64, f64),
}

/// What the analysis found, before it is drawn.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Analysis {
    pub horizontals: Vec<f64>,
    pub verticals: Vec<f64>,
    /// Angle in degrees in [0, 180) and the signed distance of the line from the origin.
    pub diagonals: Vec<(f64, f64)>,
    pub circles: Vec<FoundCircle>,
    pub anchors: Vec<Pt>,
    /// (anchor, handle) pairs.
    pub handles: Vec<(Pt, Pt)>,
    pub gaps: Vec<Gap>,
}

pub fn analyse(logo: &Logo, p: &ConstructionParams) -> Analysis {
    let scale = logo.scale();
    let tol = (p.sensitivity * scale).max(1e-6);
    let ang = p.angle_tolerance_deg.to_radians().max(1e-6);
    let segs: Vec<Seg> = logo.contours().flat_map(|c| c.segs.iter().copied()).collect();

    let (mut hs, mut vs, mut diag) = (Vec::new(), Vec::new(), Vec::new());
    // Each outline's extremes (the apex of an A, the foot of a V) sit on a line too.
    for c in logo.contours() {
        let b = c.bbox();
        hs.extend([b.y0, b.y1]);
        vs.extend([b.x0, b.x1]);
    }
    for s in &segs {
        match *s {
            Seg::Line(a, b) => {
                let d = b - a;
                if d.len() < scale * 1e-3 {
                    continue;
                }
                let theta = d.y.atan2(d.x).rem_euclid(std::f64::consts::PI);
                let off_h = theta.min(std::f64::consts::PI - theta);
                let off_v = (theta - std::f64::consts::FRAC_PI_2).abs();
                if off_h <= ang {
                    hs.push((a.y + b.y) / 2.0);
                } else if off_v <= ang {
                    vs.push((a.x + b.x) / 2.0);
                } else if d.len() >= scale * 0.02 {
                    let normal = pt(-theta.sin(), theta.cos());
                    diag.push((theta, a.dot(normal)));
                }
            }
            Seg::Cubic(p0, p1, p2, p3) => {
                for t in s.extrema(1) {
                    hs.push(s.eval(t).y);
                }
                for t in s.extrema(0) {
                    vs.push(s.eval(t).x);
                }
                // A curve that leaves or reaches an anchor flat: the anchor sits on an extreme.
                for (anchor, handle) in [(p0, p1), (p3, p2)] {
                    let d = handle - anchor;
                    if d.len() < 1e-9 {
                        continue;
                    }
                    if d.y.abs() <= d.len() * ang.sin() {
                        hs.push(anchor.y);
                    }
                    if d.x.abs() <= d.len() * ang.sin() {
                        vs.push(anchor.x);
                    }
                }
            }
        }
    }
    let line_tol = if p.clean { tol * 3.0 } else { tol };
    let horizontals = cluster_1d(hs, line_tol).into_iter().map(|(v, _)| v).collect();
    let verticals = cluster_1d(vs, line_tol).into_iter().map(|(v, _)| v).collect();
    let diagonals = cluster_lines(diag, ang, line_tol);

    // Circles: a cubic follows a circle when its quarter points sit on the circle through its ends
    // and middle. Arcs of the same circle are gathered.
    let mut found: Vec<FoundCircle> = Vec::new();
    for s in &segs {
        let Seg::Cubic(..) = s else { continue };
        if s.chord() < scale * 0.01 {
            continue;
        }
        let Some((c, r)) = circle_through(s.start(), s.eval(0.5), s.end()) else { continue };
        // Too flat to tell a circle (under about 20° of arc), or bigger than the logo makes sense of.
        if r > scale * 2.0 || s.chord() < r * 0.35 {
            continue;
        }
        let dev = (r * 0.01).min(tol * 0.5).max(scale * 1e-4);
        let fits = [0.125, 0.25, 0.375, 0.625, 0.75, 0.875].iter().all(|t| (s.eval(*t).dist(c) - r).abs() <= dev);
        if !fits {
            continue;
        }
        let ctol = tol * 2.0;
        match found.iter_mut().find(|f| f.c.dist(c) <= ctol && (f.r - r).abs() <= ctol) {
            Some(f) => {
                let k = f.support as f64;
                f.c = (f.c * k + c) * (1.0 / (k + 1.0));
                f.r = (f.r * k + r) / (k + 1.0);
                f.support += 1;
            }
            None => found.push(FoundCircle { c, r, support: 1 }),
        }
    }
    let threshold = p.circle_threshold * scale;
    found.retain(|f| match p.circle_filter {
        CircleFilter::All => true,
        CircleFilter::Big => f.r >= threshold,
        CircleFilter::Small => f.r < threshold,
    });
    if p.clean {
        // Circles barely different from a bigger-supported one are hidden.
        found.sort_by_key(|f| std::cmp::Reverse(f.support));
        let mut kept: Vec<FoundCircle> = Vec::new();
        for f in found {
            if !kept.iter().any(|k| k.c.dist(f.c) <= tol * 6.0 && (k.r - f.r).abs() <= tol * 6.0) {
                kept.push(f);
            }
        }
        found = kept;
    }

    let marker = marker_size(p.anchor_size, scale);
    let mut anchors: Vec<Pt> = Vec::new();
    let mut handles: Vec<(Pt, Pt)> = Vec::new();
    for c in logo.contours() {
        for (i, s) in c.segs.iter().enumerate() {
            let pts = if !c.closed && i + 1 == c.segs.len() { vec![s.start(), s.end()] } else { vec![s.start()] };
            for a in pts {
                let crowd = if p.clean { marker } else { 1e-6 };
                if !anchors.iter().any(|q| q.dist(a) <= crowd) {
                    anchors.push(a);
                }
            }
            if let Seg::Cubic(p0, p1, p2, p3) = *s {
                let min = if p.clean { marker * 1.5 } else { 1e-6 };
                if p0.dist(p1) > min {
                    handles.push((p0, p1));
                }
                if p3.dist(p2) > min {
                    handles.push((p3, p2));
                }
            }
        }
    }

    Analysis { horizontals, verticals, diagonals, circles: found, anchors, handles, gaps: gaps(logo, tol) }
}

fn marker_size(set: f64, scale: f64) -> f64 {
    if set > 0.0 { set } else { (scale * 0.012).max(1.5) }
}

/// Merges lines (angle, offset) whose angles and offsets are within tolerance.
fn cluster_lines(mut lines: Vec<(f64, f64)>, ang: f64, tol: f64) -> Vec<(f64, f64)> {
    lines.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let mut out: Vec<(f64, f64, usize)> = Vec::new();
    for (t, o) in lines {
        match out.iter_mut().find(|(ct, co, _)| angle_close(*ct, t, ang) && (co - same_side(*ct, t, o)).abs() <= tol) {
            Some((ct, co, k)) => {
                let kk = *k as f64;
                let o = same_side(*ct, t, o);
                *co = (*co * kk + o) / (kk + 1.0);
                *k += 1;
            }
            None => out.push((t, o, 1)),
        }
    }
    out.into_iter().map(|(t, o, _)| (t.to_degrees(), o)).collect()
}

fn angle_close(a: f64, b: f64, tol: f64) -> bool {
    let d = (a - b).abs();
    d <= tol || (std::f64::consts::PI - d) <= tol
}

/// Angles near 0 and near π describe the same direction with opposite normals.
fn same_side(reference: f64, t: f64, offset: f64) -> f64 {
    if (reference - t).abs() > std::f64::consts::FRAC_PI_2 { -offset } else { offset }
}

/// Gaps between neighbouring islands, side by side (horizontal gaps) or stacked (vertical gaps).
fn gaps(logo: &Logo, tol: f64) -> Vec<Gap> {
    let islands = logo.islands();
    let mut out = Vec::new();
    for horizontal in [true, false] {
        for a in &islands {
            // The nearest island after `a` along the axis that overlaps it across.
            let mut best: Option<(&Rect, f64)> = None;
            for b in &islands {
                let (overlap, gap) = if horizontal {
                    (a.y0.max(b.y0) < a.y1.min(b.y1), b.x0 - a.x1)
                } else {
                    (a.x0.max(b.x0) < a.x1.min(b.x1), b.y0 - a.y1)
                };
                if overlap && gap > tol && best.is_none_or(|(_, g)| gap < g) {
                    best = Some((b, gap));
                }
            }
            if let Some((b, size)) = best {
                let (at, across) = if horizontal {
                    (a.x1, (a.y0.max(b.y0), a.y1.min(b.y1)))
                } else {
                    (a.y1, (a.x0.max(b.x0), a.x1.min(b.x1)))
                };
                out.push(Gap { horizontal, size, at, across });
            }
        }
    }
    out
}

pub fn construction(logo: &Logo, artboard: Option<Rect>, p: &ConstructionParams) -> Drawing {
    let a = analyse(logo, p);
    let bbox = logo.bbox();
    let scale = logo.scale();
    let reach = match (p.extent, artboard) {
        (Extent::Artboard, Some(r)) => r,
        _ => bbox.inflate(p.overshoot * scale),
    };
    let room = if p.measurements || p.spacing { 0.25 } else { 0.15 };
    let mut bounds = reach.union(&bbox.inflate(scale * room));
    if let Some(r) = artboard {
        bounds = bounds.union(&r);
    }
    let mut d = Drawing::new(bounds);
    let st = &p.styles;

    // The outlines first: the grid is read on top of them.
    if p.outlines {
        let items = logo.contours().map(|c| Item::Path { d: c.to_path_data(), fill: None, transform: None }).collect();
        d.layer("Contours", st.outlines.clone(), items);
    }
    if p.horizontals {
        let items = a.horizontals.iter().map(|y| Item::Line { a: pt(reach.x0, *y), b: pt(reach.x1, *y) }).collect();
        d.layer("Horizontales", st.horizontals.clone(), items);
    }
    if p.verticals {
        let items = a.verticals.iter().map(|x| Item::Line { a: pt(*x, reach.y0), b: pt(*x, reach.y1) }).collect();
        d.layer("Verticales", st.verticals.clone(), items);
    }
    if p.diagonals {
        let items = a
            .diagonals
            .iter()
            .filter_map(|(deg, off)| {
                let t = deg.to_radians();
                let dir = pt(t.cos(), t.sin());
                let normal = pt(-dir.y, dir.x);
                clip_line(normal * *off, dir, &reach).map(|(a, b)| Item::Line { a, b })
            })
            .collect();
        d.layer("Diagonales", st.diagonals.clone(), items);
    }
    if p.circles {
        let items = a.circles.iter().map(|c| Item::Circle { c: c.c, r: c.r }).collect();
        d.layer("Cercles", st.circles.clone(), items);
    }
    if p.handles {
        let hs = marker_size(p.handle_size, scale) * 0.8;
        let mut lines = Vec::new();
        let mut dots = Vec::new();
        for (anchor, handle) in &a.handles {
            lines.push(Item::Line { a: *anchor, b: *handle });
            dots.push(Item::Circle { c: *handle, r: hs / 2.0 });
        }
        d.layer("Poignées", Style { fill: None, ..st.handles.clone() }, lines);
        d.layer("Points de poignée", st.handles.clone(), dots);
    }
    if p.anchors {
        let s = marker_size(p.anchor_size, scale);
        let items = a
            .anchors
            .iter()
            .map(|q| Item::Rect { rect: Rect::from_size(q.x - s / 2.0, q.y - s / 2.0, s, s), radius: 0.0 })
            .collect();
        d.layer("Ancres", st.anchors.clone(), items);
    }
    if p.spacing {
        let mut boxes = Vec::new();
        let mut labels = Vec::new();
        let text = (scale * 0.025).max(4.0);
        for g in &a.gaps {
            let s = g.size;
            let rect = match (g.horizontal, p.spacing_placement) {
                (true, Placement::Inside) => Rect::from_size(g.at, (g.across.0 + g.across.1 - s) / 2.0, s, s),
                (true, Placement::Outside) => Rect::from_size(g.at, bbox.y0 - s - scale * 0.04, s, s),
                (false, Placement::Inside) => Rect::from_size((g.across.0 + g.across.1 - s) / 2.0, g.at, s, s),
                (false, Placement::Outside) => Rect::from_size(bbox.x1 + scale * 0.04, g.at, s, s),
            };
            labels.push(Item::Text { at: pt(rect.center().x, rect.y0 - text * 0.4), text: n1(s), size: text, anchor: Anchor::Middle });
            boxes.push(Item::Rect { rect, radius: 0.0 });
        }
        d.layer("Espacements", st.spacing.clone(), boxes);
        d.layer("Valeurs d'espacement", Style { opacity: 1.0, ..Style::fill(ORACLE_RED) }, labels);
    }
    if p.measurements {
        d.layer("Cotes", st.measurements.clone(), measurements(bbox, scale));
    }
    d
}

/// Overall width below the logo and height on its left, with their values.
pub fn measurements(b: Rect, scale: f64) -> Vec<Item> {
    let off = scale * 0.08;
    let tick = scale * 0.02;
    let text = (scale * 0.03).max(4.0);
    let (y, x) = (b.y1 + off, b.x0 - off);
    vec![
        Item::Line { a: pt(b.x0, y), b: pt(b.x1, y) },
        Item::Line { a: pt(b.x0, y - tick), b: pt(b.x0, y + tick) },
        Item::Line { a: pt(b.x1, y - tick), b: pt(b.x1, y + tick) },
        Item::Text { at: pt(b.center().x, y + tick + text), text: n1(b.width()), size: text, anchor: Anchor::Middle },
        Item::Line { a: pt(x, b.y0), b: pt(x, b.y1) },
        Item::Line { a: pt(x - tick, b.y0), b: pt(x + tick, b.y0) },
        Item::Line { a: pt(x - tick, b.y1), b: pt(x + tick, b.y1) },
        Item::Text { at: pt(x - tick - text * 0.3, b.center().y + text * 0.35), text: n1(b.height()), size: text, anchor: Anchor::End },
    ]
}

/// A measurement as shown: one decimal at most.
fn n1(v: f64) -> String {
    n((v * 10.0).round() / 10.0)
}

#[cfg(all(test, feature = "svg"))]
mod tests {
    use super::*;
    use crate::logo::tests::MARK_AND_TYPE;

    fn logo(svg: &str) -> Logo {
        Logo::from_svg(svg).unwrap()
    }

    #[test]
    fn a_square_gives_two_horizontals_two_verticals_and_four_anchors() {
        let l = logo(r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect x="10" y="20" width="50" height="30"/></svg>"#);
        let a = analyse(&l, &ConstructionParams::default());
        assert_eq!(a.horizontals, vec![20.0, 50.0]);
        assert_eq!(a.verticals, vec![10.0, 60.0]);
        assert_eq!(a.anchors.len(), 4);
        assert!(a.diagonals.is_empty() && a.circles.is_empty() && a.handles.is_empty());
    }

    #[test]
    fn a_circle_is_found_once_with_its_extremes() {
        let l = logo(r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><circle cx="50" cy="50" r="40"/></svg>"#);
        let a = analyse(&l, &ConstructionParams::default());
        assert_eq!(a.circles.len(), 1, "{:?}", a.circles);
        let c = a.circles[0];
        assert!(c.c.dist(pt(50.0, 50.0)) < 0.2 && (c.r - 40.0).abs() < 0.2);
        assert!(c.support >= 4);
        assert_eq!(a.horizontals.len(), 2);
        assert!((a.horizontals[0] - 10.0).abs() < 0.01 && (a.horizontals[1] - 90.0).abs() < 0.01);
        assert_eq!(a.handles.len(), 8);
    }

    #[test]
    fn the_circle_filter_separates_big_and_small() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><circle cx="50" cy="50" r="40"/><circle cx="150" cy="50" r="5"/></svg>"#;
        let l = logo(svg);
        let big = analyse(&l, &ConstructionParams { circle_filter: CircleFilter::Big, ..Default::default() });
        let small = analyse(&l, &ConstructionParams { circle_filter: CircleFilter::Small, ..Default::default() });
        assert_eq!(big.circles.len(), 1);
        assert!((big.circles[0].r - 40.0).abs() < 0.2);
        assert_eq!(small.circles.len(), 1);
        assert!((small.circles[0].r - 5.0).abs() < 0.2);
    }

    #[test]
    fn sloped_edges_become_diagonals_and_parallel_ones_on_one_line_merge() {
        // Two triangles whose left edges lie on the same 60° line.
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200">
            <path d="M0 100 L57.735 0 L115.47 100 Z"/><path d="M-57.735 200 L0 100.0 L40 200 Z"/></svg>"#;
        let a = analyse(&logo(svg), &ConstructionParams::default());
        let sixty: Vec<_> = a.diagonals.iter().filter(|(d, _)| (d - 120.0).abs() < 0.5 || (d - 60.0).abs() < 0.5).collect();
        // The 120° edges (left sides, going up-right on screen) are collinear: one line.
        assert_eq!(a.diagonals.iter().filter(|(d, _)| (d - 120.0).abs() < 0.5).count(), 1, "{:?}", a.diagonals);
        assert!(sixty.len() >= 2);
        assert_eq!(a.horizontals.len(), 3);
    }

    #[test]
    fn sensitivity_decides_whether_near_misses_share_a_line() {
        // Two bars whose tops differ by 0.6 units in a 200-unit logo.
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><rect x="0" y="10" width="50" height="80"/><rect x="150" y="10.6" width="50" height="79.4"/></svg>"#;
        let l = logo(svg);
        let fine = analyse(&l, &ConstructionParams { sensitivity: 0.001, ..Default::default() });
        let coarse = analyse(&l, &ConstructionParams { sensitivity: 0.01, ..Default::default() });
        assert_eq!(fine.horizontals.len(), 3);
        assert_eq!(coarse.horizontals.len(), 2);
    }

    #[test]
    fn gaps_between_letters_are_measured() {
        let a = analyse(&logo(MARK_AND_TYPE), &ConstructionParams::default());
        let mut sizes: Vec<f64> = a.gaps.iter().filter(|g| g.horizontal).map(|g| (g.size * 100.0).round() / 100.0).collect();
        sizes.sort_by(f64::total_cmp);
        // mark → first bar 20, bar → bar 20, bar → triangle 20.
        assert_eq!(sizes, vec![20.0, 20.0, 20.0]);
    }

    #[test]
    fn the_drawing_has_one_layer_per_element_and_writes_svg() {
        let p = ConstructionParams { spacing: true, measurements: true, ..Default::default() };
        let l = logo(MARK_AND_TYPE);
        let d = construction(&l, None, &p);
        for name in ["Horizontales", "Verticales", "Diagonales", "Cercles", "Contours", "Ancres", "Poignées", "Espacements", "Cotes"] {
            assert!(d.find(name).is_some(), "{name}");
        }
        let svg = d.to_svg(Some(&l), Some("#ffffff"));
        assert!(svg.contains(r#"id="horizontales""#) && svg.contains("<circle"));
        assert!(usvg::Tree::from_str(&svg, &usvg::Options::default()).is_ok());
    }

    #[test]
    fn clean_hides_handles_inside_their_anchor_and_crowded_anchors() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><path d="M0 0 C0.1 0 99.9 100 100 100 L0 100 L0.05 99.95 Z"/></svg>"#;
        let l = logo(svg);
        let raw = analyse(&l, &ConstructionParams::default());
        let clean = analyse(&l, &ConstructionParams { clean: true, ..Default::default() });
        assert!(clean.handles.len() < raw.handles.len());
        assert!(clean.anchors.len() < raw.anchors.len());
    }
}
