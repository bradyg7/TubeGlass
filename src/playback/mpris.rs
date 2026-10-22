//! Desktop integration over D-Bus (pure Rust, zbus):
//!  * MPRIS — media keys, the desktop's "now playing" widget and `playerctl`
//!  * desktop notifications

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{ObjectPath, OwnedValue, Value};
use zbus::{connection::Builder, interface};

/// What the desktop asked us to do.
#[derive(Debug)]
pub enum Cmd {
    PlayPause,
    Play,
    Pause,
    Stop,
    Next,
    Previous,
    SeekBy(i64),
    SetPosition(i64),
    Raise,
}

/// Snapshot of the player that the D-Bus side reads from.
#[derive(Clone, Default, PartialEq)]
pub struct State {
    pub status: String,
    pub title: String,
    pub artist: String,
    pub art: String,
    pub length_us: i64,
    pub position_us: i64,
    pub can_next: bool,
    pub can_prev: bool,
    pub volume: f64,
    pub rate: f64,
    pub track: u64,
}

/// State shared between the D-Bus service and the app.
type Shared = Arc<Mutex<State>>;

/// The `org.mpris.MediaPlayer2` interface (identity and raise / quit).
struct Root {
    tx: Sender<Cmd>,
}

#[interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    /// Brings the window to the front.
    fn raise(&self) {
        let _ = self.tx.send(Cmd::Raise);
    }
    /// Quitting from the desktop is not supported.
    fn quit(&self) {}
    /// Always false.
    #[zbus(property)]
    fn can_quit(&self) -> bool {
        false
    }
    /// Always true.
    #[zbus(property)]
    fn can_raise(&self) -> bool {
        true
    }
    /// Always false (no TrackList interface).
    #[zbus(property)]
    fn has_track_list(&self) -> bool {
        false
    }
    /// Name shown by the desktop's media widget.
    #[zbus(property)]
    fn identity(&self) -> String {
        "TubeGlass".into()
    }
    /// Desktop file name, so the desktop can find the icon.
    #[zbus(property)]
    fn desktop_entry(&self) -> String {
        "tubeglass".into()
    }
    /// No URI schemes can be opened remotely.
    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<String> {
        vec!["file".into()]
    }
    /// No MIME types can be opened remotely.
    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<String> {
        vec![]
    }
}

/// The `org.mpris.MediaPlayer2.Player` interface (play, pause, seek, metadata).
struct Player {
    tx: Sender<Cmd>,
    st: Shared,
}

impl Player {
    /// Copy of the current shared state.
    fn s(&self) -> State {
        self.st.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    /// Skips to the next item.
    fn next(&self) {
        let _ = self.tx.send(Cmd::Next);
    }
    /// Goes to the previous item.
    fn previous(&self) {
        let _ = self.tx.send(Cmd::Previous);
    }
    /// Pauses playback.
    fn pause(&self) {
        let _ = self.tx.send(Cmd::Pause);
    }
    /// Toggles pause.
    fn play_pause(&self) {
        let _ = self.tx.send(Cmd::PlayPause);
    }
    /// Stops playback.
    fn stop(&self) {
        let _ = self.tx.send(Cmd::Stop);
    }
    /// Resumes playback.
    fn play(&self) {
        let _ = self.tx.send(Cmd::Play);
    }
    /// Seeks by `offset` microseconds.
    fn seek(&self, offset: i64) {
        let _ = self.tx.send(Cmd::SeekBy(offset));
    }
    /// Jumps to a position in microseconds.
    fn set_position(&self, _track: ObjectPath<'_>, position: i64) {
        let _ = self.tx.send(Cmd::SetPosition(position));
    }
    /// Opening URIs is not supported.
    fn open_uri(&self, _uri: String) {}

    /// "Playing", "Paused" or "Stopped".
    #[zbus(property)]
    fn playback_status(&self) -> String {
        self.s().status
    }
    /// Repeat mode as MPRIS names it.
    #[zbus(property)]
    fn loop_status(&self) -> String {
        "None".into()
    }
    /// Current playback speed.
    #[zbus(property)]
    fn rate(&self) -> f64 {
        self.s().rate
    }
    /// Whether shuffle is on.
    #[zbus(property)]
    fn shuffle(&self) -> bool {
        false
    }
    /// Title, artist, art and length of what is playing.
    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, OwnedValue> {
        let st = self.s();
        let mut m: HashMap<String, OwnedValue> = HashMap::new();
        let path = ObjectPath::try_from(format!("/org/tubeglass/track/{}", st.track)).unwrap();
        if let Ok(v) = OwnedValue::try_from(Value::from(path)) {
            m.insert("mpris:trackid".into(), v);
        }
        if let Ok(v) = OwnedValue::try_from(Value::from(st.length_us)) {
            m.insert("mpris:length".into(), v);
        }
        if !st.title.is_empty() {
            if let Ok(v) = OwnedValue::try_from(Value::from(st.title.clone())) {
                m.insert("xesam:title".into(), v);
            }
        }
        if !st.artist.is_empty() {
            if let Ok(v) = OwnedValue::try_from(Value::from(vec![st.artist.clone()])) {
                m.insert("xesam:artist".into(), v);
            }
        }
        if !st.art.is_empty() {
            if let Ok(v) = OwnedValue::try_from(Value::from(st.art.clone())) {
                m.insert("mpris:artUrl".into(), v);
            }
        }
        m
    }
    /// Current volume (0–1).
    #[zbus(property)]
    fn volume(&self) -> f64 {
        self.s().volume
    }
    /// Current position in microseconds.
    #[zbus(property)]
    fn position(&self) -> i64 {
        self.s().position_us
    }
    /// Slowest supported speed.
    #[zbus(property)]
    fn minimum_rate(&self) -> f64 {
        0.5
    }
    /// Fastest supported speed.
    #[zbus(property)]
    fn maximum_rate(&self) -> f64 {
        2.0
    }
    /// True when a next item exists.
    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        self.s().can_next
    }
    /// True while something is open.
    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        self.s().can_prev
    }
    /// True while something is open.
    #[zbus(property)]
    fn can_play(&self) -> bool {
        true
    }
    /// True while something is open.
    #[zbus(property)]
    fn can_pause(&self) -> bool {
        true
    }
    /// True while something is open.
    #[zbus(property)]
    fn can_seek(&self) -> bool {
        true
    }
    /// Always true.
    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }
}

