//! Subscription import / export: NewPipe JSON, YouTube Takeout CSV, OPML, or a plain list
//! of channel links / @handles.

use crate::yt::Channel;
use serde_json::{Value, json};

/// A channel found in an imported subscriptions file.
#[derive(Clone, Debug, PartialEq)]
pub struct Imported {
    /// A `UC…` channel id, or an `@handle` / link that still has to be resolved.
    pub key: String,
    pub name: String,
}

/// Extracts a channel id from a link or id text.
fn channel_id_from(s: &str) -> Option<String> {
    let find = |marker: &str| -> Option<String> {
        let i = s.find(marker)? + marker.len();
        let id: String = s[i..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').collect();
        (id.starts_with("UC") && id.len() == 24).then_some(id)
    };
    find("/channel/").or_else(|| find("channel_id=")).or_else(|| {
        let t = s.trim();
        (t.starts_with("UC") && t.len() == 24 && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'))
            .then(|| t.to_string())
    })
}

/// Decodes the basic XML entities.
fn unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

/// Value of an attribute inside a tag.
fn attr(tag: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let i = tag.find(&key)? + key.len();
    let j = tag[i..].find('"')?;
    Some(unescape(&tag[i..i + j]))
}

/// Splits one CSV line, honouring quotes.
fn parse_csv_line(line: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// Detects the format and returns the channels found (duplicates removed).
pub fn parse(text: &str) -> Vec<Imported> {
    let t = text.trim_start_matches('\u{feff}').trim();
    let mut out: Vec<Imported> = vec![];
    if t.starts_with('{') || t.starts_with('[') {
        if let Ok(v) = serde_json::from_str::<Value>(t) {
            let list = v.get("subscriptions").unwrap_or(&v);
            for e in list.as_array().into_iter().flatten() {
                let url = e["url"].as_str().or_else(|| e["channelUrl"].as_str()).unwrap_or("");
                let name = e["name"].as_str().or_else(|| e["title"].as_str()).unwrap_or("").to_string();
                if let Some(id) = channel_id_from(url).or_else(|| channel_id_from(e["id"].as_str().unwrap_or(""))) {
                    out.push(Imported { key: id, name });
                } else if url.contains("youtube.com/") && !url.is_empty() {
                    out.push(Imported { key: url.to_string(), name });
                }
            }
        }
    } else if t.to_ascii_lowercase().contains("<opml") {
        for chunk in t.split("<outline").skip(1) {
            let tag = chunk.split('>').next().unwrap_or("");
            let url = attr(tag, "xmlUrl").or_else(|| attr(tag, "htmlUrl")).unwrap_or_default();
            if let Some(id) = channel_id_from(&url) {
                let name = attr(tag, "title").or_else(|| attr(tag, "text")).unwrap_or_default();
                out.push(Imported { key: id, name });
            }
        }
    } else {
        let mut lines = t.lines();
        let first = lines.clone().next().unwrap_or("").to_ascii_lowercase();
        if first.contains("channel id") || first.contains("channel url") {
            lines.next();
            for l in lines {
                let f = parse_csv_line(l);
                let id = f.iter().find_map(|c| channel_id_from(c));
                let name = f.last().cloned().unwrap_or_default();
                if let Some(id) = id {
                    out.push(Imported { key: id, name: if f.len() > 2 { name } else { String::new() } });
                }
            }
        } else {
            for l in lines {
                let l = l.trim();
                if l.is_empty() || l.starts_with('#') {
                    continue;
                }
                if let Some(id) = channel_id_from(l) {
                    out.push(Imported { key: id, name: String::new() });
                } else if l.starts_with('@') || l.contains("youtube.com/") {
                    out.push(Imported { key: l.to_string(), name: String::new() });
                }
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|c| seen.insert(c.key.clone()));
    out
}

/// NewPipe-compatible subscriptions export.
pub fn export_newpipe(channels: &[Channel]) -> String {
    let subs: Vec<Value> = channels
        .iter()
        .map(|c| json!({ "service_id": 0, "url": format!("https://www.youtube.com/channel/{}", c.id), "name": c.name }))
        .collect();
    serde_json::to_string_pretty(&json!({
        "app_version": "0.27.0",
        "app_version_int": 1000,
        "subscriptions": subs,
    }))
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "UCLA_DiR1FfKNvjuUpBHmylQ";

    #[test]
    fn newpipe_json_roundtrip() {
        let ch = vec![Channel { id: ID.into(), name: "NASA".into(), avatar: String::new() }];
        let back = parse(&export_newpipe(&ch));
        assert_eq!(back, vec![Imported { key: ID.into(), name: "NASA".into() }]);
    }

    #[test]
    fn csv_opml_and_text() {
        let csv = format!("Channel Id,Channel Url,Channel Title\n{ID},http://www.youtube.com/channel/{ID},\"NASA, Inc\"\n");
        assert_eq!(parse(&csv)[0].name, "NASA, Inc");
        let opml = format!(r#"<opml><body><outline text="NASA &amp; co" title="NASA &amp; co" type="rss" xmlUrl="https://www.youtube.com/feeds/videos.xml?channel_id={ID}"/></body></opml>"#);
        let o = parse(&opml);
        assert_eq!(o[0].key, ID);
        assert_eq!(o[0].name, "NASA & co");
        let txt = parse(&format!("https://www.youtube.com/channel/{ID}\n@LinusTechTips\n# c\n"));
        assert_eq!(txt.len(), 2);
        assert_eq!(txt[1].key, "@LinusTechTips");
    }
}
