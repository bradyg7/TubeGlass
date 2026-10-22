//! Vector UI icons drawn with the painter (crisp at any scale, no font glyph lookups)
//! and the generated application icon.

use eframe::egui::{Color32, Painter, Pos2, Shape, Stroke, vec2};

/// Points along a circular arc (for drawing curved icons).
fn arc(cx: f32, cy: f32, r: f32, a0: f32, a1: f32, n: usize) -> Vec<(f32, f32)> {
    (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

/// Draws icon `name` centred on `c`; `size` is the nominal edge length in points.
pub fn draw(p: &Painter, c: Pos2, name: &str, size: f32, col: Color32) {
    let k = size / 24.0;
    let st = Stroke::new((1.9 * k).max(1.2), col);
    let pt = |x: f32, y: f32| c + vec2(x * k, y * k);
    let line = |pts: &[(f32, f32)]| {
        let v: Vec<Pos2> = pts.iter().map(|(x, y)| pt(*x, *y)).collect();
        p.add(Shape::line(v, st));
    };
    let dot = |x: f32, y: f32, r: f32| {
        p.circle_filled(pt(x, y), r * k, col);
    };
    match name {
        "home" => {
            line(&[(-10.0, 0.5), (0.0, -9.0), (10.0, 0.5)]);
            line(&[(-7.0, -2.5), (-7.0, 9.0), (-2.5, 9.0), (-2.5, 3.0), (2.5, 3.0), (2.5, 9.0), (7.0, 9.0), (7.0, -2.5)]);
        }
        "live" => {
            dot(0.0, 0.0, 2.6);
            for r in [6.5f32, 10.5] {
                line(&arc(0.0, 0.0, r, -0.75, 0.75, 10));
                line(&arc(0.0, 0.0, r, std::f32::consts::PI - 0.75, std::f32::consts::PI + 0.75, 10));
            }
        }
        "download" => {
            line(&[(0.0, -9.5), (0.0, 4.5)]);
            line(&[(-5.5, -1.0), (0.0, 4.5), (5.5, -1.0)]);
            line(&[(-9.0, 8.0), (-9.0, 10.5), (9.0, 10.5), (9.0, 8.0)]);
        }
        "open" => {
            line(&[(-10.0, -6.0), (-4.0, -6.0), (-1.5, -3.0), (10.0, -3.0), (10.0, 8.5), (-10.0, 8.5), (-10.0, -6.0)]);
            line(&[(0.0, 6.0), (0.0, 0.0)]);
            line(&[(-2.8, 2.6), (0.0, -0.2), (2.8, 2.6)]);
        }
        "heart" => {
            let pts: Vec<(f32, f32)> = (0..=48)
                .map(|i| {
                    let t = std::f32::consts::TAU * i as f32 / 48.0;
                    let x = 16.0 * t.sin().powi(3);
                    let y = -(13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos());
                    (x * 0.62, y * 0.62 + 0.5)
                })
                .collect();
            line(&pts);
        }
        "list" => {
            for (i, y) in [-6.0f32, 0.0, 6.0].iter().enumerate() {
                dot(-8.0, *y, 1.4);
                line(&[(-4.0, *y), (if i == 2 { 3.0 } else { 9.0 }, *y)]);
            }
            // little play marker on the last row
            line(&[(6.0, 3.5), (6.0, 8.5), (10.0, 6.0), (6.0, 3.5)]);
        }
        "plus" => {
            line(&[(-7.0, 0.0), (7.0, 0.0)]);
            line(&[(0.0, -7.0), (0.0, 7.0)]);
        }
        "collapse" => {
            line(&[(2.0, -7.0), (-4.0, 0.0), (2.0, 7.0)]);
            line(&[(9.0, -7.0), (3.0, 0.0), (9.0, 7.0)]);
        }
        "expand" => {
            line(&[(-2.0, -7.0), (4.0, 0.0), (-2.0, 7.0)]);
            line(&[(-9.0, -7.0), (-3.0, 0.0), (-9.0, 7.0)]);
        }
        "search" => {
            line(&arc(-2.0, -2.0, 7.0, 0.0, std::f32::consts::TAU, 28));
            line(&[(3.2, 3.2), (9.5, 9.5)]);
        }
        "close" => {
            line(&[(-6.0, -6.0), (6.0, 6.0)]);
            line(&[(-6.0, 6.0), (6.0, -6.0)]);
        }
        "pause" => {
            line(&[(-4.5, -8.0), (-4.5, 8.0)]);
            line(&[(4.5, -8.0), (4.5, 8.0)]);
        }
        "play" => line(&[(-5.0, -8.5), (-5.0, 8.5), (8.5, 0.0), (-5.0, -8.5)]),
        "history" => {
            line(&arc(1.0, 0.0, 9.0, -2.55, 4.1, 30));
            line(&[(1.0, -5.0), (1.0, 0.5), (5.0, 3.0)]);
            line(&[(-9.5, -5.5), (-8.2, -1.0), (-4.0, -2.8)]);
        }
        "more" => {
            for x in [-8.0f32, 0.0, 8.0] {
                dot(x, 0.0, 1.9);
            }
        }
        "settings" => {
            line(&arc(0.0, 0.0, 3.4, 0.0, std::f32::consts::TAU, 20));
            for i in 0..8 {
                let a = std::f32::consts::TAU * i as f32 / 8.0;
                line(&[(a.cos() * 6.4, a.sin() * 6.4), (a.cos() * 9.6, a.sin() * 9.6)]);
            }
            line(&arc(0.0, 0.0, 6.4, 0.0, std::f32::consts::TAU, 28));
        }
        "skip_next" => {
            line(&[(-6.0, -7.5), (4.0, 0.0), (-6.0, 7.5), (-6.0, -7.5)]);
            line(&[(8.0, -7.5), (8.0, 7.5)]);
        }
        "skip_prev" => {
            line(&[(6.0, -7.5), (-4.0, 0.0), (6.0, 7.5), (6.0, -7.5)]);
            line(&[(-8.0, -7.5), (-8.0, 7.5)]);
        }
        "mic" => {
            line(&[(-3.0, -9.0), (3.0, -9.0), (3.0, 1.0), (-3.0, 1.0), (-3.0, -9.0)]);
            line(&arc(0.0, 1.0, 6.5, 0.0, 3.1416, 14));
            line(&[(0.0, 7.5), (0.0, 10.0)]);
            line(&[(-4.0, 10.0), (4.0, 10.0)]);
        }
        "refresh" => {
            // almost a full circle with an arrow head at the open end
            line(&arc(0.0, 0.0, 7.0, 0.9, 5.6, 22));
            let (ex, ey) = (7.0 * 5.6f32.cos(), 7.0 * 5.6f32.sin());
            line(&[(ex - 1.0, ey - 5.0), (ex, ey), (ex + 5.0, ey - 1.0)]);
        }
        "scissors" => {
            dot(-6.0, 6.0, 2.6);
            dot(-6.0, -6.0, 2.6);
            line(&[(-4.0, 4.5), (9.0, -6.5)]);
            line(&[(-4.0, -4.5), (9.0, 6.5)]);
        }
        "chart" => {
            line(&[(-9.0, 9.0), (9.0, 9.0)]);
            line(&[(-6.0, 9.0), (-6.0, 1.0)]);
            line(&[(0.0, 9.0), (0.0, -7.0)]);
            line(&[(6.0, 9.0), (6.0, -2.0)]);
        }
        "view_grid" => {
            for (x, y) in [(-8.5f32, -8.5f32), (1.5, -8.5), (-8.5, 1.5), (1.5, 1.5)] {
                line(&[(x, y), (x + 7.0, y), (x + 7.0, y + 7.0), (x, y + 7.0), (x, y)]);
            }
        }
        "view_compact" => {
            for x in [-7.0f32, 0.0, 7.0] {
                for y in [-7.0f32, 0.0, 7.0] {
                    dot(x, y, 2.4);
                }
            }
        }
        "view_list" => {
            for y in [-6.5f32, 0.0, 6.5] {
                dot(-8.0, y, 1.6);
                line(&[(-3.5, y), (9.0, y)]);
            }
        }
        "queue" => {
            line(&[(-9.0, -6.0), (4.0, -6.0)]);
            line(&[(-9.0, 0.0), (4.0, 0.0)]);
            line(&[(-9.0, 6.0), (-1.0, 6.0)]);
            line(&[(6.0, 2.0), (6.0, 9.0), (11.0, 5.5), (6.0, 2.0)]);
        }
        "cc" => {
            line(&[(-9.5, -6.5), (9.5, -6.5), (9.5, 6.5), (-9.5, 6.5), (-9.5, -6.5)]);
            line(&arc(-3.2, 0.0, 3.0, 0.8, 5.5, 10));
            line(&arc(4.2, 0.0, 3.0, 0.8, 5.5, 10));
        }
        "repeat" => {
            line(&[(-8.0, -1.0), (-8.0, -4.0), (-5.0, -6.5), (7.0, -6.5)]);
            line(&[(4.0, -9.5), (7.5, -6.5), (4.0, -3.5)]);
            line(&[(8.0, 1.0), (8.0, 4.0), (5.0, 6.5), (-7.0, 6.5)]);
            line(&[(-4.0, 9.5), (-7.5, 6.5), (-4.0, 3.5)]);
        }
        "shuffle" => {
            line(&[(-9.0, -5.5), (-3.0, -5.5), (3.0, 5.5), (9.0, 5.5)]);
            line(&[(-9.0, 5.5), (-3.0, 5.5), (-1.0, 2.5)]);
            line(&[(1.0, -2.5), (3.0, -5.5), (9.0, -5.5)]);
            line(&[(6.0, 2.5), (9.5, 5.5), (6.0, 8.5)]);
            line(&[(6.0, -8.5), (9.5, -5.5), (6.0, -2.5)]);
        }
        "moon" => {
            line(&arc(0.0, 0.0, 8.5, 0.9, 5.4, 22));
            line(&arc(4.5, -3.5, 6.5, 2.6, 5.9, 14));
        }
        "bell" => {
            line(&[(-7.0, 5.0), (-7.0, -1.0), (-5.0, -6.0), (0.0, -8.0), (5.0, -6.0), (7.0, -1.0), (7.0, 5.0), (-7.0, 5.0)]);
            line(&[(-2.5, 8.5), (2.5, 8.5)]);
        }
        "pin" => {
            line(&[(-4.0, -8.0), (4.0, -8.0), (3.0, -1.0), (7.0, 2.0), (-7.0, 2.0), (-3.0, -1.0), (-4.0, -8.0)]);
            line(&[(0.0, 2.0), (0.0, 9.5)]);
        }
        "folder" => {
            line(&[(-10.0, -6.0), (-4.0, -6.0), (-1.5, -3.0), (10.0, -3.0), (10.0, 8.0), (-10.0, 8.0), (-10.0, -6.0)]);
        }
        "clock" => {
            line(&arc(0.0, 0.0, 9.0, 0.0, std::f32::consts::TAU, 28));
            line(&[(0.0, -5.0), (0.0, 0.0), (4.0, 2.5)]);
        }
        "shorts" => {
            line(&[(-5.0, -10.0), (5.0, -10.0), (5.0, 10.0), (-5.0, 10.0), (-5.0, -10.0)]);
            line(&[(-1.5, -3.5), (3.0, 0.0), (-1.5, 3.5), (-1.5, -3.5)]);
        }
        "check" => line(&[(-8.0, 0.5), (-2.5, 6.0), (8.0, -6.0)]),
        "chevron_up" => line(&[(-6.0, 3.0), (0.0, -3.0), (6.0, 3.0)]),
        "thumb_up" => {
            line(&[(-9.5, -1.0), (-9.5, 9.5), (-5.0, 9.5), (-5.0, -1.0), (-9.5, -1.0)]);
            line(&[
                (-5.0, -1.0), (-1.0, -9.5), (2.0, -9.5), (3.0, -6.5), (2.0, -2.5), (8.0, -2.5), (9.5, -0.5),
                (8.5, 7.5), (7.5, 9.5), (-5.0, 9.5),
            ]);
        }
        "reply" => {
            line(&[(-3.5, -7.0), (-9.5, -1.0), (-3.5, 5.0)]);
            line(&[(-9.5, -1.0), (1.0, -1.0), (6.0, 2.0), (9.0, 8.0)]);
        }
        "chevron_down" => line(&[(-6.0, -3.0), (0.0, 3.0), (6.0, -3.0)]),
        "arrow_left" => {
            line(&[(10.0, 0.0), (-9.0, 0.0)]);
            line(&[(-3.5, -6.5), (-9.5, 0.0), (-3.5, 6.5)]);
        }
        "note" => {
            let head: Vec<Pos2> = arc(-3.5, 6.0, 3.6, 0.0, std::f32::consts::TAU, 20)
                .iter()
                .map(|(x, y)| pt(*x, *y))
                .collect();
            p.add(Shape::convex_polygon(head, col, Stroke::NONE));
            line(&[(0.1, 6.0), (0.1, -9.0)]);
            line(&[(0.1, -9.0), (4.0, -7.5), (7.0, -4.5), (7.0, -1.0)]);
        }
        "heart_fill" => {
            // star-shaped around the centre, so a triangle fan fills it
            let ring: Vec<Pos2> = (0..=48)
                .map(|i| {
                    let t = std::f32::consts::TAU * i as f32 / 48.0;
                    let x = 16.0 * t.sin().powi(3);
                    let y = -(13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos());
                    pt(x * 0.62, y * 0.62 + 0.5)
                })
                .collect();
            let mut mesh = eframe::egui::Mesh::default();
            mesh.colored_vertex(pt(0.0, 0.5), col);
            for q in &ring {
                mesh.colored_vertex(*q, col);
            }
            for i in 1..ring.len() {
                mesh.add_triangle(0, i as u32, i as u32 + 1);
            }
            p.add(Shape::mesh(mesh));
        }
        "playlist_add" => {
            line(&[(-9.0, -6.0), (3.0, -6.0)]);
            line(&[(-9.0, 0.0), (3.0, 0.0)]);
            line(&[(-9.0, 6.0), (0.0, 6.0)]);
            line(&[(6.5, 1.0), (6.5, 9.0)]);
            line(&[(2.5, 5.0), (10.5, 5.0)]);
        }
        "comment" => {
            line(&[
                (-9.0, -7.0), (9.0, -7.0), (9.0, 4.5), (2.0, 4.5), (-3.0, 9.0), (-3.0, 4.5), (-9.0, 4.5), (-9.0, -7.0),
            ]);
        }
        "speaker" | "speaker_mute" => {
            line(&[(-9.0, -3.5), (-4.5, -3.5), (1.0, -8.0), (1.0, 8.0), (-4.5, 3.5), (-9.0, 3.5), (-9.0, -3.5)]);
            if name == "speaker" {
                line(&arc(1.0, 0.0, 5.0, -0.8, 0.8, 8));
                line(&arc(1.0, 0.0, 9.0, -0.75, 0.75, 10));
            } else {
                line(&[(5.0, -4.0), (11.0, 4.0)]);
                line(&[(5.0, 4.0), (11.0, -4.0)]);
            }
        }
        "fullscreen" => {
            for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                line(&[(sx * 4.0, sy * 9.5), (sx * 9.5, sy * 9.5), (sx * 9.5, sy * 4.0)]);
            }
        }
        "fullscreen_exit" => {
            for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                line(&[(sx * 9.5, sy * 4.5), (sx * 4.5, sy * 4.5), (sx * 4.5, sy * 9.5)]);
            }
        }
        "pip_expand" => {
            line(&[(1.0, -1.0), (8.5, -8.5)]);
            line(&[(3.0, -8.5), (8.5, -8.5), (8.5, -3.0)]);
            line(&[(-1.0, 1.0), (-8.5, 8.5)]);
            line(&[(-3.0, 8.5), (-8.5, 8.5), (-8.5, 3.0)]);
        }
        _ => dot(0.0, 0.0, 3.0),
    }
}

