//! Finding playable streams: tries several innertube clients, verifies the links and builds the player's stream list.

use super::*;

/// Value of `NAME="..."` / `NAME=value` in an HLS attribute list.
pub(crate) fn hls_attr(line: &str, name: &str) -> Option<String> {
    let key = format!("{name}=");
    let mut from = 0;
    while let Some(i) = line[from..].find(&key) {
        let at = from + i;
        // must be a whole attribute name (start of list or after a comma)
        if at == 0 || line.as_bytes()[at - 1] == b',' || line.as_bytes()[at - 1] == b':' {
            let rest = &line[at + key.len()..];
            return Some(if let Some(q) = rest.strip_prefix('"') {
                q.split('"').next().unwrap_or("").to_string()
            } else {
                rest.split(',').next().unwrap_or("").to_string()
            });
        }
        from = at + key.len();
    }
    None
}

/// Reads the quality variants (and audio renditions) of an HLS master playlist.
pub(crate) fn parse_hls_variants(master: &str, manifest_url: &str, ua: &str) -> Vec<VideoOpt> {
    let absolute = |u: &str| -> String {
        if u.starts_with("http") {
            u.to_string()
        } else {
            let base = manifest_url.rsplit_once('/').map(|x| x.0).unwrap_or("");
            format!("{base}/{u}")
        }
    };
    // audio renditions by group id (prefer the one marked DEFAULT, else the first)
    let mut audio: std::collections::HashMap<String, (String, bool)> = std::collections::HashMap::new();
    for line in master.lines() {
        if let Some(attrs) = line.strip_prefix("#EXT-X-MEDIA:") {
            if hls_attr(attrs, "TYPE").as_deref() == Some("AUDIO") {
                if let (Some(g), Some(u)) = (hls_attr(attrs, "GROUP-ID"), hls_attr(attrs, "URI")) {
                    let is_default = hls_attr(attrs, "DEFAULT").as_deref() == Some("YES");
                    let better = audio.get(&g).is_none_or(|(_, d)| is_default && !*d);
                    if better {
                        audio.insert(g, (absolute(&u), is_default));
                    }
                }
            }
        }
    }
    let mut out = vec![];
    let mut pending: Option<(u32, u32, Option<String>)> = None;
    for line in master.lines() {
        if let Some(attrs) = line.strip_prefix("#EXT-X-STREAM-INF:") {
            let res = hls_attr(attrs, "RESOLUTION")
                .and_then(|r| r.split_once('x').map(|(w, h)| (w.parse().unwrap_or(0), h.parse().unwrap_or(0))));
            let audio_url = hls_attr(attrs, "AUDIO").and_then(|g| audio.get(&g).map(|(u, _)| u.clone()));
            pending = res.map(|(w, h)| (w, h, audio_url));
        } else if !line.starts_with('#') && !line.trim().is_empty() {
            if let Some((width, height, audio_url)) = pending.take() {
                out.push(VideoOpt {
                    url: absolute(line.trim()),
                    width,
                    height,
                    ua: ua.to_string(),
                    // a variant with its own audio group is video-only on its own
                    muxed: audio_url.is_none(),
                    hls: true,
                    audio_url,
                });
            }
        }
    }
    out
}

/// One innertube client to ask for streams.
pub(crate) struct ClientDef {
    pub(crate) name: &'static str,
    pub(crate) id: u32,
    pub(crate) ua: &'static str,
    pub(crate) extra: Value,
    pub(crate) version: &'static str,
}

