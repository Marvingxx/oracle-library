//! Base grids drawn over an artboard: square, isometric, hexagonal and golden ratio.

use crate::drawing::{Drawing, Guide, Item, ORACLE_RED, Style};
use crate::geom::{Pt, Rect, clip_line, pt};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BaseKind {
    #[default]
    Square,
    Isometric,
    Hexagon,
    GoldenRatio,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BaseParams {
    pub kind: BaseKind,
    /// Square: cell side. Isometric: triangle side. Hexagon: hexagon radius.
    pub size: f64,
    pub stroke: f64,
    pub color: String,
    /// Golden ratio: the height of the golden rectangle; 0 fits it to the artboard.
    pub golden_height: f64,
    /// Golden ratio: how many squares the rectangle is divided into.
    pub golden_steps: usize,
}

impl Default for BaseParams {
    fn default() -> Self {
        BaseParams { kind: BaseKind::Square, size: 20.0, stroke: 0.5, color: ORACLE_RED.into(), golden_height: 0.0, golden_steps: 8 }
    }
}

pub const PHI: f64 = std::f64::consts::GOLDEN_RATIO;

pub fn base_grid(area: Rect, p: &BaseParams) -> Drawing {
    let mut d = Drawing::new(area);
    let style = Style::stroke(&p.color, p.stroke);
    let size = p.size.max(area.max_dim() / 2000.0).max(1e-3);
    match p.kind {
        BaseKind::Square => {
            let lines = families(area, size, &[0.0, 90.0]);
            // A square grid also becomes real guides.
            let c = area.center();
            for k in steps(area.x0 - c.x, area.x1 - c.x, size) {
                d.guides.push(Guide::Vertical(c.x + k));
            }
            for k in steps(area.y0 - c.y, area.y1 - c.y, size) {
                d.guides.push(Guide::Horizontal(c.y + k));
            }
            d.layer("Grille carrée", style, lines);
        }
        BaseKind::Isometric => {
            // Lines of a triangular lattice of side `size` sit `size·√3/2` apart, in three directions.
            let h = size * 3f64.sqrt() / 2.0;
            d.layer("Grille isométrique", style, families(area, h, &[90.0, 30.0, 150.0]));
        }
        BaseKind::Hexagon => {
            d.clip = Some(area);
            d.layer("Grille hexagonale", style, hexagons(area, size));
        }
        BaseKind::GoldenRatio => golden(&mut d, area, p, style),
    }
    d
}

/// Offsets from 0 in steps of `step` that fall within [lo, hi].
fn steps(lo: f64, hi: f64, step: f64) -> Vec<f64> {
    let (a, b) = ((lo / step).ceil() as i64, (hi / step).floor() as i64);
    (a..=b).map(|k| k as f64 * step).collect()
}

/// Parallel lines `spacing` apart through the centre of `area`, one family per direction (degrees).
fn families(area: Rect, spacing: f64, directions: &[f64]) -> Vec<Item> {
    let c = area.center();
    let mut out = Vec::new();
    for deg in directions {
        let t = deg.to_radians();
        let dir = pt(t.cos(), t.sin());
        let normal = pt(-dir.y, dir.x);
        let proj: Vec<f64> = area.corners().iter().map(|q| (*q - c).dot(normal)).collect();
        let (lo, hi) = proj.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(*v), b.max(*v)));
        for k in steps(lo, hi, spacing) {
            if let Some((a, b)) = clip_line(c + normal * k, dir, &area)
                && a.dist(b) > 1e-6
            {
                out.push(Item::Line { a, b });
            }
        }
    }
    out
}

/// Pointy-top hexagons of radius `r` covering `area`, one centred on it.
fn hexagons(area: Rect, r: f64) -> Vec<Item> {
    let c = area.center();
    let (dx, dy) = (3f64.sqrt() * r, 1.5 * r);
    let cover = area.inflate(r);
    let rows = (area.height() / 2.0 / dy).ceil() as i64 + 1;
    let cols = (area.width() / 2.0 / dx).ceil() as i64 + 1;
    let mut out = Vec::new();
    for j in -rows..=rows {
        for i in -cols..=cols {
            let shift = if j.rem_euclid(2) == 1 { dx / 2.0 } else { 0.0 };
            let center = pt(c.x + i as f64 * dx + shift, c.y + j as f64 * dy);
            if !cover.contains(center) {
                continue;
            }
            let points = (0..6)
                .map(|k| {
                    let a = (60.0 * k as f64 - 90.0).to_radians();
                    center + pt(a.cos(), a.sin()) * r
                })
                .collect();
            out.push(Item::Polygon { points });
        }
    }
    out
}

