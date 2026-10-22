//! YouTube Music (no account): search, album / artist / playlist pages and the home shelves.
//! Songs are ordinary YouTube videos, so playback reuses the normal player.

use crate::yt::{Kind, Res, Video, music_post, plain_get};
use serde_json::{Value, json};

/// What a Music card opens.
#[derive(Clone, Debug, PartialEq)]
pub enum CardKind {
    Album,
    Artist,
    Playlist,
    Song,
}

/// A square card: album, artist, playlist, or a playable video.
#[derive(Clone, Debug)]
pub struct Card {
    pub kind: CardKind,
    /// browse id (album / artist / playlist) or video id (song)
    pub id: String,
    pub title: String,
    pub sub: String,
    pub thumb: String,
}

/// Music search results grouped by kind.
#[derive(Clone, Debug, Default)]
pub struct Results {
    pub songs: Vec<Video>,
    pub cards: Vec<Card>,
}

/// An album, playlist or artist page.
#[derive(Clone, Debug, Default)]
pub struct Page {
    pub title: String,
    pub sub: String,
    pub thumb: String,
    pub tracks: Vec<Video>,
    pub shelves: Vec<(String, Vec<Card>)>,
}

/// True for text like "3:45".
fn is_duration(s: &str) -> bool {
    let p: Vec<&str> = s.split(':').collect();
    (p.len() == 2 || p.len() == 3) && p.iter().all(|x| !x.is_empty() && x.chars().all(|c| c.is_ascii_digit()))
}

/// Asks for a bigger version of a thumbnail.
fn sharp(url: &str, px: u32) -> String {
    // yt3 / lh3 thumbnails encode their size as =w60-h60-…
    if let Some(i) = url.find("=w") {
        let tail = &url[i..];
        if let Some(j) = tail.find("-h") {
            let rest = &tail[j + 2..];
            let k = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
            return format!("{}=w{px}-h{px}{}", &url[..i], &rest[k..]);
        }
    }
    url.to_string()
}

/// The largest thumbnail in a list.
fn best_thumb(v: &Value) -> String {
    let u = v
        .pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails")
        .or_else(|| v.pointer("/thumbnailRenderer/musicThumbnailRenderer/thumbnail/thumbnails"))
        .or_else(|| v.pointer("/musicThumbnailRenderer/thumbnail/thumbnails"))
        .and_then(Value::as_array)
        .and_then(|a| a.last())
        .and_then(|t| t["url"].as_str())
        .unwrap_or("");
    if u.contains("googleusercontent.com") { sharp(u, 400) } else { u.to_string() }
}

/// Joins the text runs of a title.
fn runs_text(v: &Value) -> String {
    v["runs"].as_array().into_iter().flatten().filter_map(|r| r["text"].as_str()).collect()
}

/// Page type of a browse link.
fn page_type(ep: &Value) -> &str {
    ep.pointer("/browseEndpoint/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType")
        .and_then(Value::as_str)
        .unwrap_or("")
}

/// Video id of a song item.
fn video_id(it: &Value) -> Option<String> {
    it.pointer("/playlistItemData/videoId")
        .and_then(Value::as_str)
        .or_else(|| it.pointer("/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/navigationEndpoint/watchEndpoint/videoId").and_then(Value::as_str))
        .or_else(|| it.pointer("/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint/watchEndpoint/videoId").and_then(Value::as_str))
        .map(String::from)
}

