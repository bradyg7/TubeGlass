//! Helpers for the captions menu (translation languages, saved tracks).

use crate::*;

/// Languages offered for auto-translated captions.
pub(crate) const TRANSLATE_TO: &[(&str, &str)] = &[
    ("en", "English"), ("es", "Spanish"), ("fr", "French"), ("de", "German"), ("pt", "Portuguese"),
    ("it", "Italian"), ("ru", "Russian"), ("ja", "Japanese"), ("ko", "Korean"), ("zh-Hans", "Chinese"),
    ("ar", "Arabic"), ("hi", "Hindi"), ("sw", "Swahili"),
];

/// Saved caption tracks cannot be translated (the translation needs YouTube).
pub(crate) fn is_local_file(tracks: &[yt::CaptionTrack]) -> bool {
    tracks.iter().any(|t| t.url.starts_with("file:") || t.url.starts_with("podtr:"))
}
