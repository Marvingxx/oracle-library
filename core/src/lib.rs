//! Oracle's asset library: indexes a folder of creative resources (textures, brushes, fonts,
//! vectors, mockups…) and searches it in French and English, accents or not.

// No panics in shipped code; tests may unwrap.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]

pub mod fold;
pub mod index;
pub mod ingest;
pub mod search;
pub mod user;

pub use index::{Asset, FormatGroup, Library};
pub use search::{Query, search};
pub use user::UserData;
