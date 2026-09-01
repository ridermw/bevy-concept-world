//! Prototype modules, exported so integration tests can exercise the pure
//! contract logic without starting a Bevy application.

pub mod character;
pub mod config;
pub mod diagnostics;
pub mod inspection;
pub mod state;

use std::path::{Path, PathBuf};

/// The Bevy asset root, resolved from the crate directory so the working
/// directory the binary is launched from does not matter.
pub fn asset_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")
}
