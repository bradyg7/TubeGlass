//! Talks to the desktop: media keys (MPRIS), notifications and live-stream alerts.

use crate::*;

impl App {
    /// Media-key / MPRIS state and commands, plus the opt-in "channel went live" check.
    pub(crate) fn desktop_integration(&mut self) {
        // publish what is playing
        let state = self.player.state();
        let status = match (&self.watching, state) {
            (None, _) => "Stopped",
            (Some(_), State::Playing | State::Buffering | State::Loading) => "Playing",
            (Some(_), State::Paused) => "Paused",
            (Some(_), _) => "Stopped",
        };
        let snapshot = mpris::State {
            status: status.to_string(),
            title: self.watching.as_ref().map(|w| w.title.clone()).unwrap_or_default(),
            artist: self.watching.as_ref().map(|w| w.channel_name.clone()).unwrap_or_default(),
            art: self
                .watching
                .as_ref()
                .map(|w| if w.thumb.starts_with("http") { w.thumb.clone() } else { String::new() })
                .unwrap_or_default(),
            length_us: (self.player.duration() * 1e6) as i64,
            position_us: (self.player.position() * 1e6) as i64,
            can_next: self.queue_idx.is_some_and(|i| i + 1 < self.queue.len()),
            can_prev: self.watching.is_some(),
            volume: if self.player.muted { 0.0 } else { self.player.volume as f64 },
            rate: self.player.speed as f64,
            track: self.watching.as_ref().map_or(0, |w| {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                w.id.hash(&mut h);
                h.finish() >> 8
            }),
        };
        let mut cmds = vec![];
        if let Some((st, rx)) = &self.mpris {
            if let Ok(mut g) = st.lock() {
                *g = snapshot;
            }
            while let Ok(c) = rx.try_recv() {
                cmds.push(c);
            }
        }
        for c in cmds {
            use mpris::Cmd;
            match c {
                Cmd::PlayPause => {
                    if self.watching.is_some() {
                        self.player.toggle_pause();
                    }
                }
                Cmd::Play => {
                    if self.watching.is_some() && self.player.paused {
                        self.player.toggle_pause();
                    }
                }
                Cmd::Pause => {
                    if self.watching.is_some() && !self.player.paused {
                        self.player.toggle_pause();
                    }
                }
                Cmd::Stop => self.close_player(),
                Cmd::Next => {
                    self.queue_next();
                }
                Cmd::Previous => self.queue_prev(),
                Cmd::SeekBy(us) => {
                    let p = self.player.position() + us as f64 / 1e6;
                    self.player.seek(p.max(0.0));
                }
                Cmd::SetPosition(us) => self.player.seek((us as f64 / 1e6).max(0.0)),
                Cmd::Raise => self.ctx.send_viewport_cmd(egui::ViewportCommand::Focus),
            }
        }

        // "channel went live" – opt-in, every ten minutes, one channel page at a time
        if self.online && self.settings.notifications && !self.channels.is_empty() && Instant::now() >= self.next_live_check {
            self.next_live_check = Instant::now() + Duration::from_secs(600);
            let chans: Vec<Channel> = self
                .channels
                .iter()
                .filter(|c| self.settings.chan_prefs.get(&c.id).is_none_or(|p| p.notify != 2))
                .cloned()
                .collect();
            let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
            std::thread::spawn(move || {
                let mut found = vec![];
                for ch in chans {
                    if let Ok(v) = yt::channel_live(&ch) {
                        for x in v {
                            found.push((ch.name.clone(), x));
                        }
                    }
                    std::thread::sleep(Duration::from_millis(400));
                }
                let _ = tx.send(Msg::LiveCheck(found));
                ctx.request_repaint();
            });
        }
    }
}
