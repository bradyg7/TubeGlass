//! Searching YouTube (videos, channels, playlists), suggestions, playlist contents and related videos.

use super::*;

/// Runs a search and returns the raw response.
pub(crate) fn search_data(query: &str, sp: &str) -> Res<Value> {
    let q: String = query
        .trim()
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            b' ' => "+".into(),
            _ => format!("%{b:02X}"),
        })
        .collect();
    if q.is_empty() {
        return Err("type something to search".into());
    }
    let html = get_text(&format!("https://www.youtube.com/results?search_query={q}&sp={sp}"))?;
    initial_data(&html)
}

/// Search filters. Values map to YouTube's own filter ids.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SearchOpts {
    /// 0 relevance, 1 rating, 2 upload date, 3 view count
    pub sort: u8,
    /// 0 any, 2 today, 3 this week, 4 this month, 5 this year
    pub date: u8,
    /// 0 any, 1 under 4 min, 3 4–20 min, 2 over 20 min
    pub duration: u8,
}

impl SearchOpts {
    /// The `sp` query value: protobuf { sort, filters { date, type = video, duration } }.
    pub fn sp(&self) -> String {
        let mut filter: Vec<u8> = vec![];
        if self.date != 0 {
            filter.extend([0x08, self.date]);
        }
        filter.extend([0x10, 0x01]); // type: video
        if self.duration != 0 {
            filter.extend([0x18, self.duration]);
        }
        let mut msg: Vec<u8> = vec![];
        if self.sort != 0 {
            msg.extend([0x08, self.sort]);
        }
        msg.push(0x12);
        msg.push(filter.len() as u8);
        msg.extend(&filter);
        msg.extend([0xF0, 0x01, 0x01]); // hide shorts / extras, as NewPipe sends
        b64(&msg).replace('=', "%3D")
    }
}

/// Searches videos.
pub fn search_videos(query: &str, opts: SearchOpts) -> Res<Vec<Video>> {
    let data = search_data(query, &opts.sp())?;
    let mut found = vec![];
    collect(&data, &mut found);
    let mut out: Vec<Video> = vec![];
    for mut v in found {
        if out.iter().any(|x| x.id == v.id) {
            continue;
        }
        v.rank = out.len();
        out.push(v);
    }
    Ok(out)
}

/// Searches channels.
pub fn search_channels(query: &str) -> Res<Vec<ChannelHit>> {
    let data = search_data(query, "EgIQAvABAQ%3D%3D")?;
    let mut out = vec![];
    /// Collects channel results from a response.
    fn walk(v: &Value, out: &mut Vec<ChannelHit>) {
        match v {
            Value::Object(m) => {
                if let Some(r) = m.get("channelRenderer") {
                    if let Some(id) = r["channelId"].as_str() {
                        let mut avatar = r
                            .pointer("/thumbnail/thumbnails/0/url")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        if avatar.starts_with("//") {
                            avatar = format!("https:{avatar}");
                        }
                        // YouTube swapped these two fields: the "subscriber" text is now the
                        // @handle and the "video count" text carries the subscriber count.
                        let a = text_of(&r["subscriberCountText"]);
                        let b = text_of(&r["videoCountText"]);
                        let (handle, subs) = if a.starts_with('@') { (a, b) } else { (b, a) };
                        out.push(ChannelHit {
                            channel: Channel { id: id.to_string(), name: text_of(&r["title"]), avatar },
                            handle,
                            subs,
                        });
                    }
                } else {
                    m.values().for_each(|c| walk(c, out));
                }
            }
            Value::Array(a) => a.iter().for_each(|c| walk(c, out)),
            _ => {}
        }
    }
    walk(&data, &mut out);
    Ok(out)
}

/// Search suggestions (same endpoint NewPipe's YoutubeSuggestionExtractor uses).
pub fn suggestions(q: &str) -> Res<Vec<String>> {
    let body = agent()
        .get(format!(
            "https://suggestqueries-clients6.youtube.com/complete/search?client=youtube&ds=yt&gl=US&q={}&xhr=t",
            urlenc(q)
        ))
        .header("Origin", "https://www.youtube.com")
        .header("Referer", "https://www.youtube.com")
        .header("User-Agent", WEB_UA)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    Ok(v[1]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s[0].as_str())
        .filter(|s| !s.trim().is_empty())
        .map(String::from)
        .collect())
}