/// The fallback clients, tried in parallel when the primary one fails.
pub(crate) fn clients() -> Vec<ClientDef> {
    vec![
        ClientDef {
            name: "IOS",
            id: 5,
            version: "20.10.4",
            ua: IOS_UA,
            extra: json!({"deviceMake":"Apple","deviceModel":"iPhone16,2","osName":"iPhone","osVersion":"18.3.2.22D82"}),
        },
        ClientDef {
            name: "ANDROID",
            id: 3,
            version: "20.10.38",
            ua: "com.google.android.youtube/20.10.38 (Linux; U; Android 11) gzip",
            extra: json!({"osName":"Android","osVersion":"11","androidSdkVersion":30}),
        },
        ClientDef {
            name: "ANDROID_VR",
            id: 28,
            version: "1.65.10",
            ua: "com.google.android.apps.youtube.vr.oculus/1.65.10 (Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A1) gzip",
            extra: json!({"deviceMake":"Oculus","deviceModel":"Quest 3","osName":"Android","osVersion":"12L","androidSdkVersion":32}),
        },
    ]
}

/// Asks one client for the player response of a video.
pub(crate) fn player_response(id: &str, c: &ClientDef, visitor: &str) -> Res<Value> {
    let mut client = json!({
        "clientName": c.name, "clientVersion": c.version, "hl": "en", "gl": "US",
        "visitorData": visitor,
    });
    for (k, v) in c.extra.as_object().unwrap() {
        client[k] = v.clone();
    }
    let body = json!({
        "videoId": id, "contentCheckOk": true, "racyCheckOk": true,
        "context": { "client": client }
    });
    retry(|| {
        let mut req = agent()
            .post("https://www.youtube.com/youtubei/v1/player?prettyPrint=false")
            .header("User-Agent", c.ua)
            .header("Cookie", COOKIE)
            .header("X-YouTube-Client-Name", c.id.to_string())
            .header("X-YouTube-Client-Version", c.version);
        if !visitor.is_empty() {
            req = req.header("X-Goog-Visitor-Id", visitor);
        }
        req.send_json(body.clone())
            .map_err(|e| format!("network error: {e}"))?
            .body_mut()
            .read_json()
            .map_err(|e| format!("bad response: {e}"))
    })
}

/// NewPipe's primary client: VISIONOS, via youtubei.googleapis.com, with a matching
/// visitorData, a content-playback nonce, and the `t`/`id` query that mobile apps send.
pub(crate) fn visionos_response(id: &str) -> Res<(Value, String)> {
    let mut client = json!({
        "clientName": "VISIONOS", "clientVersion": "1.04", "clientScreen": "WATCH",
        "platform": "MOBILE", "deviceMake": "Apple", "deviceModel": "RealityDevice17,1",
        "osName": "visionOS", "osVersion": "26.6.0.23O770",
        "hl": "en", "gl": "US", "utcOffsetMinutes": 0,
    });
    let ctx = |client: &Value| {
        json!({
            "context": {
                "client": client,
                "request": { "internalExperimentFlags": [], "useSsl": true },
                "user": { "lockedSafetyMode": false }
            }
        })
    };
    let post = |url: &str, body: Value| -> Res<Value> {
        retry(|| {
            agent()
                .post(url)
                .header("User-Agent", VISIONOS_UA)
                .header("X-Goog-Api-Format-Version", "2")
                .send_json(body.clone())
                .map_err(|e| format!("network error: {e}"))?
                .body_mut()
                .read_json()
                .map_err(|e| format!("bad response: {e}"))
        })
    };
    let vd = post(
        "https://youtubei.googleapis.com/youtubei/v1/visitor_id?prettyPrint=false",
        ctx(&client),
    )?;
    if let Some(v) = vd.pointer("/responseContext/visitorData").and_then(Value::as_str) {
        client["visitorData"] = json!(v);
    }
    let cpn = random_id(16);
    let mut body = ctx(&client);
    body["videoId"] = json!(id);
    body["cpn"] = json!(cpn);
    body["contentCheckOk"] = json!(true);
    body["racyCheckOk"] = json!(true);
    let url = format!(
        "https://youtubei.googleapis.com/youtubei/v1/player?prettyPrint=false&t={}&id={id}",
        random_id(12)
    );
    Ok((post(&url, body)?, cpn))
}

