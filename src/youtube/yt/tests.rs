//! Unit tests (the ones that need the network are marked `#[ignore]`).

use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "needs network"]
    fn live_extraction() {
        let ch = resolve_channel("@NASA").expect("resolve");
        println!("{ch:?}");
        let vids = channel_videos(&ch).expect("videos");
        println!("{} videos", vids.len());
        for v in vids.iter().take(8) {
            println!("{:?} | {} | {} | {} | {}", v.kind, v.id, v.title, v.duration, v.meta);
        }
        assert!(vids.len() > 5);
        let first = vids.iter().find(|v| v.kind == Kind::Video).unwrap();
        for id in ["dQw4w9WgXcQ", first.id.as_str()] {
            let s = streams(id).expect("streams");
            println!("{id}: {:?}", s.video.iter().map(|v| (v.height, v.muxed, v.hls)).collect::<Vec<_>>());
        }
        let s = streams(&first.id).expect("streams");
        println!("{} {:?} audio={}", s.title, s.video.iter().map(|v| (v.height, v.muxed)).collect::<Vec<_>>(), s.audio.is_some());
        if let Some(l) = vids.iter().find(|v| v.kind == Kind::Live) {
            let s = streams(&l.id).expect("live streams");
            println!("LIVE {} {:?}", s.title, s.video.iter().map(|v| (v.width, v.height)).collect::<Vec<_>>());
        }
    }

    #[test]
    #[ignore = "needs network"]
    fn stream_timing() {
        for id in ["dQw4w9WgXcQ", "-jPQmjMWy9I", "M3HKLzjvKPc"] {
            let t = std::time::Instant::now();
            let r = streams(id);
            println!("TIMING {id}: {:?} ok={}", t.elapsed(), r.is_ok());
        }
    }

    #[test]
    #[ignore = "needs network"]
    fn new_endpoints() {
        println!("SUGG {:?}", suggestions("linus").unwrap().iter().take(4).collect::<Vec<_>>());
        let pls = search_playlists("nasa").unwrap();
        println!("PLAYLISTS {} first={:?}", pls.len(), pls.first());
        let (t, v) = playlist_videos(&pls[0].id).unwrap();
        println!("PLAYLIST {t} -> {} videos, first={:?}", v.len(), v.first().map(|x| &x.title));
        let live = live_now().unwrap();
        println!("LIVE {} first={:?}", live.len(), live.first().map(|x| (&x.title, &x.channel_name)));
        let pg = comments_page("dQw4w9WgXcQ", None).unwrap();
        let (c, next) = (pg.list, pg.next);
        println!("COMMENTS {} next={} first={:?}", c.len(), next.is_some(), c.first());
        let c2 = comments_page("dQw4w9WgXcQ", next.as_deref()).unwrap().list;
        println!("COMMENTS page2 {}", c2.len());
        let t = std::time::Instant::now();
        let s = streams("dQw4w9WgXcQ").unwrap();
        println!("TIMING streams {:?} qualities={:?}", t.elapsed(), s.video.iter().map(|v| v.height).collect::<Vec<_>>());
    }

    #[test]
    #[ignore = "needs network"]
    fn channel_paging() {
        let ch = Channel { id: "UCLA_DiR1FfKNvjuUpBHmylQ".into(), name: "NASA".into(), avatar: String::new() };
        for tab in ["videos", "streams", "playlists"] {
            let (_, d, next) = channel_tab(&ch, tab).unwrap();
            let n = match &d { TabData::Videos(v) => v.len(), TabData::Playlists(p) => p.len(), _ => 0 };
            println!("PAGING {tab}: first page {n}, token={}", next.is_some());
            if let Some(t) = next {
                let (d2, n2) = channel_more(&ch, tab, &t).unwrap();
                let m = match &d2 { TabData::Videos(v) => v.len(), TabData::Playlists(p) => p.len(), _ => 0 };
                println!("PAGING {tab}: second page {m}, more={}", n2.is_some());
            }
        }
    }

    #[test]
    fn chapters_from_description() {
        let d = "Intro text\n0:00 Intro\n1:30 - Setting up\n05:10 | Deep dive\n1:02:03 Wrap up (long)\nlinks: https://x.y";
        let c = parse_chapters(d, 4000.0);
        assert_eq!(c.len(), 4, "{c:?}");
        assert_eq!(c[0].title, "Intro");
        assert_eq!(c[1].start, 90.0);
        assert_eq!(c[2].title, "Deep dive");
        assert_eq!(c[3].start, 3723.0);
        // not starting at 0:00, or too few entries -> no chapters
        assert!(parse_chapters("1:00 a\n2:00 b\n3:00 c", 600.0).is_empty());
        assert!(parse_chapters("0:00 a\n1:00 b", 600.0).is_empty());
    }

    #[test]
    fn storyboard_tiles() {
        let spec = "https://i.ytimg.com/sb/ID/storyboard3_L$L/$N.jpg?sqp=abc|48#27#100#10#10#0#default#rs$AAA|160#90#128#5#5#5000#M$M#rs$BBB";
        let sb = parse_storyboard(spec).unwrap();
        assert_eq!(sb.width, 160);
        let (url, uv) = sb.frame_at(42.0, 640.0).unwrap();
        assert!(url.contains("storyboard3_L1/M0.jpg"), "{url}");
        assert!(url.contains("sigh=rs%24BBB"), "{url}");
        assert!(uv[2] > uv[0] && uv[3] > uv[1]);
        let (url2, _) = sb.frame_at(630.0, 640.0).unwrap();
        assert!(url2.contains("/M5.jpg"), "{url2}");
    }

    #[test]
    fn search_param_matches_known_values() {
        // the unfiltered "videos" parameter NewPipe uses
        assert_eq!(SearchOpts::default().sp(), "EgIQAfABAQ%3D%3D");
        // filtering by today adds the date field in front of the type
        let today = SearchOpts { date: 2, ..Default::default() }.sp();
        assert_ne!(today, "EgIQAfABAQ%3D%3D");
    }

    #[test]
    #[ignore = "needs network"]
    fn stage_d_live() {
        let st = streams("TnYw0smD1ok").unwrap();
        println!("D captions: {} tracks, chapters {}", st.captions.len(), st.chapters.len());
        if let Some(tr) = st.captions.first() {
            let cues = caption_cues(&tr.url).unwrap();
            println!("D cues: {} first at {:.1}s ({} chars)", cues.len(), cues[0].start, cues[0].text.len());
        }
        let sb = sponsor_segments("dQw4w9WgXcQ");
        println!("D sponsorblock: {:?}", sb.as_ref().map(|v| v.len()));
        let rel = related("TnYw0smD1ok").unwrap();
        println!("D related: {} first={:?}", rel.len(), rel.first().map(|v| (&v.channel_name, v.title.chars().take(40).collect::<String>())));
    }

    #[test]
    #[ignore = "needs network"]
    fn live_debug() {
        for id in ["awQzjn72bI0", "M3HKLzjvKPc"] {
            match streams(id) {
                Ok(s) => println!(
                    "LIVE {id}: live={} opts={:?} audio={}",
                    s.is_live,
                    s.video.iter().map(|v| (v.height, v.muxed, v.hls)).collect::<Vec<_>>(),
                    s.audio.is_some()
                ),
                Err(e) => println!("LIVE {id}: ERR {e}"),
            }
        }
    }

    /// Live streams currently on air must give a playable stream (a regression canary).
    #[test]
    #[ignore = "needs network"]
    fn live_now_streams_resolve() {
        let live = live_now().expect("live list");
        assert!(!live.is_empty(), "no live streams were listed");
        let mut ok = 0;
        for v in live.iter().take(40) {
            match streams(&v.id) {
                Ok(s) => {
                    ok += 1;
                    println!("OK  {} live={} opts={:?} audio={}", v.id, s.is_live, s.video.iter().map(|o| (o.height, o.muxed, o.hls)).collect::<Vec<_>>(), s.audio.is_some());
                }
                Err(e) => println!("ERR {} {}: {e}", v.id, v.title.chars().take(40).collect::<String>()),
            }
        }
        assert!(ok > 0, "none of the live streams could be resolved");
    }

    /// Prints how a named live stream would be opened (debugging aid: TG_LIVE_TITLE=nbc).
    #[test]
    fn dubbed_tracks_never_beat_the_original() {
        let mk = |track: &str, original: bool, br: u64| Cand {
            url: String::new(), ua: String::new(), mime: "audio/mp4; codecs=\"mp4a.40.2\"".into(),
            itag: 140, width: 0, height: 0, bitrate: br, clen: 0, muxed: false,
            track: track.into(), original, trusted: true,
        };
        // the dub comes first and has the higher bitrate – the original must still win
        let mut c = vec![mk("es.3", false, 133_968), mk("hi.3", false, 133_987), mk("en.4", true, 133_134)];
        if c.iter().any(|x| x.original) {
            c.retain(|x| x.original);
        }
        let best = c.iter().max_by_key(|x| x.bitrate).unwrap();
        assert_eq!(best.track, "en.4");
    }

    #[test]
    #[ignore = "needs network"]
    fn mrbeast_audio_is_original() {
        let st = streams("plN7JMbadRg").unwrap();
        let a = st.audio.expect("audio");
        let tags = a.url.split(['?', '&']).find_map(|kv| kv.strip_prefix("xtags=")).unwrap_or("");
        println!("AUDIO xtags={tags}");
        assert!(tags.contains("original") || !tags.contains("dubbed"), "picked a dub: {tags}");
    }

    #[test]
    #[ignore = "needs network"]
    fn shorts_comments_playlist_live() {
        let ch = Channel { id: "UCLA_DiR1FfKNvjuUpBHmylQ".into(), name: "NASA".into(), avatar: String::new() };
        match channel_tab(&ch, "shorts").unwrap().1 {
            TabData::Videos(v) => {
                println!("SHORTS {} items, first = {:?}", v.len(), v.first().map(|x| (&x.id, &x.title, x.short)));
                assert!(v.iter().all(|x| x.short) && v.len() > 5);
            }
            _ => panic!(),
        }
        let p = comments_page("jNQXAC9IVRw", None).unwrap();
        println!("COMMENTS {} sorts={:?} pinned={} hearted={} creator={}",
            p.list.len(), p.sorts.iter().map(|s| (&s.title, s.selected)).collect::<Vec<_>>(),
            p.list.iter().filter(|c| c.pinned.is_some()).count(),
            p.list.iter().filter(|c| c.hearted.is_some()).count(),
            p.list.iter().filter(|c| c.by_creator).count());
        let newest = p.sorts.iter().find(|s| s.title == "Newest").unwrap();
        let q = comments_page("jNQXAC9IVRw", Some(&newest.token)).unwrap();
        println!("NEWEST first: {:?} / top first: {:?}", q.list.first().map(|c| &c.time), p.list.first().map(|c| &c.time));
        let (t, v) = playlist_videos("PL2aBZuCeDwlQvNtHhNJtarm9OewQwVntv").unwrap();
        println!("PLAYLIST {t}: {} videos", v.len());
    }
}