// ---- application icon ---------------------------------------------------------------

/// Signed distance to a triangle, used to draw the app icon.
fn sd_triangle(p: (f32, f32), a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> f32 {
    let sub = |u: (f32, f32), v: (f32, f32)| (u.0 - v.0, u.1 - v.1);
    let dot = |u: (f32, f32), v: (f32, f32)| u.0 * v.0 + u.1 * v.1;
    let (e0, e1, e2) = (sub(b, a), sub(c, b), sub(a, c));
    let (v0, v1, v2) = (sub(p, a), sub(p, b), sub(p, c));
    let proj = |v: (f32, f32), e: (f32, f32)| {
        let t = (dot(v, e) / dot(e, e)).clamp(0.0, 1.0);
        (v.0 - e.0 * t, v.1 - e.1 * t)
    };
    let (q0, q1, q2) = (proj(v0, e0), proj(v1, e1), proj(v2, e2));
    let s = (e0.0 * e2.1 - e0.1 * e2.0).signum();
    let cand = [
        (dot(q0, q0), s * (v0.0 * e0.1 - v0.1 * e0.0)),
        (dot(q1, q1), s * (v1.0 * e1.1 - v1.1 * e1.0)),
        (dot(q2, q2), s * (v2.0 * e2.1 - v2.1 * e2.0)),
    ];
    let dx = cand.iter().map(|c| c.0).fold(f32::MAX, f32::min);
    let dy = cand.iter().map(|c| c.1).fold(f32::MAX, f32::min);
    -dx.sqrt() * dy.signum()
}

/// Smooth step between two edges.
fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Rounded-square glass tile with a violet→pink gradient and a soft rounded play triangle.
pub fn render_app_icon(size: usize) -> Vec<u8> {
    let mut out = vec![0u8; size * size * 4];
    let aa = 2usize;
    let px = 2.0 / size as f32;
    for y in 0..size {
        for x in 0..size {
            let (mut r, mut g, mut b, mut a) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
            for sy in 0..aa {
                for sx in 0..aa {
                    let u = ((x * aa + sx) as f32 + 0.5) / (size * aa) as f32 * 2.0 - 1.0;
                    let v = ((y * aa + sy) as f32 + 0.5) / (size * aa) as f32 * 2.0 - 1.0;
                    // tile mask
                    let half = 0.86 - 0.30;
                    let q = (u.abs() - half, v.abs() - half);
                    let d = (q.0.max(0.0).powi(2) + q.1.max(0.0).powi(2)).sqrt() + q.0.max(q.1).min(0.0) - 0.30;
                    let mask = 1.0 - smooth(-px * 0.5, px * 0.5, d);
                    if mask <= 0.0 {
                        continue;
                    }
                    // warm orange top, teal-charcoal body, soft red-brown glow in the corner
                    let t = ((u + v) * 0.5 * 0.5 + 0.5).clamp(0.0, 1.0);
                    let mix = |a: (f32, f32, f32), b: (f32, f32, f32), k: f32| {
                        (a.0 + (b.0 - a.0) * k, a.1 + (b.1 - a.1) * k, a.2 + (b.2 - a.2) * k)
                    };
                    let (o0, o1) = ((255.0, 176.0, 48.0), (242.0, 104.0, 26.0));
                    let (d0, d1) = ((36.0, 54.0, 60.0), (17.0, 25.0, 28.0));
                    let orange = mix(o0, o1, (t / 0.5).clamp(0.0, 1.0));
                    let teal = mix(d0, d1, ((t - 0.5) / 0.5).clamp(0.0, 1.0));
                    let k = smooth(0.46, 0.58, t);
                    let (mut cr, mut cg, mut cb) = mix(orange, teal, k);
                    let warm = smooth(0.78, 1.0, t) * 0.5;
                    cr += (120.0 - cr) * warm;
                    cg += (44.0 - cg) * warm;
                    cb += (30.0 - cb) * warm;
                    // glass highlight from the top-left, subtle darkening at the bottom
                    let hl = (1.0 - ((u + 0.45).hypot(v + 0.55) / 1.05)).clamp(0.0, 1.0);
                    let hl = smooth(0.0, 1.0, hl) * 0.30;
                    cr += (255.0 - cr) * hl;
                    cg += (255.0 - cg) * hl;
                    cb += (255.0 - cb) * hl;
                    let dark = smooth(0.1, 0.9, v * 0.5 + 0.5) * 0.18;
                    cr *= 1.0 - dark;
                    cg *= 1.0 - dark;
                    cb *= 1.0 - dark;
                    // play triangle, rounded, with a soft shadow beneath it
                    let (ta, tb, tc) = ((-0.20, -0.34), (-0.20, 0.34), (0.38, 0.0));
                    let rad = 0.075;
                    let tri = sd_triangle((u - 0.03, v), ta, tb, tc) - rad;
                    let shadow = sd_triangle((u - 0.03, v - 0.05), ta, tb, tc) - rad;
                    let sh = (1.0 - smooth(-0.0, 0.12, shadow)) * 0.28;
                    cr *= 1.0 - sh;
                    cg *= 1.0 - sh;
                    cb *= 1.0 - sh;
                    let fill = 1.0 - smooth(-px * 0.5, px * 0.5, tri);
                    cr += (255.0 - cr) * fill;
                    cg += (255.0 - cg) * fill;
                    cb += (255.0 - cb) * fill;
                    r += cr * mask;
                    g += cg * mask;
                    b += cb * mask;
                    a += mask;
                }
            }
            let n = (aa * aa) as f32;
            let i = (y * size + x) * 4;
            if a > 0.0 {
                out[i] = (r / a).clamp(0.0, 255.0) as u8;
                out[i + 1] = (g / a).clamp(0.0, 255.0) as u8;
                out[i + 2] = (b / a).clamp(0.0, 255.0) as u8;
                out[i + 3] = (a / n * 255.0).clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

/// Colour helper so callers can tint icons without importing egui types here.
pub fn muted_icon() -> Color32 {
    Color32::from_rgb(186, 186, 210)
}

#[cfg(test)]
mod tests {
    #[test]
    fn dump_icon() {
        if let Ok(path) = std::env::var("ICON_OUT") {
            // ICON_SIZE picks the edge length (default 256); the output is raw RGBA
            let size = std::env::var("ICON_SIZE").ok().and_then(|v| v.parse().ok()).unwrap_or(256);
            std::fs::write(path, super::render_app_icon(size)).unwrap();
        }
    }
}