/// Builds a Video for a song.
fn song_video(id: &str, title: String, artist: String, artist_id: String, album: String, duration: String, thumb: String) -> Video {
    Video {
        id: id.to_string(),
        title,
        thumb: if thumb.is_empty() { format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg") } else { thumb },
        duration,
        meta: album,
        kind: Kind::Video,
        channel_id: artist_id,
        channel_name: artist,
        rank: 0,
        short: false,
        music: true,
    }
}

/// A list row. `fallback_thumb` / `fallback_artist` fill in what album pages leave out.
fn parse_row(it: &Value, fallback_thumb: &str, fallback_artist: &str) -> Option<Result2> {
    let cols: Vec<&Value> = it["flexColumns"]
        .as_array()?
        .iter()
        .filter_map(|c| c.get("musicResponsiveListItemFlexColumnRenderer"))
        .collect();
    let title = runs_text(cols.first()?.get("text")?);
    if title.is_empty() {
        return None;
    }
    let thumb = {
        let t = best_thumb(it);
        if t.is_empty() { fallback_thumb.to_string() } else { t }
    };
    if let Some(id) = video_id(it) {
        let (mut artist, mut artist_id, mut album, mut dur) = (String::new(), String::new(), String::new(), String::new());
        for col in cols.iter().skip(1) {
            for r in col["text"]["runs"].as_array().into_iter().flatten() {
                let t = r["text"].as_str().unwrap_or("");
                if t.trim() == "•" || t.trim().is_empty() || t.trim() == "&" || t.trim() == "," {
                    continue;
                }
                match page_type(&r["navigationEndpoint"]) {
                    "MUSIC_PAGE_TYPE_ARTIST" | "MUSIC_PAGE_TYPE_USER_CHANNEL" => {
                        if artist.is_empty() {
                            artist_id = r["navigationEndpoint"]["browseEndpoint"]["browseId"].as_str().unwrap_or("").to_string();
                            artist = t.to_string();
                        } else {
                            artist.push_str(&format!(", {t}"));
                        }
                    }
                    "MUSIC_PAGE_TYPE_ALBUM" => album = t.to_string(),
                    _ if is_duration(t) => dur = t.to_string(),
                    _ => {}
                }
            }
        }
        if artist.is_empty() {
            // plain-text columns (album pages): first non-numeric run of column 1
            artist = cols
                .get(1)
                .map(|c| runs_text(&c["text"]))
                .filter(|t| !t.contains(" plays") && !is_duration(t))
                .unwrap_or_default();
        }
        if artist.is_empty() {
            artist = fallback_artist.to_string();
        }
        if dur.is_empty() {
            dur = it
                .pointer("/fixedColumns/0/musicResponsiveListItemFixedColumnRenderer/text/runs/0/text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
        }
        return Some(Result2::Song(song_video(&id, title, artist, artist_id, album, dur, thumb)));
    }
    let nav = &it["navigationEndpoint"];
    let browse = nav["browseEndpoint"]["browseId"].as_str()?.to_string();
    let kind = match page_type(nav) {
        "MUSIC_PAGE_TYPE_ALBUM" | "MUSIC_PAGE_TYPE_AUDIOBOOK" => CardKind::Album,
        "MUSIC_PAGE_TYPE_ARTIST" | "MUSIC_PAGE_TYPE_USER_CHANNEL" => CardKind::Artist,
        "MUSIC_PAGE_TYPE_PLAYLIST" => CardKind::Playlist,
        _ => return None,
    };
    let sub = cols.get(1).map(|c| runs_text(&c["text"])).unwrap_or_default();
    Some(Result2::Card(Card { kind, id: browse, title, sub, thumb }))
}

/// A parsed search item.
enum Result2 {
    Song(Video),
    Card(Card),
}

/// Reads a card from the two-row layout.
fn parse_two_row(it: &Value) -> Option<Result2> {
    let title = runs_text(&it["title"]);
    let sub = runs_text(&it["subtitle"]);
    let thumb = best_thumb(it);
    if let Some(id) = it
        .pointer("/navigationEndpoint/watchEndpoint/videoId")
        .and_then(Value::as_str)
        .or_else(|| it.pointer("/doubleTapNavigationEndpoint/watchEndpoint/videoId").and_then(Value::as_str))
    {
        let artist = sub.split(" • ").next().unwrap_or("").to_string();
        return Some(Result2::Song(song_video(id, title, artist, String::new(), String::new(), String::new(), thumb)));
    }
    let nav = &it["navigationEndpoint"];
    let id = nav["browseEndpoint"]["browseId"].as_str()?.to_string();
    let kind = match page_type(nav) {
        "MUSIC_PAGE_TYPE_ALBUM" => CardKind::Album,
        "MUSIC_PAGE_TYPE_ARTIST" | "MUSIC_PAGE_TYPE_USER_CHANNEL" => CardKind::Artist,
        "MUSIC_PAGE_TYPE_PLAYLIST" => CardKind::Playlist,
        _ => return None,
    };
    Some(Result2::Card(Card { kind, id, title, sub, thumb }))
}

/// Collects the rows of a response.
fn collect_rows(v: &Value, thumb: &str, artist: &str, out: &mut Vec<Result2>) {
    match v {
        Value::Object(m) => {
            if let Some(r) = m.get("musicResponsiveListItemRenderer") {
                if let Some(x) = parse_row(r, thumb, artist) {
                    out.push(x);
                }
            } else {
                m.values().for_each(|c| collect_rows(c, thumb, artist, out));
            }
        }
        Value::Array(a) => a.iter().for_each(|c| collect_rows(c, thumb, artist, out)),
        _ => {}
    }
}

/// Collects the shelves of the home page.
fn collect_shelves(v: &Value, out: &mut Vec<(String, Vec<Card>, Vec<Video>)>) {
    match v {
        Value::Object(m) => {
            if let Some(c) = m.get("musicCarouselShelfRenderer") {
                let title = c
                    .pointer("/header/musicCarouselShelfBasicHeaderRenderer/title")
                    .map(runs_text)
                    .unwrap_or_default();
                let (mut cards, mut songs) = (vec![], vec![]);
                for it in c["contents"].as_array().into_iter().flatten() {
                    let x = if let Some(t) = it.get("musicTwoRowItemRenderer") {
                        parse_two_row(t)
                    } else if let Some(r) = it.get("musicResponsiveListItemRenderer") {
                        parse_row(r, "", "")
                    } else {
                        None
                    };
                    match x {
                        Some(Result2::Card(c)) => cards.push(c),
                        Some(Result2::Song(s)) => songs.push(s),
                        None => {}
                    }
                }
                if !title.is_empty() && (!cards.is_empty() || !songs.is_empty()) {
                    out.push((title, cards, songs));
                }
            } else {
                m.values().for_each(|c| collect_shelves(c, out));
            }
        }
        Value::Array(a) => a.iter().for_each(|c| collect_shelves(c, out)),
        _ => {}
    }
}

/// A card for a song.
pub fn song_card(v: &Video) -> Card {
    Card { kind: CardKind::Song, id: v.id.clone(), title: v.title.clone(), sub: v.channel_name.clone(), thumb: v.thumb.clone() }
}

/// What the Music search looks for.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SearchKind {
    Songs,
    Albums,
    Artists,
    Playlists,
}

/// Searches YouTube Music.
pub fn search(query: &str, kind: SearchKind) -> Res<Results> {
    let params = match kind {
        SearchKind::Songs => "EgWKAQIIAWoMEA4QChADEAQQCRAF",
        SearchKind::Albums => "EgWKAQIYAWoMEA4QChADEAQQCRAF",
        SearchKind::Artists => "EgWKAQIgAWoMEA4QChADEAQQCRAF",
        SearchKind::Playlists => "EgWKAQIoAWoMEA4QChADEAQQCRAF",
    };
    let q = query.trim();
    if q.is_empty() {
        return Err("type something to search for".into());
    }
    let resp = music_post("search", json!({ "query": q, "params": params }))?;
    let mut rows = vec![];
    collect_rows(&resp, "", "", &mut rows);
    let mut out = Results::default();
    for r in rows {
        match r {
            Result2::Song(s) => {
                if !out.songs.iter().any(|x| x.id == s.id) {
                    out.songs.push(s);
                }
            }
            Result2::Card(c) => out.cards.push(c),
        }
    }
    Ok(out)
}

/// An album or playlist page (a header plus its track list).
pub fn tracklist(browse_id: &str) -> Res<Page> {
    let resp = music_post("browse", json!({ "browseId": browse_id }))?;
    let mut page = Page::default();
    let header = ["musicResponsiveHeaderRenderer", "musicDetailHeaderRenderer", "musicEditablePlaylistDetailHeaderRenderer"]
        .iter()
        .find_map(|k| find_key(&resp, k));
    if let Some(h) = header {
        let h = h.get("header").and_then(|x| x.get("musicDetailHeaderRenderer")).unwrap_or(h);
        page.title = runs_text(&h["title"]);
        page.sub = h.get("subtitle").map(runs_text).unwrap_or_default();
        if let Some(s) = h.get("straplineTextOne") {
            let a = runs_text(s);
            if !a.is_empty() && !page.sub.contains(&a) {
                page.sub = format!("{a}  •  {}", page.sub);
            }
        }
        page.thumb = best_thumb(h);
    }
    let artist = page.sub.split(" • ").nth(1).unwrap_or("").to_string();
    let mut rows = vec![];
    collect_rows(&resp, &page.thumb, &artist, &mut rows);
    for r in rows {
        if let Result2::Song(s) = r {
            if !page.tracks.iter().any(|x| x.id == s.id) {
                page.tracks.push(s);
            }
        }
    }
    if page.tracks.is_empty() {
        return Err("no tracks found".into());
    }
    if page.title.is_empty() {
        page.title = "Playlist".into();
    }
    Ok(page)
}

/// Finds the first value under a key.
fn find_key<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    match v {
        Value::Object(m) => m.get(key).or_else(|| m.values().find_map(|c| find_key(c, key))),
        Value::Array(a) => a.iter().find_map(|c| find_key(c, key)),
        _ => None,
    }
}

