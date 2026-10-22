//! Reading podcast feeds: RSS parsing, dates and durations, transcripts (SRT / WebVTT / JSON) and directory search.

use crate::podcasts::*;

// ---------------------------------------------------------------------- parsing

/// Days since 1970-01-01 of a calendar date.
pub(crate) fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// "Tue, 10 Mar 2020 08:00:00 +0200" (RFC 2822) or "2020-03-10T08:00:00Z" → unix seconds.
pub fn parse_date(s: &str) -> u64 {
    parse_date_opt(s).unwrap_or(0)
}

/// Parses an RSS (RFC 2822) or ISO date to unix seconds.
pub(crate) fn parse_date_opt(s: &str) -> Option<u64> {
    let s = s.trim();
    let hms = |t: &str| -> Option<i64> {
        let mut it = t.split(':').map(|x| x.parse::<i64>().ok());
        Some(it.next()?? * 3600 + it.next().flatten().unwrap_or(0) * 60 + it.next().flatten().unwrap_or(0))
    };
    let (days, secs, tz) = if s.as_bytes().get(4) == Some(&b'-') {
        let (d, t) = s.split_once(['T', ' ']).unwrap_or((s, "00:00:00"));
        let mut dp = d.split('-').map(|x| x.parse::<i64>().ok());
        let (y, m, dd) = (dp.next()??, dp.next()??, dp.next()??);
        let t = t.trim_end_matches('Z');
        let (t, tz) = match t.find(['+', '-']) {
            Some(i) => (&t[..i], tz_secs(&t[i..])),
            None => (t, 0),
        };
        (days_from_civil(y, m, dd), hms(t)?, tz)
    } else {
        let s = s.split_once(',').map_or(s, |x| x.1).trim();
        let p: Vec<&str> = s.split_whitespace().collect();
        if p.len() < 4 {
            return None;
        }
        let d: i64 = p[0].parse().ok()?;
        let months = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
        let m = months.iter().position(|m| p[1].to_lowercase().starts_with(m))? as i64 + 1;
        let mut y: i64 = p[2].parse().ok()?;
        if y < 100 {
            y += if y < 70 { 2000 } else { 1900 };
        }
        (days_from_civil(y, m, d), hms(p[3])?, p.get(4).map_or(0, |z| tz_secs(z)))
    };
    let t = days * 86_400 + secs - tz;
    (t > 0).then_some(t as u64)
}

/// Seconds offset of a "+0200" style time zone.
pub(crate) fn tz_secs(z: &str) -> i64 {
    let sign = match z.as_bytes().first() {
        Some(b'+') => 1,
        Some(b'-') => -1,
        _ => return 0,
    };
    let d: String = z[1..].chars().filter(|c| c.is_ascii_digit()).collect();
    if d.len() < 4 {
        return 0;
    }
    sign * (d[..2].parse::<i64>().unwrap_or(0) * 3600 + d[2..4].parse::<i64>().unwrap_or(0) * 60)
}

/// "1:02:03", "45:12", "2723" or "2723.5" → seconds.
pub fn parse_duration(s: &str) -> f64 {
    let mut total = 0.0;
    for part in s.trim().split(':') {
        match part.trim().parse::<f64>() {
            Ok(v) => total = total * 60.0 + v,
            Err(_) => return 0.0,
        }
    }
    total
}

/// Feeds often double-encode their text ("&lt;b&gt;" inside a CDATA block), so the usual
/// entities are decoded before and after the tags are removed.
pub(crate) fn decode_entities(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&mdash;", "—")
        .replace("&ndash;", "–")
        .replace("&hellip;", "…")
        .replace("&amp;", "&")
}

