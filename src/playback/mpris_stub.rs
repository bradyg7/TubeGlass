//! Stand-in for the Linux media-key service on systems without D-Bus (macOS, Windows):
//! same interface, so the rest of the app does not care. Media keys and the desktop's
//! "now playing" widget are not wired up there; notifications use the system where possible.

#![allow(dead_code)]

use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

/// What the desktop asked us to do (never produced on these systems).
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

/// Snapshot of the player (kept so the app can publish it the same way everywhere).
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

/// State shared with the (absent) desktop service.
pub type Shared = Arc<Mutex<State>>;

/// Returns the shared state and a command channel that never yields anything.
pub fn start() -> (Shared, Receiver<Cmd>) {
    let (_tx, rx) = channel();
    (Arc::new(Mutex::new(State::default())), rx)
}

/// Shows a notification with the system's own facility (macOS); elsewhere it does nothing.
pub fn notify(summary: &str, body: &str) {
    #[cfg(target_os = "macos")]
    {
        // AppleScript string literals use double quotes
        let esc = |s: &str| s.replace('\\', "").replace('"', "'");
        let script = format!("display notification \"{}\" with title \"{}\"", esc(body), esc(summary));
        let _ = std::process::Command::new("osascript").args(["-e", &script]).spawn();
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (summary, body);
}
