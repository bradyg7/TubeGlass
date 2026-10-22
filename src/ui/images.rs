//! Loading thumbnails and avatars from the network or the disk cache.

use crate::*;

/// Maps a remote image URL to a `file://` URI of an on-disk copy, downloading it in the
/// background the first time. Returns an empty string until the file is available.
pub(crate) fn img_uri(ctx: &egui::Context, url: &str) -> String {
    use std::hash::{Hash, Hasher};
    use std::sync::{Mutex, OnceLock};
    /// Images that finished loading: address -> local file address.
    static READY: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    /// Images being fetched right now.
    static PENDING: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    if url.starts_with('/') {
        return format!("file://{url}");
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
            let uri = format!("file://{}", path.display());
            ready.lock().unwrap().insert(url.to_string(), uri.clone());
            return uri;
        }
    }
    let pending = PENDING.get_or_init(Default::default);
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
                let path = dir.join("img").join(format!("{key}.{ext}"));
                let tmp = dir.join("img").join(format!("{key}.part"));
                if std::fs::write(&tmp, bytes).is_ok() && std::fs::rename(&tmp, &path).is_ok() {
                    let uri = format!("file://{}", path.display());
                    READY.get().unwrap().lock().unwrap().insert(url.clone(), uri);
                }
            }
            PENDING.get().unwrap().lock().unwrap().remove(&url);
            ctx.request_repaint();
        });
    }
    String::new()
}
