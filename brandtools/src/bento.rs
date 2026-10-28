//! Bento grids: a rectangle divided into tiles for presenting a brand (portfolio pages, slides,
//! social posts). Four styles, a tile count, rounded corners, a gutter, and a seed that shuffles
//! the composition without changing the settings.

use crate::drawing::{Anchor, Drawing, Item, Style};
use crate::geom::{Rect, Rng, pt};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BentoStyle {
    /// Even rows of even tiles.
    #[default]
    Simple,
    /// One large tile in the middle, the others around it.
    Centered,
    /// Rows offset by half a tile, like brickwork.
    Brick,
    /// Tiles of mixed sizes, split at random.
    Random,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BentoParams {
    pub style: BentoStyle,
    pub sections: usize,
    pub radius: f64,
    pub gutter: f64,
    pub seed: u64,
    pub fill: String,
    pub numbers: bool,
}

impl Default for BentoParams {
    fn default() -> Self {
        BentoParams {
            style: BentoStyle::Simple,
            sections: 6,
            radius: 12.0,
            gutter: 12.0,
            seed: 0,
            fill: "#ecebe6".into(),
            numbers: false,
        }
    }
}

pub const MAX_SECTIONS: usize = 64;

/// The tiles, gutter and corners not yet applied: they share edges exactly.
pub fn layout(area: Rect, p: &BentoParams) -> Vec<Rect> {
    let n = p.sections.clamp(1, MAX_SECTIONS);
    let mut rng = Rng::new(p.seed);
    match p.style {
        BentoStyle::Simple => rows(area, &row_counts(area, n, p.seed, &mut rng), None),
        BentoStyle::Brick => {
            let counts = row_counts(area, n, p.seed, &mut rng);
            let first_offset = p.seed % 2 == 1;
            rows(area, &counts, Some(first_offset))
        }
        BentoStyle::Centered => centered(area, n, p.seed, &mut rng),
        BentoStyle::Random => random(area, n, &mut rng),
    }
}

/// The tiles as drawn: separated by the gutter, and their corner radius.
pub fn tiles(area: Rect, p: &BentoParams) -> Vec<(Rect, f64)> {
    let g = p.gutter.max(0.0) / 2.0;
    let eps = area.max_dim() * 1e-9;
    layout(area, p)
        .into_iter()
        .map(|t| {
            let r = Rect {
                x0: if (t.x0 - area.x0).abs() > eps { t.x0 + g } else { t.x0 },
                y0: if (t.y0 - area.y0).abs() > eps { t.y0 + g } else { t.y0 },
                x1: if (t.x1 - area.x1).abs() > eps { t.x1 - g } else { t.x1 },
                y1: if (t.y1 - area.y1).abs() > eps { t.y1 - g } else { t.y1 },
            };
            let r = Rect::new(r.x0, r.y0, r.x1.max(r.x0), r.y1.max(r.y0));
            (r, p.radius.max(0.0).min(r.min_dim() / 2.0))
        })
        .collect()
}

pub fn bento(area: Rect, p: &BentoParams) -> Drawing {
    let mut d = Drawing::new(area);
    let ts = tiles(area, p);
    let mut labels = Vec::new();
    if p.numbers {
        for (i, (r, _)) in ts.iter().enumerate() {
            let s = (r.min_dim() * 0.18).clamp(6.0, 48.0);
            labels.push(Item::Text { at: pt(r.center().x, r.center().y + s * 0.35), text: (i + 1).to_string(), size: s, anchor: Anchor::Middle });
        }
    }
    d.layer("Bento", Style::fill(&p.fill), ts.into_iter().map(|(rect, radius)| Item::Rect { rect, radius }).collect());
    d.layer("Numéros", Style::fill("#8a8a8a"), labels);
    d
}

/// How many tiles go in each row: rows near-square for the area, the extra tiles spread over
/// rows chosen by the seed.
fn row_counts(area: Rect, n: usize, seed: u64, rng: &mut Rng) -> Vec<usize> {
    let aspect = area.width() / area.height().max(1e-9);
    let r = ((n as f64 / aspect).sqrt().round() as usize).clamp(1, n);
    let (base, extra) = (n / r, n % r);
    let mut counts = vec![base; r];
    let mut order: Vec<usize> = (0..r).collect();
    if seed != 0 {
        for i in (1..r).rev() {
            order.swap(i, rng.below(i + 1));
        }
    }
    for i in order.into_iter().take(extra) {
        counts[i] += 1;
    }
    // A shuffle may also move one tile between two rows, for a different composition.
    if seed != 0 && r >= 2 {
        let (a, b) = (rng.below(r), rng.below(r));
        if a != b && counts[a] > 1 && counts[a] >= counts[b] {
            counts[a] -= 1;
            counts[b] += 1;
        }
    }
    counts
}

/// Rows of tiles; with `offset`, alternate rows start and end with half tiles.
fn rows(area: Rect, counts: &[usize], offset: Option<bool>) -> Vec<Rect> {
    let rh = area.height() / counts.len() as f64;
    let mut out = Vec::new();
    for (i, &k) in counts.iter().enumerate() {
        let y = area.y0 + rh * i as f64;
        let shifted = offset.is_some_and(|first| (i % 2 == 0) == first) && k >= 2;
        let widths: Vec<f64> = if shifted {
            // k tiles, the two ends half as wide: k - 1 full widths in all.
            let w = area.width() / (k - 1) as f64;
            (0..k).map(|j| if j == 0 || j == k - 1 { w / 2.0 } else { w }).collect()
        } else {
            vec![area.width() / k as f64; k]
        };
        let mut x = area.x0;
        for (j, w) in widths.iter().enumerate() {
            let x1 = if j + 1 == k { area.x1 } else { x + w };
            out.push(Rect::new(x, y, x1, if i + 1 == counts.len() { area.y1 } else { y + rh }));
            x = x1;
        }
    }
    out
}

/// A large centre tile; the others in a column on each side and, when there are many, in bands
/// above and below the centre.
fn centered(area: Rect, n: usize, seed: u64, rng: &mut Rng) -> Vec<Rect> {
    if n < 3 {
        return rows(area, &[n], None);
    }
    let others = n - 1;
    // Left, right, top, bottom, in turn; a shuffle starts the turn elsewhere.
    let mut counts = [0usize; 4];
    let start = if seed == 0 { 0 } else { rng.below(2) };
    for i in 0..others {
        counts[if others <= 4 { (start + i) % 2 } else { (start + i) % 4 }] += 1;
    }
    let side = if seed == 0 { 0.25 } else { rng.range(0.2, 0.3) };
    let band = if seed == 0 { 0.28 } else { rng.range(0.22, 0.32) };
    let (w, h) = (area.width(), area.height());
    let lx = if counts[0] > 0 { area.x0 + w * side } else { area.x0 };
    let rx = if counts[1] > 0 { area.x1 - w * side } else { area.x1 };
    let ty = if counts[2] > 0 { area.y0 + h * band } else { area.y0 };
    let by = if counts[3] > 0 { area.y1 - h * band } else { area.y1 };
    let mut out = vec![Rect::new(lx, ty, rx, by)];
    let column = |x0: f64, x1: f64, k: usize, out: &mut Vec<Rect>| {
        for i in 0..k {
            let y0 = area.y0 + h * i as f64 / k as f64;
            let y1 = if i + 1 == k { area.y1 } else { area.y0 + h * (i + 1) as f64 / k as f64 };
            out.push(Rect::new(x0, y0, x1, y1));
        }
    };
    let bandf = |y0: f64, y1: f64, k: usize, out: &mut Vec<Rect>| {
        for i in 0..k {
            let x0 = lx + (rx - lx) * i as f64 / k as f64;
            let x1 = if i + 1 == k { rx } else { lx + (rx - lx) * (i + 1) as f64 / k as f64 };
            out.push(Rect::new(x0, y0, x1, y1));
        }
    };
    column(area.x0, lx, counts[0], &mut out);
    column(rx, area.x1, counts[1], &mut out);
    bandf(area.y0, ty, counts[2], &mut out);
    bandf(by, area.y1, counts[3], &mut out);
    out
}

/// Repeatedly splits a tile, larger tiles more likely, across its longer side.
fn random(area: Rect, n: usize, rng: &mut Rng) -> Vec<Rect> {
    let mut tiles = vec![area];
    while tiles.len() < n {
        let weights: Vec<f64> = tiles.iter().map(|t| t.area().powf(1.5)).collect();
        let total: f64 = weights.iter().sum();
        let mut pick = rng.f() * total;
        let mut idx = 0;
        for (i, w) in weights.iter().enumerate() {
            if pick < *w {
                idx = i;
                break;
            }
            pick -= w;
            idx = i;
        }
        let t = tiles.swap_remove(idx);
        let ratio = rng.range(0.38, 0.62);
        // Split across the longer side, now and then across the other for variety.
        let vertical_cut = (t.width() >= t.height()) ^ (rng.f() < 0.15 && t.min_dim() > t.max_dim() * 0.7);
        let (a, b) = if vertical_cut {
            let x = t.x0 + t.width() * ratio;
            (Rect::new(t.x0, t.y0, x, t.y1), Rect::new(x, t.y0, t.x1, t.y1))
        } else {
            let y = t.y0 + t.height() * ratio;
            (Rect::new(t.x0, t.y0, t.x1, y), Rect::new(t.x0, y, t.x1, t.y1))
        };
        tiles.push(a);
        tiles.push(b);
    }
    // Reading order: top to bottom, then left to right.
    tiles.sort_by(|a, b| a.y0.total_cmp(&b.y0).then(a.x0.total_cmp(&b.x0)));
    tiles
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> Rect {
        Rect::from_size(0.0, 0.0, 1200.0, 800.0)
    }

    /// The tiles fill the area exactly, without overlapping.
    fn assert_partition(ts: &[Rect]) {
        let total: f64 = ts.iter().map(|t| t.area()).sum();
        assert!((total - area().area()).abs() < 1e-3, "{total}");
        for (i, a) in ts.iter().enumerate() {
            assert!(area().contains_rect(a, 1e-6));
            for b in &ts[i + 1..] {
                let ox = a.x1.min(b.x1) - a.x0.max(b.x0);
                let oy = a.y1.min(b.y1) - a.y0.max(b.y0);
                assert!(ox <= 1e-6 || oy <= 1e-6, "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn every_style_gives_the_count_asked_and_fills_the_area() {
        for style in [BentoStyle::Simple, BentoStyle::Centered, BentoStyle::Brick, BentoStyle::Random] {
            for n in [1, 2, 3, 5, 6, 7, 9, 12, 20] {
                for seed in [0, 1, 2, 99] {
                    let ts = layout(area(), &BentoParams { style, sections: n, seed, ..Default::default() });
                    assert_eq!(ts.len(), n, "{style:?} {n} {seed}");
                    assert_partition(&ts);
                }
            }
        }
    }

    #[test]
    fn the_gutter_separates_neighbours_and_the_edges_stay_flush() {
        let p = BentoParams { sections: 4, gutter: 20.0, ..Default::default() };
        let ts = tiles(area(), &p);
        // 4 tiles in 2 rows of 2.
        let xs: Vec<f64> = ts.iter().map(|(r, _)| r.x0).collect();
        assert!(xs.contains(&0.0) && xs.contains(&610.0));
        assert!(ts.iter().any(|(r, _)| r.x1 == 590.0) && ts.iter().any(|(r, _)| r.x1 == 1200.0));
    }

    #[test]
    fn corners_never_exceed_half_a_tile() {
        let ts = tiles(area(), &BentoParams { sections: 30, radius: 500.0, ..Default::default() });
        assert!(ts.iter().all(|(r, rad)| *rad <= r.min_dim() / 2.0 + 1e-9));
    }

    #[test]
    fn the_same_seed_replays_and_a_shuffle_changes_the_composition() {
        let p = BentoParams { style: BentoStyle::Random, sections: 7, seed: 5, ..Default::default() };
        assert_eq!(layout(area(), &p), layout(area(), &p));
        let others = (6..12).filter(|s| layout(area(), &BentoParams { seed: *s, ..p.clone() }) != layout(area(), &p)).count();
        assert!(others >= 5);
    }

    #[test]
    fn centered_has_the_biggest_tile_in_the_middle() {
        for n in [3, 5, 9] {
            let ts = layout(area(), &BentoParams { style: BentoStyle::Centered, sections: n, ..Default::default() });
            let big = ts.iter().max_by(|a, b| a.area().total_cmp(&b.area())).unwrap();
            assert!(big.contains(area().center()), "{n}");
        }
    }

    #[test]
    fn brick_rows_alternate_half_tiles() {
        let ts = layout(area(), &BentoParams { style: BentoStyle::Brick, sections: 9, ..Default::default() });
        let first_row: Vec<&Rect> = ts.iter().filter(|r| r.y0 == 0.0).collect();
        // 9 tiles in 1200 × 800: two rows of 5 and 4, the second offset.
        let second_row: Vec<&Rect> = ts.iter().filter(|r| (r.y0 - 400.0).abs() < 1e-6).collect();
        assert_eq!((first_row.len(), second_row.len()), (5, 4));
        let w = |row: &[&Rect]| row.iter().map(|r| (r.width() * 100.0).round() as i64).collect::<Vec<_>>();
        let (a, b) = (w(&first_row), w(&second_row));
        assert!(a.windows(2).all(|x| x[0] == x[1]) || b.windows(2).all(|x| x[0] == x[1]));
        assert!(a.first() != a.get(1) || b.first() != b.get(1));
    }
}
