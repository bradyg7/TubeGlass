//! Subscribing, refreshing feeds in the background and finding an episode by id.

use crate::*;
use crate::podcasts::*;
// these two names also exist for the channel list, so they are named explicitly
use crate::podcasts::{Saved, save};

impl App {
    /// Finds an episode (and its show) by its `pod:` id.
    pub fn pod_episode(&self, id: &str) -> Option<(&Show, &Episode)> {
        let short = id.strip_prefix("pod:")?;
        self.pods.iter().find_map(|s| s.episodes.iter().find(|e| e.id == short).map(|e| (&s.show, e)))
    }

    /// Starts reading the playing episode's transcript (once). A downloaded copy is used first,
    /// so it also works without a connection.
    pub fn ensure_pod_transcript(&mut self) {
        let Some(w) = self.watching.clone() else { return };
        if !w.id.starts_with("pod:") || self.tr.as_ref().is_some_and(|t| t.0 == w.id) {
            return;
        }
        self.tr = Some((w.id.clone(), None));
        self.tr_idx = usize::MAX;
        let saved = self.library.iter().find(|d| d.id == w.id).map(download::offline_captions).and_then(|v| v.into_iter().next());
        let url = self.pod_episode(&w.id).map(|(_, e)| e.transcript.clone()).unwrap_or_default();
        let (tx, ctx, id) = (self.tx.clone(), self.ctx.clone(), w.id.clone());
        std::thread::spawn(move || {
            let r = match saved {
                Some(t) => yt::caption_cues(&t.url),
                None if url.is_empty() => Err("this episode's feed does not include a transcript".to_string()),
                None => fetch_transcript(&url),
            };
            let _ = tx.send(Msg::Transcript(id, r));
            ctx.request_repaint();
        });
    }

    /// Prepares an episode for the player in the background.
    pub fn fetch_pod_streams(&mut self, id: &str) {
        let Some((_, ep)) = self.pod_episode(id) else {
            self.player.fail("This episode is no longer in its feed. Subscribe to the show again to play it.".into());
            return;
        };
        let ep = ep.clone();
        let (tx, ctx, id) = (self.tx.clone(), self.ctx.clone(), id.to_string());
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Streams(id, Ok(build_streams(&ep))));
            ctx.request_repaint();
        });
    }

    /// Reads a feed in the background; with `subscribe` it adds the show.
    pub fn refresh_pod(&mut self, feed: &str, subscribe: bool) {
        if !self.pod_busy.insert(feed.to_string()) {
            return;
        }
        let (tx, ctx, feed) = (self.tx.clone(), self.ctx.clone(), feed.to_string());
        std::thread::spawn(move || {
            let r = resolve_link(&feed).and_then(|f| fetch(&f).map(|(s, e)| (f, s, e)));
            let _ = tx.send(Msg::Podcast(feed, subscribe, r));
            ctx.request_repaint();
        });
    }

    /// Refreshes every subscribed show.
    pub fn refresh_all_pods(&mut self) {
        for feed in self.pods.iter().map(|p| p.show.feed.clone()).collect::<Vec<_>>() {
            self.refresh_pod(&feed, false);
        }
    }

    /// Refresh shows that have not been read for a while (called every frame; cheap).
    pub fn pod_auto_refresh(&mut self) {
        if !self.online || Instant::now() < self.pod_next_check {
            return;
        }
        self.pod_next_check = Instant::now() + Duration::from_secs(600);
        let now = now_secs();
        for feed in self.pods.iter().filter(|p| now.saturating_sub(p.fetched) > 1800).map(|p| p.show.feed.clone()).collect::<Vec<_>>() {
            self.refresh_pod(&feed, false);
        }
    }

    /// Merges a fetched feed into the subscriptions (or adds a new show).
    pub fn apply_podcast(&mut self, asked: String, subscribe: bool, r: Result<(String, Show, Vec<Episode>), String>) {
        self.pod_busy.remove(&asked);
        let (feed, show, eps) = match r {
            Ok(x) => x,
            Err(e) => {
                if subscribe {
                    self.pod_err = Some(e);
                }
                return;
            }
        };
        self.pod_busy.remove(&feed);
        let known = self.pods.iter().position(|p| p.show.feed == feed);
        if known.is_none() && !subscribe {
            return;
        }
        let i = match known {
            Some(i) => i,
            None => {
                // a new subscription: what is already there is the baseline, not "new"
                for e in &eps {
                    self.seen.insert(video_id(e));
                }
                self.seen_dirty = true;
                self.pods.push(Saved { show: show.clone(), episodes: vec![], fetched: 0 });
                self.pod_input.clear();
                self.pod_hits = None;
                self.pods.len() - 1
            }
        };
        let p = &mut self.pods[i];
        p.show = show;
        p.fetched = now_secs();
        for e in eps {
            match p.episodes.iter_mut().find(|x| x.id == e.id) {
                Some(old) => *old = e,
                None => p.episodes.push(e),
            }
        }
        p.episodes.sort_by(|a, b| b.published.cmp(&a.published));
        p.episodes.truncate(400);
        save(&self.pods);
        if known.is_none() {
            let f = self.pods[i].show.feed.clone();
            self.toast(format!("Subscribed to {}", self.pods[i].show.title));
            self.sel = Sel::Podcast(f);
        }
    }

    /// Removes a show and goes back to the Podcasts page if needed.
    pub fn unsubscribe_pod(&mut self, feed: &str) {
        self.pods.retain(|p| p.show.feed != feed);
        save(&self.pods);
        if self.sel == Sel::Podcast(feed.to_string()) {
            self.sel = Sel::Podcasts;
        }
    }

    /// Handles what was typed in the Podcasts search box.
    pub fn start_pod_input(&mut self) {
        let text = self.pod_input.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.pod_err = None;
        match classify(&text) {
            Input::Feed(f) => self.refresh_pod(&f, true),
            Input::Search(q) => {
                self.pod_hits = Some((q.clone(), None));
                let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
                std::thread::spawn(move || {
                    let r = search(&q);
                    let _ = tx.send(Msg::PodSearch(q, r));
                    ctx.request_repaint();
                });
            }
        }
    }

    // ------------------------------------------------------------------ pages

    /// How many episodes of the show you have not opened yet.
    pub fn pod_new_count(&self, p: &Saved) -> usize {
        p.episodes.iter().filter(|e| !self.seen.contains(&video_id(e))).count()
    }
}
