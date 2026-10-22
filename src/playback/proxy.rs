//! googlevideo rejects open-ended range requests, so ffmpeg talks to this loopback proxy
//! which fetches the upstream file in bounded chunks (like YouTube's own players do).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Size of each request to the upstream server (4 MiB).
const CHUNK: u64 = 4 * 1024 * 1024;

/// Lengths already known for some addresses (podcast files, resolved once up front).
static KNOWN: std::sync::Mutex<Vec<(String, u64)>> = std::sync::Mutex::new(Vec::new());

/// Remembers the size of a file we already looked up (podcast files).
pub fn remember_len(url: &str, len: u64) {
    if let Ok(mut k) = KNOWN.lock() {
        k.retain(|(u, _)| u != url);
        k.push((url.to_string(), len));
        if k.len() > 64 {
            k.remove(0);
        }
    }
}

/// Size of the upstream file: remembered, from the URL's `clen`, or unknown.
fn upstream_len(url: &str) -> Option<u64> {
    if let Some(l) = KNOWN.lock().ok().and_then(|k| k.iter().find(|(u, _)| u == url).map(|(_, l)| *l)) {
        return Some(l);
    }
    url.split(['?', '&'])
        .find_map(|kv| kv.strip_prefix("clen="))
        .and_then(|v| v.parse().ok())
}

/// One HTTP agent so connections to the same host are reused.
fn shared_agent() -> ureq::Agent {
    /// The shared agent.
    static AGENT: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    AGENT
        .get_or_init(|| {
            ureq::Agent::config_builder()
                .timeout_global(Some(Duration::from_secs(30)))
                .max_idle_connections_per_host(16)
                .build()
                .into()
        })
        .clone()
}

/// Why the last upstream request failed (shown when playback cannot start).
static LAST_ERROR: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// Why the last upstream request failed (shown when playback cannot start).
pub fn last_error() -> String {
    LAST_ERROR.lock().map(|g| g.clone()).unwrap_or_default()
}

/// Requests a byte range upstream, retrying brief failures.
fn fetch(agent: &ureq::Agent, url: &str, ua: &str, a: u64, b: u64) -> Result<ureq::Body, String> {
    // a dropped connection or a hiccup on YouTube's side usually works the second time
    let mut last = String::new();
    for attempt in 0..3 {
        match agent.get(url).header("User-Agent", ua).header("Range", format!("bytes={a}-{b}")).call() {
            Ok(r) => return Ok(r.into_body()),
            Err(ureq::Error::StatusCode(code)) => {
                last = format!("YouTube refused the stream (HTTP {code})");
                // forbidden / gone will not change by asking again
                if code == 403 || code == 404 || code == 410 {
                    break;
                }
            }
            Err(e) => last = e.to_string(),
        }
        std::thread::sleep(Duration::from_millis(250 * (attempt + 1)));
    }
    if let Ok(mut g) = LAST_ERROR.lock() {
        *g = last.clone();
    }
    Err(last)
}

