//! Backing up and restoring the app's data, and the "Check YouTube access" self-test.

use crate::*;

/// Files in the config folder that make up a backup.
pub(crate) const BACKUP_FILES: &[&str] = &[
    "channels.json", "settings.json", "history.json", "playlists.json", "groups.json", "artists.json", "seen.json", "recent.json", "stats.json", "bookmarks.json", "clips.json", "podcasts.json",
];

/// Collects the app's data files into one JSON text for "Back up everything".
pub(crate) fn backup_bundle() -> String {
    let mut files = serde_json::Map::new();
    for name in BACKUP_FILES {
        if let Some(v) = small_list_path(name)
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        {
            files.insert((*name).to_string(), v);
        }
    }
    serde_json::to_string_pretty(&serde_json::json!({ "app": "tubeglass", "version": 1, "files": files })).unwrap_or_default()
}

/// Writes a backup bundle back to the config folder. Only known file names are accepted.
pub(crate) fn restore_bundle(text: &str) -> Result<usize, String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|_| "this is not a TubeGlass backup".to_string())?;
    if v["app"] != "tubeglass" {
        return Err("this is not a TubeGlass backup".into());
    }
    let files = v["files"].as_object().ok_or("the backup has no data")?;
    let mut n = 0;
    for (name, content) in files {
        // only the known files, never a path from the backup
        if !BACKUP_FILES.contains(&name.as_str()) {
            continue;
        }
        let p = small_list_path(name).ok_or("no config folder")?;
        std::fs::write(p, serde_json::to_vec(content).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        n += 1;
    }
    if n == 0 {
        return Err("the backup is empty".into());
    }
    Ok(n)
}

/// Quick health check of every kind of request the app depends on.
pub(crate) fn run_diagnostics() -> Vec<(String, bool, String)> {
    let mut out = vec![];
    let mut check = |name: &str, r: Result<String, String>| {
        out.push(match r {
            Ok(note) => (name.to_string(), true, note),
            Err(e) => (name.to_string(), false, e),
        });
    };
    check("Search", yt::search_videos("music", Default::default()).map(|v| format!("{} results", v.len())));
    check(
        "Playback (video streams)",
        yt::streams("jNQXAC9IVRw").map(|s| format!("{} video + {} captions", s.video.len(), s.captions.len())),
    );
    check("Comments", yt::comments_page("jNQXAC9IVRw", None).map(|c| format!("{} comments", c.list.len())));
    check("Channel pages", yt::resolve_channel("@YouTube").and_then(|c| yt::channel_videos(&c)).map(|v| format!("{} videos", v.len())));
    check("YouTube Music", ytmusic::suggestions("daft").map(|s| format!("{} suggestions", s.len())));
    check("Lyrics", ytmusic::lyrics("", "Bohemian Rhapsody", "Queen", 355.0).map(|l| format!("{} lines", l.lines.len())));
    out
}
