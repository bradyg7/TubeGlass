//! Your own lists: playlists, history, "Watch later" and what the video pages show.

use crate::*;

impl App {
    /// Adds the video to a playlist, or removes it when it is already there.
    pub(crate) fn toggle_in_list(&mut self, list_id: &str, v: &Video) {
        if let Some(l) = self.lists.iter_mut().find(|l| l.id == list_id) {
            if let Some(i) = l.videos.iter().position(|x| x.id == v.id) {
                l.videos.remove(i);
            } else {
                let mut v = v.clone();
                v.kind = if v.kind == Kind::Live { Kind::Video } else { v.kind };
                l.videos.insert(0, v);
            }
            save_lists(&self.lists);
        }
    }

    /// Carries out a choice from a video card's right-click menu.
    pub(crate) fn apply_card_action(&mut self, v: Video, a: CardAction) {
        match a {
            CardAction::Download(audio, q) => self.start_download(&v, audio, q),
            CardAction::ToggleList(id) => self.toggle_in_list(&id, &v),
            CardAction::NewList => self.new_list = Some((String::new(), Some(v))),
            CardAction::RemoveFromHistory => {
                self.history.retain(|h| h.video.id != v.id);
                save_history(&self.history);
            }
            CardAction::NotInterested => {
                self.settings.hidden.insert(v.id.clone());
                self.save_settings();
                self.toast("Hidden from your recommendations");
            }
            CardAction::FewerFromChannel => {
                self.settings.chan_prefs.entry(v.channel_id.clone()).or_default().fewer = true;
                self.save_settings();
                self.toast(format!("You'll see less from {}", v.channel_name));
            }
            CardAction::PlayNext => self.queue_add(QItem::Yt(v), true),
            CardAction::AddToQueue => self.queue_add(QItem::Yt(v), false),
            CardAction::PlayAudio => {
                self.play(v);
                self.player.audio_mode = true;
            }
            CardAction::RemoveFromList => {
                if let Sel::Local(id) = self.sel.clone() {
                    self.toggle_in_list(&id, &v);
                }
            }
        }
    }

    /// The videos of the open page, narrowed by the list's own search box.
    pub(crate) fn visible_videos(&self) -> Vec<Video> {
        let mut v = self.visible_videos_inner();
        if matches!(self.sel, Sel::Home | Sel::Group(_)) {
            v.retain(|x| !self.settings.hidden.contains(&x.id));
            if self.sel == Sel::Home && self.settings.home_for_you {
                v.sort_by(|a, b| self.recommend_score(b).total_cmp(&self.recommend_score(a)));
            }
        }
        let needle = fold(self.lib_filter.trim());
        if !needle.is_empty() && matches!(self.sel, Sel::History | Sel::Local(_)) {
            v.retain(|x| fold(&x.title).contains(&needle) || fold(&x.channel_name).contains(&needle));
        }
        v
    }

    /// Forget the list search when the page changes.
    pub(crate) fn sync_lib_filter(&mut self) {
        let k = self.page_key();
        if k != self.lib_filter_page {
            self.lib_filter_page = k;
            self.lib_filter.clear();
        }
    }

