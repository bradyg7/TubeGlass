//! The data types YouTube extraction returns (videos, channels, streams, comments) and parsers for chapters and storyboards.

use super::*;

/// Result type used by every extractor; errors are plain messages.
pub type Res<T> = Result<T, String>;

/// A channel: id, name and avatar.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub avatar: String,
}

/// Normal video, live now, or upcoming live.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Video,
    Live,
    Upcoming,
}

/// A video (or song) as listed on a page.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Video {
    pub id: String,
    pub title: String,
    pub thumb: String,
    pub duration: String,
    pub meta: String,
    pub kind: Kind,
    pub channel_id: String,
    pub channel_name: String,
    /// Position in the channel's list (lower = more recent).
    pub rank: usize,
    /// A YouTube Short (vertical, up to a minute).
    #[serde(default)]
    pub short: bool,
    /// A YouTube Music track: opens in audio mode.
    #[serde(default)]
    pub music: bool,
}

/// One playable video option (quality, address, whether it carries sound).
#[derive(Clone, Debug)]
pub struct VideoOpt {
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub ua: String,
    /// The file already carries audio (HLS variants, progressive formats).
    pub muxed: bool,
    /// HLS / DASH playlists are opened by ffmpeg directly instead of via the range proxy.
    pub hls: bool,
    /// HLS variants that reference a separate audio rendition: its playlist URL.
    pub audio_url: Option<String>,
}

/// A separate audio stream.
#[derive(Clone, Debug)]
pub struct AudioSrc {
    pub url: String,
    pub ua: String,
}

/// A chapter of a video.
#[derive(Clone, Debug)]
pub struct Chapter {
    pub start: f64,
    pub title: String,
}

/// One storyboard level: sprite sheets of small preview frames.
#[derive(Clone, Debug)]
pub struct Storyboard {
    pub(crate) url_tpl: String,
    pub(crate) sigh: String,
    pub(crate) level: usize,
    pub(crate) name: String,
    pub width: u32,
    pub height: u32,
    pub(crate) count: u32,
    pub(crate) cols: u32,
    pub(crate) rows: u32,
    pub(crate) interval_ms: u32,
}

impl Storyboard {
    /// Address of one storyboard sheet.
    fn sheet_url(&self, sheet: u32) -> String {
        let name = self.name.replace("$M", &sheet.to_string());
        let mut u = self
            .url_tpl
            .replace("$L", &self.level.to_string())
            .replace("$N", &name);
        let enc: String = self
            .sigh
            .bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
                _ => format!("%{b:02X}"),
            })
            .collect();
        u.push_str(&format!("&sigh={enc}"));
        u
    }

    /// Sheet URL and the normalised (u0, v0, u1, v1) rectangle of the frame at `secs`.
    pub fn frame_at(&self, secs: f64, duration: f64) -> Option<(String, [f32; 4])> {
        if self.count == 0 || self.cols == 0 || self.rows == 0 {
            return None;
        }
        let per_sheet = self.cols * self.rows;
        let idx = if self.interval_ms > 0 {
            (secs * 1000.0 / self.interval_ms as f64) as u32
        } else if duration > 0.0 {
            (secs / duration * self.count as f64) as u32
        } else {
            0
        }
        .min(self.count - 1);
        let sheet = idx / per_sheet;
        let in_sheet = idx % per_sheet;
        let (col, row) = (in_sheet % self.cols, in_sheet / self.cols);
        // the last sheet is only as tall as the frames it holds
        let frames_here = (self.count - sheet * per_sheet).min(per_sheet);
        let rows_here = frames_here.div_ceil(self.cols).max(1);
        let cols_here = frames_here.min(self.cols).max(1);
        let (sw, sh) = ((cols_here * self.width) as f32, (rows_here * self.height) as f32);
        let (x0, y0) = ((col * self.width) as f32, (row * self.height) as f32);
        Some((
            self.sheet_url(sheet),
            [x0 / sw, y0 / sh, (x0 + self.width as f32) / sw, (y0 + self.height as f32) / sh],
        ))
    }
}

/// Reads the storyboard description of hover previews.
pub(crate) fn parse_storyboard(spec: &str) -> Option<Storyboard> {
    let mut parts = spec.split('|');
    let base = parts.next()?.to_string();
    let levels: Vec<&str> = parts.collect();
    let mut best: Option<Storyboard> = None;
    for (i, l) in levels.iter().enumerate() {
        let f: Vec<&str> = l.split('#').collect();
        if f.len() < 8 {
            continue;
        }
        let n = |k: usize| f[k].parse::<u32>().unwrap_or(0);
        let sb = Storyboard {
            url_tpl: base.clone(),
            sigh: f[7].to_string(),
            level: i,
            name: f[6].to_string(),
            width: n(0),
            height: n(1),
            count: n(2),
            cols: n(3),
            rows: n(4),
            interval_ms: n(5),
        };
        // prefer a level that is sharp enough for a hover card but still light to download
        let score = |s: &Storyboard| {
            let w = s.width as i32;
            (if (150..=330).contains(&w) { 1000 } else { 0 }) + w.min(320) + if s.interval_ms > 0 { 50 } else { 0 }
        };
        if sb.width > 0 && best.as_ref().is_none_or(|b| score(&sb) > score(b)) {
            best = Some(sb);
        }
    }
    best
}

