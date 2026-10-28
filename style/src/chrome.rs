//! Edit's interface metrics, reused by the native suite applications.

use egui::{Context, CornerRadius, FontId, Margin, TextStyle, vec2};

/// Finish an application's theme without changing its accent or semantic colours.
/// Fixed themes must populate both egui branches, including on a light Windows desktop.
pub fn finish(ctx: &Context, fixed: bool) {
    let current = ctx.global_style();
    ctx.all_styles_mut(|s| {
        if fixed {
            *s = (*current).clone();
        }
        s.spacing.item_spacing = vec2(8.0, 6.0);
        s.spacing.button_padding = vec2(10.0, 4.0);
        s.spacing.interact_size = vec2(24.0, 24.0);
        s.spacing.slider_width = 150.0;
        s.spacing.combo_width = 120.0;
        s.spacing.menu_margin = Margin::same(6);
        s.spacing.window_margin = Margin::same(16);
        s.spacing.icon_width = 14.0;
        for (style, size) in [
            (TextStyle::Small, 10.5),
            (TextStyle::Body, 12.0),
            (TextStyle::Button, 12.0),
            (TextStyle::Heading, 15.0),
            (TextStyle::Monospace, 12.0),
        ] {
            s.text_styles
                .entry(style)
                .or_insert_with(|| FontId::proportional(size))
                .size = size;
        }
        s.visuals.window_corner_radius = CornerRadius::same(2);
        s.visuals.menu_corner_radius = CornerRadius::ZERO;
        s.visuals.slider_trailing_fill = true;
        s.visuals.handle_shape = egui::style::HandleShape::Circle;
        s.visuals.striped = false;
        // Contextual text lets selected buttons use the foreground of their selection.
        s.visuals.override_text_color = None;
        for w in [
            &mut s.visuals.widgets.noninteractive,
            &mut s.visuals.widgets.inactive,
            &mut s.visuals.widgets.hovered,
            &mut s.visuals.widgets.active,
            &mut s.visuals.widgets.open,
        ] {
            w.corner_radius = CornerRadius::ZERO;
            w.expansion = 0.0;
        }
        let alpha = if s.visuals.dark_mode { 180 } else { 60 };
        s.visuals.window_shadow = egui::epaint::Shadow {
            offset: [0, 10],
            blur: 32,
            spread: 0,
            color: egui::Color32::from_black_alpha(alpha),
        };
        s.visuals.popup_shadow = egui::epaint::Shadow {
            offset: [0, 6],
            blur: 20,
            spread: 0,
            color: egui::Color32::from_black_alpha(alpha),
        };
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_theme_keeps_its_selection_in_both_system_appearances() {
        let ctx = Context::default();
        let accent = egui::Color32::from_rgb(255, 106, 61);
        ctx.global_style_mut(|s| s.visuals.selection.bg_fill = accent);
        finish(&ctx, true);
        for theme in [egui::Theme::Light, egui::Theme::Dark] {
            let style = ctx.style_of(theme);
            assert_eq!(style.visuals.selection.bg_fill, accent);
            assert_eq!(
                style.visuals.widgets.active.corner_radius,
                CornerRadius::ZERO
            );
            assert_eq!(style.spacing.interact_size.y, 24.0);
        }
    }
}