/// Removes tags and entities from feed text.
pub(crate) fn strip_html(s: &str) -> String {
    let s = &decode_entities(s);
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    decode_entities(&out).split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Text of the first child element with this name.
pub(crate) fn text_of(n: roxmltree::Node, name: &str) -> String {
    n.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
        .map(|c| c.text().unwrap_or("").trim().to_string())
        .unwrap_or_default()
}

/// Cover image address of a show or an episode.
pub(crate) fn image_of(n: roxmltree::Node) -> String {
    for c in n.children().filter(|c| c.is_element()) {
        match c.tag_name().name() {
            "image" => {
                if let Some(h) = c.attribute("href") {
                    return h.to_string();
                }
                let u = text_of(c, "url");
                if !u.is_empty() {
                    return u;
                }
            }
            _ => {}
        }
    }
    String::new()
}

/// The best timed transcript an item offers (`<podcast:transcript url=… type=…>`); plain HTML or
/// text transcripts have no timing, so they are skipped.
pub(crate) fn transcript_of(item: roxmltree::Node) -> String {
    let rank = |t: &str, u: &str| -> Option<u8> {
        let t = t.to_lowercase();
        let u = u.to_lowercase();
        if t.contains("json") || u.ends_with(".json") {
            Some(0)
        } else if t.contains("vtt") || u.ends_with(".vtt") {
            Some(1)
        } else if t.contains("srt") || t.contains("subrip") || u.ends_with(".srt") {
            Some(2)
        } else {
            None
        }
    };
    item.children()
        .filter(|c| c.is_element() && c.tag_name().name() == "transcript")
        .filter_map(|c| {
            let url = c.attribute("url")?;
            Some((rank(c.attribute("type").unwrap_or(""), url)?, url.to_string()))
        })
        .min_by_key(|(r, _)| *r)
        .map(|(_, u)| u)
        .unwrap_or_default()
}

/// "00:01:02,500" / "01:02.500" / "62.5" → seconds.
pub(crate) fn clock(s: &str) -> Option<f64> {
    let s = s.trim().replace(',', ".");
    let mut total = 0.0;
    for part in s.split(':') {
        total = total * 60.0 + part.trim().parse::<f64>().ok()?;
    }
    Some(total)
}

/// SubRip or WebVTT text → cues.
pub fn parse_srt_vtt(text: &str) -> Vec<crate::yt::Cue> {
    let mut out = vec![];
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        if let Some((a, b)) = l.split_once("-->") {
            let (start, end) = (clock(a), clock(b.split_whitespace().next().unwrap_or("")));
            let mut body = vec![];
            i += 1;
            while i < lines.len() && !lines[i].trim().is_empty() {
                body.push(lines[i].trim());
                i += 1;
            }
            if let (Some(start), Some(end)) = (start, end) {
                let t = strip_html(&body.join(" "));
                if !t.is_empty() {
                    out.push(crate::yt::Cue { start, end, text: t });
                }
            }
        }
        i += 1;
    }
    out
}

/// The Podcasting 2.0 JSON transcript: `{"segments":[{"startTime":…,"endTime":…,"body":"…"}]}`.
pub fn parse_transcript_json(text: &str) -> Vec<crate::yt::Cue> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { return vec![] };
    v["segments"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| {
            let body = s["body"].as_str()?.trim();
            (!body.is_empty()).then(|| crate::yt::Cue {
                start: s["startTime"].as_f64().unwrap_or(0.0),
                end: s["endTime"].as_f64().unwrap_or(0.0),
                text: body.to_string(),
            })
        })
        .collect()
}

/// Download and read a transcript, whichever of the supported formats it is in.
pub fn fetch_transcript(url: &str) -> Result<Vec<crate::yt::Cue>, String> {
    let body = crate::yt::plain_get(url)?;
    let mut cues = if body.trim_start().starts_with('{') { parse_transcript_json(&body) } else { parse_srt_vtt(&body) };
    cues.sort_by(|a, b| a.start.total_cmp(&b.start));
    // merge a speaker's tiny fragments so the list reads as sentences
    if cues.is_empty() {
        return Err("the transcript could not be read".into());
    }
    Ok(cues)
}

