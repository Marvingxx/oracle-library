//! What ties the Oracle apps together, shared by all of them (they already share the library):
//!
//! - [`App`]: the Oracle apps, where each one is installed and its command-line engine;
//! - [`OracleFormat`]: Oracle's own file formats and the app that makes each one;
//! - [`inbox`]: asking another Oracle app to place or open a file (Send to), starting it if needed;
//! - [`render`]: a native file drawn by the app that made it (an `.oracle-vector` logo as SVG by
//!   Oracle Vector, a `.pcraft` as PNG by Oracle Edit), cached and redrawn when the file changes, so
//!   any app can place it without an export step.
//!
//! Nothing here needs a window: engines and command-line tools use it as well as interfaces.

// No panics in shipped code; tests may unwrap.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]

mod apps;
mod formats;
pub mod inbox;
mod render;

pub use apps::{App, edit_original, open_in_app};
pub use formats::{OracleFormat, Preview, oracle_format};
pub use render::{Rendered, cache_dir, cached, refresh_stable, render, render_stable, stable_source};
