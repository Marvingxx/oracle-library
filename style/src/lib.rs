//! Oracle's own look, shared by every app of the suite.
//!
//! It comes from Marving's studio identity (the LAB site and the brand material in *Graphic
//! design*): a near-black ground, the application's icon accent for active elements, paper white
//! text; sharp corners and hairlines; labels set in JetBrains Mono capitals with wide tracking,
//! a short dash before them; HUD corner marks around what is framed; a fine grid behind the work.
//!
//! - [`palette`]: the colours, for the dark and the light interface;
//! - [`fonts`]: Space Grotesk (interface) and JetBrains Mono (labels, numbers), built in;
//! - [`hud`]: corner marks, the grid, labels and tags.

pub mod chrome;
pub mod fonts;
pub mod hud;
pub mod palette;

pub use palette::Palette;
