//! Prototype modules, exported so integration tests can exercise the pure
//! contract logic without starting a Bevy application.

pub mod character;
pub mod config;
pub mod diagnostics;
pub mod inspection;
pub mod locomotion;
pub mod perf;
pub mod state;

use std::path::{Path, PathBuf};
use thiserror::Error;

/// Overrides asset-root discovery entirely. Set it to the directory that
/// *is* the Bevy asset root (the one containing `characters/`), not to its
/// parent.
pub const ASSET_ROOT_ENV: &str = "BEVY_CONCEPT_WORLD_ASSET_ROOT";

/// How many directories at and above the executable's own directory are
/// searched for `assets/`. Four is enough to reach a repository root from
/// `target/<profile>/` and from `target/<triple>/<profile>/`.
const EXECUTABLE_SEARCH_DEPTH: usize = 4;

/// Which rule produced the asset root, so a run can report where it looked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetRootSource {
    /// Taken verbatim from [`ASSET_ROOT_ENV`].
    Override,
    /// `<current working directory>/assets`; this is the `cargo run` case.
    WorkingDirectory,
    /// `assets` beside the executable, or in one of its parent directories.
    ExecutableDirectory,
}

/// A resolved Bevy asset root and the rule that found it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetRoot {
    pub path: PathBuf,
    pub source: AssetRootSource,
}

/// Why asset-root discovery failed. Every variant names the exact paths
/// involved, because "assets not found" without the candidate list is not
/// actionable.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AssetRootError {
    #[error("{env} is set to '{path}', which is not an existing directory")]
    OverrideMissing { env: &'static str, path: PathBuf },

    #[error("no asset root found; looked for: {}", format_candidates(.candidates))]
    NotFound { candidates: Vec<PathBuf> },

    #[error("could not determine the {what}: {reason}")]
    Environment { what: &'static str, reason: String },
}

fn format_candidates(candidates: &[PathBuf]) -> String {
    if candidates.is_empty() {
        "nothing (no working directory and no executable path were available)".to_string()
    } else {
        candidates
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Resolves the Bevy asset root from explicit inputs.
///
/// This is the whole policy, kept free of process state so it can be tested:
///
/// 1. [`ASSET_ROOT_ENV`], if set. A set-but-missing override is an error, not
///    a fallback — silently ignoring it would load a *different* character
///    than the operator asked for.
/// 2. `<working_dir>/assets`. This is what `cargo run` produces, because cargo
///    sets the working directory to the package root.
/// 3. `assets` beside `executable`, then in each of its parents up to
///    [`EXECUTABLE_SEARCH_DEPTH`]. This covers both a copied binary shipped
///    next to its own `assets/` directory and `target/release/…` launched from
///    an unrelated working directory.
///
/// `is_dir` decides whether a candidate exists; production passes
/// [`Path::is_dir`].
pub fn resolve_asset_root_from(
    override_path: Option<&Path>,
    working_dir: &Path,
    executable: Option<&Path>,
    is_dir: &dyn Fn(&Path) -> bool,
) -> Result<AssetRoot, AssetRootError> {
    if let Some(path) = override_path {
        return if is_dir(path) {
            Ok(AssetRoot {
                path: path.to_path_buf(),
                source: AssetRootSource::Override,
            })
        } else {
            Err(AssetRootError::OverrideMissing {
                env: ASSET_ROOT_ENV,
                path: path.to_path_buf(),
            })
        };
    }

    let mut candidates = Vec::new();

    let from_working_dir = working_dir.join("assets");
    if is_dir(&from_working_dir) {
        return Ok(AssetRoot {
            path: from_working_dir,
            source: AssetRootSource::WorkingDirectory,
        });
    }
    candidates.push(from_working_dir);

    if let Some(directory) = executable.and_then(Path::parent) {
        for ancestor in directory.ancestors().take(EXECUTABLE_SEARCH_DEPTH) {
            let candidate = ancestor.join("assets");
            if is_dir(&candidate) {
                return Ok(AssetRoot {
                    path: candidate,
                    source: AssetRootSource::ExecutableDirectory,
                });
            }
            candidates.push(candidate);
        }
    }

    Err(AssetRootError::NotFound { candidates })
}

/// Resolves the Bevy asset root from the real process environment using the
/// policy in [`resolve_asset_root_from`].
pub fn resolve_asset_root() -> Result<AssetRoot, AssetRootError> {
    let override_path = std::env::var_os(ASSET_ROOT_ENV)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty());
    let working_dir = std::env::current_dir().map_err(|error| AssetRootError::Environment {
        what: "current working directory",
        reason: error.to_string(),
    })?;
    // A missing executable path is not fatal on its own: the working
    // directory may still hold `assets/`.
    let executable = std::env::current_exe().ok();

    resolve_asset_root_from(
        override_path.as_deref(),
        &working_dir,
        executable.as_deref(),
        &|path| path.is_dir(),
    )
}