/// Reads an RSS document into a show and its episodes (items without audio are skipped).
pub fn parse_feed(feed_url: &str, xml: &str) -> Result<(Show, Vec<Episode>), String> {
    let doc = roxmltree::Document::parse_with_options(xml, roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() })
        .map_err(|_| "this address is not a podcast feed".to_string())?;
    let channel = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "channel")
        .ok_or("this address is not a podcast feed")?;
    let mut author = text_of(channel, "author");
    if author.is_empty() {
        author = text_of(channel, "owner");
    }
    let show = Show {
        feed: feed_url.to_string(),
        title: strip_html(&text_of(channel, "title")),
        author,
        image: image_of(channel),
        description: strip_html(&{
            let d = text_of(channel, "description");
            if d.is_empty() { text_of(channel, "summary") } else { d }
        })
        .chars()
        .take(500)
        .collect(),
    };
    let mut episodes = vec![];
    for item in channel.children().filter(|c| c.is_element() && c.tag_name().name() == "item") {
        let Some(enc) = item.children().find(|c| c.is_element() && c.tag_name().name() == "enclosure") else { continue };
        let Some(url) = enc.attribute("url").map(|u| u.trim().to_string()).filter(|u| u.starts_with("http")) else { continue };
        let kind = enc.attribute("type").unwrap_or("");
        if !kind.is_empty() && !kind.starts_with("audio") && !kind.starts_with("video") && kind != "application/octet-stream" {
            continue;
        }
        let guid = text_of(item, "guid");
        let key = if guid.is_empty() { url.clone() } else { guid };
        let mut summary = strip_html(&text_of(item, "summary"));
        if summary.is_empty() {
            summary = strip_html(&text_of(item, "description"));
        }
        episodes.push(Episode {
            id: short_id(&format!("{feed_url}|{key}")),
            title: strip_html(&text_of(item, "title")),
            url,
            duration: parse_duration(&text_of(item, "duration")),
            published: parse_date(&text_of(item, "pubDate")),
            summary: summary.chars().take(300).collect(),
            image: image_of(item),
            transcript: transcript_of(item),
        });
    }
    if episodes.is_empty() {
        return Err("this feed has no episodes".into());
    }
    Ok((show, episodes))
}

/// Downloads and reads a podcast feed.
pub fn fetch(feed: &str) -> Result<(Show, Vec<Episode>), String> {
    let body = crate::yt::plain_get(feed)?;
    parse_feed(feed, &body)
}

/// Percent-encodes text for a URL.
pub(crate) fn urlenc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Podcast directory search (Apple's public iTunes Search API, no account or key).
pub fn search(q: &str) -> Result<Vec<Hit>, String> {
    let body = crate::yt::plain_get(&format!("https://itunes.apple.com/search?media=podcast&entity=podcast&limit=15&term={}", urlenc(q)))?;
    hits_from(&body)
}

/// Reads the directory's JSON search answer.
pub(crate) fn hits_from(body: &str) -> Result<Vec<Hit>, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    Ok(v["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            Some(Hit {
                title: r["collectionName"].as_str()?.to_string(),
                author: r["artistName"].as_str().unwrap_or("").to_string(),
                feed: r["feedUrl"].as_str()?.to_string(),
                image: r["artworkUrl600"].as_str().or_else(|| r["artworkUrl100"].as_str()).unwrap_or("").to_string(),
            })
        })
        .collect())
}

/// Resolves what the user typed: a feed address, an Apple Podcasts link, or search words.
pub enum Input {
    Feed(String),
    Search(String),
}

/// Decides whether typed text is a feed address or search words.
pub fn classify(text: &str) -> Input {
    let t = text.trim();
    let t = t.strip_prefix("feed://").map(|r| format!("https://{r}")).unwrap_or_else(|| t.to_string());
    if t.starts_with("http://") || t.starts_with("https://") {
        Input::Feed(t)
    } else {
        Input::Search(t)
    }
}

/// An Apple Podcasts page link carries an id; the directory knows that show's feed.
pub fn resolve_link(url: &str) -> Result<String, String> {
    if url.contains("podcasts.apple.com") {
        let id = url
            .rsplit("/id")
            .next()
            .map(|r| r.chars().take_while(|c| c.is_ascii_digit()).collect::<String>())
            .filter(|d| !d.is_empty())
            .ok_or("could not read the podcast id from that link")?;
        let body = crate::yt::plain_get(&format!("https://itunes.apple.com/lookup?id={id}"))?;
        return hits_from(&body)?.into_iter().next().map(|h| h.feed).ok_or_else(|| "that podcast has no public feed".to_string());
    }
    Ok(url.to_string())
}