/// Collects playlist results from a response.
pub(crate) fn collect_playlists(v: &Value, out: &mut Vec<PlaylistHit>) {
    match v {
        Value::Object(m) => {
            if let Some(r) = m.get("playlistRenderer") {
                if let Some(id) = r["playlistId"].as_str() {
                    let thumb = r
                        .pointer("/thumbnails/0/thumbnails")
                        .or_else(|| r.pointer("/thumbnail/thumbnails"))
                        .and_then(Value::as_array)
                        .and_then(|a| a.last())
                        .and_then(|t| t["url"].as_str())
                        .unwrap_or("")
                        .to_string();
                    let thumb = if thumb.starts_with("//") { format!("https:{thumb}") } else { thumb };
                    out.push(PlaylistHit {
                        id: id.to_string(),
                        title: text_of(&r["title"]),
                        thumb,
                        owner: text_of(&r["shortBylineText"]),
                        count: {
                            let c = text_of(&r["videoCountText"]);
                            if c.is_empty() { r["videoCount"].as_str().map(|n| format!("{n} videos")).unwrap_or_default() } else { c }
                        },
                    });
                }
            } else if let Some(l) = m.get("lockupViewModel") {
                if l["contentType"] == "LOCKUP_CONTENT_TYPE_PLAYLIST" {
                    if let (Some(id), Some(title)) = (
                        l["contentId"].as_str(),
                        l.pointer("/metadata/lockupMetadataViewModel/title/content").and_then(Value::as_str),
                    ) {
                        out.push(PlaylistHit {
                            id: id.to_string(),
                            title: title.to_string(),
                            thumb: l
                                .pointer("/contentImage/collectionThumbnailViewModel/primaryThumbnail/thumbnailViewModel/image/sources")
                                .and_then(Value::as_array)
                                .and_then(|a| a.last())
                                .and_then(|s| s["url"].as_str())
                                .unwrap_or("")
                                .to_string(),
                            owner: l
                                .pointer("/metadata/lockupMetadataViewModel/metadata/contentMetadataViewModel/metadataRows/0/metadataParts/0/text/content")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string(),
                            count: first_text(l, "thumbnailBadgeViewModel").unwrap_or_default(),
                        });
                    }
                }
            } else {
                m.values().for_each(|c| collect_playlists(c, out));
            }
        }
        Value::Array(a) => a.iter().for_each(|c| collect_playlists(c, out)),
        _ => {}
    }
}

/// Searches playlists.
pub fn search_playlists(query: &str) -> Res<Vec<PlaylistHit>> {
    let data = search_data(query, "EgIQA_ABAQ%3D%3D")?;
    let mut out = vec![];
    collect_playlists(&data, &mut out);
    Ok(out)
}

/// All videos of a playlist (first page, as served on the playlist page).
pub fn playlist_videos(id: &str) -> Res<(String, Vec<Video>)> {
    let html = get_text(&format!("https://www.youtube.com/playlist?list={}", urlenc(id)))?;
    let data = initial_data(&html)?;
    let title = data
        .pointer("/metadata/playlistMetadataRenderer/title")
        .and_then(Value::as_str)
        .unwrap_or("Playlist")
        .to_string();
    let mut out: Vec<Video> = vec![];
    let add = |found: Vec<Video>, out: &mut Vec<Video>| {
        for mut v in found {
            if out.iter().any(|x| x.id == v.id) {
                continue;
            }
            v.rank = out.len();
            out.push(v);
        }
    };
    let mut found = vec![];
    collect(&data, &mut found);
    add(found, &mut out);
    // follow the continuation so long playlists are complete (capped for safety)
    let mut token = last_continuation(&data);
    let mut pages = 0;
    while let (Some(t), true) = (token.take(), pages < 12 && out.len() < 1000) {
        pages += 1;
        let Ok(resp) = web_post("browse", json!({ "continuation": t })) else { break };
        let mut found = vec![];
        collect(&resp, &mut found);
        if found.is_empty() {
            break;
        }
        add(found, &mut out);
        token = last_continuation(&resp);
    }
    if out.is_empty() {
        return Err("this playlist is empty or unavailable".into());
    }
    Ok((title, out))
}

/// "Up next" style recommendations shown next to a video.
pub fn related(video_id: &str) -> Res<Vec<Video>> {
    let resp = web_post("next", json!({ "videoId": video_id }))?;
    let mut found = vec![];
    collect(&resp, &mut found);
    let mut out: Vec<Video> = vec![];
    for mut v in found {
        if v.id == video_id || out.iter().any(|x| x.id == v.id) {
            continue;
        }
        v.rank = out.len();
        out.push(v);
    }
    if out.is_empty() {
        return Err("no related videos found".into());
    }
    Ok(out)
}
