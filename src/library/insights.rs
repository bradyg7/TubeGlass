//! Local listening / watching statistics ("Insights") and the recommendation score built
//! from them. Everything stays on this computer.

use crate::*;
use std::collections::BTreeMap;

/// Local watching and listening statistics (never leaves this computer).
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Stats {
    /// "YYYY-MM-DD" -> (seconds of video, seconds of music)
    pub days: BTreeMap<String, (f64, f64)>,
    /// channel id -> (name, seconds of video)
    pub channels: HashMap<String, (String, f64)>,
    /// artist -> seconds of music
    pub artists: HashMap<String, f64>,
    /// video id -> (title, channel, seconds)
    pub titles: HashMap<String, (String, String, f64)>,
}

/// Where stats.json lives.
fn path() -> Option<std::path::PathBuf> {
    small_list_path("stats.json")
}

/// Reads the statistics, trimming them to the size limits.
pub fn load() -> Stats {
    let mut s: Stats = path().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    s.prune();
    s
}

/// Writes the statistics.
pub fn save(s: &Stats) {
    if let Some(p) = path() {
        let _ = std::fs::write(p, serde_json::to_vec(s).unwrap_or_default());
    }
}

/// How many entries of each kind are remembered, and for how many days.
const KEEP_ITEMS: usize = 300;
/// How many days of history are kept.
const KEEP_DAYS: usize = 800;

/// Local calendar day of a unix time.
pub fn day_key(t: u64) -> String {
    crate::platform::local_day_key(t)
}

/// Formats seconds as "1h 05m", "12m" or "<1m".
fn hours(secs: f64) -> String {
    if secs < 60.0 {
        return if secs < 5.0 { "0m".into() } else { "<1m".into() };
    }
    let m = (secs / 60.0).round() as u64;
    if m >= 60 { format!("{}h {:02}m", m / 60, m % 60) } else { format!("{m}m") }
}

/// Keeps the `keep` entries with the most time, dropping the long tail of one-off plays.
fn keep_top<V>(map: &mut HashMap<String, V>, keep: usize, secs: impl Fn(&V) -> f64) {
    if map.len() <= keep {
        return;
    }
    let mut all: Vec<(String, f64)> = map.iter().map(|(k, v)| (k.clone(), secs(v))).collect();
    all.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (k, _) in all.into_iter().skip(keep) {
        map.remove(&k);
    }
}

impl Stats {
    /// Stops the file and the lists from growing without limit as years of history pile up.
    pub fn prune(&mut self) {
        while self.days.len() > KEEP_DAYS {
            let Some(first) = self.days.keys().next().cloned() else { break };
            self.days.remove(&first);
        }
        keep_top(&mut self.channels, KEEP_ITEMS, |v| v.1);
        keep_top(&mut self.artists, KEEP_ITEMS, |v| *v);
        keep_top(&mut self.titles, KEEP_ITEMS, |v| v.2);
    }

    /// Adds `secs` of playback of `w` at time `when`.
    pub fn add(&mut self, w: &Video, secs: f64, when: u64) {
        let d = self.days.entry(day_key(when)).or_default();
        if w.music {
            d.1 += secs;
            if !w.channel_name.is_empty() {
                *self.artists.entry(w.channel_name.clone()).or_default() += secs;
            }
        } else {
            d.0 += secs;
            if !w.channel_id.is_empty() {
                self.channels.entry(w.channel_id.clone()).or_insert((w.channel_name.clone(), 0.0)).1 += secs;
            }
        }
        if !w.id.starts_with("local:") {
            self.titles.entry(w.id.clone()).or_insert((w.title.clone(), w.channel_name.clone(), 0.0)).2 += secs;
        }
    }

    /// Total (video, music) seconds over the last `days_back` days.
    fn range(&self, days_back: u64, now: u64) -> (f64, f64) {
        let mut out = (0.0, 0.0);
        for i in 0..days_back {
            if let Some(d) = self.days.get(&day_key(now.saturating_sub(i * 86_400))) {
                out.0 += d.0;
                out.1 += d.1;
            }
        }
        out
    }

