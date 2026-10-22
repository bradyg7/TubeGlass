//! Moving between pages: back / forward, the saved page and window, theme sync and toasts.

use crate::*;

impl App {
    /// Equalizer, loudness and silence filters for what is playing.
    /// Theme, accent and text size: re-style whenever one of them changed.
    pub(crate) fn sync_look(&mut self, ctx: &egui::Context) {
        let light = match self.settings.theme {
            1 => true,
            2 => ctx.system_theme() == Some(egui::Theme::Light),
            _ => false,
        };
        let sig = (light, self.settings.accent, self.settings.accent_custom);
        if sig != self.look_sig {
            self.look_sig = sig;
            LIGHT.store(light, std::sync::atomic::Ordering::Relaxed);
            FORCE_DARK.store(false, std::sync::atomic::Ordering::Relaxed);
            style(ctx);
        }
        let z = self.settings.font_scale.clamp(0.8, 1.5);
        if (ctx.zoom_factor() - z).abs() > 0.001 {
            ctx.set_zoom_factor(z);
        }
    }

    /// Record page changes (made from anywhere in the app) so Back can undo them.
    /// Compact description of the open page, restored on the next launch.
    pub(crate) fn page_key(&self) -> String {
        match &self.sel {
            Sel::Home => "home".into(),
            Sel::History => "history".into(),
            Sel::Stats => "stats".into(),
            Sel::Clips => "clips".into(),
            Sel::Podcasts => "podcasts".into(),
            Sel::Podcast(f) => format!("podcast:{f}"),
            Sel::Downloads => "downloads".into(),
            Sel::Settings => "settings".into(),
            Sel::Music | Sel::MusicAlbum(_) | Sel::MusicArtist(_) => "music".into(),
            Sel::Live => "live".into(),
            Sel::Group(id) => format!("group:{id}"),
            Sel::Local(id) => format!("local:{id}"),
            Sel::Playlist(id) => format!("playlist:{id}"),
            Sel::Channel(id) => format!("channel:{id}:{}", tab_index(self.chan_tab)),
            // search results are not worth restoring
            Sel::Search => "home".into(),
        }
    }

    /// Opens the page described by a saved page key (used at start-up).
    pub(crate) fn restore_page(&mut self, key: &str) {
        let (kind, rest) = key.split_once(':').unwrap_or((key, ""));
        match kind {
            "history" => self.sel = Sel::History,
            "stats" => self.sel = Sel::Stats,
            "clips" => self.sel = Sel::Clips,
            "podcasts" => self.sel = Sel::Podcasts,
            "podcast" if self.pods.iter().any(|p| p.show.feed == rest) => self.sel = Sel::Podcast(rest.to_string()),
            "downloads" => self.sel = Sel::Downloads,
            "settings" => self.sel = Sel::Settings,
            "music" => self.music_open_home(),
            "live" => {
                self.sel = Sel::Live;
                if self.live.is_none() {
                    self.load_live();
                }
            }
            "group" if self.groups.iter().any(|g| g.id == rest) => self.sel = Sel::Group(rest.to_string()),
            "local" if self.lists.iter().any(|l| l.id == rest) => self.sel = Sel::Local(rest.to_string()),
            "playlist" if !rest.is_empty() => self.open_playlist(rest.to_string()),
            "channel" => {
                let (id, tab) = rest.rsplit_once(':').unwrap_or((rest, "0"));
                if self.channels.iter().any(|c| c.id == id) {
                    self.chan_tab = tab_from_index(tab.parse().unwrap_or(0));
                    self.sel = Sel::Channel(id.to_string());
                }
            }
            _ => {}
        }
        self.nav_cur = NavEntry { sel: self.sel.clone(), tab: self.chan_tab, query: String::new() };
    }

    /// Remember size, position, maximised state and page (written a moment after changes).
    pub(crate) fn track_window(&mut self, ctx: &egui::Context) {
        let (inner, outer, maxi, full) = ctx.input(|i| {
            let v = i.viewport();
            (v.inner_rect, v.outer_rect, v.maximized.unwrap_or(false), v.fullscreen.unwrap_or(false))
        });
        let mut now = self.win.clone();
        now.page = self.page_key();
        now.maximized = maxi;
        if !maxi && !full && !self.mini {
            if let Some(r) = inner {
                now.w = r.width();
                now.h = r.height();
            }
            if let Some(r) = outer {
                now.x = Some(r.min.x);
                now.y = Some(r.min.y);
            }
        }
        if now != self.win {
            self.win = now;
            self.win_dirty_since.get_or_insert_with(Instant::now);
        }
        if self.win_dirty_since.is_some_and(|t| t.elapsed() > Duration::from_millis(1200)) {
            self.win_dirty_since = None;
            save_win(&self.win);
        }
    }

    /// Records page changes so Back can undo them.
    pub(crate) fn track_navigation(&mut self) {
        if self.nav_cur.sel != self.sel {
            if !self.nav_skip {
                let dup = self.nav_back.last().is_some_and(|l| l.sel == self.nav_cur.sel);
                if !dup {
                    self.nav_back.push(self.nav_cur.clone());
                    if self.nav_back.len() > 60 {
                        self.nav_back.remove(0);
                    }
                }
            }
            self.nav_skip = false;
        }
        self.nav_cur = NavEntry { sel: self.sel.clone(), tab: self.chan_tab, query: self.search_query.clone() };
    }

    /// True when there is a page to go back to.
    pub(crate) fn can_go_back(&self) -> bool {
        !self.nav_back.is_empty()
    }

    /// Goes back to the previous page.
    pub(crate) fn go_back(&mut self) {
        let Some(e) = self.nav_back.pop() else { return };
        self.nav_skip = true;
        self.filter = Filter::All;
        self.chan_tab = e.tab;
        match &e.sel {
            Sel::Playlist(id) => {
                let loaded = self.playlist.as_ref().is_some_and(|p| &p.0 == id && p.1.is_some());
                if loaded {
                    self.sel = e.sel.clone();
                } else {
                    self.open_playlist(id.clone());
                }
            }
            Sel::Search => {
                if !e.query.is_empty() && e.query != self.search_query {
                    self.search_input = e.query.clone();
                    self.start_search();
                } else {
                    self.sel = Sel::Search;
                }
            }
            Sel::Live => {
                if self.live.is_none() || now_secs().saturating_sub(self.fcache.live_updated) > 180 {
                    self.load_live();
                }
                self.sel = Sel::Live;
            }
            Sel::Music => self.music_open_home(),
            Sel::MusicAlbum(id) | Sel::MusicArtist(id) => {
                let artist = matches!(e.sel, Sel::MusicArtist(_));
                let kind = if artist { ytmusic::CardKind::Artist } else { ytmusic::CardKind::Album };
                self.music_open(&kind, id);
            }
            other => self.sel = other.clone(),
        }
        self.nav_cur = NavEntry { sel: self.sel.clone(), tab: self.chan_tab, query: self.search_query.clone() };
    }

    /// Shows a short message at the bottom of the window.
    pub(crate) fn toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now()));
        self.ctx.request_repaint();
    }

    /// Writes the settings file.
    pub(crate) fn save_settings(&self) {
        save_settings(&self.settings);
    }

    /// Collapses or expands the left sidebar.
    pub(crate) fn toggle_sidebar(&mut self) {
        use std::sync::atomic::Ordering;
        let now = !SIDE_COLLAPSED.load(Ordering::Relaxed);
        SIDE_COLLAPSED.store(now, Ordering::Relaxed);
        save(&self.channels);
    }
}
