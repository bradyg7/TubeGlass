//! Podcasts: subscribing to RSS feeds, reading them, and turning episodes into playable streams.
//! (The pages that show them live in `ui/podcasts_view.rs`.)

mod model;
mod feed;
mod stream;
mod subscriptions;

#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use model::*;
pub use feed::*;
pub use stream::*;
