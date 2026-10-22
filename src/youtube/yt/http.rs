//! HTTP helpers shared by every extractor: one pooled agent, retries, the consent cookie and innertube requests.

use super::*;

/// User agent of the iOS client.
pub const IOS_UA: &str =
    "com.google.ios.youtube/20.10.4 (iPhone16,2; U; CPU iOS 18_3_2 like Mac OS X;)";

/// User agent used for web pages.
pub(crate) const WEB_UA: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0";

/// Consent cookie so YouTube serves pages without the consent wall.
pub(crate) const COOKIE: &str = "CONSENT=YES+1; SOCS=CAI";

/// One shared agent so TLS connections to YouTube are reused between requests.
pub(crate) fn agent() -> ureq::Agent {
    /// The one shared HTTP agent.
    static AGENT: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    AGENT
        .get_or_init(|| {
            ureq::Agent::config_builder()
                .timeout_global(Some(Duration::from_secs(25)))
                .build()
                .into()
        })
        .clone()
}

/// Run a request, and if it fails with a transport error (typically a pooled connection
/// that the server has already closed) try once more on a fresh connection.
pub(crate) fn retry<T>(mut f: impl FnMut() -> Res<T>) -> Res<T> {
    match f() {
        Err(e) if e.starts_with("network error") || e.starts_with("read error") || e.starts_with("bad response") => {
            std::thread::sleep(Duration::from_millis(200));
            f()
        }
        other => other,
    }
}

/// GETs a page as text, retrying once on a transport error.
pub(crate) fn get_text(url: &str) -> Res<String> {
    retry(|| get_text_once(url))
}

/// One attempt of `get_text`.
pub(crate) fn get_text_once(url: &str) -> Res<String> {
    agent()
        .get(url)
        .header("User-Agent", WEB_UA)
        .header("Accept-Language", "en-US,en;q=0.9")
        .header("Cookie", COOKIE)
        .call()
        .map_err(|e| format!("network error: {e}"))?
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("read error: {e}"))
}

/// Extracts `ytInitialData` from a page.
pub(crate) fn initial_data(html: &str) -> Res<Value> {
    let start = html
        .find("ytInitialData = ")
        .ok_or("channel page had no data (is the name right?)")?
        + "ytInitialData = ".len();
    let rest = &html[start..];
    let end = rest.find(";</script>").ok_or("malformed channel page")?;
    serde_json::from_str(&rest[..end]).map_err(|e| format!("bad channel json: {e}"))
}

/// Plain text of a title / text object.
pub(crate) fn text_of(v: &Value) -> String {
    if let Some(s) = v.get("simpleText").and_then(Value::as_str) {
        return s.to_string();
    }
    if let Some(runs) = v.get("runs").and_then(Value::as_array) {
        return runs.iter().filter_map(|r| r["text"].as_str()).collect();
    }
    v.get("content").and_then(Value::as_str).unwrap_or("").to_string()
}