/// A golden rectangle cut into squares, the spiral through them and the circle of each square.
fn golden(d: &mut Drawing, area: Rect, p: &BaseParams, style: Style) {
    let fit = area.height().min(area.width() / PHI) * 0.9;
    let h = if p.golden_height > 0.0 { p.golden_height } else { fit };
    let w = h * PHI;
    let c = area.center();
    let mut rest = Rect::from_size(c.x - w / 2.0, c.y - h / 2.0, w, h);
    let (mut squares, mut arcs, mut circles) = (vec![Item::Rect { rect: rest, radius: 0.0 }], Vec::new(), Vec::new());
    let mut cur = pt(rest.x0, rest.y1);
    for step in 0..p.golden_steps.max(1) {
        let (x, y, w, h) = (rest.x0, rest.y0, rest.width(), rest.height());
        let s = w.min(h);
        if s < 1e-3 {
            break;
        }
        // Squares are taken in turn from the left, top, right and bottom: the spiral turns inwards.
        let (square, next) = match step % 4 {
            0 => (Rect::from_size(x, y, s, s), Rect::from_size(x + s, y, w - s, h)),
            1 => (Rect::from_size(x, y, s, s), Rect::from_size(x, y + s, w, h - s)),
            2 => (Rect::from_size(x + w - s, y, s, s), Rect::from_size(x, y, w - s, h)),
            _ => (Rect::from_size(x, y + h - s, s, s), Rect::from_size(x, y, w, h - s)),
        };
        // The two corners the square shares with what is left: the spiral turns around the one
        // at distance `s` from where it arrived and leaves through the other.
        let shared: Vec<Pt> =
            square.corners().into_iter().filter(|q| next.inflate(1e-6).contains(*q)).collect();
        if shared.len() == 2 {
            let (center, end) = if (shared[0].dist(cur) - s).abs() < (shared[1].dist(cur) - s).abs() {
                (shared[0], shared[1])
            } else {
                (shared[1], shared[0])
            };
            let clockwise = (cur - center).cross(end - center) > 0.0;
            arcs.push(Item::Arc { c: center, r: s, a: cur, b: end, clockwise });
            cur = end;
        }
        squares.push(Item::Rect { rect: square, radius: 0.0 });
        circles.push(Item::Circle { c: square.center(), r: s / 2.0 });
        rest = next;
    }
    d.layer("Rectangles d'or", style.clone(), squares);
    d.layer("Cercles d'or", style.clone().with_opacity(0.6), circles);
    d.layer("Spirale d'or", Style { width: style.width * 2.0, ..style }, arcs);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> Rect {
        Rect::from_size(0.0, 0.0, 200.0, 100.0)
    }

    #[test]
    fn a_square_grid_has_a_line_and_a_guide_every_cell() {
        let d = base_grid(area(), &BaseParams { size: 20.0, ..BaseParams::default() });
        // Centred on (100, 50): x = 0, 20, …, 200 (11 lines) and y = 10, 30, …, 90 (5 lines).
        assert_eq!(d.count("Grille carrée"), 11 + 5);
        assert_eq!(d.guides.len(), 16);
        assert!(d.guides.contains(&Guide::Horizontal(50.0)));
    }

    #[test]
    fn isometric_lines_run_in_three_directions() {
        let d = base_grid(area(), &BaseParams { kind: BaseKind::Isometric, size: 20.0, ..BaseParams::default() });
        let layer = d.find("Grille isométrique").unwrap();
        let mut angles: Vec<i64> = layer
            .items
            .iter()
            .map(|i| match i {
                Item::Line { a, b } => ((b.y - a.y).atan2(b.x - a.x).to_degrees().rem_euclid(180.0)).round() as i64,
                _ => -1,
            })
            .collect();
        angles.sort();
        angles.dedup();
        assert_eq!(angles, vec![30, 90, 150]);
        for i in &layer.items {
            if let Item::Line { a, b } = i {
                assert!(area().inflate(1e-6).contains(*a) && area().inflate(1e-6).contains(*b));
            }
        }
    }

    #[test]
    fn hexagons_cover_the_artboard_and_are_clipped_to_it() {
        let d = base_grid(area(), &BaseParams { kind: BaseKind::Hexagon, size: 10.0, ..BaseParams::default() });
        assert_eq!(d.clip, Some(area()));
        assert!(d.count("Grille hexagonale") > 40);
    }

    #[test]
    fn the_golden_spiral_is_continuous_and_squares_shrink_by_phi() {
        let d = base_grid(area(), &BaseParams { kind: BaseKind::GoldenRatio, ..BaseParams::default() });
        let outer = &d.find("Rectangles d'or").unwrap().items[0];
        let Item::Rect { rect, .. } = outer else { panic!() };
        assert!((rect.height() - 90.0).abs() < 1e-9, "fitted to 90 % of the artboard height");
        let arcs: Vec<&Item> = d.find("Spirale d'or").unwrap().items.iter().collect();
        assert_eq!(arcs.len(), 8);
        for w in arcs.windows(2) {
            let (Item::Arc { b, .. }, Item::Arc { a, .. }) = (w[0], w[1]) else { panic!() };
            assert!(b.dist(*a) < 1e-6);
        }
        let radii: Vec<f64> = arcs.iter().map(|i| if let Item::Arc { r, .. } = i { *r } else { 0.0 }).collect();
        for w in radii.windows(2) {
            assert!((w[0] / w[1] - PHI).abs() < 1e-6);
        }
        // Every arc turns the same way.
        let turns: Vec<bool> = arcs.iter().map(|i| matches!(i, Item::Arc { clockwise: true, .. })).collect();
        assert!(turns.iter().all(|t| *t == turns[0]));
    }
}
