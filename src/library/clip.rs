//! Cut a section out of a video: re-encoded MP4, audio-only M4A or an animated GIF.
//! Works from the same streams the player uses (through the range proxy) and from
//! downloaded files.

use crate::yt::Streams;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// What a clip is saved as.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Video,
    Audio,
    Gif,
}

impl Format {
    /// File extension of the format.
    fn ext(self) -> &'static str {
        match self {
            Format::Video => "mp4",
            Format::Audio => "m4a",
            Format::Gif => "gif",
        }
    }
}

/// Folder the clips are saved in (inside the downloads folder).
pub fn dir() -> PathBuf {
    crate::download::dir().join("Clips")
}

/// "1:23" / "83" / "1:02:03" → seconds.
pub fn parse_time(s: &str) -> Option<f64> {
    let mut total = 0.0;
    for part in s.trim().split(':') {
        total = total * 60.0 + part.trim().parse::<f64>().ok()?;
    }
    Some(total)
}

/// Cuts `start..end` out of the stream with ffmpeg. Reports progress, and stops when `cancel` is set.
pub fn run(
    streams: &Streams,
    quality: u32,
    start: f64,
    end: f64,
    format: Format,
    title: &str,
    cancel: &Arc<AtomicBool>,
    on_progress: &dyn Fn(f32),
) -> Result<PathBuf, String> {
    if streams.is_live {
        return Err("live streams can't be clipped".into());
    }
    let len = end - start;
    if len < 0.5 {
        return Err("the clip is too short".into());
    }
    let opt = streams
        .video
        .iter()
        .filter(|v| v.height <= quality.max(144) || v.height == 0)
        .max_by_key(|v| v.height)
        .or_else(|| streams.video.iter().min_by_key(|v| v.height))
        .ok_or("no stream available")?;
    let d = dir();
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let name = format!(
        "{} [{}-{}].{}",
        crate::download::sanitize(title),
        crate::fmt_time(start).replace(':', "."),
        crate::fmt_time(end).replace(':', "."),
        format.ext()
    );
    let out = d.join(name);

    let stop = Arc::new(AtomicBool::new(false));
    let open = |url: &str, ua: &str, hls: bool| -> Result<String, String> {
        if hls || crate::platform::is_local_path(&url) { Ok(url.to_string()) } else { crate::proxy::serve(url, ua, stop.clone()) }
    };
    let local = crate::platform::is_local_path(&opt.url);
    // where the picture and the sound come from
    let vurl = open(&opt.url, &opt.ua, opt.hls)?;
    let aurl = match (&streams.audio, opt.muxed) {
        (Some(a), false) if !local => Some(open(&a.url, &a.ua, false)?),
        (None, false) => opt.audio_url.clone(),
        _ => None,
    };
    let audio_only_src = match (&streams.audio, format) {
        (Some(a), Format::Audio) if !local && !opt.muxed => Some(open(&a.url, &a.ua, false)?),
        _ => None,
    };

    let mut cmd = crate::platform::command("ffmpeg");
    cmd.args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"]);
    let input = |cmd: &mut Command, url: &str, ua: &str| {
        if !crate::platform::is_local_path(&url) {
            cmd.args(["-user_agent", ua]);
        }
        cmd.args(["-ss", &format!("{start:.3}"), "-i", url]);
    };
    match format {
        Format::Audio => {
            if let Some(a) = &audio_only_src {
                input(&mut cmd, a, "TubeGlass");
            } else {
                input(&mut cmd, &vurl, &opt.ua);
                if let Some(a) = &aurl {
                    input(&mut cmd, a, "TubeGlass");
                }
            }
        }
        _ => {
            input(&mut cmd, &vurl, &opt.ua);
            if let (Some(a), Format::Video) = (&aurl, format) {
                input(&mut cmd, a, "TubeGlass");
            }
        }
    }
    cmd.args(["-t", &format!("{len:.3}")]);
    match format {
        Format::Video => {
            let audio_map = if aurl.is_some() { "1:a:0" } else { "0:a:0?" };
            cmd.args(["-map", "0:v:0", "-map", audio_map]);
            cmd.args(["-c:v", "libx264", "-preset", "superfast", "-crf", "21", "-pix_fmt", "yuv420p"]);
            cmd.args(["-c:a", "aac", "-b:a", "160k", "-movflags", "+faststart"]);
        }
        Format::Audio => {
            let m = if audio_only_src.is_some() || aurl.is_none() { "0:a:0" } else { "1:a:0" };
            cmd.args(["-map", m, "-vn", "-c:a", "aac", "-b:a", "192k"]);
        }
        Format::Gif => {
            cmd.args(["-map", "0:v:0", "-an"]);
            cmd.args(["-vf", "fps=10,scale=420:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse"]);
        }
    }
    cmd.args(["-progress", "pipe:1", "-nostats", "-stats_period", "0.2"]);
    cmd.arg(&out);
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("could not start ffmpeg ({e})"))?;
    let pid = child.id();
    // a watcher so Cancel works even while ffmpeg is still opening the stream
    let finished = Arc::new(AtomicBool::new(false));
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let (fin, can, flag) = (finished.clone(), cancelled.clone(), cancel.clone());
        std::thread::spawn(move || {
            while !fin.load(Ordering::Relaxed) {
                if flag.load(Ordering::Relaxed) {
                    can.store(true, Ordering::SeqCst);
                    crate::platform::kill_process(pid);
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(80));
            }
        });
    }
    let mut err = child.stderr.take().unwrap();
    let err_thread = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = std::io::Read::read_to_string(&mut err, &mut s);
        s
    });
    for line in BufReader::new(child.stdout.take().unwrap()).lines().map_while(Result::ok) {
        if let Some(us) = line.strip_prefix("out_time_us=").and_then(|v| v.trim().parse::<f64>().ok()) {
            on_progress(((us / 1e6) / len).clamp(0.0, 0.99) as f32);
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    finished.store(true, Ordering::SeqCst);
    stop.store(true, Ordering::SeqCst);
    if cancelled.load(Ordering::SeqCst) || cancel.load(Ordering::Relaxed) {
        let _ = std::fs::remove_file(&out);
        return Err("cancelled".into());
    }
    let stderr = err_thread.join().unwrap_or_default();
    if !status.success() {
        let _ = std::fs::remove_file(&out);
        let tail: Vec<&str> = stderr.lines().rev().take(2).collect();
        return Err(format!("ffmpeg failed: {}", tail.into_iter().rev().collect::<Vec<_>>().join(" ")));
    }
    on_progress(1.0);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_parse() {
        assert_eq!(parse_time("1:23"), Some(83.0));
        assert_eq!(parse_time("90"), Some(90.0));
        assert_eq!(parse_time("1:02:03"), Some(3723.0));
        assert_eq!(parse_time("x"), None);
    }

    #[test]
    #[ignore = "network"]
    fn cuts_a_clip() {
        let st = crate::yt::streams("jNQXAC9IVRw").expect("streams");
        let tmp = std::env::temp_dir().join("tg-clip-test");
        let _ = std::fs::remove_dir_all(&tmp);
        crate::download::set_dir(Some(tmp.clone()));
        for f in [Format::Video, Format::Audio, Format::Gif] {
            let p = run(&st, 360, 2.0, 6.0, f, "Me at the zoo", &Arc::new(AtomicBool::new(false)), &|_| {}).expect("clip");
            let len = std::fs::metadata(&p).unwrap().len();
            println!("{} {} bytes", p.display(), len);
            assert!(len > 2000);
        }
        crate::download::set_dir(None);
    }
}
