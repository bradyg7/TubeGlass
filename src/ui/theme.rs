//! Colours, accent presets and the egui visuals for the dark and light themes.

use crate::*;

// ---- palette -------------------------------------------------------------------------
// pop-ups and menus
/// True while the light theme is active.
pub(crate) static LIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The video page stays dark in the light theme (it sits on black).
pub(crate) static FORCE_DARK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// True when dark colours are in use (the video page always is).
pub fn dark_mode() -> bool {
    use std::sync::atomic::Ordering::Relaxed;
    FORCE_DARK.load(Relaxed) || !LIGHT.load(Relaxed)
}

/// Chooses the dark or the light variant of a colour.
pub(crate) fn pick(dark: (u8, u8, u8), light: (u8, u8, u8)) -> Color32 {
    let c = if dark_mode() { dark } else { light };
    Color32::from_rgb(c.0, c.1, c.2)
}

/// Page background.
pub(crate) fn bg() -> Color32 { pick((13, 20, 23), (244, 246, 247)) }

// pop-ups and menus
/// Pop-ups and menus.
pub(crate) fn panel() -> Color32 { pick((20, 29, 32), (255, 255, 255)) }

/// Cards and inputs.
pub(crate) fn card() -> Color32 { pick((27, 38, 42), (232, 237, 239)) }

/// Hovered cards.
pub(crate) fn card_hi() -> Color32 { pick((36, 51, 57), (217, 225, 228)) }

/// Main text colour.
pub(crate) fn ink() -> Color32 { pick((238, 241, 239), (22, 30, 34)) }

/// Secondary text colour.
pub(crate) fn muted() -> Color32 { pick((127, 148, 153), (92, 108, 114)) }

/// Subtle overlay (hover highlights, hairlines): light on dark, dark on light.
pub fn ov(a: u8) -> Color32 {
    if dark_mode() { Color32::from_white_alpha(a) } else { Color32::from_black_alpha((a as f32 * 0.55) as u8) }
}

// warm orange accent, taken from the colour reference
/// Accent colour presets (primary, secondary). The first one comes from the colour reference.
pub(crate) const ACCENTS: [(&str, Color32, Color32); 5] = [
    ("Orange", Color32::from_rgb(255, 154, 31), Color32::from_rgb(242, 97, 29)),
    ("Teal", Color32::from_rgb(38, 190, 180), Color32::from_rgb(20, 140, 160)),
    ("Violet", Color32::from_rgb(150, 110, 250), Color32::from_rgb(120, 80, 230)),
    ("Rose", Color32::from_rgb(244, 90, 140), Color32::from_rgb(220, 60, 110)),
    ("Lime", Color32::from_rgb(160, 210, 60), Color32::from_rgb(110, 180, 50)),
];

/// Chosen accent preset (the number of presets means "custom").
pub(crate) static ACCENT_IDX: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// RGB of the custom accent (used when the accent index is `ACCENTS.len()`).
pub(crate) static ACCENT_CUSTOM: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0xFF9A1F);

/// The custom accent colour.
pub(crate) fn custom_rgb() -> (u8, u8, u8) {
    let v = ACCENT_CUSTOM.load(std::sync::atomic::Ordering::Relaxed);
    ((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// Primary accent colour.
pub(crate) fn accent() -> Color32 {
    let i = ACCENT_IDX.load(std::sync::atomic::Ordering::Relaxed);
    if i >= ACCENTS.len() {
        let (r, g, b) = custom_rgb();
        return Color32::from_rgb(r, g, b);
    }
    ACCENTS[i].1
}

/// Darker partner of the accent (gradients).
pub(crate) fn accent2() -> Color32 {
    let i = ACCENT_IDX.load(std::sync::atomic::Ordering::Relaxed);
    if i >= ACCENTS.len() {
        let (r, g, b) = custom_rgb();
        let d = |c: u8| (c as f32 * 0.78) as u8;
        return Color32::from_rgb(d(r), d(g), d(b));
    }
    ACCENTS[i].2
}

/// Text colour on top of the accent.
pub(crate) const ON_ACCENT: Color32 = Color32::from_rgb(26, 20, 12);

/// Colour for "saved" states.
pub(crate) const TEAL: Color32 = Color32::from_rgb(38, 166, 160);

/// Colour for live badges and destructive buttons.
pub(crate) const LIVE: Color32 = Color32::from_rgb(240, 62, 62);

/// egui visuals for the palette that is active right now.
pub(crate) fn build_visuals() -> egui::Visuals {
    let base = if dark_mode() { egui::Visuals::dark() } else { egui::Visuals::light() };
    let mut v = egui::Visuals {
        panel_fill: bg(),
        window_fill: panel(),
        window_stroke: Stroke::new(1.0, ov(46)),
        window_corner_radius: CornerRadius::same(18),
        menu_corner_radius: CornerRadius::same(14),
        window_shadow: egui::Shadow { offset: [0, 14], blur: 40, spread: 0, color: Color32::from_black_alpha(if dark_mode() { 120 } else { 50 }) },
        popup_shadow: egui::Shadow { offset: [0, 10], blur: 30, spread: 0, color: Color32::from_black_alpha(if dark_mode() { 110 } else { 45 }) },
        extreme_bg_color: card(),
        override_text_color: Some(ink()),
        selection: egui::style::Selection {
            bg_fill: accent().gamma_multiply(0.5),
            stroke: Stroke::new(1.0, accent()),
        },
        ..base
    };
    v.interact_cursor = Some(egui::CursorIcon::PointingHand);
    v.widgets.inactive.corner_radius = CornerRadius::same(14);
    v.widgets.hovered.corner_radius = CornerRadius::same(14);
    v.widgets.active.corner_radius = CornerRadius::same(14);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, ov(26));
    v.widgets.active.bg_stroke = Stroke::new(1.0, ov(70));
    v.widgets.inactive.bg_fill = card();
    v.widgets.inactive.weak_bg_fill = card();
    v.widgets.hovered.weak_bg_fill = card_hi();
    v.widgets.hovered.bg_fill = card_hi();
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, accent());
    v
}

/// Installs the visuals and spacing for the current theme.
pub(crate) fn style(ctx: &egui::Context) {
    // whichever theme egui considers active gets the same, already themed, visuals
    ctx.set_visuals_of(egui::Theme::Dark, build_visuals());
    ctx.set_visuals_of(egui::Theme::Light, build_visuals());
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = vec2(10.0, 10.0);
    });
}

/// Fills a rectangle with a top-to-bottom gradient.
pub(crate) fn vgradient(p: &egui::Painter, r: Rect, top: Color32, bottom: Color32) {
    let mut m = egui::Mesh::default();
    let (a, b, c, d) = (r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom());
    for (pt, col) in [(a, top), (b, top), (c, bottom), (d, bottom)] {
        m.colored_vertex(pt, col);
    }
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    p.add(egui::Shape::mesh(m));
}

/// Fills a rectangle with a left-to-right gradient.
pub(crate) fn hgradient(p: &egui::Painter, r: Rect, l: Color32, rr: Color32) {
    let mut m = egui::Mesh::default();
    for (pt, col) in [
        (r.left_top(), l),
        (r.right_top(), rr),
        (r.right_bottom(), rr),
        (r.left_bottom(), l),
    ] {
        m.colored_vertex(pt, col);
    }
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    p.add(egui::Shape::mesh(m));
}
