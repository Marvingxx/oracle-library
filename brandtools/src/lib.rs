//! Oracle's brand tools: logo grids (base, construction, clearspace, lockups) and bento layouts.
//!
//! Functional equivalents of the tools designers use to present a logo's construction, written for
//! Oracle from what such tools are documented to do (Akrivi Studio's public guides were the
//! reference for the feature list); none of their code or templates is used. Everything is computed
//! from the logo's own vector outlines and returned as plain layers ([`Drawing`]) that Oracle Vector,
//! Oracle Layout and Brand Kit turn into their own objects.

pub mod base;
pub mod bento;
pub mod clearspace;
pub mod construction;
pub mod drawing;
pub mod geom;
pub mod lockup;
pub mod logo;

pub use base::{BaseKind, BaseParams, base_grid};
pub use bento::{BentoParams, BentoStyle, bento};
pub use clearspace::{ClearUnit, ClearspaceParams, clearspace};
pub use construction::{CircleFilter, ConstructionParams, Extent, Placement, analyse, construction};
pub use drawing::{Drawing, Guide, Item, Layer, Style};
pub use geom::{Pt, Rect};
pub use lockup::{LockupKind, LockupParams, lockup};
pub use logo::Logo;
