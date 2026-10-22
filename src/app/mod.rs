//! Application state and logic. Each file adds one area of behaviour to [`App`].

pub mod state;
pub mod init;
pub mod channels;
pub mod play;
pub mod downloads;
pub mod library;
pub mod messages;
pub mod navigation;
pub mod integration;
pub mod frame;

#[allow(unused_imports)]
pub(crate) use state::*;
