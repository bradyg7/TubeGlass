//! Flat surfaces for the UI: solid rounded panels with a hairline border, no blur or
//! gradients. Palette is deep teal-charcoal with a warm orange accent.

use eframe::egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke, StrokeKind};
use eframe::epaint::CornerRadius;


/// Corner radius as egui wants it.
fn cr(r: f32) -> CornerRadius {
    CornerRadius::same(r.clamp(0.0, 255.0).round() as u8)
}

/// A solid rounded surface. `opacity` fades it (player controls), `tint`, when given,
/// colours the border (used for hover / selection highlights).
pub fn panel(rect: Rect, r: f32, opacity: f32, tint: Option<Color32>, _shadow: bool) -> Shape {
    let o = opacity.clamp(0.0, 1.0);
    let c = crate::card();
    let fill = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (238.0 * o) as u8);
    let border = match tint {
        Some(t) => t.gamma_multiply((2.6 * o).min(1.0)),
        None => crate::ov((17.0 * o) as u8),
    };
    Shape::Vec(vec![
        Shape::rect_filled(rect, cr(r), fill),
        Shape::rect_stroke(rect, cr(r), Stroke::new(1.0, border), StrokeKind::Inside),
    ])
}

/// Fully rounded ends (chips, pills, bars).
pub fn capsule(rect: Rect, opacity: f32, tint: Option<Color32>, shadow: bool) -> Shape {
    panel(rect, rect.height() / 2.0, opacity, tint, shadow)
}

/// Solid colour fill with rounded corners.
pub fn filled(rect: Rect, r: f32, col: Color32) -> Shape {
    Shape::rect_filled(rect, cr(r), col)
}

/// Window backdrop: near-black teal that warms up very slightly towards the bottom.
pub fn background(p: &Painter, rect: Rect) {
    let (top, bottom) = if crate::dark_mode() {
        (Color32::from_rgb(13, 20, 23), Color32::from_rgb(22, 20, 21))
    } else {
        (Color32::from_rgb(244, 246, 247), Color32::from_rgb(236, 240, 241))
    };
    let mut m = Mesh::default();
    m.colored_vertex(rect.left_top(), top);
    m.colored_vertex(rect.right_top(), top);
    m.colored_vertex(rect.right_bottom(), bottom);
    m.colored_vertex(rect.left_bottom(), bottom);
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    p.add(Shape::mesh(m));
    let _ = Pos2::ZERO;
}
