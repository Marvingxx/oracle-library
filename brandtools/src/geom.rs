//! Plain 2D geometry: points, rectangles, line and cubic segments.

use serde::{Deserialize, Serialize};
use std::ops::{Add, Mul, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Pt {
    pub x: f64,
    pub y: f64,
}

pub const fn pt(x: f64, y: f64) -> Pt {
    Pt { x, y }
}

impl Pt {
    pub fn len(self) -> f64 {
        self.x.hypot(self.y)
    }
    pub fn dist(self, o: Pt) -> f64 {
        (self - o).len()
    }
    pub fn dot(self, o: Pt) -> f64 {
        self.x * o.x + self.y * o.y
    }
    pub fn cross(self, o: Pt) -> f64 {
        self.x * o.y - self.y * o.x
    }
    pub fn lerp(self, o: Pt, t: f64) -> Pt {
        self + (o - self) * t
    }
}

impl Add for Pt {
    type Output = Pt;
    fn add(self, o: Pt) -> Pt {
        pt(self.x + o.x, self.y + o.y)
    }
}
impl Sub for Pt {
    type Output = Pt;
    fn sub(self, o: Pt) -> Pt {
        pt(self.x - o.x, self.y - o.y)
    }
}
impl Mul<f64> for Pt {
    type Output = Pt;
    fn mul(self, k: f64) -> Pt {
        pt(self.x * k, self.y * k)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Rect {
    pub fn new(x0: f64, y0: f64, x1: f64, y1: f64) -> Rect {
        Rect { x0: x0.min(x1), y0: y0.min(y1), x1: x0.max(x1), y1: y0.max(y1) }
    }
    pub fn from_size(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect::new(x, y, x + w, y + h)
    }
    /// An empty rectangle that any union replaces.
    pub fn empty() -> Rect {
        Rect { x0: f64::INFINITY, y0: f64::INFINITY, x1: f64::NEG_INFINITY, y1: f64::NEG_INFINITY }
    }
    pub fn is_empty(&self) -> bool {
        !(self.x1 >= self.x0 && self.y1 >= self.y0)
    }
    pub fn width(&self) -> f64 {
        self.x1 - self.x0
    }
    pub fn height(&self) -> f64 {
        self.y1 - self.y0
    }
    pub fn center(&self) -> Pt {
        pt((self.x0 + self.x1) / 2.0, (self.y0 + self.y1) / 2.0)
    }
    pub fn max_dim(&self) -> f64 {
        self.width().max(self.height())
    }
    pub fn min_dim(&self) -> f64 {
        self.width().min(self.height())
    }
    pub fn area(&self) -> f64 {
        self.width() * self.height()
    }
    pub fn union(&self, o: &Rect) -> Rect {
        Rect { x0: self.x0.min(o.x0), y0: self.y0.min(o.y0), x1: self.x1.max(o.x1), y1: self.y1.max(o.y1) }
    }
    pub fn add_point(&self, p: Pt) -> Rect {
        Rect { x0: self.x0.min(p.x), y0: self.y0.min(p.y), x1: self.x1.max(p.x), y1: self.y1.max(p.y) }
    }
    pub fn inflate(&self, d: f64) -> Rect {
        Rect { x0: self.x0 - d, y0: self.y0 - d, x1: self.x1 + d, y1: self.y1 + d }
    }
    pub fn contains(&self, p: Pt) -> bool {
        p.x >= self.x0 && p.x <= self.x1 && p.y >= self.y0 && p.y <= self.y1
    }
    /// True when `o` lies inside this rectangle, with `eps` of slack.
    pub fn contains_rect(&self, o: &Rect, eps: f64) -> bool {
        o.x0 >= self.x0 - eps && o.y0 >= self.y0 - eps && o.x1 <= self.x1 + eps && o.y1 <= self.y1 + eps
    }
    pub fn corners(&self) -> [Pt; 4] {
        [pt(self.x0, self.y0), pt(self.x1, self.y0), pt(self.x1, self.y1), pt(self.x0, self.y1)]
    }
}

/// One piece of an outline. Quadratic curves are raised to cubics when they are read.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Seg {
    Line(Pt, Pt),
    Cubic(Pt, Pt, Pt, Pt),
}

impl Seg {
    pub fn start(&self) -> Pt {
        match *self {
            Seg::Line(a, _) | Seg::Cubic(a, _, _, _) => a,
        }
    }
    pub fn end(&self) -> Pt {
        match *self {
            Seg::Line(_, b) | Seg::Cubic(_, _, _, b) => b,
        }
    }
    pub fn eval(&self, t: f64) -> Pt {
        match *self {
            Seg::Line(a, b) => a.lerp(b, t),
            Seg::Cubic(p0, p1, p2, p3) => {
                let u = 1.0 - t;
                p0 * (u * u * u) + p1 * (3.0 * u * u * t) + p2 * (3.0 * u * t * t) + p3 * (t * t * t)
            }
        }
    }
    pub fn derivative(&self, t: f64) -> Pt {
        match *self {
            Seg::Line(a, b) => b - a,
            Seg::Cubic(p0, p1, p2, p3) => {
                let u = 1.0 - t;
                ((p1 - p0) * (u * u) + (p2 - p1) * (2.0 * u * t) + (p3 - p2) * (t * t)) * 3.0
            }
        }
    }
    /// The interior parameters where the curve turns back along x (`axis` 0) or y (`axis` 1).
    pub fn extrema(&self, axis: usize) -> Vec<f64> {
        let Seg::Cubic(p0, p1, p2, p3) = *self else { return Vec::new() };
        let c = |p: Pt| if axis == 0 { p.x } else { p.y };
        let (a, b, cc) = (c(p1) - c(p0), c(p2) - c(p1), c(p3) - c(p2));
        // Derivative / 3 = A t² + B t + C.
        let qa = a - 2.0 * b + cc;
        let qb = 2.0 * (b - a);
        let qc = a;
        let mut roots = Vec::new();
        if qa.abs() < 1e-12 {
            if qb.abs() > 1e-12 {
                roots.push(-qc / qb);
            }
        } else {
            let disc = qb * qb - 4.0 * qa * qc;
            if disc >= 0.0 {
                let s = disc.sqrt();
                roots.push((-qb + s) / (2.0 * qa));
                roots.push((-qb - s) / (2.0 * qa));
            }
        }
        roots.retain(|t| *t > 1e-6 && *t < 1.0 - 1e-6);
        roots
    }
    pub fn bbox(&self) -> Rect {
        let mut r = Rect::empty().add_point(self.start()).add_point(self.end());
        for axis in 0..2 {
            for t in self.extrema(axis) {
                r = r.add_point(self.eval(t));
            }
        }
        r
    }
    pub fn chord(&self) -> f64 {
        self.start().dist(self.end())
    }
}

/// A closed or open run of segments.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Contour {
    pub segs: Vec<Seg>,
    pub closed: bool,
}

impl Contour {
    pub fn bbox(&self) -> Rect {
        self.segs.iter().fold(Rect::empty(), |r, s| r.union(&s.bbox()))
    }
    /// The contour as SVG path data.
    pub fn to_path_data(&self) -> String {
        let mut d = String::new();
        let Some(first) = self.segs.first() else { return d };
        d.push_str(&format!("M{} {}", n(first.start().x), n(first.start().y)));
        for s in &self.segs {
            match *s {
                Seg::Line(_, b) => d.push_str(&format!("L{} {}", n(b.x), n(b.y))),
                Seg::Cubic(_, a, b, c) => {
                    d.push_str(&format!("C{} {} {} {} {} {}", n(a.x), n(a.y), n(b.x), n(b.y), n(c.x), n(c.y)))
                }
            }
        }
        if self.closed {
            d.push('Z');
        }
        d
    }
}

/// A number written for SVG: at most three decimals, no trailing zeros.
pub fn n(v: f64) -> String {
    let s = format!("{:.3}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

/// The circle through three points, if they are not aligned.
pub fn circle_through(a: Pt, b: Pt, c: Pt) -> Option<(Pt, f64)> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-9 {
        return None;
    }
    let (a2, b2, c2) = (a.dot(a), b.dot(b), c.dot(c));
    let ux = (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d;
    let uy = (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d;
    let center = pt(ux, uy);
    Some((center, center.dist(a)))
}

/// The part of the infinite line through `p` along `dir` that lies inside `r` (Liang–Barsky).
pub fn clip_line(p: Pt, dir: Pt, r: &Rect) -> Option<(Pt, Pt)> {
    let (mut t0, mut t1) = (f64::NEG_INFINITY, f64::INFINITY);
    for (d, lo, hi, v) in [(dir.x, r.x0, r.x1, p.x), (dir.y, r.y0, r.y1, p.y)] {
        if d.abs() < 1e-12 {
            if v < lo || v > hi {
                return None;
            }
        } else {
            let (a, b) = ((lo - v) / d, (hi - v) / d);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        }
    }
    (t1 > t0).then(|| (p + dir * t0, p + dir * t1))
}

/// Groups sorted values that lie within `tol` of their neighbour; returns each group's mean and size.
pub fn cluster_1d(mut values: Vec<f64>, tol: f64) -> Vec<(f64, usize)> {
    values.sort_by(f64::total_cmp);
    let mut out: Vec<(f64, usize, f64)> = Vec::new(); // (sum, count, last)
    for v in values {
        match out.last_mut() {
            Some((sum, count, last)) if v - *last <= tol => {
                *sum += v;
                *count += 1;
                *last = v;
            }
            _ => out.push((v, 1, v)),
        }
    }
    out.into_iter().map(|(s, c, _)| (s / c as f64, c)).collect()
}

/// A small deterministic random generator (SplitMix64), so a shuffle can be replayed from its seed.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in [0, 1).
    pub fn f(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f()
    }
    /// Uniform in 0..n (n > 0).
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n.max(1) as u64) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cubic_extrema_find_the_top_of_an_arch() {
        let s = Seg::Cubic(pt(0.0, 0.0), pt(0.0, -10.0), pt(10.0, -10.0), pt(10.0, 0.0));
        let ts = s.extrema(1);
        assert_eq!(ts.len(), 1);
        assert!((ts[0] - 0.5).abs() < 1e-9);
        assert!((s.bbox().y0 + 7.5).abs() < 1e-9);
    }

    #[test]
    fn circle_through_three_points_of_a_unit_circle() {
        let (c, r) = circle_through(pt(1.0, 0.0), pt(0.0, 1.0), pt(-1.0, 0.0)).unwrap();
        assert!(c.len() < 1e-9 && (r - 1.0).abs() < 1e-9);
        assert!(circle_through(pt(0.0, 0.0), pt(1.0, 1.0), pt(2.0, 2.0)).is_none());
    }

    #[test]
    fn clipping_a_diagonal_to_a_square() {
        let (a, b) = clip_line(pt(0.0, 0.0), pt(1.0, 1.0), &Rect::new(-1.0, -1.0, 1.0, 1.0)).unwrap();
        assert!(a.dist(pt(-1.0, -1.0)) < 1e-9 && b.dist(pt(1.0, 1.0)) < 1e-9);
        assert!(clip_line(pt(5.0, 0.0), pt(0.0, 1.0), &Rect::new(-1.0, -1.0, 1.0, 1.0)).is_none());
    }

    #[test]
    fn values_cluster_within_tolerance() {
        let c = cluster_1d(vec![10.0, 0.0, 0.1, 10.05, 20.0], 0.2);
        assert_eq!(c.len(), 3);
        assert_eq!(c[0].1, 2);
        assert!((c[1].0 - 10.025).abs() < 1e-9);
    }

    #[test]
    fn the_generator_replays_from_its_seed() {
        let (mut a, mut b) = (Rng::new(7), Rng::new(7));
        for _ in 0..10 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        assert_ne!(Rng::new(7).next_u64(), Rng::new(8).next_u64());
    }
}