/// Base64 encoding (for protobuf search parameters).
pub(crate) fn b64(bytes: &[u8]) -> String {
    /// The base64 alphabet.
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// Percent-encodes a query.
pub(crate) fn urlenc(q: &str) -> String {
    q.trim()
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            b' ' => "+".into(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// First text found under a key.
pub(crate) fn first_text(v: &Value, key: &str) -> Option<String> {
    match v {
        Value::Object(m) => {
            if let Some(o) = m.get(key) {
                if let Some(t) = o["text"].as_str() {
                    return Some(t.to_string());
                }
            }
            m.values().find_map(|c| first_text(c, key))
        }
        Value::Array(a) => a.iter().find_map(|c| first_text(c, key)),
        _ => None,
    }
}

/// Plain GET for non-YouTube services (lyrics etc.), with its own polite user agent.
pub fn plain_get(url: &str) -> Res<String> {
    retry(|| {
        agent()
            .get(url)
            .header("User-Agent", "TubeGlass/0.1")
            .call()
            .map_err(|e| format!("network error: {e}"))?
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("read error: {e}"))
    })
}

/// POST to the YouTube Music (WEB_REMIX) innertube client.
pub fn music_post(endpoint: &str, mut body: Value) -> Res<Value> {
    /// Cached web client version.
    static VER: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let ver = VER.get_or_init(|| {
        get_text("https://music.youtube.com/")
            .ok()
            .and_then(|h| {
                let k = "\"INNERTUBE_CONTEXT_CLIENT_VERSION\":\"";
                let i = h.find(k)? + k.len();
                Some(h[i..].split('"').next()?.to_string())
            })
            .unwrap_or_else(|| "1.20261013.15.00".into())
    });
    body["context"] = json!({"client": {
        "clientName": "WEB_REMIX", "clientVersion": ver, "hl": "en", "gl": "US"
    }});
    retry(|| {
        agent()
            .post(format!("https://music.youtube.com/youtubei/v1/{endpoint}?prettyPrint=false"))
            .header("User-Agent", WEB_UA)
            .header("Cookie", COOKIE)
            .header("Origin", "https://music.youtube.com")
            .send_json(body.clone())
            .map_err(|e| format!("network error: {e}"))?
            .body_mut()
            .read_json()
            .map_err(|e| format!("bad response: {e}"))
    })
}

/// Finds the first value stored under a key anywhere in a JSON tree.
pub(crate) fn find_key<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    match v {
        Value::Object(m) => m.get(key).or_else(|| m.values().find_map(|c| find_key(c, key))),
        Value::Array(a) => a.iter().find_map(|c| find_key(c, key)),
        _ => None,
    }
}

/// Visitor token and current WEB client version, scraped once from the home page.
pub(crate) fn home_info() -> &'static (Option<String>, String) {
    /// Cached visitor data and client version.
    static CACHE: std::sync::OnceLock<(Option<String>, String)> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| {
        let html = get_text("https://www.youtube.com/").unwrap_or_default();
        let grab = |key: &str| {
            let i = html.find(key)? + key.len();
            Some(html[i..].split('"').next()?.to_string())
        };
        (
            grab("\"visitorData\":\""),
            grab("\"INNERTUBE_CONTEXT_CLIENT_VERSION\":\"").unwrap_or_else(|| "2.20261015.01.00".into()),
        )
    })
}

/// Pre-fetch the cached home-page info so the first comment / fallback request is fast.
pub fn warm() {
    let _ = home_info();
}

/// The visitor id YouTube expects with innertube calls.
pub(crate) fn visitor_data() -> Option<String> {
    home_info().0.clone()
}

/// POST to the WEB (desktop) innertube client, as NewPipe does for comments, browse etc.
pub(crate) fn web_post(endpoint: &str, body: Value) -> Res<Value> {
    retry(|| web_post_once(endpoint, body.clone()))
}

/// One attempt of a WEB-client innertube call.
pub(crate) fn web_post_once(endpoint: &str, mut body: Value) -> Res<Value> {
    let ver = &home_info().1;
    body["context"] = json!({"client": {
        "clientName": "WEB", "clientVersion": ver, "hl": "en", "gl": "US", "utcOffsetMinutes": 0
    }});
    agent()
        .post(format!("https://www.youtube.com/youtubei/v1/{endpoint}?prettyPrint=false"))
        .header("User-Agent", WEB_UA)
        .header("Cookie", COOKIE)
        .header("Origin", "https://www.youtube.com")
        .header("X-YouTube-Client-Name", "1")
        .header("X-YouTube-Client-Version", ver.as_str())
        .send_json(body)
        .map_err(|e| format!("network error: {e}"))?
        .body_mut()
        .read_json()
        .map_err(|e| format!("bad response: {e}"))
}

/// User agent of the visionOS client.
pub(crate) const VISIONOS_UA: &str =
    "com.google.visionos.youtube/1.04(RealityDevice17,1; U; CPU visionOS 26_6_0 like Mac OS X; US)";

/// A random id of `n` characters (client playback nonce).
pub(crate) fn random_id(n: usize) -> String {
    /// Alphabet for random ids.
    const A: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_";
    let mut x = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1)
        ^ (std::process::id() as u64) << 32;
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            A[(x % A.len() as u64) as usize] as char
        })
        .collect()
}