/// Opens an artist page.
pub fn artist(browse_id: &str) -> Res<Page> {
    let resp = music_post("browse", json!({ "browseId": browse_id }))?;
    let mut page = Page::default();
    if let Some(h) = find_key(&resp, "musicImmersiveHeaderRenderer").or_else(|| find_key(&resp, "musicVisualHeaderRenderer")) {
        page.title = runs_text(&h["title"]);
        page.sub = h
            .pointer("/subscriptionButton/subscribeButtonRenderer/subscriberCountText")
            .map(runs_text)
            .map(|s| format!("{s} subscribers"))
            .unwrap_or_default();
        page.thumb = best_thumb(h);
    }
    let mut rows = vec![];
    // the "Songs" shelf is a musicShelfRenderer of list rows
    if let Some(shelf) = find_key(&resp, "musicShelfRenderer") {
        collect_rows(shelf, &page.thumb, &page.title, &mut rows);
    }
    for r in rows {
        if let Result2::Song(s) = r {
            page.tracks.push(s);
        }
    }
    let mut shelves = vec![];
    collect_shelves(&resp, &mut shelves);
    for (t, mut cards, songs) in shelves {
        cards.extend(songs.iter().map(song_card));
        page.shelves.push((t, cards));
    }
    if page.title.is_empty() && page.tracks.is_empty() && page.shelves.is_empty() {
        return Err("artist page is empty".into());
    }
    Ok(page)
}

