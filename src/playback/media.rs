//! Local media support: inspect a file with ffprobe and describe it as a `Streams`
//! the player can open.

use crate::yt::{Streams, VideoOpt};
use serde_json::Value;
use std::path::Path;
use std::process::Command;

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
        if bytes[i] == b'%' && i + 2 < bytes.len() + 0 && i + 2 <= bytes.len() - 1 + 1 {
            if let Ok(b) = u8::from_str_radix(&rest[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Returns (streams, title, artist).
pub fn probe(path: &str) -> Result<(Streams, String, String), String> {
    if !Path::new(path).is_file() {
        return Err("file not found".into());
    }
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams", path])
        .output()
        .map_err(|e| format!("ffprobe is required ({e})"))?;
    if !out.status.success() {
        return Err("not a media file".into());
    }
    let v: Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let (mut w, mut h, mut has_audio) = (0u32, 0u32, false);
    for st in v["streams"].as_array().into_iter().flatten() {
        match st["codec_type"].as_str() {
            Some("video") => {
                let cover = st.pointer("/disposition/attached_pic").and_then(Value::as_i64) == Some(1);
                if !cover && h == 0 {
                    w = st["width"].as_u64().unwrap_or(0) as u32;
                    h = st["height"].as_u64().unwrap_or(0) as u32;
                }
            }
            Some("audio") => has_audio = true,
            _ => {}
        }
    }
    if h == 0 && !has_audio {
        return Err("no audio or video stream found".into());
    }
    let duration = v
        .pointer("/format/duration")
        .and_then(Value::as_str)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let tag = |name: &str| -> String {
        v.pointer("/format/tags")
            .and_then(Value::as_object)
            .and_then(|m| m.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)))
            .and_then(|(_, val)| val.as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let stem = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Media")
        .to_string();
    let title = Some(tag("title")).filter(|t| !t.is_empty()).unwrap_or(stem);
    let artist = tag("artist");
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