/// A candidate stream before it is verified.
#[derive(Clone)]
pub(crate) struct Cand {
    pub(crate) url: String,
    pub(crate) ua: String,
    pub(crate) mime: String,
    pub(crate) itag: u64,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) bitrate: u64,
    pub(crate) clen: u64,
    pub(crate) muxed: bool,
    /// Audio track id for multi-language videos (`en.4`, `es.3`, …), empty otherwise.
    pub(crate) track: String,
    /// The creator's original audio (or a video with a single audio track).
    pub(crate) original: bool,
    /// Came from the primary client whose URLs are known to stream fully.
    pub(crate) trusted: bool,
}

/// googlevideo hands out URLs that die after ~1 MB when it wants a PO token, so every
/// candidate is verified with a ranged read deep inside the file before it is trusted.
pub(crate) fn probe(c: &Cand) -> bool {
    let at = if c.clen > 6_000_000 { c.clen / 2 } else { 2_500_000.min(c.clen.saturating_sub(200)) };
    agent()
        .get(&c.url)
        .header("User-Agent", &c.ua)
        .header("Range", format!("bytes={at}-{}", at + 63))
        .call()
        .map(|r| r.status().as_u16() == 206 || r.status().as_u16() == 200)
        .unwrap_or(false)
}

/// Resolve playable stream URLs for a video id.
pub fn streams(id: &str) -> Res<Streams> {
    streams_opt(id, false)
}

/// Same, but skipping the primary client: used when its streams were refused at playback.
/// The other clients' streams are tested before they are offered.
pub fn streams_alt(id: &str) -> Res<Streams> {
    streams_opt(id, true)
}

