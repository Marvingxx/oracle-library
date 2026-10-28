//! The typefaces, built in (SIL OFL 1.1, licences beside them in `assets/fonts`):
//! Space Grotesk for the interface, JetBrains Mono for labels and numbers.

use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily, FontId};

pub const SANS_REGULAR: &[u8] = include_bytes!("../assets/fonts/SpaceGrotesk-Regular.ttf");
pub const SANS_MEDIUM: &[u8] = include_bytes!("../assets/fonts/SpaceGrotesk-Medium.ttf");
pub const SANS_BOLD: &[u8] = include_bytes!("../assets/fonts/SpaceGrotesk-Bold.ttf");
pub const MONO_REGULAR: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");
pub const MONO_BOLD: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf");

/// Font names as registered in egui's definitions.
pub const SANS: &str = "oracle-sans";
pub const SANS_MEDIUM_NAME: &str = "oracle-sans-medium";
pub const SANS_BOLD_NAME: &str = "oracle-sans-bold";
pub const MONO: &str = "oracle-mono";
pub const MONO_BOLD_NAME: &str = "oracle-mono-bold";

/// The family of labels (JetBrains Mono Bold, capitals): see [`crate::hud::label`].
pub const LABEL_FAMILY: &str = "oracle-label";
/// Space Grotesk Medium, Bold.
pub const MEDIUM_FAMILY: &str = "oracle-medium";
pub const BOLD_FAMILY: &str = "oracle-bold";

/// Space Grotesk is drawn a touch larger than Inter at the same size: this keeps panels laid out
/// for Inter in place.
pub const SANS_SCALE: f32 = 0.96;

fn data(bytes: &'static [u8], scale: f32) -> Arc<FontData> {
    let mut d = FontData::from_static(bytes);
    d.tweak.scale = scale;
    Arc::new(d)
}

/// Add Oracle's faces to `defs`: Space Grotesk in front of the proportional stack, JetBrains Mono
/// in front of the monospace one, and the named families ([`add_named`]). What was there stays
/// behind, for the glyphs these faces lack.
pub fn add(defs: &mut FontDefinitions) {
    add_named(defs);
    defs.families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, SANS.into());
    defs.families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, MONO.into());
}

/// Register Oracle's faces and the named families [`LABEL_FAMILY`], [`MEDIUM_FAMILY`] and
/// [`BOLD_FAMILY`], leaving the proportional and monospace stacks as they are: an app whose
/// interface font is another one still draws labels ([`label`]) — egui panics on a family with
/// no font, so call this whenever Oracle's look may draw.
pub fn add_named(defs: &mut FontDefinitions) {
    defs.font_data
        .insert(SANS.into(), data(SANS_REGULAR, SANS_SCALE));
    defs.font_data
        .insert(SANS_MEDIUM_NAME.into(), data(SANS_MEDIUM, SANS_SCALE));
    defs.font_data
        .insert(SANS_BOLD_NAME.into(), data(SANS_BOLD, SANS_SCALE));
    defs.font_data.insert(MONO.into(), data(MONO_REGULAR, 1.0));
    defs.font_data
        .insert(MONO_BOLD_NAME.into(), data(MONO_BOLD, 1.0));
    let fallback: Vec<String> = defs
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for (family, face) in [
        (LABEL_FAMILY, MONO_BOLD_NAME),
        (MEDIUM_FAMILY, SANS_MEDIUM_NAME),
        (BOLD_FAMILY, SANS_BOLD_NAME),
    ] {
        let mut stack = vec![face.to_string()];
        stack.extend(fallback.iter().filter(|f| f.as_str() != face).cloned());
        defs.families.insert(FontFamily::Name(family.into()), stack);
    }
}

pub fn label(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(LABEL_FAMILY.into()))
}

pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MEDIUM_FAMILY.into()))
}

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BOLD_FAMILY.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_faces_are_registered_in_front() {
        let mut defs = FontDefinitions::default();
        add(&mut defs);
        assert_eq!(defs.families[&FontFamily::Proportional][0], SANS);
        assert_eq!(defs.families[&FontFamily::Monospace][0], MONO);
        assert_eq!(
            defs.families[&FontFamily::Name(LABEL_FAMILY.into())][0],
            MONO_BOLD_NAME
        );
        for bytes in [
            SANS_REGULAR,
            SANS_MEDIUM,
            SANS_BOLD,
            MONO_REGULAR,
            MONO_BOLD,
        ] {
            assert!(
                bytes.len() > 50_000 && &bytes[..4] == b"\0\x01\0\0",
                "a TrueType font"
            );
        }
    }

    #[test]
    fn text_lays_out_in_the_faces() {
        let ctx = egui::Context::default();
        let mut defs = FontDefinitions::default();
        add(&mut defs);
        ctx.set_fonts(defs);
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .textures_delta
            .clear();
        ctx.fonts_mut(|f| {
            assert_eq!(f.definitions().families[&FontFamily::Proportional][0], SANS);
            let width = |f: &mut egui::epaint::FontsView<'_>, font: FontId| {
                f.layout_no_wrap("iiii".into(), font, egui::Color32::WHITE)
                    .size()
                    .x
            };
            // JetBrains Mono's i is as wide as any letter, Space Grotesk's is narrow.
            let (sans, mono, label) = (
                width(f, FontId::proportional(13.0)),
                width(f, FontId::monospace(13.0)),
                width(f, label(13.0)),
            );
            assert!(
                sans > 5.0 && mono > sans * 1.5 && label > sans * 1.5,
                "{sans} {mono} {label}"
            );
            assert!(width(f, bold(13.0)) >= sans && width(f, medium(13.0)) >= sans);
        });
    }
}

/// Match Edit's configured Lexend face while preserving all script and symbol fallbacks.
/// Only Latin UI families change; document, monospace and RTL-specific families stay intact.
pub fn add_lexend(defs: &mut FontDefinitions) {
    let bytes = include_bytes!("../assets/fonts/Lexend-Variable.ttf");
    for (name, weight, family) in [
        ("oracle-lexend", 400.0, FontFamily::Proportional),
        (
            "oracle-lexend-medium",
            500.0,
            FontFamily::Name("medium".into()),
        ),
        (
            "oracle-lexend-semibold",
            600.0,
            FontFamily::Name("semibold".into()),
        ),
        (
            "oracle-lexend-display",
            600.0,
            FontFamily::Name("display".into()),
        ),
    ] {
        let tweak = egui::epaint::text::FontTweak {
            scale: 0.88,
            coords: egui::epaint::text::VariationCoords::new([(*b"wght", weight)]),
            ..Default::default()
        };
        defs.font_data.insert(
            name.into(),
            Arc::new(FontData::from_static(bytes).tweak(tweak)),
        );
        let stack = defs.families.entry(family).or_default();
        stack.retain(|face| face != name);
        stack.insert(0, name.into());
    }
}