    /// How much you like a channel: grows with the time spent, flattening out.
    pub fn affinity(&self, channel_id: &str, channel_name: &str) -> f32 {
        let secs = self.channels.get(channel_id).map(|c| c.1).unwrap_or(0.0)
            + self.artists.get(channel_name).copied().unwrap_or(0.0);
        (1.0 + secs / 900.0).ln() as f32
    }
}

impl App {
    /// Called every frame: adds the time that passed while something was playing.
    pub fn track_stats(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.stat_tick).as_secs_f64();
        self.stat_tick = now;
        if let Some(w) = self.watching.clone() {
            if self.player.state() == State::Playing && !self.player.paused && dt < 2.0 {
                self.stats.add(&w, dt, now_secs());
                self.stats_dirty = true;
            }
        }
        if self.stats_dirty && self.stat_saved.elapsed() > Duration::from_secs(45) {
            self.stats_dirty = false;
            self.stat_saved = Instant::now();
            self.stats.prune();
            save(&self.stats);
        }
    }

    /// Score used to order the "For you" feed: liked channels, fresh and unseen first.
    pub fn recommend_score(&self, v: &Video) -> f32 {
        let aff = self.stats.affinity(&v.channel_id, &v.channel_name);
        let fresh = 1.6 / (1.0 + v.rank as f32 * 0.18);
        let unseen = if self.seen.contains(&v.id) { 0.0 } else { 0.8 };
        let live = if v.kind == Kind::Live { 0.6 } else { 0.0 };
        let mut s = aff + fresh + unseen + live;
        if self.settings.chan_prefs.get(&v.channel_id).is_some_and(|p| p.fewer) {
            s *= 0.3;
        }
        s
    }

    /// Draws the Insights page.
    pub fn insights_view(&mut self, ui: &mut egui::Ui) {
        let now = now_secs();
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Insights").size(26.0).strong());
            ui.label(egui::RichText::new("kept on this computer only").size(12.0).color(muted()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(if self.stats_confirm { "Confirm reset" } else { "Reset insights" }).clicked() {
                    if self.stats_confirm {
                        self.stats = Stats::default();
                        save(&self.stats);
                        self.stats_confirm = false;
                    } else {
                        self.stats_confirm = true;
                    }
                }
            });
        });
        ui.add_space(8.0);
        let total: (f64, f64) = self.stats.days.values().fold((0.0, 0.0), |a, d| (a.0 + d.0, a.1 + d.1));
        if total.0 + total.1 < 30.0 {
            ui.add_space(60.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("Nothing to show yet.").size(18.0).color(muted()));
                ui.label(egui::RichText::new("Watch or listen to something and your totals, favourite channels and artists appear here.").color(muted()));
            });
            return;
        }
        let today = self.stats.range(1, now);
        let week = self.stats.range(7, now);
        let month = self.stats.range(30, now);
        let mut top_channels: Vec<(&String, f64)> = self.stats.channels.values().map(|(n, s)| (n, *s)).collect();
        top_channels.retain(|c| c.1 >= 30.0);
        top_channels.sort_by(|a, b| b.1.total_cmp(&a.1));
        let mut top_artists: Vec<(&String, f64)> = self.stats.artists.iter().map(|(n, s)| (n, *s)).collect();
        top_artists.retain(|c| c.1 >= 30.0);
        top_artists.sort_by(|a, b| b.1.total_cmp(&a.1));
        let mut top_titles: Vec<(&String, &String, f64)> = self.stats.titles.values().map(|(t, c, s)| (t, c, *s)).collect();
        top_titles.retain(|c| c.2 >= 30.0);
        top_titles.sort_by(|a, b| b.2.total_cmp(&a.2));
        let days: Vec<(String, (f64, f64))> = (0..14u64)
            .rev()
            .map(|i| {
                let k = day_key(now.saturating_sub(i * 86_400));
                let v = self.stats.days.get(&k).copied().unwrap_or((0.0, 0.0));
                (k, v)
            })
            .collect();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let w = ui.available_width() - 6.0;
            // summary cards: four across when there is room, otherwise two by two
            let per_row = if w >= 760.0 { 4 } else { 2 };
            let gap = 12.0;
            let cw = (w - gap * (per_row as f32 - 1.0)) / per_row as f32;
            let big = if cw < 210.0 { 22.0 } else { 26.0 };
            let cards = [("Today", today), ("Last 7 days", week), ("Last 30 days", month), ("All time", total)];
            for chunk in cards.chunks(per_row) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for (label, v) in chunk {
                        let (r, _) = ui.allocate_exact_size(vec2(cw, 92.0), Sense::hover());
                        ui.painter().add(surface::panel(r, 18.0, 1.0, None, false));
                        ui.painter().text(r.min + vec2(16.0, 14.0), Align2::LEFT_TOP, *label, FontId::proportional(12.5), muted());
                        ui.painter().text(r.min + vec2(16.0, 34.0), Align2::LEFT_TOP, hours(v.0 + v.1), FontId::proportional(big), ink());
                        let sub = truncate(ui, &format!("{} video  •  {} music", hours(v.0), hours(v.1)), FontId::proportional(12.0), cw - 32.0, 1);
                        ui.painter().text(r.min + vec2(16.0, 68.0), Align2::LEFT_TOP, sub, FontId::proportional(12.0), muted());
                    }
                });
                ui.add_space(gap);
            }
            ui.add_space(18.0);
            // last two weeks
            ui.label(egui::RichText::new("LAST 14 DAYS").size(11.0).color(muted()).strong());
            let (area, _) = ui.allocate_exact_size(vec2(w, 170.0), Sense::hover());
            ui.painter().add(surface::panel(area, 18.0, 1.0, None, false));
            let max = days.iter().map(|(_, v)| v.0 + v.1).fold(600.0f64, f64::max);
            let inner = area.shrink2(vec2(18.0, 16.0));
            let bar_w = inner.width() / days.len() as f32;
            for (i, (k, v)) in days.iter().enumerate() {
                let x = inner.min.x + i as f32 * bar_w;
                let full_h = inner.height() - 20.0;
                let (hv, hm) = ((v.0 / max) as f32 * full_h, (v.1 / max) as f32 * full_h);
                let base = inner.max.y - 20.0;
                let bw = bar_w * 0.56;
                let bx = x + (bar_w - bw) / 2.0;
                if hv > 0.5 {
                    ui.painter().rect_filled(Rect::from_min_max(Pos2::new(bx, base - hv), Pos2::new(bx + bw, base)), CornerRadius::same(4), accent());
                }
                if hm > 0.5 {
                    ui.painter().rect_filled(Rect::from_min_max(Pos2::new(bx, base - hv - hm), Pos2::new(bx + bw, base - hv)), CornerRadius::same(4), TEAL);
                }
                let day = k.rsplit('-').next().unwrap_or("");
                ui.painter().text(Pos2::new(x + bar_w / 2.0, inner.max.y - 8.0), Align2::CENTER_CENTER, day, FontId::proportional(11.0), muted());
                let hit = Rect::from_min_max(Pos2::new(x, inner.min.y), Pos2::new(x + bar_w, inner.max.y));
                if ui.rect_contains_pointer(hit) {
                    ui.painter().text(
                        Pos2::new(x + bar_w / 2.0, inner.min.y),
                        Align2::CENTER_TOP,
                        hours(v.0 + v.1),
                        FontId::proportional(12.0),
                        ink(),
                    );
                }
            }
            ui.horizontal(|ui| {
                ui.colored_label(accent(), "■");
                ui.label(egui::RichText::new("video").size(12.0).color(muted()));
                ui.colored_label(TEAL, "■");
                ui.label(egui::RichText::new("music").size(12.0).color(muted()));
            });
            ui.add_space(14.0);
            // rankings
            let lists: [(&str, Vec<(String, f64)>); 2] = [
                ("TOP CHANNELS", top_channels.iter().take(8).map(|(n, s)| ((*n).clone(), *s)).collect()),
                ("TOP ARTISTS", top_artists.iter().take(8).map(|(n, s)| ((*n).clone(), *s)).collect()),
            ];
            ui.horizontal_top(|ui| {
                for (title, rows) in &lists {
                    ui.vertical(|ui| {
                        ui.set_width((w - 14.0) / 2.0);
                        ui.label(egui::RichText::new(*title).size(11.0).color(muted()).strong());
                        let top = rows.first().map(|r| r.1).unwrap_or(1.0).max(1.0);
                        let (area, _) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0 * rows.len().max(1) as f32 + 20.0), Sense::hover());
                        ui.painter().add(surface::panel(area, 18.0, 1.0, None, false));
                        if rows.is_empty() {
                            ui.painter().text(area.center(), Align2::CENTER_CENTER, "nothing yet", FontId::proportional(13.0), muted());
                        }
                        for (i, (name, secs)) in rows.iter().enumerate() {
                            let y = area.min.y + 14.0 + i as f32 * 30.0;
                            let bar = Rect::from_min_size(Pos2::new(area.min.x + 14.0, y), vec2((area.width() - 28.0) * (*secs / top) as f32, 24.0));
                            ui.painter().rect_filled(bar, CornerRadius::same(8), accent().gamma_multiply(0.22));
                            let label = truncate(ui, name, FontId::proportional(13.5), area.width() - 120.0, 1);
                            ui.painter().text(Pos2::new(area.min.x + 22.0, y + 12.0), Align2::LEFT_CENTER, label, FontId::proportional(13.5), ink());
                            ui.painter().text(Pos2::new(area.max.x - 20.0, y + 12.0), Align2::RIGHT_CENTER, hours(*secs), FontId::proportional(12.5), muted());
                        }
                    });
                }
            });
            ui.add_space(14.0);
            ui.label(egui::RichText::new("MOST PLAYED").size(11.0).color(muted()).strong());
            let rows: Vec<_> = top_titles.iter().take(8).collect();
            let (area, _) = ui.allocate_exact_size(vec2(w, 30.0 * rows.len().max(1) as f32 + 20.0), Sense::hover());
            ui.painter().add(surface::panel(area, 18.0, 1.0, None, false));
            for (i, (title, chan, secs)) in rows.iter().enumerate() {
                let y = area.min.y + 14.0 + i as f32 * 30.0;
                let label = truncate(ui, &format!("{title}  —  {chan}"), FontId::proportional(13.5), area.width() - 140.0, 1);
                ui.painter().text(Pos2::new(area.min.x + 22.0, y + 12.0), Align2::LEFT_CENTER, label, FontId::proportional(13.5), ink());
                ui.painter().text(Pos2::new(area.max.x - 20.0, y + 12.0), Align2::RIGHT_CENTER, hours(*secs), FontId::proportional(12.5), muted());
            }
            ui.add_space(20.0);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_histories_are_trimmed() {
        let mut s = Stats::default();
        for i in 0..2000 {
            s.titles.insert(format!("v{i}"), ("t".into(), "c".into(), i as f64));
            s.artists.insert(format!("a{i}"), i as f64);
            s.days.insert(format!("2000-01-{i:04}"), (1.0, 0.0));
        }
        s.prune();
        assert_eq!(s.titles.len(), KEEP_ITEMS);
        assert_eq!(s.artists.len(), KEEP_ITEMS);
        assert_eq!(s.days.len(), KEEP_DAYS);
        // the most played survive
        assert!(s.titles.contains_key("v1999"));
        assert!(!s.titles.contains_key("v0"));
        assert_eq!(hours(3.0), "0m");
        assert_eq!(hours(20.0), "<1m");
        assert_eq!(hours(90.0), "2m");
    }
}
