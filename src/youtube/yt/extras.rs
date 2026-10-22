//! Caption tracks (as timed cues) and SponsorBlock segments.

use super::*;

/// Timed captions for one track.
pub fn caption_cues(url: &str) -> Res<Vec<Cue>> {
    if url.starts_with("file:") {
        return crate::download::read_cues(url);
    }
    // a podcast's own transcript, offered like a caption track
    if let Some(t) = url.strip_prefix("podtr:") {
        return crate::podcasts::fetch_transcript(t);
    }
    let body = get_text(&format!("{url}&fmt=json3"))?;
    let v: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let mut out = vec![];
    for e in v["events"].as_array().into_iter().flatten() {
        let (Some(start), Some(dur)) = (e["tStartMs"].as_f64(), e["dDurationMs"].as_f64().or(Some(2000.0))) else {
            continue;
        };
        let text: String = e["segs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s["utf8"].as_str())
            .collect::<String>()
            .replace('\n', " ");
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if !text.is_empty() {
            out.push(Cue { start: start / 1000.0, end: (start + dur) / 1000.0, text });
        }
    }
    if out.is_empty() {
        return Err("this caption track is empty".into());
    }
    Ok(out)
}

/// Community-marked segments (sponsors, intros …) from SponsorBlock, looked up by the
/// first 4 hex characters of the video id's SHA-256 so the exact video is never revealed.
pub fn sponsor_segments(video_id: &str) -> Res<Vec<SkipSeg>> {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(video_id.as_bytes());
    let prefix: String = hash.iter().take(2).map(|b| format!("{b:02x}")).collect();
    let url = format!(
        "https://sponsor.ajay.app/api/skipSegments/{prefix}?categories=%5B%22sponsor%22%2C%22selfpromo%22%2C%22interaction%22%2C%22intro%22%2C%22outro%22%5D&actionTypes=%5B%22skip%22%5D"
    );
    let resp = match agent().get(&url).header("User-Agent", WEB_UA).call() {
        Ok(r) => r,
        // 404 simply means nobody has marked anything in this hash bucket
        Err(ureq::Error::StatusCode(404)) => return Ok(vec![]),
        Err(e) => return Err(format!("network error: {e}")),
    };
    let v: Value = resp
        .into_body()
        .read_json()
        .map_err(|e| format!("bad response: {e}"))?;
    let mut out = vec![];
    for entry in v.as_array().into_iter().flatten() {
        if entry["videoID"].as_str() != Some(video_id) {
            continue;
        }
        for s in entry["segments"].as_array().into_iter().flatten() {
            let seg = &s["segment"];
            if let (Some(a), Some(b)) = (seg[0].as_f64(), seg[1].as_f64()) {
                out.push(SkipSeg { start: a, end: b, category: s["category"].as_str().unwrap_or("sponsor").to_string() });
            }
        }
    }
    out.sort_by(|a, b| a.start.total_cmp(&b.start));
    Ok(out)
}
