//! Ambient mode: a soft glow of the video's colours behind the player.

use crate::*;

impl App {
    /// Paint the soft colour glow behind the video. The sampled grid is eased towards the
    /// newest colours, uploaded as a tiny texture and stretched over the whole stage, so the
    /// GPU's bilinear filtering does the blurring.
    pub(crate) fn paint_ambient(&mut self, ui: &mut egui::Ui, vr: Rect) {
        let target = self.player.ambient();
        let n = player::AMBIENT_W * player::AMBIENT_H;
        if target.len() != n {
            return;
        }
        if self.amb_cur.len() != n {
            self.amb_cur = target.iter().map(|c| [c[0] as f32, c[1] as f32, c[2] as f32]).collect();
        }
        let dt = ui.ctx().input(|i| i.stable_dt).clamp(0.0, 0.1);
        let k = 1.0 - (-dt * 3.2).exp();
        let mut moving = false;
        for (cur, t) in self.amb_cur.iter_mut().zip(target) {
            for c in 0..3 {
                let d = t[c] as f32 - cur[c];
                if d.abs() > 0.4 {
                    moving = true;
                }
                cur[c] += d * k;
            }
        }
        let pixels: Vec<Color32> = self
            .amb_cur
            .iter()
            .map(|c| {
                // lift saturation a little and keep it from going muddy-dark
                let l = 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
                let boost = |v: f32| ((l + (v - l) * 1.55) * 1.18).clamp(0.0, 255.0);
                Color32::from_rgb(boost(c[0]) as u8, boost(c[1]) as u8, boost(c[2]) as u8)
            })
            .collect();
        let img = egui::ColorImage {
            size: [player::AMBIENT_W, player::AMBIENT_H],
            source_size: egui::vec2(player::AMBIENT_W as f32, player::AMBIENT_H as f32),
            pixels,
        };
        match &mut self.amb_tex {
            Some(t) => t.set(img, egui::TextureOptions::LINEAR),
            None => self.amb_tex = Some(ui.ctx().load_texture("ambient", img, egui::TextureOptions::LINEAR)),
        }
        if moving || self.player.state() == State::Playing {
            ui.ctx().request_repaint();
        }
        self.amb_vr = vr;
    }

    /// Paints the glow over the whole window. It runs at the start of the frame, before the
    /// panels, so the (translucent) sidebar and control bar sit on top of it.
    pub(crate) fn draw_ambient(&self, ctx: &egui::Context) {
        let Some(tex) = &self.amb_tex else { return };
        let vr = self.amb_vr;
        if vr.width() < 1.0 || vr.height() < 1.0 {
            return;
        }
        let win = ctx.content_rect();
        // the colour grid is mapped onto the video's rectangle and its edge colours are
        // stretched outwards, so each side of the window takes the colour nearest to it
        let uv = Rect::from_min_max(
            Pos2::new((win.min.x - vr.min.x) / vr.width(), (win.min.y - vr.min.y) / vr.height()),
            Pos2::new((win.max.x - vr.min.x) / vr.width(), (win.max.y - vr.min.y) / vr.height()),
        );
        let bg = ctx.layer_painter(egui::LayerId::background());
        bg.image(tex.id(), win, uv, Color32::WHITE);
        // keep it a glow rather than a flood
        bg.rect_filled(win, 0.0, Color32::from_black_alpha(70));
    }
}