/// Serves one local request by fetching the wanted range upstream in bounded chunks.
fn handle(mut sock: TcpStream, url: &str, ua: &str, len: u64, stop: &AtomicBool) -> std::io::Result<()> {
    let mut buf = vec![];
    let mut tmp = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = sock.read(&mut tmp)?;
        if n == 0 || buf.len() > 16384 {
            return Ok(());
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    let head = String::from_utf8_lossy(&buf).to_string();
    let is_head = head.starts_with("HEAD");
    let range = head.lines().find_map(|l| {
        let l = l.to_ascii_lowercase();
        let v = l.strip_prefix("range: bytes=")?.to_string();
        let (a, b) = v.trim().split_once('-')?;
        Some((a.parse::<u64>().ok()?, b.parse::<u64>().ok()))
    });
    let (start, end, status) = match range {
        Some((a, b)) => (a.min(len.saturating_sub(1)), b.unwrap_or(len - 1).min(len - 1), "206 Partial Content"),
        None => (0, len - 1, "200 OK"),
    };
    // get the first chunk before answering, so an upstream failure becomes a clean HTTP error
    // for ffmpeg instead of a cut-off body
    let agent = shared_agent();
    let mut first = if is_head {
        None
    } else {
        match fetch(&agent, url, ua, start, (start + CHUNK - 1).min(end)) {
            Ok(b) => Some(b),
            Err(e) => {
                let _ = sock.write_all(b"HTTP/1.1 502 Bad Gateway\r\nConnection: close\r\nContent-Length: 0\r\n\r\n");
                return Err(std::io::Error::other(e));
            }
        }
    };
    let mut h = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/octet-stream\r\nAccept-Ranges: bytes\r\nContent-Length: {}\r\nConnection: close\r\n",
        end - start + 1
    );
    if status.starts_with("206") {
        h += &format!("Content-Range: bytes {start}-{end}/{len}\r\n");
    }
    h += "\r\n";
    sock.write_all(h.as_bytes())?;
    if is_head {
        return Ok(());
    }
    // one shared client: googlevideo keeps connections alive, so a seek's follow-up
    // requests skip the TCP + TLS handshake
    let mut pos = start;
    let mut data = vec![0u8; 64 * 1024];
    while pos <= end && !stop.load(Ordering::Relaxed) {
        let to = (pos + CHUNK - 1).min(end);
        let mut body = match first.take() {
            Some(b) => b,
            None => fetch(&agent, url, ua, pos, to).map_err(std::io::Error::other)?,
        };
        let mut rd = body.as_reader();
        let mut got = 0u64;
        loop {
            let n = rd.read(&mut data)?;
            if n == 0 {
                break;
            }
            sock.write_all(&data[..n])?;
            got += n as u64;
        }
        if got == 0 {
            return Err(std::io::Error::other("empty upstream chunk"));
        }
        pos += got;
    }
    Ok(())
}

/// Start a proxy for `url`; returns the local URL ffmpeg should open.
pub fn serve(url: &str, ua: &str, stop: Arc<AtomicBool>) -> Result<String, String> {
    let len = match upstream_len(url) {
        Some(l) => l,
        None => {
            let agent = ureq::Agent::new_with_defaults();
            let r = agent
                .get(url)
                .header("User-Agent", ua)
                .header("Range", "bytes=0-0")
                .call()
                .map_err(|e| e.to_string())?;
            r.headers()
                .get("content-range")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.rsplit('/').next()?.parse().ok())
                .ok_or("unknown stream length")?
        }
    };
    let l = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    l.set_nonblocking(true).map_err(|e| e.to_string())?;
    let port = l.local_addr().map_err(|e| e.to_string())?.port();
    let url = url.to_string();
    let ua = ua.to_string();
    std::thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            match l.accept() {
                Ok((s, _)) => {
                    let _ = s.set_nonblocking(false);
                    let (u, a, st) = (url.clone(), ua.clone(), stop.clone());
                    std::thread::spawn(move || {
                        if let Err(e) = handle(s, &u, &a, len, &st) { eprintln!("proxy: {e}"); }
                    });
                }
                Err(_) => std::thread::sleep(Duration::from_millis(40)),
            }
        }
    });
    Ok(format!("http://127.0.0.1:{port}/s"))
}

#[cfg(test)]
mod seek_timing {
    use super::*;
    use std::process::Command;
    use std::time::Instant;

    #[test]
    #[ignore = "needs network"]
    fn time_far_seeks() {
        let st = crate::yt::streams("plN7JMbadRg").unwrap();
        let opt = st.video.iter().filter(|v| v.height <= 1080).max_by_key(|v| v.height).unwrap().clone();
        let stop = Arc::new(AtomicBool::new(false));
        let v = serve(&opt.url, &opt.ua, stop.clone()).unwrap();
        let a = st.audio.as_ref().map(|a| serve(&a.url, &a.ua, stop.clone()).unwrap());
        println!("SEEK {}p duration {:.0}s", opt.height, st.duration);
        for pos in [0.0f64, 120.0, 400.0, 700.0] {
            let t = Instant::now();
            let mut c = crate::platform::command("ffmpeg");
            c.args(["-hide_banner", "-loglevel", "error", "-nostdin", "-probesize", "1500000", "-analyzeduration", "1000000"]);
            if pos > 0.5 { c.args(["-ss", &pos.to_string()]); }
            c.args(["-i", &v]);
            if let Some(a) = &a {
                if pos > 0.5 { c.args(["-ss", &pos.to_string()]); }
                c.args(["-i", a, "-map", "0:v:0", "-map", "1:a:0"]);
            }
            c.args(["-frames:v", "1", "-f", "null", "-"]);
            let out = c.output().unwrap();
            println!("SEEK to {pos:>6.0}s: {:.2}s ok={}", t.elapsed().as_secs_f32(), out.status.success());
        }
    }

