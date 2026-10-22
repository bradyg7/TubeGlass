//! Turning an episode into something the player can open (following tracker redirects, learning the length).

use crate::*;
use crate::podcasts::*;
use crate::yt::{Streams, Video, VideoOpt};

// ---------------------------------------------------------------------- playing

/// Describes an episode as a Video so the rest of the app can treat it like any other.
pub fn episode_video(show: &Show, e: &Episode) -> Video {
    Video {
        id: video_id(e),
        title: e.title.clone(),
        thumb: if e.image.is_empty() { show.image.clone() } else { e.image.clone() },
        duration: if e.duration > 0.0 { fmt_time(e.duration) } else { String::new() },
        meta: if e.published > 0 { ago(e.published) } else { String::new() },
        kind: yt::Kind::Video,
        channel_id: show.feed.clone(),
        channel_name: show.title.clone(),
        rank: 0,
        short: false,
        music: false,
    }
}

/// Asks ffmpeg for the length of a file when the feed does not say.
pub(crate) fn probe_duration(url: &str) -> f64 {
    crate::playback::media::inspect(url, &["-user_agent", UA]).map(|i| i.duration).unwrap_or(0.0)
}

/// Podcast links usually hop through two or three trackers before the file. Follow them once
/// and learn the file's size, so playback can then talk to the final address directly.
pub(crate) fn resolve(url: &str) -> Option<(String, u64)> {
    // redirects are followed by hand: some tracker hops send a broken body that would otherwise
    // fail the whole lookup, and their bodies are not needed
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .max_redirects(0)
        .http_status_as_error(false)
        .build()
        .into();
    let mut cur = url.to_string();
    for _ in 0..12 {
        let r = match agent.get(&cur).header("User-Agent", UA).header("Range", "bytes=0-0").call() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[resolve] {e}");
                return None;
            }
        };
        let h = |n: &str| r.headers().get(n).and_then(|v| v.to_str().ok()).map(|s| s.to_string());
        let code = r.status().as_u16();
        if (300..400).contains(&code) {
            let next = h("location")?;
            cur = join_url(&cur, &next);
            continue;
        }
        let len: u64 = match h("content-range") {
            Some(cr) if code == 206 => cr.rsplit('/').next()?.parse().ok()?,
            // the server ignored the range: it cannot be read in pieces, so it is played as a stream
            _ => return None,
        };
        return (len > 1000).then_some((cur, len));
    }
    None
}

/// Resolves a redirect target against the address that sent it.
fn join_url(base: &str, r: &str) -> String {
    if r.starts_with("http://") || r.starts_with("https://") {
        return r.to_string();
    }
    let scheme_end = base.find("://").map(|i| i + 3).unwrap_or(0);
    if r.starts_with('/') {
        let host_end = base[scheme_end..].find('/').map(|i| i + scheme_end).unwrap_or(base.len());
        return format!("{}{}", &base[..host_end], r);
    }
    let path = base.split(['?', '#']).next().unwrap_or(base);
    let dir = path.rfind('/').filter(|i| *i >= scheme_end).map(|i| &path[..=i]).unwrap_or(path);
    format!("{dir}{r}")
}

/// What the player needs to open an episode. The length comes from the feed, or from the file.
pub fn build_streams(e: &Episode) -> Streams {
    let resolved = resolve(&e.url);
    let duration = if e.duration > 0.0 { e.duration } else { probe_duration(resolved.as_ref().map_or(e.url.as_str(), |r| r.0.as_str())) };
    let opt = match resolved {
        Some((url, len)) => {
            crate::proxy::remember_len(&url, len);
            VideoOpt { url, width: 0, height: 0, ua: UA.into(), muxed: true, hls: false, audio_url: None }
        }
        None => VideoOpt { url: e.url.clone(), width: 0, height: 0, ua: UA.into(), muxed: true, hls: true, audio_url: None },
    };
    let captions = if e.transcript.is_empty() {
        vec![]
    } else {
        vec![crate::yt::CaptionTrack { lang: "transcript".into(), name: "Transcript".into(), auto: false, url: format!("podtr:{}", e.transcript) }]
    };
    Streams {
        captions,
        chapters: vec![],
        storyboard: None,
        title: e.title.clone(),
        is_live: false,
        duration,
        video: vec![opt],
        audio: None,
    }
}
