//! Spotify import: turns pasted Spotify track links into YouTube Music songs. Each link's title,
//! artists and length come from Spotify's public embed page (no account), and the song is the
//! best YouTube Music search result by title, artist and length.

use crate::*;
use serde_json::Value;

/// One Spotify track as read from its embed page.
pub struct Track {
    pub title: String,
    pub artists: Vec<String>,
    /// length in seconds (0 when unknown)
    pub secs: u64,
}

/// The track ids in a block of pasted text, in order, without repeats.
pub fn track_ids(text: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for part in text.split("open.spotify.com/").skip(1) {
        let rest = part.strip_prefix("intl-").and_then(|p| p.split_once('/')).map(|(_, r)| r).unwrap_or(part);
        if let Some(id) = rest.strip_prefix("track/") {
            let id: String = id.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
            if id.len() == 22 && !out.contains(&id) {
                out.push(id);
            }
        }
    }
    out
}

/// Reads a track's title, artists and length from its public embed page.
pub fn track(id: &str) -> Result<Track, String> {
    let html = yt::plain_get(&format!("https://open.spotify.com/embed/track/{id}"))?;
    let start = html.find("id=\"__NEXT_DATA__\"").ok_or("Spotify page had no track data")?;
    let json_start = html[start..].find('>').map(|i| start + i + 1).ok_or("bad Spotify page")?;
    let json_end = html[json_start..].find("</script>").map(|i| json_start + i).ok_or("bad Spotify page")?;
    let v: Value = serde_json::from_str(&html[json_start..json_end]).map_err(|e| e.to_string())?;
    let e = v.pointer("/props/pageProps/state/data/entity").ok_or("Spotify track not found")?;
    let title = e.get("name").or_else(|| e.get("title")).and_then(Value::as_str).unwrap_or("").to_string();
    if title.is_empty() {
        return Err("Spotify track has no title".into());
    }
    let artists = e
        .get("artists")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.get("name").and_then(Value::as_str).map(String::from)).collect())
        .unwrap_or_default();
    let secs = e.get("duration").and_then(Value::as_u64).unwrap_or(0) / 1000;
    Ok(Track { title, artists, secs })
}

/// Lowercase words of a title or name, without brackets, "feat." credits or a "- Live" suffix.
fn words(s: &str) -> Vec<String> {
    let mut t = String::new();
    let mut depth = 0u32;
    for c in s.to_lowercase().chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => t.push(c),
            _ => {}
        }
    }
    let t = t.split(" - ").next().unwrap_or("").split("feat").next().unwrap_or("").to_string();
    t.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(String::from).collect()
}

/// Share of words two strings have in common (0 to 1).
fn overlap(a: &str, b: &str) -> f32 {
    let (a, b) = (words(a), words(b));
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let n = a.iter().filter(|w| b.contains(w)).count();
    n as f32 / a.len().max(b.len()) as f32
}

/// "4:12" or "1:02:03" as seconds.
fn secs_of(d: &str) -> Option<u64> {
    d.split(':').try_fold(0u64, |t, x| x.trim().parse::<u64>().ok().map(|n| t * 60 + n))
}

/// How well a YouTube Music song fits a Spotify track: title, artist and length.
pub fn score(t: &Track, v: &Video) -> f32 {
    let title = overlap(&t.title, &v.title);
    let artist = t
        .artists
        .iter()
        .map(|a| if v.channel_name.to_lowercase().contains(&a.to_lowercase()) { 1.0 } else { overlap(a, &v.channel_name) })
        .fold(0.0, f32::max);
    let length = match secs_of(&v.duration) {
        Some(s) if t.secs > 0 => match s.abs_diff(t.secs) {
            0..=3 => 1.0,
            4..=10 => 0.7,
            11..=30 => 0.3,
            _ => 0.0,
        },
        _ => 0.0,
    };
    title * 0.5 + artist * 0.35 + length * 0.15
}

/// The confident YouTube Music match for a Spotify track, or why there is none.
pub fn find(id: &str) -> Result<Video, String> {
    let t = track(id)?;
    let q = format!("{} {}", t.title, t.artists.first().map(String::as_str).unwrap_or(""));
    let res = ytmusic::search(&q, ytmusic::SearchKind::Songs)?;
    let best = res.songs.into_iter().take(8).map(|v| (score(&t, &v), v)).max_by(|a, b| a.0.total_cmp(&b.0));
    match best {
        Some((s, v)) if s >= 0.8 => Ok(v),
        _ => Err(format!("{} — {}", t.title, t.artists.join(", "))),
    }
}

#[cfg(test)]
mod spotify_tests {
    use super::*;

    #[test]
    fn finds_track_ids_in_pasted_links() {
        let text = "https://open.spotify.com/track/17o5FKq21SxalO0fcSADE6\r\nhttps://open.spotify.com/intl-de/track/6RBG17wfpJ8MWCTJsLJ7sw?si=x\nhttps://open.spotify.com/album/17o5FKq21SxalO0fcSADE7\nhttps://open.spotify.com/track/17o5FKq21SxalO0fcSADE6";
        assert_eq!(track_ids(text), vec!["17o5FKq21SxalO0fcSADE6", "6RBG17wfpJ8MWCTJsLJ7sw"]);
    }

    #[test]
    fn scores_the_right_song_highest() {
        let t = Track { title: "Hold Of Me - Live".into(), artists: vec!["James Wilson".into()], secs: 442 };
        let song = |title: &str, artist: &str, d: &str| Video {
            id: "x".into(), title: title.into(), thumb: String::new(), duration: d.into(), meta: String::new(),
            kind: Kind::Video, channel_id: String::new(), channel_name: artist.into(), rank: 0, short: false, music: true,
        };
        assert!(score(&t, &song("Hold Of Me (Live)", "James Wilson, Jaaken Jackson", "7:22")) >= 0.8);
        assert!(score(&t, &song("Hold On", "Someone Else", "3:10")) < 0.8);
    }
}