    #[test]
    #[ignore = "needs network"]
    fn time_decode_throughput() {
        let st = crate::yt::streams("plN7JMbadRg").unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        for h in [1080u32, 720] {
            let Some(opt) = st.video.iter().filter(|v| v.height <= h).max_by_key(|v| v.height) else { continue };
            let v = serve(&opt.url, &opt.ua, stop.clone()).unwrap();
            let (w, hh) = ((opt.width & !1) as usize, (opt.height & !1) as usize);
            let t = Instant::now();
            let out = crate::platform::command("ffmpeg")
                .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-ss", "400", "-i", &v, "-t", "10",
                    "-vf", &format!("fps=30,scale={w}:{hh}:flags=fast_bilinear,format=rgba"), "-an", "-f", "null", "-"])
                .output()
                .unwrap();
            let el = t.elapsed().as_secs_f32();
            println!("DEC {}p ({}x{}): 10s of video took {:.2}s -> {:.1}x realtime ok={}", opt.height, w, hh, el, 10.0 / el, out.status.success());
        }
        println!("DEC cpus: {}", std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0));
    }

    #[test]
    #[ignore = "needs network"]
    fn time_flag_variants() {
        let st = crate::yt::streams("plN7JMbadRg").unwrap();
        let opt = st.video.iter().filter(|v| v.height <= 1080).max_by_key(|v| v.height).unwrap().clone();
        let stop = Arc::new(AtomicBool::new(false));
        let a = st.audio.clone().unwrap();
        let (w, h) = ((opt.width & !1) as usize, (opt.height & !1) as usize);
        // (label, extra input flags, filter)
        let variants: Vec<(&str, Vec<&str>, String)> = vec![
            ("app today", vec!["-probesize", "1500000", "-analyzeduration", "1000000", "-reconnect", "1", "-reconnect_streamed", "1", "-reconnect_delay_max", "5"], format!("fps=30,scale={w}:{h}:flags=fast_bilinear,format=rgba")),
            ("no reconnect", vec!["-probesize", "1500000", "-analyzeduration", "1000000"], format!("fps=30,scale={w}:{h}:flags=fast_bilinear,format=rgba")),
            ("small probe", vec!["-probesize", "200000", "-analyzeduration", "0", "-fflags", "+nobuffer"], format!("fps=30,scale={w}:{h}:flags=fast_bilinear,format=rgba")),
            ("small probe 720p scale", vec!["-probesize", "200000", "-analyzeduration", "0", "-fflags", "+nobuffer"], "fps=30,scale=1280:720:flags=fast_bilinear,format=rgba".to_string()),
        ];
        for (label, flags, vf) in variants {
            let mut best = f32::MAX;
            for pos in [900.0f64, 2400.0, 3100.0] {
                let v = serve(&opt.url, &opt.ua, stop.clone()).unwrap();
                let au = serve(&a.url, &a.ua, stop.clone()).unwrap();
                let t = Instant::now();
                let mut c = crate::platform::command("ffmpeg");
                c.args(["-hide_banner", "-loglevel", "error", "-nostdin"]);
                c.args(&flags);
                c.args(["-ss", &pos.to_string(), "-i", &v]);
                c.args(&flags);
                c.args(["-ss", &pos.to_string(), "-i", &au]);
                c.args(["-map", "0:v:0", "-vf", &vf, "-an", "-frames:v", "1", "-f", "null", "-"]);
                c.output().unwrap();
                best = best.min(t.elapsed().as_secs_f32());
                let _ = pos;
            }
            println!("FLAGS {label:<26} best of 3 far seeks: {best:.2}s");
        }
    }

    #[test]
    #[ignore = "needs network"]
    fn time_parallel_inputs() {
        let st = crate::yt::streams("plN7JMbadRg").unwrap();
        let opt = st.video.iter().filter(|v| v.height <= 1080).max_by_key(|v| v.height).unwrap().clone();
        let a = st.audio.clone().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let flags = ["-probesize", "1500000", "-analyzeduration", "1000000"];
        let one = |url: String, pos: f64, map: &[&str]| {
            let mut c = crate::platform::command("ffmpeg");
            c.args(["-hide_banner", "-loglevel", "error", "-nostdin"]).args(flags);
            c.args(["-ss", &pos.to_string(), "-i", &url]).args(map).args(["-f", "null", "-"]);
            c
        };
        for round in 0..3 {
            let pos = 700.0 + 900.0 * round as f64;
            let v = serve(&opt.url, &opt.ua, stop.clone()).unwrap();
            let au = serve(&a.url, &a.ua, stop.clone()).unwrap();
            // sequential: one process, two inputs
            let t = Instant::now();
            let mut c = crate::platform::command("ffmpeg");
            c.args(["-hide_banner", "-loglevel", "error", "-nostdin"]).args(flags);
            c.args(["-ss", &pos.to_string(), "-i", &v]).args(flags);
            c.args(["-ss", &pos.to_string(), "-i", &au]);
            c.args(["-map", "0:v:0", "-frames:v", "1", "-f", "null", "-"]);
            c.output().unwrap();
            let seq = t.elapsed().as_secs_f32();
            // parallel: two processes at once
            let v2 = serve(&opt.url, &opt.ua, stop.clone()).unwrap();
            let a2 = serve(&a.url, &a.ua, stop.clone()).unwrap();
            let t = Instant::now();
            let mut p1 = one(v2, pos + 5.0, &["-map", "0:v:0", "-frames:v", "1"]).spawn().unwrap();
            let mut p2 = one(a2, pos + 5.0, &["-map", "0:a:0", "-frames:a", "4"]).spawn().unwrap();
            p1.wait().unwrap();
            p2.wait().unwrap();
            let par = t.elapsed().as_secs_f32();
            println!("PAR round {round}: one process {seq:.2}s   two processes {par:.2}s");
        }
    }
}