    /// The videos of the open page before any search box or Home ordering is applied.
    pub(crate) fn visible_videos_inner(&self) -> Vec<Video> {
        let mut vids: Vec<Video> = match &self.sel {
            Sel::Downloads | Sel::Settings | Sel::Stats | Sel::Clips | Sel::Podcasts | Sel::Podcast(_) | Sel::Music | Sel::MusicAlbum(_) | Sel::MusicArtist(_) => vec![],
            Sel::Group(gid) => {
                let members: HashSet<&String> = self
                    .groups
                    .iter()
                    .find(|g| &g.id == gid)
                    .map(|g| g.channels.iter().collect())
                    .unwrap_or_default();
                let mut all = vec![];
                for c in self.channels.iter().filter(|c| members.contains(&c.id)) {
                    if let Some(Ok(v)) = self.feeds.get(&c.id) {
                        all.extend(v.iter().filter(|x| x.rank < 15).cloned());
                    }
                }
                let k = |x: &Video| match x.kind {
                    Kind::Live => 0,
                    Kind::Upcoming => 1,
                    Kind::Video => 2,
                };
                // live first, then newest first (rank only breaks ties within the same age)
                all.sort_by_key(|v| (k(v), if v.kind == Kind::Video { age_secs(&v.meta) } else { 0 }, v.rank));
                all
            }
            Sel::History => self.history.iter().map(|h| h.video.clone()).collect(),
            Sel::Local(id) => self
                .lists
                .iter()
                .find(|l| &l.id == id)
                .map(|l| l.videos.clone())
                .unwrap_or_default(),
            Sel::Live => self.live.as_ref().and_then(|r| r.as_ref().ok()).cloned().unwrap_or_default(),
            Sel::Playlist(_) => self
                .playlist
                .as_ref()
                .and_then(|p| p.1.as_ref())
                .and_then(|r| r.as_ref().ok())
                .map(|(_, v)| v.clone())
                .unwrap_or_default(),
            Sel::Search => self
                .svideos
                .as_ref()
                .and_then(|r| r.as_ref().ok())
                .cloned()
                .unwrap_or_default(),
            Sel::Channel(_) if self.active_chan_search().is_some() => {
                let results = self
                    .active_chan_search()
                    .and_then(|cs| cs.res.as_ref())
                    .and_then(|r| r.as_ref().ok())
                    .map(|(v, _)| v.clone())
                    .unwrap_or_default();
                let streamed = |v: &Video| v.kind != Kind::Video || v.meta.contains("Streamed");
                match self.chan_tab {
                    ChanTab::Live => results.into_iter().filter(|v| streamed(v)).collect(),
                    ChanTab::Videos => results.into_iter().filter(|v| !streamed(v)).collect(),
                    _ => vec![],
                }
            }
            Sel::Channel(id) => {
                let key = format!("{id}:{}", self.chan_tab.key());
                match self.fcache.tab_videos.get(&key) {
                    Some(v) => v.clone(),
                    // until the tab arrives, show what the cached feed already knows
                    None => {
                        let live = self.chan_tab == ChanTab::Live;
                        let none = self.chan_tab == ChanTab::Shorts;
                        self.feeds
                            .get(id)
                            .and_then(|r| r.as_ref().ok())
                            .map(|v| {
                                v.iter()
                                    .filter(|x| !none && (x.kind != Kind::Video) == live)
                                    .cloned()
                                    .collect()
                            })
                            .unwrap_or_default()
                    }
                }
            }
            Sel::Home => {
                let mut all = vec![];
                for c in &self.channels {
                    if let Some(Ok(v)) = self.feeds.get(&c.id) {
                        let mut v: Vec<Video> = v.iter().filter(|x| x.rank < 15).cloned().collect();
                        all.append(&mut v);
                    }
                }
                let k = |x: &Video| match x.kind {
                    Kind::Live => 0,
                    Kind::Upcoming => 1,
                    Kind::Video => 2,
                };
                // live first, then newest first (rank only breaks ties within the same age)
                all.sort_by_key(|v| (k(v), if v.kind == Kind::Video { age_secs(&v.meta) } else { 0 }, v.rank));
                all
            }
        };
        vids.retain(|v| match self.filter {
            Filter::All => true,
            Filter::New => !self.seen.contains(&v.id),
            Filter::Live => v.kind != Kind::Video,
            Filter::Videos => v.kind == Kind::Video,
        });
        vids
    }

    /// Keep a copy of a YouTube playlist as a local one that can be edited.
    pub(crate) fn save_playlist_locally(&mut self, name: String, videos: Vec<Video>) {
        let n = videos.len();
        let id = format!("pl-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        let mut name = name;
        if self.lists.iter().any(|l| l.name == name) {
            name = format!("{name} (copy)");
        }
        let videos = videos
            .into_iter()
            .map(|mut v| {
                if v.kind == Kind::Live {
                    v.kind = Kind::Video;
                }
                v
            })
            .collect();
        self.lists.push(LocalList { id: id.clone(), name: name.clone(), videos, offline: false });
        save_lists(&self.lists);
        self.toast(format!("Saved “{name}” with {n} videos"));
        self.sel = Sel::Local(id);
    }
}
