//! Unit tests for the pieces that live at the top of the crate (link parsing, backups).

mod links {
    use crate::*;

    /// Video and playlist addresses are detected; ordinary searches are not.
    #[test]
    fn pasted_links_are_recognised() {
        let pl = |q: &str| matches!(classify_link(q), Link::Playlist(ref id) if id == "PL2aBZuCeDwlQvNtHhNJtarm9OewQwVntv");
        assert!(pl("https://www.youtube.com/playlist?list=PL2aBZuCeDwlQvNtHhNJtarm9OewQwVntv"));
        assert!(pl("https://youtube.com/playlist?list=PL2aBZuCeDwlQvNtHhNJtarm9OewQwVntv&si=abc"));
        let v = |q: &str| matches!(classify_link(q), Link::Video(ref id) if id == "dQw4w9WgXcQ");
        assert!(v("https://www.youtube.com/watch?v=dQw4w9WgXcQ"));
        assert!(v("https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PLabcdefghijk"));
        assert!(v("https://youtu.be/dQw4w9WgXcQ?t=42"));
        assert!(v("https://www.youtube.com/shorts/dQw4w9WgXcQ"));
        // ordinary searches are not links
        assert!(matches!(classify_link("youtube tutorial"), Link::None));
        assert!(matches!(classify_link("how to watch?v=1"), Link::None));
    }

    /// Favorites and Watch later cannot be deleted.
    #[test]
    fn builtin_lists_are_protected() {
        assert!(is_builtin_list("favorites") && is_builtin_list("watchlater"));
        assert!(!is_builtin_list("pl-123"));
    }
}

mod backup {
    use crate::*;

    /// A backup from another app or with odd file names is refused.
    #[test]
    fn restore_rejects_foreign_files() {
        assert!(restore_bundle("not json").is_err());
        assert!(restore_bundle(r#"{"app":"other","files":{}}"#).is_err());
        // names outside the known set are ignored, so nothing is written
        assert!(restore_bundle(r#"{"app":"tubeglass","files":{"../../evil.json":{}}}"#).is_err());
    }
}

#[test]
fn video_age_is_read_from_its_text() {
    use crate::util::age_secs;
    assert_eq!(age_secs("1.2M  •  3d ago"), 3 * 86_400);
    assert_eq!(age_secs("N views  •  Streamed 2 months ago".replace('N', "5").as_str()), 2 * 2_592_000);
    assert_eq!(age_secs("4K  •  12\u{a0}min ago"), 720);
    assert_eq!(age_secs("9K  •  1 year ago"), 31_536_000);
    assert_eq!(age_secs("2w ago"), 2 * 604_800);
    assert!(age_secs("12 watching") == u64::MAX);
    assert!(age_secs("2h ago") < age_secs("1d ago"));
    assert!(age_secs("3w ago") < age_secs("1mo ago"));
}

#[test]
fn list_tools_read_their_inputs() {
    use crate::util::{duration_secs, playlist_ids, view_count};
    assert_eq!(view_count("Chill Music Lab  •  15K  •  2d ago"), Some(15_000));
    assert_eq!(view_count("1.2M  •  3d ago"), Some(1_200_000));
    assert_eq!(view_count("845 views  •  Streamed 2mo ago"), Some(845));
    assert_eq!(view_count("1,234 views"), Some(1234));
    assert_eq!(view_count("MrBeast  •  3d ago"), None);
    assert_eq!(duration_secs("1:02:03"), 3723);
    assert_eq!(duration_secs("4:05"), 245);
    assert_eq!(duration_secs(""), 0);
    let t = "https://www.youtube.com/playlist?list=PLabcdefghijklmnop&si=x https://youtu.be/x?list=PLzzzzzzzzzzzzzz, PLabcdefghijklmnop, hello";
    assert_eq!(playlist_ids(t), vec!["PLabcdefghijklmnop".to_string(), "PLzzzzzzzzzzzzzz".to_string()]);
}
