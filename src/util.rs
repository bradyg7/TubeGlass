//! Small helpers: ids, clocks, text and size formatting, the audio spectrum.

use crate::*;

/// A widget id from any hashable value (egui deprecated `Id::new` in favour of salts).
#[allow(deprecated)]
pub(crate) fn uid(salt: impl std::hash::Hash + std::fmt::Debug) -> egui::Id {
    egui::Id::new(salt)
}

/// `owner/name` of the GitHub project, taken from the `repository` field of Cargo.toml.
pub(crate) fn github_repo() -> Option<String> {
    let r = env!("CARGO_PKG_REPOSITORY").trim_end_matches('/').trim_end_matches(".git");
    r.split_once("github.com/").map(|(_, p)| p.to_string()).filter(|p| p.contains('/'))
}

/// Current unix time in seconds.
pub(crate) fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// "5 min ago" style text for a unix time.
pub(crate) fn ago(ts: u64) -> String {
    let d = now_secs().saturating_sub(ts);
    match d {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", d / 60),
        3600..=86399 => format!("{} h ago", d / 3600),
        _ => format!("{} d ago", d / 86400),
    }
}

/// Formats bytes as KB, MB or GB.
pub(crate) fn fmt_size(b: u64) -> String {
    let b = b as f64;
    if b >= 1e9 { format!("{:.1} GB", b / 1e9) } else if b >= 1e6 { format!("{:.0} MB", b / 1e6) } else { format!("{:.0} KB", b / 1e3) }
}

/// Band levels (0..1) for `samples`, log-spaced 50 Hz – 14 kHz, using Goertzel filters.
pub(crate) fn spectrum(samples: &[f32], rate: u32, bands: usize) -> Vec<f32> {
    let n = samples.len();
    if n < 8 || bands == 0 {
        return vec![0.0; bands];
    }
    let win: Vec<f32> = samples
        .iter()
        .enumerate()
        .map(|(i, s)| s * (0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / (n - 1) as f32).cos()))
        .collect();
    let nyq = rate as f32 * 0.5 * 0.95;
    (0..bands)
        .map(|k| {
            let f = (50.0 * (14000.0f32 / 50.0).powf(k as f32 / (bands - 1).max(1) as f32)).min(nyq);
            let coeff = 2.0 * (std::f32::consts::TAU * f / rate as f32).cos();
            let (mut s1, mut s2) = (0.0f32, 0.0f32);
            for x in &win {
                let s0 = x + coeff * s1 - s2;
                s2 = s1;
                s1 = s0;
            }
            let power = (s1 * s1 + s2 * s2 - coeff * s1 * s2).max(0.0);
            let amp = power.sqrt() / (n as f32 * 0.25);
            let db = 20.0 * (amp + 1e-6).log10();
            // gentle tilt so the highs are visible next to loud bass
            let tilt = 6.0 * (k as f32 / bands as f32);
            ((db + 64.0 - tilt * -1.0) / 54.0).clamp(0.0, 1.0)
        })
        .collect()
}

/// Lower-cases and strips common accents so "nova" finds "Nóva".
pub(crate) fn fold(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
            'ç' | 'ć' | 'č' => 'c',
            'ď' | 'đ' => 'd',
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ę' | 'ě' => 'e',
            'ì' | 'í' | 'î' | 'ï' | 'ī' | 'ı' => 'i',
            'ł' => 'l',
            'ñ' | 'ń' | 'ň' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' => 'o',
            'ř' => 'r',
            'ś' | 'š' | 'ş' => 's',
            'ť' => 't',
            'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' => 'u',
            'ý' | 'ÿ' => 'y',
            'ź' | 'ż' | 'ž' => 'z',
            other => other,
        })
        .collect()
}

/// Formats seconds as m:ss or h:mm:ss.
pub(crate) fn fmt_time(s: f64) -> String {
    let s = s.max(0.0) as u64;
    let (h, m, sec) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 { format!("{h}:{m:02}:{sec:02}") } else { format!("{m}:{sec:02}") }
}