/// Music home: charts, new releases and the "quick picks" shelves.
pub fn home() -> Res<Page> {
    let mut page = Page { title: "Music".into(), ..Default::default() };
    let mut ok = false;
    for id in ["FEmusic_home", "FEmusic_charts", "FEmusic_new_releases_albums"] {
        let Ok(resp) = music_post("browse", json!({ "browseId": id })) else { continue };
        ok = true;
        let mut shelves = vec![];
        collect_shelves(&resp, &mut shelves);
        // grids (new releases) are not carousels
        let mut grid = vec![];
        fn walk(v: &Value, out: &mut Vec<Card>) {
            match v {
                Value::Object(m) => {
                    if let Some(t) = m.get("musicTwoRowItemRenderer") {
                        if let Some(Result2::Card(c)) = parse_two_row(t) {
                            out.push(c);
                        }
                    } else {
                        m.values().for_each(|c| walk(c, out));
                    }
                }
                Value::Array(a) => a.iter().for_each(|c| walk(c, out)),
                _ => {}
            }
        }
        if shelves.is_empty() {
            walk(&resp, &mut grid);
            if !grid.is_empty() {
                page.shelves.push(("New releases".into(), grid));
            }
        }
        for (t, mut cards, songs) in shelves {
            if id == "FEmusic_charts" && t.to_lowercase().contains("video") {
                // chart entries are playable videos
            }
            cards.extend(songs.iter().map(song_card));
            if !cards.is_empty() && !page.shelves.iter().any(|(x, _)| x == &t) {
                page.shelves.push((t, cards));
            }
        }
    }
    if !ok {
        return Err("could not reach YouTube Music".into());
    }
    Ok(page)
}

/// Autocomplete for the music search box.
pub fn suggestions(q: &str) -> Res<Vec<String>> {
    let resp = music_post("music/get_search_suggestions", json!({ "input": q.trim() }))?;
    let mut out = vec![];
    /// Collects text values.
    fn walk(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(m) => {
                if let Some(r) = m.get("searchSuggestionRenderer") {
                    let t = runs_text(&r["suggestion"]);
                    if !t.is_empty() {
                        out.push(t);
                    }
                } else {
                    m.values().for_each(|c| walk(c, out));
                }
            }
            Value::Array(a) => a.iter().for_each(|c| walk(c, out)),
            _ => {}
        }
    }
    walk(&resp, &mut out);
    out.dedup();
    Ok(out)
}

