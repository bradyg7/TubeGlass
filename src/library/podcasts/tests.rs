//! Unit tests (the ones that need the network are marked `#[ignore]`).

use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    const FEED: &str = r#"<?xml version="1.0"?>
<rss xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd" version="2.0"><channel>
<title>The Show</title><itunes:author>Some One</itunes:author>
<description><![CDATA[<p>About <b>things</b></p>]]></description>
<itunes:image href="http://x/cover.jpg"/>
<item><title>Ep 2</title><guid>b</guid><pubDate>Tue, 10 Mar 2020 08:00:00 +0200</pubDate>
<itunes:duration>1:02:03</itunes:duration><enclosure url="http://x/2.mp3" type="audio/mpeg" length="1"/></item>
<item><title>Ep 1</title><pubDate>Mon, 09 Mar 2020 08:00:00 GMT</pubDate>
<itunes:duration>2723</itunes:duration><enclosure url="https://x/1.m4a" type="audio/mp4"/></item>
<item><title>No audio</title></item>
</channel></rss>"#;

    #[test]
    fn feed_is_parsed() {
        let (show, eps) = parse_feed("http://feed", FEED).unwrap();
        assert_eq!(show.title, "The Show");
        assert_eq!(show.author, "Some One");
        assert_eq!(show.image, "http://x/cover.jpg");
        assert_eq!(show.description, "About things");
        assert_eq!(strip_html("a&nbsp;b &lt;b&gt;c&lt;/b&gt; &amp; d &mdash; e"), "a b c & d — e");
        assert_eq!(eps.len(), 2);
        assert_eq!(eps[0].title, "Ep 2");
        assert_eq!(eps[0].duration, 3723.0);
        assert_eq!(eps[1].duration, 2723.0);
        assert_ne!(eps[0].id, eps[1].id);
        assert!(eps[0].published > eps[1].published);
        assert!(parse_feed("http://feed", "<html></html>").is_err());
    }

    #[test]
    fn dates_and_durations() {
        // 2020-03-10 08:00 at +02:00 is 06:00 UTC
        assert_eq!(parse_date("Tue, 10 Mar 2020 08:00:00 +0200"), 1_583_820_000);
        assert_eq!(parse_date("2020-03-10T06:00:00Z"), 1_583_820_000);
        assert_eq!(parse_date("garbage"), 0);
        assert_eq!(parse_duration("45:12"), 2712.0);
        assert_eq!(parse_duration("90"), 90.0);
        assert_eq!(parse_duration("abc"), 0.0);
    }

    #[test]
    fn typed_input_is_understood() {
        assert!(matches!(classify("https://a/feed.xml"), Input::Feed(_)));
        assert!(matches!(classify("feed://a/rss"), Input::Feed(f) if f == "https://a/rss"));
        assert!(matches!(classify("lex fridman"), Input::Search(_)));
    }
}

#[cfg(test)]
mod live {
    use super::*;

    #[test]
    #[ignore = "network"]
    fn search_and_read_a_real_feed() {
        let hits = search("darknet diaries").expect("search");
        println!("hits: {:?}", hits.iter().map(|h| (&h.title, &h.feed)).take(3).collect::<Vec<_>>());
        assert!(!hits.is_empty());
        let (show, eps) = fetch(&hits[0].feed).expect("feed");
        println!("{} by {} — {} episodes; newest: {} ({:.0}s, published {})", show.title, show.author, eps.len(), eps[0].title, eps[0].duration, eps[0].published);
        assert!(eps.len() > 5);
        assert!(eps.iter().all(|e| e.url.starts_with("http")));
        let st = build_streams(&eps[0]);
        println!("stream duration {:.0}s", st.duration);
        assert!(st.duration > 60.0);
    }

