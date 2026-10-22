//! Everything that is saved to disk: channels, history, playlists, groups, caches, window state.

use crate::*;

/// The saved channel list and sidebar state.
#[derive(Serialize, Deserialize, Default)]
pub(crate) struct Saved {
    pub(crate) channels: Vec<Channel>,
    #[serde(default)]
    pub(crate) collapsed: bool,
}

// ================= Window state =================
/// Window size, position and the last open page, restored at the next start.
#[derive(Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub(crate) struct WinState {
    pub(crate) w: f32,
    pub(crate) h: f32,
    pub(crate) x: Option<f32>,
    pub(crate) y: Option<f32>,
    pub(crate) maximized: bool,
    /// last page, e.g. "home", "channel:UC…:2", "group:g-1"
    pub(crate) page: String,
}

/// Where window.json lives.
pub(crate) fn win_path() -> Option<std::path::PathBuf> {
    small_list_path("window.json")
}

/// Reads the saved window state.
pub(crate) fn load_win() -> WinState {
    win_path()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Writes the window state.
pub(crate) fn save_win(w: &WinState) {
    if let Some(p) = win_path() {
        let _ = std::fs::write(p, serde_json::to_vec(w).unwrap_or_default());
    }
}

/// Path of a small data file inside the config folder (created on demand).
pub(crate) fn small_list_path(name: &str) -> Option<std::path::PathBuf> {
    let d = dirs::config_dir()?.join("tubeglass");
    std::fs::create_dir_all(&d).ok()?;
    Some(d.join(name))
}

/// Reads the set of video ids already seen.
pub(crate) fn load_seen() -> HashSet<String> {
    small_list_path("seen.json")
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice::<Vec<String>>(&b).ok())
        .map(|v| v.into_iter().collect())
        .unwrap_or_default()
}

/// Writes the set of seen video ids.
pub(crate) fn save_seen(s: &HashSet<String>) {
    if let Some(p) = small_list_path("seen.json") {
        // keep the file bounded; ids are only ever compared for membership
        let v: Vec<&String> = s.iter().take(20_000).collect();
        let _ = std::fs::write(p, serde_json::to_vec(&v).unwrap_or_default());
    }
}

/// Reads the recent search phrases.
pub(crate) fn load_recent() -> Vec<String> {
    small_list_path("recent.json")
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Writes the recent search phrases.
pub(crate) fn save_recent(r: &[String]) {
    if let Some(p) = small_list_path("recent.json") {
        let _ = std::fs::write(p, serde_json::to_vec(r).unwrap_or_default());
    }
}

/// One watched video: when, and how far you got.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct HistEntry {
    pub(crate) video: Video,
    pub(crate) last_viewed: u64,
    /// where playback stopped, in seconds (0 once a video has been watched to the end)
    pub(crate) position: f64,
    pub(crate) duration: f64,
}

/// Where history.json lives.
pub(crate) fn history_path() -> Option<std::path::PathBuf> {
    let d = dirs::config_dir()?.join("tubeglass");
    std::fs::create_dir_all(&d).ok()?;
    Some(d.join("history.json"))
}

/// Reads the watch history.
pub(crate) fn load_history() -> Vec<HistEntry> {
    history_path()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Writes the watch history atomically.
pub(crate) fn save_history(h: &[HistEntry]) {
    if let Some(p) = history_path() {
        if let Ok(b) = serde_json::to_vec(h) {
            let tmp = p.with_extension("json.tmp");
            if std::fs::write(&tmp, b).is_ok() {
                let _ = std::fs::rename(tmp, p);
            }
        }
    }
}

/// A playlist kept on this computer (Favorites, Liked songs, Watch later or your own).
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct LocalList {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) videos: Vec<Video>,
    /// keep every item saved on this computer (new ones download by themselves)
    #[serde(default)]
    pub(crate) offline: bool,
}

/// Where playlists.json lives.
pub(crate) fn lists_path() -> Option<std::path::PathBuf> {
    let d = dirs::config_dir()?.join("tubeglass");
    std::fs::create_dir_all(&d).ok()?;
    Some(d.join("playlists.json"))
}

/// Reads the playlists, making sure the built-in ones exist.
pub(crate) fn load_lists() -> Vec<LocalList> {
    let mut v: Vec<LocalList> = lists_path()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    if !v.iter().any(|l| l.id == "favorites") {
        v.insert(0, LocalList { id: "favorites".into(), name: "Favorites".into(), videos: vec![], offline: false });
    }
    if !v.iter().any(|l| l.id == "liked") {
        let at = 1.min(v.len());
        v.insert(at, LocalList { id: "liked".into(), name: "Liked songs".into(), videos: vec![], offline: false });
    }
    if !v.iter().any(|l| l.id == "watchlater") {
        let at = 1.min(v.len());
        v.insert(at, LocalList { id: "watchlater".into(), name: "Watch later".into(), videos: vec![], offline: false });
    }
    v
}

// ================= Followed artists (YouTube Music) =================
/// An artist followed in YouTube Music.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct FollowedArtist {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) thumb: String,
}