/// Start the MPRIS service on its own thread. Returns the shared state to publish to and
/// the receiver of commands coming from the desktop. If there is no session bus the
/// receiver just never yields anything.
pub fn start() -> (Shared, Receiver<Cmd>) {
    let st: Shared = Arc::new(Mutex::new(State { status: "Stopped".into(), rate: 1.0, volume: 1.0, ..Default::default() }));
    let (tx, rx) = channel();
    let shared = st.clone();
    std::thread::spawn(move || {
        zbus::block_on(async move {
            let built = Builder::session()
                .and_then(|b| b.name("org.mpris.MediaPlayer2.tubeglass"))
                .and_then(|b| b.serve_at("/org/mpris/MediaPlayer2", Root { tx: tx.clone() }))
                .and_then(|b| b.serve_at("/org/mpris/MediaPlayer2", Player { tx, st: shared.clone() }));
            let conn = match built {
                Ok(b) => match b.build().await {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("mpris disabled: {e}");
                        return;
                    }
                },
                Err(e) => {
                    eprintln!("mpris disabled: {e}");
                    return;
                }
            };
            let mut last = State::default();
            loop {
                std::thread::sleep(Duration::from_millis(400));
                let now = shared.lock().map(|g| g.clone()).unwrap_or_default();
                if now == last {
                    continue;
                }
                if let Ok(iface) = conn.object_server().interface::<_, Player>("/org/mpris/MediaPlayer2").await {
                    let em: &SignalEmitter<'_> = iface.signal_emitter();
                    let g = iface.get().await;
                    if now.status != last.status {
                        let _ = g.playback_status_changed(em).await;
                    }
                    if now.title != last.title
                        || now.artist != last.artist
                        || now.length_us != last.length_us
                        || now.track != last.track
                        || now.art != last.art
                    {
                        let _ = g.metadata_changed(em).await;
                    }
                    if now.can_next != last.can_next {
                        let _ = g.can_go_next_changed(em).await;
                    }
                    if now.can_prev != last.can_prev {
                        let _ = g.can_go_previous_changed(em).await;
                    }
                    if (now.rate - last.rate).abs() > f64::EPSILON {
                        let _ = g.rate_changed(em).await;
                    }
                    if (now.volume - last.volume).abs() > f64::EPSILON {
                        let _ = g.volume_changed(em).await;
                    }
                }
                last = now;
            }
        });
    });
    (st, rx)
}

/// Show a desktop notification (org.freedesktop.Notifications). Best effort.
pub fn notify(summary: &str, body: &str) {
    let (summary, body) = (summary.to_string(), body.to_string());
    std::thread::spawn(move || {
        let Ok(conn) = zbus::blocking::Connection::session() else { return };
        let hints: HashMap<&str, Value<'_>> = HashMap::new();
        let _ = conn.call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "Notify",
            &("TubeGlass", 0u32, "tubeglass", summary, body, Vec::<String>::new(), hints, 6000i32),
        );
    });
}