    #[test]
    #[ignore = "network"]
    fn decode_speed() {
        let ff = std::env::var("TUBEGLASS_FFMPEG").unwrap_or("ffmpeg".into());
        for q in ["joe rogan", "diary of a ceo", "techcrunch daily crunch", "dateline nbc", "darknet diaries", "bbc"] {
            let Ok(hits) = search(q) else { continue };
            let Some(h) = hits.first() else { continue };
            let Ok((_, eps)) = fetch(&h.feed) else { continue };
            let st = build_streams(&eps[0]);
            let o = &st.video[0];
            let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let url = if o.hls { crate::proxy::serve_hls(&o.url, &o.ua, stop.clone()) } else { crate::proxy::serve(&o.url, &o.ua, stop.clone()) }.unwrap();
            let t = std::time::Instant::now();
            let out = std::process::Command::new(&ff).args(["-hide_banner", "-loglevel", "error", "-nostdin", "-ss", "600", "-i", &url, "-t", "60", "-vn", "-f", "null", "-"]).output().unwrap();
            println!("{} :: hls={} {:.1}s for 60s audio :: {} {}", h.title, o.hls, t.elapsed().as_secs_f32(), out.status, String::from_utf8_lossy(&out.stderr).chars().take(150).collect::<String>());
            stop.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    #[test]
    #[ignore = "network"]
    fn survey_many_shows() {
        for q in ["news", "true crime", "comedy", "history", "science", "business", "sports", "technology", "bbc", "npr", "joe rogan", "kenya", "swahili", "sermon", "meditation"] {
            let Ok(hits) = search(q) else { println!("SEARCH FAIL {q}"); continue };
            for h in hits.iter().take(4) {
                match fetch(&h.feed) {
                    Err(e) => println!("FEED FAIL {} :: {e}", h.title),
                    Ok((_, eps)) => match eps.first() {
                        None => println!("NO EPISODES {}", h.title),
                        Some(e) => {
                            let r = resolve(&e.url);
                            println!("{} :: {} :: {}", if r.is_some() { "ok      " } else { "NORESOLVE" }, h.title, e.url.chars().take(400).collect::<String>());
                        }
                    },
                }
            }
        }
    }
}

#[cfg(test)]
mod transcript_tests {
    use super::*;

    #[test]
    fn transcripts_are_read() {
        let srt = "1\n00:00:01,000 --> 00:00:03,500\nHello <i>there</i>\n\n2\n00:01:02,000 --> 00:01:04,000\nSecond\nline\n";
        let c = parse_srt_vtt(srt);
        assert_eq!(c.len(), 2);
        assert_eq!((c[0].start, c[0].end, c[0].text.as_str()), (1.0, 3.5, "Hello there"));
        assert_eq!(c[1].start, 62.0);
        assert_eq!(c[1].text, "Second line");
        let vtt = "WEBVTT\n\n01:02.000 --> 01:04.000 align:start\nHi\n";
        assert_eq!(parse_srt_vtt(vtt)[0].start, 62.0);
        let j = r#"{"version":"1.0.0","segments":[{"speaker":"A","startTime":2.5,"endTime":4,"body":"Yo"},{"startTime":5,"endTime":6,"body":""}]}"#;
        let c = parse_transcript_json(j);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].text, "Yo");
    }

    #[test]
    fn best_transcript_is_picked() {
        let xml = r#"<rss xmlns:podcast="https://podcastindex.org/namespace/1.0"><channel><title>S</title><item><title>E</title>
<enclosure url="http://x/a.mp3" type="audio/mpeg"/>
<podcast:transcript url="http://x/t.html" type="text/html"/>
<podcast:transcript url="http://x/t.srt" type="application/x-subrip"/>
<podcast:transcript url="http://x/t.json" type="application/json"/></item></channel></rss>"#;
        let (_, eps) = parse_feed("f", xml).unwrap();
        assert_eq!(eps[0].transcript, "http://x/t.json");
    }
}

#[cfg(test)]
mod live_transcript {
    use super::*;

    #[test]
    #[ignore = "network"]
    fn real_transcript() {
        let (show, eps) = fetch("https://feeds.podcastindex.org/pc20.xml").expect("feed");
        let with: Vec<_> = eps.iter().filter(|e| !e.transcript.is_empty()).collect();
        println!("{}: {} episodes, {} with a transcript", show.title, eps.len(), with.len());
        let e = with.first().expect("an episode with a transcript");
        println!("{} -> {}", e.title, e.transcript);
        let cues = fetch_transcript(&e.transcript).expect("cues");
        println!("{} cues, first: {:?}", cues.len(), cues.first().map(|c| (c.start, c.text.chars().take(60).collect::<String>())));
        assert!(cues.len() > 20);
    }
}
