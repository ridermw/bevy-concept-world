//! Character asset contract: parses and validates the checked-in
//! `character.ron` manifest and `asset.lock.ron` integrity lock before any
//! Bevy asset loading takes place.

use bevy::prelude::Resource;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};
use thiserror::Error;

/// Path to the character manifest, relative to the asset root (`assets/`).
const CONFIG_PATH: &str = "characters/quaternius/character.ron";
/// Path to the integrity lock, relative to the asset root (`assets/`).
const LOCK_PATH: &str = "characters/quaternius/asset.lock.ron";

/// The validated humanoid character manifest, checked in at
/// `assets/characters/quaternius/character.ron`.
#[derive(Resource, Debug, Clone, Deserialize)]
pub struct CharacterConfig {
    pub id: String,
    pub gltf_path: String,
    pub source_url: String,
    pub pack_version: String,
    pub downloaded_on: String,
    pub license: String,
    pub license_path: String,
    pub scene_name: String,
    pub animation_name: String,
    pub expected_animation_players: usize,
    pub scale: f32,
    pub yaw_degrees: f32,
    pub root_motion: bool,
}

/// The integrity lock recorded alongside the manifest, checked in at
/// `assets/characters/quaternius/asset.lock.ron`. Kept private: callers only
/// ever observe its effect through [`load_character_config`].
#[derive(Debug, Deserialize)]
struct AssetLock {
    gltf_path: String,
    sha256: String,
    byte_size: u64,
}

/// Failure modes for [`load_character_config`]. Every variant carries the
/// actionable value or path involved so a failure can be diagnosed without
/// re-running the loader under a debugger.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse {path}: {source}")]
    Parse {
        path: PathBuf,
        source: Box<ron::error::SpannedError>,
    },

    #[error("{path} is not valid UTF-8: {source}")]
    Utf8 {
        path: PathBuf,
        source: std::str::Utf8Error,
    },

    #[error("character scale must be positive and finite, got {0}")]
    InvalidScale(f32),

    #[error("expected_animation_players must be at least 1, got 0")]
    ZeroAnimationPlayers,

    #[error("root_motion must be false for this prototype; in-place clips only")]
    RootMotionEnabled,

    #[error("required field '{field}' must not be blank")]
    BlankField { field: &'static str },

    #[error("field '{field}' has an invalid path '{path}': {reason}")]
    InvalidPath {
        field: &'static str,
        path: String,
        reason: &'static str,
    },

    #[error(
        "asset.lock.ron gltf_path '{locked}' does not match character.ron gltf_path '{declared}'"
    )]
    LockPathMismatch { locked: String, declared: String },

    #[error(
        "{path} declares sha256 '{value}', which is not exactly 64 ASCII hex characters \
         (got {length} characters)"
    )]
    InvalidDigest {
        path: PathBuf,
        value: String,
        length: usize,
    },

    #[error(
        "field '{field}' resolves outside the asset root: {resolved} is not contained in {root}"
    )]
    EscapesAssetRoot {
        field: &'static str,
        root: PathBuf,
        resolved: PathBuf,
    },

    #[error("license file {path} exists but is empty")]
    EmptyLicense { path: PathBuf },

    #[error(
        "asset integrity mismatch for {path}: expected sha256={expected_hash} \
         byte_size={expected_size}, got sha256={actual_hash} byte_size={actual_size}"
    )]
    Integrity {
        path: PathBuf,
        expected_hash: String,
        expected_size: u64,
        actual_hash: String,
        actual_size: u64,
    },
}

impl CharacterConfig {
    /// Validates every semantic field that RON deserialization cannot
    /// enforce on its own: non-blank identifiers, safe relative paths, a
    /// positive finite scale, a nonzero expected player count, and
    /// `root_motion: false`.
    fn validate(&self) -> Result<(), ConfigError> {
        check_blank("id", &self.id)?;
        check_blank("source_url", &self.source_url)?;
        check_blank("pack_version", &self.pack_version)?;
        check_blank("downloaded_on", &self.downloaded_on)?;
        check_blank("license", &self.license)?;
        check_blank("scene_name", &self.scene_name)?;
        check_blank("animation_name", &self.animation_name)?;
        validate_relative_path("gltf_path", &self.gltf_path)?;
        validate_relative_path("license_path", &self.license_path)?;

        if !self.scale.is_finite() || self.scale <= 0.0 {
            return Err(ConfigError::InvalidScale(self.scale));
        }
        if self.expected_animation_players == 0 {
            return Err(ConfigError::ZeroAnimationPlayers);
        }
        if self.root_motion {
            return Err(ConfigError::RootMotionEnabled);
        }
        Ok(())
    }
}

fn check_blank(field: &'static str, value: &str) -> Result<(), ConfigError> {
    if value.trim().is_empty() {
        Err(ConfigError::BlankField { field })
    } else {
        Ok(())
    }
}

