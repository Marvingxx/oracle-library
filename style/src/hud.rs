//! The HUD marks: corner brackets around what is framed, the grid behind the work, and labels in
//! tracked mono capitals with a dash in front.

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, Painter, Pos2, Rect, Stroke, Ui, Vec2, pos2, vec2};

use crate::fonts;

/// Letter spacing of labels, as a fraction of their size (the LAB's 0.14–0.24 em).
pub const LABEL_TRACKING: f32 = 0.16;

/// Corner brackets inside `rect`: two strokes of `len` at each corner.
pub fn corners(painter: &Painter, rect: Rect, len: f32, stroke: Stroke) {
    let len = len.min(rect.width() / 2.0).min(rect.height() / 2.0).max(0.0);
    for (corner, dx, dy) in [
        (rect.left_top(), 1.0, 1.0),
        (rect.right_top(), -1.0, 1.0),
        (rect.left_bottom(), 1.0, -1.0),
        (rect.right_bottom(), -1.0, -1.0),
    ] {
        painter.line_segment([corner, corner + vec2(dx * len, 0.0)], stroke);
        painter.line_segment([corner, corner + vec2(0.0, dy * len)], stroke);
    }
}

/// A grid of hairlines every `step` points, aligned on the painter's origin so it doesn't swim
/// when `rect` moves; a plus mark where every fourth lines cross.
pub fn grid(painter: &Painter, rect: Rect, step: f32, color: Color32) {
    if !(step >= 4.0) || !rect.is_finite() || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let stroke = Stroke::new(1.0, color);
    let first = |v: f32| (v / step).ceil() * step;
    let mut x = first(rect.left());
    while x <= rect.right() {
        painter.vline(x, rect.y_range(), stroke);
        x += step;
    }
    let mut y = first(rect.top());
    while y <= rect.bottom() {
        painter.hline(rect.x_range(), y, stroke);
        y += step;
    }
    let big = step * 4.0;
    let mark = Stroke::new(1.0, color.gamma_multiply(2.2));
    let mut x = (rect.left() / big).ceil() * big;
    while x <= rect.right() {
        let mut y = (rect.top() / big).ceil() * big;
        while y <= rect.bottom() {
            let c = pos2(x, y);
            painter.line_segment([c - vec2(3.0, 0.0), c + vec2(3.0, 0.0)], mark);
            painter.line_segment([c - vec2(0.0, 3.0), c + vec2(0.0, 3.0)], mark);
            y += big;
        }
        x += big;
    }
}

/// `text` as a label: JetBrains Mono Bold capitals, tracked.
pub fn label_job(text: &str, size: f32, color: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.append(&text.to_uppercase(), 0.0, TextFormat { font_id: fonts::label(size), color, extra_letter_spacing: size * LABEL_TRACKING, ..Default::default() });
    job
}

/// A label, as the LAB's `.tag`: a short dash in `accent`, then the text.
pub fn tag(ui: &mut Ui, text: &str, size: f32, color: Color32, accent: Color32) -> egui::Response {
    let galley = ui.painter().layout_job(label_job(text, size, color));
    let dash = vec2(size * 1.6, 2.0);
    let gap = size * 0.7;
    let (rect, response) = ui.allocate_exact_size(vec2(dash.x + gap + galley.size().x, galley.size().y.max(size)), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let mid = rect.center().y;
        ui.painter().rect_filled(Rect::from_min_size(pos2(rect.left(), mid - dash.y / 2.0), dash), 0.0, accent);
        ui.painter().galley(Pos2::new(rect.left() + dash.x + gap, mid - galley.size().y / 2.0), galley, color);
    }
    response
}

/// The underline under the active tab or item: a 2-point bar along the bottom of `rect`.
pub fn active_bar(painter: &Painter, rect: Rect, color: Color32) {
    painter.rect_filled(Rect::from_min_max(pos2(rect.left(), rect.bottom() - 2.0), rect.right_bottom()), 0.0, color);
}

/// Size of a corner bracket for a frame of `size`: about a twelfth of its shorter side, 6 to 18.
pub fn corner_len(size: Vec2) -> f32 {
    (size.x.min(size.y) / 12.0).clamp(6.0, 18.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_tracked_capitals() {
        let job = label_job("Calques", 10.0, Color32::WHITE);
        assert_eq!(job.text, "CALQUES");
        assert!(job.sections[0].format.extra_letter_spacing > 1.0);
    }

    #[test]
    fn marks_draw_without_panicking_on_odd_rects() {
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            let ctx = ui.ctx();
            let painter = ctx.layer_painter(egui::LayerId::background());
            for rect in [Rect::NOTHING, Rect::ZERO, Rect::from_min_size(pos2(-5.0, 3.0), vec2(2.0, 900.0)), Rect::EVERYTHING] {
                corners(&painter, rect, 12.0, Stroke::new(1.0, Color32::RED));
                grid(&painter, rect, 16.0, Color32::GRAY);
            }
            grid(&painter, Rect::from_min_size(Pos2::ZERO, vec2(100.0, 100.0)), f32::NAN, Color32::GRAY);
        })
        .textures_delta
        .clear();
        assert_eq!(corner_len(vec2(400.0, 120.0)), 10.0);
    }
}
