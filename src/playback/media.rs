//! Local media support: inspect a file with ffmpeg and describe it as a `Streams`
//! the player can open.

use crate::yt::{Streams, VideoOpt};
use std::path::Path;

/// File extensions the app offers in the Open dialog.
pub const EXTENSIONS: &[&str] = &[
    "mp4", "m4v", "mkv", "webm", "mov", "avi", "wmv", "flv", "ts", "3gp", "ogv", "mpg", "mpeg", "mp3",
    "m4a", "flac", "ogg", "oga", "opus", "wav", "aac", "wma", "aiff", "ape", "mka",
];


/// Accepts plain paths and `file://` URIs (what desktop launchers pass for `%u`).
pub fn normalize_arg(arg: &str) -> String {
    let Some(rest) = arg.strip_prefix("file://") else { return arg.to_string() };
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Some(b) = rest.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    let path = String::from_utf8_lossy(&out).into_owned();
    // "file:///C:/x" leaves "/C:/x": drop the slash in front of a Windows drive letter
    let b = path.as_bytes();
    if cfg!(windows) && b.len() > 2 && b[0] == b'/' && b[2] == b':' {
        return path[1..].to_string();
    }
    path
}

/// What `ffmpeg -i` reports about a file.
#[derive(Debug, Default, PartialEq)]
pub struct Info {
    /// length in seconds (0 when unknown)
    pub duration: f64,
    /// size of the first real picture stream (cover art does not count); 0 for audio files
    pub width: u32,
    pub height: u32,
    pub has_audio: bool,
    pub title: String,
    pub artist: String,
}