/// Rejects blank, absolute, and traversing paths. Only a normalized relative
/// path made of plain components is accepted, so a manifest can never point
/// outside `asset_root`.
///
/// The checks are deliberately performed on the raw string rather than on
/// `Path`, because `Path` is platform-specific: `std::path` on Unix reads
/// `C:\Windows\evil.glb` as one ordinary file name, and `..\..` as one
/// ordinary file name too. A manifest is authored once and loaded on every
/// platform, so every absolute syntax and every separator must be rejected on
/// every host. The `Path`-component pass is then kept as a belt-and-braces
/// check for anything the host understands and the string scan did not.
fn validate_relative_path(field: &'static str, raw: &str) -> Result<(), ConfigError> {
    let invalid = |reason: &'static str| ConfigError::InvalidPath {
        field,
        path: raw.to_string(),
        reason,
    };

    if raw.trim().is_empty() {
        return Err(invalid("must not be blank"));
    }
    if raw.starts_with('/') || raw.starts_with('\\') {
        return Err(invalid("must be a relative path, not rooted"));
    }
    if has_drive_prefix(raw) {
        return Err(invalid(
            "must be a relative path, not drive-qualified (e.g. 'C:...')",
        ));
    }

    for segment in raw.split(['/', '\\']) {
        if segment == ".." {
            return Err(invalid("must not contain '..' path traversal"));
        }
    }

    for component in Path::new(raw).components() {
        match component {
            Component::ParentDir => {
                return Err(invalid("must not contain '..' path traversal"));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(invalid("must be relative, not rooted or drive-qualified"));
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }

    Ok(())
}

/// True for a Windows drive-qualified path in either its absolute
/// (`C:\dir`, `C:/dir`) or drive-relative (`C:dir`) form.
fn has_drive_prefix(raw: &str) -> bool {
    let mut chars = raw.chars();
    matches!(
        (chars.next(), chars.next()),
        (Some(drive), Some(':')) if drive.is_ascii_alphabetic()
    )
}

/// Confirms that `path`, after the operating system has resolved every
/// symlink, junction, and `.` component, still lives inside `asset_root`.
/// The relative-path check alone cannot see through a link, so this is what
/// actually keeps a checked-in manifest from reading arbitrary files.
///
/// Both sides are canonicalized so the comparison is between two real paths
/// (this also normalizes macOS's `/var` -> `/private/var` and Windows's
/// verbatim `\\?\` prefix consistently).
fn ensure_within_root(
    field: &'static str,
    asset_root: &Path,
    path: &Path,
) -> Result<(), ConfigError> {
    let root = canonicalize(asset_root)?;
    let resolved = canonicalize(path)?;

    if resolved.starts_with(&root) {
        Ok(())
    } else {
        Err(ConfigError::EscapesAssetRoot {
            field,
            root,
            resolved,
        })
    }
}

fn canonicalize(path: &Path) -> Result<PathBuf, ConfigError> {
    fs::canonicalize(path).map_err(|source| ConfigError::Read {
        path: path.to_owned(),
        source,
    })
}

/// A lock digest must be exactly 64 ASCII hex characters: anything else is a
/// corrupt or hand-edited lock, not a mismatch to report as tampering.
fn validate_digest(path: &Path, value: &str) -> Result<(), ConfigError> {
    if value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(ConfigError::InvalidDigest {
            path: path.to_owned(),
            value: value.to_string(),
            length: value.chars().count(),
        })
    }
}

/// Loads and fully validates the humanoid character contract rooted at
/// `asset_root` (the Bevy asset directory, e.g. `assets/`).
///
/// This reads `character.ron` and `asset.lock.ron` from their fixed relative
/// locations, validates every semantic field, confirms the declared license
/// file exists, is non-empty, and canonically resolves inside `asset_root`,
/// confirms the lock's `gltf_path` matches the manifest's and carries a
/// well-formed digest, and re-hashes the actual GLB to confirm its byte size
/// and SHA-256 match the lock. No Bevy asset loading happens here.
pub fn load_character_config(asset_root: &Path) -> Result<CharacterConfig, ConfigError> {
    let config_path = asset_root.join(CONFIG_PATH);
    let lock_path = asset_root.join(LOCK_PATH);

    let config: CharacterConfig = parse_ron(&config_path)?;
    let lock: AssetLock = parse_ron(&lock_path)?;

    config.validate()?;
    validate_digest(&lock_path, &lock.sha256)?;

    let license_path = asset_root.join(&config.license_path);
    let license_bytes = read(&license_path)?;
    ensure_within_root("license_path", asset_root, &license_path)?;
    if license_bytes.is_empty() {
        return Err(ConfigError::EmptyLicense { path: license_path });
    }

    if lock.gltf_path != config.gltf_path {
        return Err(ConfigError::LockPathMismatch {
            locked: lock.gltf_path,
            declared: config.gltf_path,
        });
    }

    let model_path = asset_root.join(&config.gltf_path);
    let model_bytes = read(&model_path)?;
    ensure_within_root("gltf_path", asset_root, &model_path)?;
    let actual_hash = format!("{:x}", Sha256::digest(&model_bytes));
    let actual_size = model_bytes.len() as u64;
    if !actual_hash.eq_ignore_ascii_case(&lock.sha256) || actual_size != lock.byte_size {
        return Err(ConfigError::Integrity {
            path: model_path,
            expected_hash: lock.sha256,
            expected_size: lock.byte_size,
            actual_hash,
            actual_size,
        });
    }

    Ok(config)
}

/// Parses strict UTF-8 RON. Lossy decoding is deliberately avoided: silently
/// substituting U+FFFD for invalid bytes would let a corrupted manifest parse
/// into a plausible-looking contract.
fn parse_ron<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, ConfigError> {
    let bytes = read(path)?;
    let text = std::str::from_utf8(&bytes).map_err(|source| ConfigError::Utf8 {
        path: path.to_owned(),
        source,
    })?;
    ron::from_str(text).map_err(|source| ConfigError::Parse {
        path: path.to_owned(),
        source: Box::new(source),
    })
}

fn read(path: &Path) -> Result<Vec<u8>, ConfigError> {
    fs::read(path).map_err(|source| ConfigError::Read {
        path: path.to_owned(),
        source,
    })
}