/// Reads the followed artists.
pub(crate) fn load_followed() -> Vec<FollowedArtist> {
    small_list_path("artists.json")
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Writes the followed artists.
pub(crate) fn save_followed(a: &[FollowedArtist]) {
    if let Some(p) = small_list_path("artists.json") {
        let _ = std::fs::write(p, serde_json::to_vec_pretty(a).unwrap_or_default());
    }
}

// ================= Subscription groups =================
/// A named set of channels with its own feed page.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Group {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) channels: Vec<String>,
}

/// Reads the channel groups.
pub(crate) fn load_groups() -> Vec<Group> {
    small_list_path("groups.json")
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Writes the channel groups.
pub(crate) fn save_groups(g: &[Group]) {
    if let Some(p) = small_list_path("groups.json") {
        let _ = std::fs::write(p, serde_json::to_vec_pretty(g).unwrap_or_default());
    }
}

/// Writes the playlists.
pub(crate) fn save_lists(lists: &[LocalList]) {
    if let Some(p) = lists_path() {
        if let Ok(b) = serde_json::to_vec_pretty(lists) {
            let _ = std::fs::write(p, b);
        }
    }
}

/// Where channels.json lives.
pub(crate) fn store_path() -> Option<std::path::PathBuf> {
    let d = dirs::config_dir()?.join("tubeglass");
    std::fs::create_dir_all(&d).ok()?;
    Some(d.join("channels.json"))
}

/// Reads the saved channel list.
pub(crate) fn load_saved() -> Saved {
    store_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Writes the saved channel list.
pub(crate) fn save(channels: &[Channel]) {
    if let Some(p) = store_path() {
        let s = Saved {
            channels: channels.to_vec(),
            collapsed: SIDE_COLLAPSED.load(std::sync::atomic::Ordering::Relaxed),
        };
        let _ = std::fs::write(p, serde_json::to_string_pretty(&s).unwrap_or_default());
    }
}

/// Feeds, channel pages and headers kept on disk so the app opens with content already there.
#[derive(Serialize, Deserialize, Default)]
pub(crate) struct FeedCache {
    pub(crate) feeds: HashMap<String, Vec<Video>>,
    pub(crate) updated: HashMap<String, u64>,
    pub(crate) live: Vec<Video>,
    pub(crate) live_updated: u64,
    #[serde(default)]
    pub(crate) tab_videos: HashMap<String, Vec<Video>>,
    #[serde(default)]
    pub(crate) tab_playlists: HashMap<String, Vec<PlaylistHit>>,
    #[serde(default)]
    pub(crate) about: HashMap<String, ChannelAbout>,
    #[serde(default)]
    pub(crate) headers: HashMap<String, ChannelHeader>,
    /// Continuation tokens for the next page of a channel tab ("id:tab").
    #[serde(default)]
    pub(crate) tab_tokens: HashMap<String, String>,
}

/// What the app keeps on disk.
#[derive(Clone, Default)]
pub(crate) struct StorageInfo {
    pub(crate) downloads: u64,
    pub(crate) items: usize,
    pub(crate) clips: u64,
    pub(crate) images: u64,
    pub(crate) feeds: u64,
    pub(crate) free: u64,
}

/// Total size in bytes of everything inside a folder.
pub(crate) fn dir_size(p: &std::path::Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(p) else { return 0 };
    rd.flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => dir_size(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

/// Free bytes on the disk holding `p` (0 when unknown).
pub(crate) fn free_space(p: &std::path::Path) -> u64 {
    use std::os::unix::ffi::OsStrExt;
    let mut path = p.to_path_buf();
    while !path.exists() && path.pop() {}
    let Ok(c) = std::ffi::CString::new(path.as_os_str().as_bytes()) else { return 0 };
    unsafe {
        let mut st: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c.as_ptr(), &mut st) == 0 {
            st.f_bavail as u64 * st.f_frsize as u64
        } else {
            0
        }
    }
}

/// Measures downloads, clips and caches for the Storage section of the settings.
pub(crate) fn storage_info(items: usize) -> StorageInfo {
    let d = download::dir();
    let clips = dir_size(&d.join("Clips"));
    let cache = cache_dir();
    StorageInfo {
        downloads: dir_size(&d).saturating_sub(clips),
        items,
        clips,
        images: cache.as_ref().map(|c| dir_size(&c.join("img"))).unwrap_or(0),
        feeds: cache.as_ref().and_then(|c| std::fs::metadata(c.join("feeds.json")).ok()).map(|m| m.len()).unwrap_or(0),
        free: free_space(&d),
    }
}

/// The cache folder (thumbnails, feed cache), created on demand.
pub(crate) fn cache_dir() -> Option<std::path::PathBuf> {
    let d = dirs::cache_dir()?.join("tubeglass");
    std::fs::create_dir_all(d.join("img")).ok()?;
    Some(d)
}

/// Reads the feed cache.
pub(crate) fn load_feed_cache() -> FeedCache {
    cache_dir()
        .and_then(|d| std::fs::read(d.join("feeds.json")).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Writes the feed cache.
pub(crate) fn save_feed_cache(c: &FeedCache) {
    if let Some(d) = cache_dir() {
        if let Ok(b) = serde_json::to_vec(c) {
            let tmp = d.join("feeds.json.tmp");
            if std::fs::write(&tmp, b).is_ok() {
                let _ = std::fs::rename(tmp, d.join("feeds.json"));
            }
        }
    }
}