#[cfg(test)]
mod failure_tests {
    use super::*;
    use std::process::Command;

    /// A server that answers every request with 403, like googlevideo does for a bad link.
    #[test]
    fn refused_stream_is_reported_not_cut_off() {
        // the check feeds the proxy to ffmpeg, so it needs ffmpeg to be installed
        if !crate::platform::ffmpeg_available() {
            eprintln!("skipped: ffmpeg is not installed");
            return;
        }
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for s in l.incoming().flatten() {
                let mut s = s;
                let mut b = [0u8; 2048];
                let _ = s.read(&mut b);
                let _ = s.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            }
        });
        let stop = Arc::new(AtomicBool::new(false));
        let url = format!("http://127.0.0.1:{port}/v?clen=100000");
        let local = serve(&url, "t", stop.clone()).unwrap();
        let out = crate::platform::command("ffmpeg").args(["-hide_banner", "-loglevel", "error", "-nostdin", "-i", &local, "-f", "null", "-"]).output().unwrap();
        assert!(!out.status.success());
        assert!(last_error().contains("403"), "got: {}", last_error());
        stop.store(true, Ordering::SeqCst);
    }

    #[test]
    #[ignore = "network"]
    fn alt_clients_give_playable_streams() {
        let st = crate::yt::streams_alt("jNQXAC9IVRw").expect("streams");
        let v = st.video.iter().max_by_key(|v| v.height).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let local = serve(&v.url, &v.ua, stop.clone()).unwrap();
        let out = crate::platform::command("ffmpeg").args(["-hide_banner", "-loglevel", "error", "-nostdin", "-i", &local, "-frames:v", "5", "-f", "null", "-"]).output().unwrap();
        println!("alt: {}p muxed={} ok={}", v.height, v.muxed, out.status.success());
        assert!(out.status.success());
        stop.store(true, Ordering::SeqCst);
    }
}
