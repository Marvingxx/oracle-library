//! The colours. Dark is the default interface; light is the same identity on paper.

use egui::Color32;

/// Edit's icon red, shared by its light and dark interfaces.
pub const RED: Color32 = Color32::from_rgb(0xd3, 0x1c, 0x3b);
pub const HOT: Color32 = RED;
/// Acid yellow is reserved for warnings, never for active tools or tabs.
pub const ACID: Color32 = Color32::from_rgb(0xe9, 0xff, 0x41);
/// Acid yellow readable on paper (an olive of the same hue).
pub const ACID_ON_PAPER: Color32 = Color32::from_rgb(0x6b, 0x7a, 0x00);
/// Cyan: the third accent (information, links).
pub const CYAN: Color32 = Color32::from_rgb(0x16, 0xe0, 0xff);

/// One interface's colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub dark: bool,
    /// Behind everything: the window, the canvas surround.
    pub void: Color32,
    /// Panels.
    pub ink: Color32,
    /// Raised surfaces: fields, hovered rows, menus.
    pub steel: Color32,
    /// Pressed, selected rows.
    pub steel_hi: Color32,
    /// Hairlines between panels and around fields.
    pub line: Color32,
    /// Lines that must be seen (field borders, focus-less frames).
    pub line_strong: Color32,
    /// Text.
    pub text: Color32,
    /// Secondary text.
    pub text_dim: Color32,
    /// Hints, disabled text, labels' quiet part.
    pub text_faint: Color32,
    /// The brand red as this interface shows it.
    pub accent: Color32,
    /// The red behind selected things, quiet.
    pub accent_soft: Color32,
    /// The application accent for active tools, tabs and focus.
    pub active: Color32,
    /// Text on the accent.
    pub on_accent: Color32,
    /// The grid behind the work.
    pub grid: Color32,
}

impl Palette {
    /// The default interface: void black, ink panels, paper text.
    pub const DARK: Palette = Palette {
        dark: true,
        void: Color32::from_rgb(0x07, 0x07, 0x0a),
        ink: Color32::from_rgb(0x0e, 0x0e, 0x14),
        steel: Color32::from_rgb(0x17, 0x17, 0x1f),
        steel_hi: Color32::from_rgb(0x22, 0x22, 0x2d),
        line: Color32::from_rgb(0x1f, 0x1f, 0x29),
        line_strong: Color32::from_rgb(0x34, 0x34, 0x42),
        text: Color32::from_rgb(0xf3, 0xf1, 0xe7),
        text_dim: Color32::from_rgb(0xb4, 0xb7, 0xc4),
        text_faint: Color32::from_rgb(0x6c, 0x6c, 0x7d),
        accent: HOT,
        accent_soft: Color32::from_rgb(0x3a, 0x10, 0x1a),
        active: RED,
        on_accent: Color32::WHITE,
        grid: Color32::from_rgb(0x12, 0x12, 0x19),
    };

    /// The same identity on paper.
    pub const LIGHT: Palette = Palette {
        dark: false,
        void: Color32::from_rgb(0xe6, 0xe4, 0xda),
        ink: Color32::from_rgb(0xf3, 0xf1, 0xe7),
        steel: Color32::from_rgb(0xfb, 0xfa, 0xf4),
        steel_hi: Color32::from_rgb(0xdc, 0xd9, 0xcc),
        line: Color32::from_rgb(0xd6, 0xd3, 0xc6),
        line_strong: Color32::from_rgb(0xb0, 0xad, 0xa0),
        text: Color32::from_rgb(0x12, 0x12, 0x15),
        text_dim: Color32::from_rgb(0x3c, 0x3d, 0x48),
        text_faint: Color32::from_rgb(0x6e, 0x70, 0x7c),
        accent: RED,
        accent_soft: Color32::from_rgb(0xf6, 0xd3, 0xd9),
        active: RED,
        on_accent: Color32::WHITE,
        grid: Color32::from_rgb(0xdd, 0xda, 0xce),
    };

    pub fn for_dark(dark: bool) -> Palette {
        if dark { Palette::DARK } else { Palette::LIGHT }
    }

    /// Keep Edit's neutral surfaces while using the host application's icon colour.
    pub fn with_accent(mut self, accent: Color32) -> Self {
        self.accent = accent;
        self.active = accent;
        self.accent_soft = self
            .ink
            .lerp_to_gamma(accent, if self.dark { 0.22 } else { 0.16 });
        // Bright accents (orange and neutral white) need dark selected text.
        let linear = |channel: u8| {
            let value = f32::from(channel) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        let luma =
            0.2126 * linear(accent.r()) + 0.7152 * linear(accent.g()) + 0.0722 * linear(accent.b());
        self.on_accent = if luma > 0.179 {
            Color32::BLACK
        } else {
            Color32::WHITE
        };
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG relative luminance.
    fn lum(c: Color32) -> f64 {
        let f = |v: u8| {
            let v = f64::from(v) / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f64 {
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn application_accents_have_readable_selected_text() {
        for dark in [false, true] {
            for accent in [
                RED,
                Color32::from_rgb(255, 106, 61),
                Color32::from_rgb(138, 63, 252),
                Color32::from_rgb(31, 111, 235),
                Color32::from_rgb(14, 138, 111),
                Color32::from_rgb(109, 74, 255),
                Color32::from_rgb(242, 241, 238),
            ] {
                let palette = Palette::for_dark(dark).with_accent(accent);
                assert!(contrast(palette.on_accent, accent) >= 4.5, "{accent:?}");
            }
        }
    }

    #[test]
    fn text_reads_on_every_surface() {
        for p in [Palette::DARK, Palette::LIGHT] {
            for bg in [p.void, p.ink, p.steel] {
                assert!(contrast(p.text, bg) >= 7.0, "{p:?}");
                assert!(contrast(p.text_dim, bg) >= 4.5, "{p:?}");
                assert!(contrast(p.accent, bg) >= 3.0, "accent on {bg:?}");
                assert!(contrast(p.active, bg) >= 3.0, "active on {bg:?}");
            }
            assert!(contrast(p.on_accent, p.accent) >= 3.0);
        }
    }
}
