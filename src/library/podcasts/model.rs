//! Podcast data types and how they are saved (`podcasts.json`).

use crate::*;
use sha2::{Digest, Sha256};

/// User agent sent to podcast servers.
pub(crate) const UA: &str = "TubeGlass/0.1";

/// A podcast: its feed address and what the feed says about it.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Show {
    /// the feed address; also the show's identity
    pub feed: String,
    pub title: String,
    pub author: String,
    pub image: String,
    pub description: String,
}

/// One episode of a podcast.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Episode {
    /// short stable id derived from the feed's guid
    pub id: String,
    pub title: String,
    pub url: String,
    pub duration: f64,
    /// unix seconds (0 when the feed gives none)
    pub published: u64,
    pub summary: String,
    pub image: String,
    /// address of a timed transcript (SRT, WebVTT or the Podcasting 2.0 JSON), when the feed has one
    #[serde(default)]
    pub transcript: String,
}

/// A subscribed show with its episodes.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Saved {
    pub show: Show,
    pub episodes: Vec<Episode>,
    /// when the feed was last read successfully (unix seconds)
    #[serde(default)]
    pub fetched: u64,
}

/// One result of the podcast directory search.
#[derive(Clone)]
pub struct Hit {
    pub title: String,
    pub author: String,
    pub feed: String,
    pub image: String,
}

/// Where podcasts.json lives.
pub(crate) fn path() -> Option<std::path::PathBuf> {
    small_list_path("podcasts.json")
}

/// Reads the subscriptions.
pub fn load() -> Vec<Saved> {
    path().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Writes the subscriptions atomically.
pub fn save(v: &[Saved]) {
    if let Some(p) = path() {
        let tmp = p.with_extension("json.tmp");
        if std::fs::write(&tmp, serde_json::to_vec(v).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(tmp, p);
        }
    }
}

/// `pod:<short id>` is how an episode is known everywhere else in the app.
pub fn video_id(ep: &Episode) -> String {
    format!("pod:{}", ep.id)
}

/// A short stable id made from a feed address and an episode guid.
pub fn short_id(key: &str) -> String {
    Sha256::digest(key.as_bytes()).iter().take(6).map(|b| format!("{b:02x}")).collect()
}