/// Finds streams; `alt` skips the primary client.
pub(crate) fn streams_opt(id: &str, alt: bool) -> Res<Streams> {
    let mut title = String::new();
    let mut duration = 0.0;
    let mut is_live = false;
    let mut description = String::new();
    let mut storyboard: Option<Storyboard> = None;
    let mut captions: Vec<CaptionTrack> = vec![];
    let mut hls: Option<(String, String)> = None;
    let mut cands: Vec<Cand> = vec![];
    let mut last_err = String::from("no playable streams found");

    // (client name, user agent, cpn to append to stream urls, response)
    let mut responses: Vec<(&'static str, String, Option<String>, Res<Value>)> = vec![];
    if alt {
        responses.push(("VISIONOS", VISIONOS_UA.to_string(), None, Err("skipped".into())));
    } else {
        match visionos_response(id) {
            Ok((v, cpn)) => responses.push(("VISIONOS", VISIONOS_UA.to_string(), Some(cpn), Ok(v))),
            Err(e) => responses.push(("VISIONOS", VISIONOS_UA.to_string(), None, Err(e))),
        }
    }
    // Fallback clients are only queried when VISIONOS did not give us a usable set.
    let visionos_ok = responses[0].3.as_ref().is_ok_and(|v| {
        v.pointer("/playabilityStatus/status").and_then(Value::as_str) == Some("OK")
            && v["streamingData"]["adaptiveFormats"].as_array().is_some_and(|a| a.len() > 4)
    });
    let visionos_live = responses[0].3.as_ref().is_ok_and(|v| {
        v.pointer("/videoDetails/isLive").and_then(Value::as_bool) == Some(true)
            && v["streamingData"]["hlsManifestUrl"].is_null()
    });
    if !visionos_ok || visionos_live {
        let visitor = visitor_data().unwrap_or_default();
        let defs = clients();
        let more: Vec<Res<Value>> = std::thread::scope(|sc| {
            let hs: Vec<_> = defs
                .iter()
                .map(|c| {
                    let v = visitor.as_str();
                    sc.spawn(move || player_response(id, c, v))
                })
                .collect();
            hs.into_iter()
                .map(|h| h.join().unwrap_or_else(|_| Err("thread panic".into())))
                .collect()
        });
        for (c, r) in defs.iter().zip(more) {
            responses.push((c.name, c.ua.to_string(), None, r));
        }
    }
    for (cname, cua, cpn, resp) in responses {
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                last_err = e;
                continue;
            }
        };
        let status = resp.pointer("/playabilityStatus/status").and_then(Value::as_str).unwrap_or("");
        if status != "OK" {
            last_err = resp
                .pointer("/playabilityStatus/reason")
                .and_then(Value::as_str)
                .unwrap_or("not playable")
                .to_string();
            continue;
        }
        if title.is_empty() {
            title = resp.pointer("/videoDetails/title").and_then(Value::as_str).unwrap_or("").to_string();
            duration = resp
                .pointer("/videoDetails/lengthSeconds")
                .and_then(Value::as_str)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            is_live = resp.pointer("/videoDetails/isLive").and_then(Value::as_bool).unwrap_or(false);
            description = resp
                .pointer("/videoDetails/shortDescription")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            captions = resp
                .pointer("/captions/playerCaptionsTracklistRenderer/captionTracks")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|t| {
                            Some(CaptionTrack {
                                lang: t["languageCode"].as_str()?.to_string(),
                                name: t
                                    .pointer("/name/simpleText")
                                    .or_else(|| t.pointer("/name/runs/0/text"))
                                    .and_then(Value::as_str)
                                    .unwrap_or("Captions")
                                    .to_string(),
                                auto: t["kind"].as_str() == Some("asr"),
                                url: t["baseUrl"].as_str()?.to_string(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            storyboard = resp
                .pointer("/storyboards/playerStoryboardSpecRenderer/spec")
                .and_then(Value::as_str)
                .and_then(parse_storyboard);
        }
        let sd = &resp["streamingData"];
        if hls.is_none() {
            if let Some(h) = sd["hlsManifestUrl"].as_str() {
                hls = Some((h.to_string(), cua.clone()));
            }
        }
        for (list, muxed) in [("adaptiveFormats", false), ("formats", true)] {
            for f in sd[list].as_array().into_iter().flatten() {
                let Some(url) = f["url"].as_str() else { continue };
                cands.push(Cand {
                    url: match &cpn {
                        Some(n) => format!("{url}&cpn={n}"),
                        None => url.to_string(),
                    },
                    ua: cua.clone(),
                    mime: f["mimeType"].as_str().unwrap_or("").to_string(),
                    itag: f["itag"].as_u64().unwrap_or(0),
                    width: f["width"].as_u64().unwrap_or(0) as u32,
                    height: f["height"].as_u64().unwrap_or(0) as u32,
                    bitrate: f["bitrate"].as_u64().unwrap_or(0),
                    clen: f["contentLength"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0),
                    muxed,
                    track: f.pointer("/audioTrack/id").and_then(Value::as_str).unwrap_or("").to_string(),
                    // YouTube marks the creator's own audio with audioIsDefault, and its
                    // stream url carries xtags=acont=original; dubs say acont=dubbed.
                    original: match f.pointer("/audioTrack/audioIsDefault").and_then(Value::as_bool) {
                        Some(d) => d,
                        None => !url.contains("acont%3Ddubbed") && !url.contains("acont=dubbed"),
                    },
                    trusted: cname == "VISIONOS" && cpn.is_some(),
                });
            }
        }
        if is_live && hls.is_some() {
            break;
        }
        // Enough material from this client? (separate video + audio already present)
        if cname == "VISIONOS" && !is_live && visionos_ok {
            break;
        }
    }

    if is_live || (cands.is_empty() && hls.is_some()) {
        if let Some((h, ua)) = hls {
            let master = get_text(&h)?;
            let mut video = parse_hls_variants(&master, &h, &ua);
            if video.is_empty() {
                video.push(VideoOpt { url: h, width: 1280, height: 720, ua, muxed: true, hls: true, audio_url: None });
            }
            video.sort_by_key(|v| v.height);
            let chapters = if is_live { vec![] } else { parse_chapters(&description, duration) };
            return Ok(Streams { captions, chapters, storyboard, title, is_live, duration, video, audio: None });
        }
    }
    if cands.is_empty() {
        return Err(last_err);
    }

    // Verify candidates in parallel; only keep codecs/containers ffmpeg + our pipeline handle.
    let usable = |c: &Cand| {
        c.mime.starts_with("audio/") && c.mime.contains("mp4a")
            || c.mime.starts_with("video/") && (c.muxed || c.mime.contains("avc1"))
    };
    // Videos with auto-dubbing list one audio stream per language under the same itag.
    // Keep only the original track (when there is one), so a dub is never picked by accident.
    let has_original = cands.iter().any(|c| c.mime.starts_with("audio/") && c.original);
    if has_original {
        cands.retain(|c| !c.mime.starts_with("audio/") || c.original);
    }
    let mut seen = std::collections::HashSet::new();
    let todo: Vec<Cand> = cands
        .into_iter()
        .filter(usable)
        .filter(|c| seen.insert((c.itag, c.muxed, c.track.clone())))
        .collect();
    let ok: Vec<bool> = std::thread::scope(|sc| {
        let hs: Vec<_> = todo.iter().map(|c| sc.spawn(move || c.trusted || probe(c))).collect();
        hs.into_iter().map(|h| h.join().unwrap_or(false)).collect()
    });
    let good: Vec<Cand> = todo.into_iter().zip(ok).filter(|(_, o)| *o).map(|(c, _)| c).collect();

    // audio: best verified audio-only, else the audio of a verified muxed format
    let audio_only = good
        .iter()
        .filter(|c| c.mime.starts_with("audio/"))
        .max_by_key(|c| c.bitrate);
    let muxed_best = good
        .iter()
        .filter(|c| c.muxed && c.mime.starts_with("video/"))
        .max_by_key(|c| c.height);
    let audio = audio_only
        .or(muxed_best)
        .map(|c| AudioSrc { url: c.url.clone(), ua: c.ua.clone() });

    let mut video: Vec<VideoOpt> = vec![];
    let rank = |c: &Cand| (c.mime.contains("avc1") as u8, c.bitrate);
    let mut vids: Vec<&Cand> = good
        .iter()
        .filter(|c| c.mime.starts_with("video/") && !c.muxed && c.height > 0 && c.height <= 1080)
        .collect();
    vids.sort_by_key(|c| std::cmp::Reverse(rank(c)));
    for c in vids {
        if audio.is_some() && !video.iter().any(|v| v.height == c.height) {
            video.push(VideoOpt {
                url: c.url.clone(), width: c.width, height: c.height,
                ua: c.ua.clone(), muxed: false, hls: false, audio_url: None,
            });
        }
    }
    if let Some(m) = muxed_best {
        if !video.iter().any(|v| v.height == m.height) {
            video.push(VideoOpt {
                url: m.url.clone(), width: m.width, height: m.height,
                ua: m.ua.clone(), muxed: true, hls: false, audio_url: None,
            });
        }
    }
    // HLS variants (muxed, no PO token needed) fill in qualities the progressive files lack.
    let best = video.iter().map(|v| v.height).max().unwrap_or(0);
    if let (Some((h, ua)), true) = (&hls, best < 720) {
        if let Ok(master) = get_text(h) {
            for v in parse_hls_variants(&master, h, ua) {
                if v.height <= 1080 && !video.iter().any(|x| x.height >= v.height) {
                    video.push(v);
                }
            }
        }
    }
    video.sort_by_key(|v| v.height);
    if video.is_empty() {
        return Err("YouTube is withholding the full streams for this video (it wants a PO token).".into());
    }
    let _ = good.iter().map(|c| c.itag).count();
    let chapters = if is_live { vec![] } else { parse_chapters(&description, duration) };
    Ok(Streams { captions, chapters, storyboard, title, is_live, duration, video, audio })
}