/// Chapters from timestamps in a video description ("0:00 Intro", "12:34 - Topic").
/// Follows YouTube's own rules: the list must start at 0:00, have at least 3 entries and
/// strictly increasing times.
pub fn parse_chapters(desc: &str, duration: f64) -> Vec<Chapter> {
    /// Reads a time stamp like 1:02:03.
    fn parse_ts(tok: &str) -> Option<f64> {
        let parts: Vec<&str> = tok.split(':').collect();
        if parts.len() < 2 || parts.len() > 3 {
            return None;
        }
        let mut secs = 0.0;
        for p in &parts {
            if p.is_empty() || p.len() > 2 && p != parts.first().unwrap() || !p.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            secs = secs * 60.0 + p.parse::<f64>().ok()?;
        }
        Some(secs)
    }
    let mut out: Vec<Chapter> = vec![];
    for line in desc.lines() {
        let t = line.trim().trim_start_matches(|c: char| "-–—•·*>▶►▸→ 	".contains(c));
        // timestamp at the start of the line, optionally in () or []
        let t = t.trim_start_matches(['(', '[']);
        let first = t.split_whitespace().next().unwrap_or("");
        let tok = first.trim_end_matches([')', ']', '-', '–', '—', ':', '.']);
        let Some(start) = parse_ts(tok) else { continue };
        let rest = t[first.len()..]
            .trim()
            .trim_start_matches(|c: char| "-–—:|.)]•· 	".contains(c))
            .trim();
        if rest.is_empty() {
            continue;
        }
        if let Some(last) = out.last() {
            if start <= last.start + 1.0 {
                break;
            }
        } else if start != 0.0 {
            continue;
        }
        if duration > 0.0 && start >= duration {
            break;
        }
        out.push(Chapter { start, title: rest.chars().take(120).collect() });
    }
    if out.len() >= 3 { out } else { vec![] }
}

/// One caption track of a video.
#[derive(Clone, Debug)]
pub struct CaptionTrack {
    pub lang: String,
    pub name: String,
    pub auto: bool,
    pub url: String,
}

/// One timed caption line.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cue {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// A SponsorBlock segment to skip.
#[derive(Clone, Debug)]
pub struct SkipSeg {
    pub start: f64,
    pub end: f64,
    pub category: String,
}

/// Everything the player needs for one video: streams, captions, chapters, previews.
#[derive(Clone, Debug)]
pub struct Streams {
    pub captions: Vec<CaptionTrack>,
    pub chapters: Vec<Chapter>,
    pub storyboard: Option<Storyboard>,
    pub title: String,
    pub is_live: bool,
    pub duration: f64,
    pub video: Vec<VideoOpt>,
    /// Separate audio stream; `None` when the video options are already muxed (HLS).
    pub audio: Option<AudioSrc>,
}

/// A channel in search results.
#[derive(Clone, Debug)]
pub struct ChannelHit {
    pub channel: Channel,
    pub handle: String,
    pub subs: String,
}

/// A playlist in search results.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlaylistHit {
    pub id: String,
    pub title: String,
    pub thumb: String,
    pub owner: String,
    pub count: String,
}

/// A comment or reply.
#[derive(Clone, Debug)]
pub struct Comment {
    pub id: String,
    /// Continuation token that loads this comment's replies (top-level comments only).
    pub reply_token: Option<String>,
    pub author: String,
    pub avatar: String,
    pub text: String,
    pub time: String,
    pub likes: String,
    pub replies: String,
    /// "Pinned by @creator"
    pub pinned: Option<String>,
    /// "❤ by @creator" when the creator hearted it
    pub hearted: Option<String>,
    pub by_creator: bool,
    pub verified: bool,
}

/// One entry of the comment sort menu.
#[derive(Clone, Debug)]
pub struct SortOpt {
    pub title: String,
    pub token: String,
    pub selected: bool,
}

/// One page of comments with its sort options.
pub struct CommentsPage {
    pub list: Vec<Comment>,
    pub next: Option<String>,
    /// Top / Newest, only on the first page of a sort order.
    pub sorts: Vec<SortOpt>,
}

/// A channel page's header details.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ChannelHeader {
    pub handle: String,
    pub subs: String,
    pub videos: String,
    pub description: String,
    pub banner: String,
}

/// A channel's About details.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ChannelAbout {
    pub description: String,
    pub subs: String,
    pub videos: String,
    pub views: String,
    pub joined: String,
    pub country: String,
    pub links: Vec<(String, String)>,
}

/// What a channel tab holds.
pub enum TabData {
    Videos(Vec<Video>),
    Playlists(Vec<PlaylistHit>),
    About(ChannelAbout),
}
