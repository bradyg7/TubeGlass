//! YouTube extraction without any API key, in the spirit of NewPipe's extractor:
//! channel pages are scraped for `ytInitialData`, streams come from the internal
//! player endpoint using the iOS client (which hands out unciphered URLs).

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;

mod models;
mod http;
mod channel;
mod search;
mod comments;
mod extras;
mod streams;

#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use models::*;
pub use http::*;
pub use channel::*;
pub use search::*;
pub use comments::*;
pub use extras::*;
pub use streams::*;