/// Reads the report `ffmpeg -i <file>` prints on stderr (container tags, `Duration:` and one
/// `Stream` line per stream). `None` when the text is not about a media file at all.
pub fn parse_info(text: &str) -> Option<Info> {
    if !text.contains("Input #0") {
        return None;
    }
    let mut info = Info::default();
    let mut in_tags = false;
    for line in text.lines() {
        // container-level tags are indented by four spaces under a two-space "Metadata:" line
        if line == "  Metadata:" {
            in_tags = true;
            continue;
        }
        if in_tags {
            if let Some(rest) = line.strip_prefix("    ").filter(|r| !r.starts_with(' ')) {
                if let Some((k, v)) = rest.split_once(':') {
                    match k.trim().to_ascii_lowercase().as_str() {
                        "title" => info.title = v.trim().to_string(),
                        "artist" => info.artist = v.trim().to_string(),
                        _ => {}
                    }
                }
                continue;
            }
            in_tags = false;
        }
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("Duration:") {
            let clock = rest.split(',').next().unwrap_or("").trim();
            let mut secs = 0.0;
            for part in clock.split(':') {
                match part.trim().parse::<f64>() {
                    Ok(v) => secs = secs * 60.0 + v,
                    Err(_) => {
                        secs = 0.0;
                        break;
                    }
                }
            }
            info.duration = secs;
        } else if t.starts_with("Stream #") {
            if t.contains(": Audio:") {
                info.has_audio = true;
            } else if let Some((_, rest)) = t.split_once(": Video:") {
                let cover = rest.contains("(attached pic)");
                if !cover && info.height == 0 {
                    // the size is the first "1920x1080" among the comma-separated details
                    for part in rest.split(',') {
                        let word = part.trim().split_whitespace().next().unwrap_or("");
                        if let Some((w, h)) = word.split_once('x') {
                            if let (Ok(w), Ok(h)) = (w.parse::<u32>(), h.parse::<u32>()) {
                                info.width = w;
                                info.height = h;
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
    Some(info)
}

/// Runs `ffmpeg -i` on a file or address and reads what it says about it.
pub fn inspect(source: &str, extra: &[&str]) -> Result<Info, String> {
    let out = crate::platform::command("ffmpeg")
        .args(["-hide_banner", "-nostdin"])
        .args(extra)
        .args(["-i", source])
        .output()
        .map_err(|e| format!("ffmpeg is required ({e})"))?;
    // ffmpeg reports on stderr and then complains that no output file was given: that is expected
    parse_info(&String::from_utf8_lossy(&out.stderr)).ok_or_else(|| "not a media file".to_string())
}

/// Returns (streams, title, artist).
pub fn probe(path: &str) -> Result<(Streams, String, String), String> {
    if !Path::new(path).is_file() {
        return Err("file not found".into());
    }
    let info = inspect(path, &[])?;
    let (w, h) = (info.width, info.height);
    if h == 0 && !info.has_audio {
        return Err("no audio or video stream found".into());
    }
    let duration = info.duration;
    let (title_tag, artist) = (info.title.clone(), info.artist.clone());
    let stem = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Media")
        .to_string();
    let title = Some(title_tag).filter(|t| !t.is_empty()).unwrap_or(stem);
    let streams = Streams {
        captions: vec![],
        chapters: vec![],
        storyboard: None,
        title: title.clone(),
        is_live: false,
        duration,
        video: vec![VideoOpt {
            url: path.to_string(),
            width: if h == 0 { 0 } else { w.max(2) },
            height: h,
            ua: String::new(),
            muxed: true,
            hls: true,
            audio_url: None,
        }],
        audio: None,
    };
    Ok((streams, title, artist))
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIDEO: &str = "Input #0, mov,mp4,m4a,3gp,3g2,mj2, from 'a.mp4':
  Metadata:
    major_brand     : isom
    title           : A Talk
    artist          : Someone
  Duration: 00:02:51.04, start: 0.000000, bitrate: 484 kb/s
  Stream #0:0[0x1](und): Video: h264 (High) (avc1 / 0x31637661), yuv420p(tv, bt709, progressive), 1920x1080 [SAR 1:1 DAR 16:9], 346 kb/s, 60 fps (default)
      Metadata:
        handler_name    : ISO Media file produced by Google Inc.
        title           : not the file title
  Stream #0:1[0x2](und): Audio: aac (LC) (mp4a / 0x6134706D), 44100 Hz, stereo, fltp, 159 kb/s (default)
At least one output file must be specified";

    #[test]
    fn a_video_report_is_read() {
        let i = parse_info(VIDEO).unwrap();
        assert_eq!((i.width, i.height), (1920, 1080));
        assert!(i.has_audio);
        assert!((i.duration - 171.04).abs() < 0.01);
        // tags of single streams (deeper indentation) are not the file's tags
        assert_eq!((i.title.as_str(), i.artist.as_str()), ("A Talk", "Someone"));
    }

    #[test]
    fn cover_art_does_not_make_a_song_a_video() {
        let t = "Input #0, mp3, from 'a.mp3':
  Duration: 00:03:50.04, start: 0.025057, bitrate: 320 kb/s
  Stream #0:0: Audio: mp3, 44100 Hz, stereo, fltp, 320 kb/s
  Stream #0:1: Video: mjpeg (Baseline), yuvj444p(pc, bt470bg/unknown/unknown), 500x500 [SAR 1:1 DAR 1:1], 90k tbr (attached pic)";
        let i = parse_info(t).unwrap();
        assert_eq!(i.height, 0);
        assert!(i.has_audio);
        assert!((i.duration - 230.04).abs() < 0.01);
    }

    #[test]
    fn upper_case_tags_and_unknown_length() {
        let t = "Input #0, matroska,webm, from 'a.mkv':
  Metadata:
    TITLE           : Song
    ARTIST          : Band
  Duration: N/A, start: 0.000000, bitrate: N/A
  Stream #0:0: Audio: opus, 48000 Hz, stereo, fltp";
        let i = parse_info(t).unwrap();
        assert_eq!((i.title.as_str(), i.artist.as_str(), i.duration), ("Song", "Band", 0.0));
    }

    #[test]
    fn other_text_is_not_a_media_file() {
        assert!(parse_info("a.txt: Invalid data found when processing input").is_none());
    }
}
