//! Loading thumbnails and avatars from the network or the disk cache.

use crate::*;

/// Longest side of a stored picture. Covers and thumbnails are shown small, and a podcast cover
/// is often 3000 pixels wide: decoded, one such picture takes 36 MB of memory.
const MAX_SIDE: u32 = 640;

/// How many pictures may download at once (a long episode list asks for hundreds).
const MAX_FETCHES: usize = 6;

/// Scales a picture down to `MAX_SIDE` and re-encodes it. Returns the bytes and their extension,
/// or `None` when the picture is small already or cannot be read.
fn shrink(bytes: &[u8]) -> Option<(&'static str, Vec<u8>)> {
    let img = image::load_from_memory(bytes).ok()?;
    if img.width() <= MAX_SIDE && img.height() <= MAX_SIDE && bytes.len() < 300_000 {
        return None;
    }
    let img = if img.width() > MAX_SIDE || img.height() > MAX_SIDE { img.thumbnail(MAX_SIDE, MAX_SIDE) } else { img };
    let mut out = std::io::Cursor::new(vec![]);
    if img.color().has_alpha() {
        img.write_to(&mut out, image::ImageFormat::Png).ok()?;
        Some(("png", out.into_inner()))
    } else {
        image::DynamicImage::ImageRgb8(img.to_rgb8()).write_to(&mut out, image::ImageFormat::Jpeg).ok()?;
        Some(("jpg", out.into_inner()))
    }
}

/// Maps a remote image URL to a `file://` URI of an on-disk copy, downloading it in the
/// background the first time. Returns an empty string until the file is available.
pub(crate) fn img_uri(ctx: &egui::Context, url: &str) -> String {
    use std::hash::{Hash, Hasher};
    use std::sync::{Mutex, OnceLock};
    /// Images that finished loading: address -> local file address.
    static READY: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    /// Images being fetched right now.
    static PENDING: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    if crate::platform::is_local_path(&url) {
        return crate::platform::file_uri(url);
    }
    if url.is_empty() {
        return String::new();
    }
    let ready = READY.get_or_init(Default::default);
    if let Some(u) = ready.lock().unwrap().get(url) {
        return u.clone();
    }
    let Some(dir) = cache_dir() else { return url.to_string() };
    let mut h = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut h);
    let key = format!("{:016x}", h.finish());
    for ext in ["jpg", "png", "webp"] {
        let path = dir.join("img").join(format!("{key}.{ext}"));
        if path.exists() {
            // pictures saved by an older version may be huge: shrink them once, in the background
            if std::fs::metadata(&path).is_ok_and(|m| m.len() > 400_000) {
                static TRIED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
                if TRIED.get_or_init(Default::default).lock().unwrap().insert(key.clone()) {
                    let (path, dir, key) = (path.clone(), dir.clone(), key.clone());
                    std::thread::spawn(move || {
                        let Ok(bytes) = std::fs::read(&path) else { return };
                        if let Some((ext, small)) = shrink(&bytes) {
                            // only a same-format swap keeps the address the app already holds valid
                            if path.extension().is_some_and(|e| e == ext) {
                                let tmp = dir.join("img").join(format!("{key}.part"));
                                if std::fs::write(&tmp, small).is_ok() {
                                    let _ = std::fs::rename(&tmp, &path);
                                }
                            }
                        }
                    });
                }
            }
            let uri = crate::platform::file_uri(&path.display().to_string());
            ready.lock().unwrap().insert(url.to_string(), uri.clone());
            return uri;
        }
    }
    let pending = PENDING.get_or_init(Default::default);
    {
        let mut p = pending.lock().unwrap();
        if p.len() >= MAX_FETCHES && !p.contains(url) {
            // too many at once: ask again shortly
            ctx.request_repaint_after(Duration::from_millis(150));
            return String::new();
        }
    }
    if pending.lock().unwrap().insert(url.to_string()) {
        let (url, ctx) = (url.to_string(), ctx.clone());
        std::thread::spawn(move || {
            let res = ureq::get(&url)
                .header("User-Agent", "Mozilla/5.0")
                .call()
                .ok()
                .and_then(|mut r| {
                    let ext = match r.headers().get("content-type").and_then(|v| v.to_str().ok()) {
                        Some(t) if t.contains("webp") => "webp",
                        Some(t) if t.contains("png") => "png",
                        _ => "jpg",
                    };
                    let bytes = r.body_mut().with_config().limit(8_000_000).read_to_vec().ok()?;
                    Some((ext, bytes))
                });
            if let Some((ext, bytes)) = res {
                let (ext, bytes) = shrink(&bytes).unwrap_or((ext, bytes));
                let path = dir.join("img").join(format!("{key}.{ext}"));
                let tmp = dir.join("img").join(format!("{key}.part"));
                if std::fs::write(&tmp, bytes).is_ok() && std::fs::rename(&tmp, &path).is_ok() {
                    let uri = crate::platform::file_uri(&path.display().to_string());
                    READY.get().unwrap().lock().unwrap().insert(url.clone(), uri);
                }
            }
            PENDING.get().unwrap().lock().unwrap().remove(&url);
            ctx.request_repaint();
        });
    }
    String::new()
}
