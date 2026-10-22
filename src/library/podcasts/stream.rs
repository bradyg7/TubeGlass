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

/// Asks ffprobe for the length of a file when the feed does not say.
pub(crate) fn probe_duration(url: &str) -> f64 {
    crate::platform::command("ffprobe")
        .args(["-v", "error", "-user_agent", UA, "-show_entries", "format=duration", "-of", "csv=p=0", url])
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<f64>().ok())
        .unwrap_or(0.0)
}

/// Podcast links usually hop through two or three trackers before the file. Follow them once
/// and learn the file's size, so playback can then talk to the final address directly.
pub(crate) fn resolve(url: &str) -> Option<(String, u64)> {
    use ureq::ResponseExt;
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(15))).build().into();
    let r = agent.get(url).header("User-Agent", UA).header("Range", "bytes=0-0").call().ok()?;
    let fin = r.get_uri().to_string();
    let h = |n: &str| r.headers().get(n).and_then(|v| v.to_str().ok()).map(|s| s.to_string());
    let len = match h("content-range") {
        Some(cr) => cr.rsplit('/').next()?.parse().ok()?,
        // the server ignored the range: only trust it when it says how long the whole file is
        None if r.status() == 200 => h("content-length")?.parse().ok()?,
        None => return None,
    };
    (len > 1000).then_some((fin, len))
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
