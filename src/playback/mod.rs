//! Playing media: the ffmpeg pipeline, the local range proxy, local files and media keys.

pub mod player;
pub mod proxy;
pub mod media;
#[cfg(target_os = "linux")]
pub mod mpris;
#[cfg(not(target_os = "linux"))]
#[path = "mpris_stub.rs"]
pub mod mpris;