/// Songs "like" `video_id` – YouTube Music's radio for that track. Needs no account.
pub fn radio(video_id: &str) -> Res<Vec<Video>> {
    let resp = music_post("next", json!({ "videoId": video_id, "playlistId": format!("RDAMVM{video_id}") }))?;
    let mut out: Vec<Video> = vec![];
    /// Collects songs, skipping one id.
    fn walk(v: &Value, out: &mut Vec<Video>, skip: &str) {
        match v {
            Value::Object(m) => {
                if let Some(r) = m.get("playlistPanelVideoRenderer") {
                    if let Some(id) = r["videoId"].as_str() {
                        if id != skip && !out.iter().any(|x| x.id == id) {
                            let title = runs_text(&r["title"]);
                            let by: Vec<String> = r["longBylineText"]["runs"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(|x| x["text"].as_str())
                                .map(String::from)
                                .collect();
                            let artist = by.first().cloned().unwrap_or_default();
                            let artist_id = r
                                .pointer("/longBylineText/runs/0/navigationEndpoint/browseEndpoint/browseId")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string();
                            let album = by.iter().skip(2).find(|t| t.as_str() != " • ").cloned().unwrap_or_default();
                            let dur = runs_text(&r["lengthText"]);
                            let thumb = r
                                .pointer("/thumbnail/thumbnails")
                                .and_then(Value::as_array)
                                .and_then(|a| a.last())
                                .and_then(|t| t["url"].as_str())
                                .map(|u| if u.contains("googleusercontent.com") { sharp(u, 400) } else { u.to_string() })
                                .unwrap_or_default();
                            out.push(song_video(id, title, artist, artist_id, album, dur, thumb));
                        }
                    }
                } else {
                    m.values().for_each(|c| walk(c, out, skip));
                }
            }
            Value::Array(a) => a.iter().for_each(|c| walk(c, out, skip)),
            _ => {}
        }
    }
    walk(&resp, &mut out, video_id);
    if out.is_empty() {
        return Err("no similar songs found".into());
    }
    Ok(out)
}

/// Lyrics, timed when available.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Lyrics {
    /// (start seconds, line); start is 0 for every line when the lyrics are not timed
    pub lines: Vec<(f64, String)>,
    pub synced: bool,
    pub source: String,
}

/// Parses LRC text ("[01:23.45] words"). Lines without a time stamp are dropped.
pub fn parse_lrc(lrc: &str) -> Vec<(f64, String)> {
    let mut out = vec![];
    for line in lrc.lines() {
        let mut rest = line.trim();
        let mut stamps = vec![];
        while let Some(r) = rest.strip_prefix('[') {
            let Some(end) = r.find(']') else { break };
            let tag = &r[..end];
            let t = tag.split_once(':').and_then(|(m, s)| Some(m.parse::<f64>().ok()? * 60.0 + s.parse::<f64>().ok()?));
            match t {
                Some(t) => stamps.push(t),
                None => {
                    // metadata tag such as [ar:Artist]
                    rest = &r[end + 1..];
                    stamps.clear();
                    break;
                }
            }
            rest = &r[end + 1..];
        }
        let text = rest.trim().to_string();
        for t in stamps {
            out.push((t, text.clone()));
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

/// Percent-encodes a query.
fn urlenc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            b' ' => "+".into(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Strips "(Official Audio)", "- Remastered" and similar from a title for lyric lookups.
fn clean_title(t: &str) -> String {
    let mut t = t.to_string();
    for open in ['(', '['] {
        while let Some(i) = t.find(open) {
            let close = if open == '(' { ')' } else { ']' };
            match t[i..].find(close) {
                Some(j) => t.replace_range(i..i + j + 1, ""),
                None => break,
            }
        }
    }
    t.split(" - ").next().unwrap_or("").trim().to_string()
}

/// Lyrics from LRCLIB (timed when available), falling back to YouTube Music's own text.
/// Lower-cased words of a title (letters and digits only).
fn words(s: &str) -> std::collections::HashSet<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_string())
        .collect()
}

/// Share of the shorter word set that also appears in the other (0 – 1).
fn overlap(a: &str, b: &str) -> f32 {
    let (wa, wb) = (words(a), words(b));
    let small = wa.len().min(wb.len());
    if small == 0 {
        return 0.0;
    }
    wa.intersection(&wb).count() as f32 / small as f32
}

/// Is this LRCLIB entry really the song we asked for? Stops a search from returning some
/// unrelated track when the video is not a song at all.
fn same_song(e: &Value, title: &str, artist: &str) -> bool {
    let name = e["trackName"].as_str().or_else(|| e["name"].as_str()).unwrap_or("");
    let by = e["artistName"].as_str().unwrap_or("");
    let t = overlap(name, title);
    let a = if artist.is_empty() { 0.0 } else { overlap(by, artist) };
    // the title has to match well; an artist match makes a looser title acceptable
    t >= 0.99 || (t >= 0.6 && a > 0.0)
}

/// Finds lyrics for a song (LRCLIB first, then YouTube Music), checking that the match is the right song.
pub fn lyrics(video_id: &str, title: &str, artist: &str, duration_secs: f64) -> Res<Lyrics> {
    let (t, a) = (clean_title(title), artist.split(',').next().unwrap_or("").trim().to_string());
    // title strings from "Artist - Song (Official Audio)" videos
    let t = t.rsplit(" – ").next().unwrap_or(&t).to_string();
    let t = match t.split_once(" - ") {
        Some((l, r)) if l.trim().eq_ignore_ascii_case(&a) => r.to_string(),
        _ => t,
    };
    let dur = if duration_secs > 0.0 { format!("&duration={}", duration_secs.round() as i64) } else { String::new() };
    let tries = [
        format!("https://lrclib.net/api/get?artist_name={}&track_name={}{dur}", urlenc(&a), urlenc(&t)),
        format!("https://lrclib.net/api/get?artist_name={}&track_name={}", urlenc(&a), urlenc(&t)),
        format!("https://lrclib.net/api/search?q={}", urlenc(&format!("{a} {t}"))),
    ];
    let mut transient = false;
    for u in tries {
        let body = match plain_get(&u) {
            Ok(b) => b,
            Err(_) => {
                transient = true;
                continue;
            }
        };
        let Ok(v) = serde_json::from_str::<Value>(&body) else { continue };
        // a search returns several versions: prefer timed lyrics, then the closest length
        let first = if let Some(list) = v.as_array() {
            list.iter()
                .filter(|e| e["instrumental"].as_bool() != Some(true) && same_song(e, &t, &a))
                .min_by(|a, b| {
                    let key = |e: &Value| {
                        let timed = e["syncedLyrics"].as_str().is_some_and(|s| !s.trim().is_empty());
                        let diff = if duration_secs > 0.0 {
                            (e["duration"].as_f64().unwrap_or(0.0) - duration_secs).abs()
                        } else {
                            0.0
                        };
                        (!timed, (diff * 10.0) as i64)
                    };
                    key(a).cmp(&key(b))
                })
                .cloned()
                .unwrap_or(Value::Null)
        } else if same_song(&v, &t, &a) {
            v
        } else {
            Value::Null
        };
        if let Some(l) = first["syncedLyrics"].as_str().filter(|s| !s.trim().is_empty()) {
            let lines = parse_lrc(l);
            if lines.len() > 2 {
                return Ok(Lyrics { lines, synced: true, source: "LRCLIB".into() });
            }
        }
        if let Some(p) = first["plainLyrics"].as_str().filter(|s| !s.trim().is_empty()) {
            let lines = p.lines().map(|l| (0.0, l.to_string())).collect();
            return Ok(Lyrics { lines, synced: false, source: "LRCLIB".into() });
        }
    }
    // YouTube Music's own lyrics
    if let Ok(n) = music_post("next", json!({ "videoId": video_id })) {
        let id = {
            fn find(v: &Value) -> Option<String> {
                match v {
                    Value::Object(m) => {
                        for (k, c) in m {
                            if k == "browseId" {
                                if let Some(s) = c.as_str().filter(|s| s.starts_with("MPLYt_")) {
                                    return Some(s.to_string());
                                }
                            }
                            if let Some(x) = find(c) {
                                return Some(x);
                            }
                        }
                        None
                    }
                    Value::Array(a) => a.iter().find_map(find),
                    _ => None,
                }
            }
            find(&n)
        };
        if let Some(id) = id {
            if let Ok(b) = music_post("browse", json!({ "browseId": id })) {
                if let Some(sh) = find_key(&b, "musicDescriptionShelfRenderer") {
                    let text = runs_text(&sh["description"]);
                    if !text.trim().is_empty() {
                        let lines = text.lines().map(|l| (0.0, l.to_string())).collect();
                        return Ok(Lyrics { lines, synced: false, source: "YouTube Music".into() });
                    }
                }
            }
        }
    }
    if transient {
        Err(RETRYABLE.into())
    } else {
        Err("no lyrics found for this song".into())
    }
}

/// Error text of a lookup that failed because the lyrics service could not be reached.
pub const RETRYABLE: &str = "the lyrics service is not responding";

/// A song card as a playable video (used when a shelf item is a track).
pub fn card_to_video(c: &Card) -> Video {
    song_video(&c.id, c.title.clone(), c.sub.split(" • ").next().unwrap_or("").to_string(), String::new(), String::new(), String::new(), c.thumb.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbnails_are_upscaled_and_durations_detected() {
        let u = "https://yt3.googleusercontent.com/abc=w60-h60-l90-rj";
        assert_eq!(sharp(u, 400), "https://yt3.googleusercontent.com/abc=w400-h400-l90-rj");
        assert!(is_duration("3:08") && is_duration("1:02:03") && !is_duration("98M plays"));
    }

    #[test]
    #[ignore = "needs network"]
    fn music_live() {
        let s = search("daft punk", SearchKind::Songs).unwrap();
        println!("MUSIC songs {} first={:?}", s.songs.len(), s.songs.first().map(|v| (&v.title, &v.channel_name, &v.meta, &v.duration)));
        let a = search("daft punk random access memories", SearchKind::Albums).unwrap();
        println!("MUSIC albums {} first={:?}", a.cards.len(), a.cards.first().map(|c| (&c.title, &c.sub, &c.id)));
        let ar = search("daft punk", SearchKind::Artists).unwrap();
        println!("MUSIC artists {} first={:?}", ar.cards.len(), ar.cards.first().map(|c| (&c.title, &c.id)));
        let pl = search("daft punk", SearchKind::Playlists).unwrap();
        println!("MUSIC playlists {}", pl.cards.len());
        let album = tracklist(&a.cards[0].id).unwrap();
        println!("MUSIC album {:?} / {:?} tracks={} first={:?}", album.title, album.sub, album.tracks.len(), album.tracks.first().map(|t| (&t.title, &t.channel_name, &t.duration)));
        let art = artist(&ar.cards[0].id).unwrap();
        println!("MUSIC artist {:?} {:?} songs={} shelves={:?}", art.title, art.sub, art.tracks.len(), art.shelves.iter().map(|(t, c)| (t.as_str(), c.len())).collect::<Vec<_>>());
        let h = home().unwrap();
        println!("MUSIC home shelves={:?}", h.shelves.iter().map(|(t, c)| (t.as_str(), c.len())).collect::<Vec<_>>());
    }

    #[test]
    fn lrc_parsing() {
        let l = parse_lrc("[ar:Daft Punk]
[00:12.50] Hello
[00:05.00][01:00.00] Chorus
[bad] x
");
        assert_eq!(l.len(), 3);
        assert_eq!(l[0], (5.0, "Chorus".to_string()));
        assert_eq!(l[1], (12.5, "Hello".to_string()));
        assert_eq!(l[2].0, 60.0);
    }

    #[test]
    fn titles_are_cleaned() {
        assert_eq!(clean_title("Get Lucky (Official Audio) [HD]"), "Get Lucky");
        assert_eq!(clean_title("Robot Rock - Remastered 2021"), "Robot Rock");
    }

    #[test]
    #[ignore = "needs network"]
    fn music_extras_live() {
        println!("SUGG {:?}", suggestions("daft pu").unwrap());
        let r = radio("IluRBvnYMoY").unwrap();
        println!("RADIO {} first={:?}", r.len(), r.first().map(|v| (&v.title, &v.channel_name, &v.duration)));
        let l = lyrics("0wTOi2TCSSk", "Get Lucky (Radio Edit - feat. Pharrell Williams and Nile Rodgers)", "Daft Punk", 248.0).unwrap();
        println!("LYRICS {} lines synced={} via {} first={:?}", l.lines.len(), l.synced, l.source, l.lines.first());
    }
}

#[cfg(test)]
mod match_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unrelated_tracks_are_rejected() {
        let e = json!({"trackName": "Bohemian Rhapsody", "artistName": "Queen"});
        assert!(same_song(&e, "Bohemian Rhapsody", "Queen"));
        assert!(same_song(&e, "Bohemian Rhapsody", ""));
        assert!(!same_song(&e, "How I Built a Rust Desktop App", "Mental Outlaw"));
        assert!(!same_song(&e, "Welcome To The Jungle", "Queen"));
    }
}
