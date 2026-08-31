//! Contract tests for `bevy_concept_world::config`.
//!
//! These tests build real temporary-directory fixtures (never mocks) so the
//! loader's filesystem, RON-parsing, and integrity-hashing behavior is
//! exercised end to end. A final test validates the real checked-in
//! Quaternius contract under `assets/`.
//!
//! Each test constructs a valid `Fixture::default()` and mutates the one
//! field it wants to break; that single-field mutation is the point, so
//! `field_reassign_with_default` is silenced for the whole file.
#![allow(clippy::field_reassign_with_default)]

use bevy_concept_world::config::{ConfigError, load_character_config};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{TempDir, tempdir};

/// Bytes written as the fixture "GLB" — never a real glTF, just known
/// content whose hash and size the fixture lock is built from.
const MODEL_BYTES: &[u8] = b"fixture model bytes used only by config_contract tests";

/// A mutable copy of every `character.ron` field, with valid defaults.
/// Individual tests mutate exactly the field(s) they want to break.
struct Fixture {
    id: String,
    gltf_path: String,
    source_url: String,
    pack_version: String,
    downloaded_on: String,
    license: String,
    license_path: String,
    scene_name: String,
    animation_name: String,
    expected_animation_players: usize,
    scale: f32,
    yaw_degrees: f32,
    root_motion: bool,
}

impl Default for Fixture {
    fn default() -> Self {
        Self {
            id: "fixture".to_string(),
            gltf_path: "characters/quaternius/model.glb".to_string(),
            source_url: "https://example.invalid/model".to_string(),
            pack_version: "1".to_string(),
            downloaded_on: "2026-08-31".to_string(),
            license: "CC0-1.0".to_string(),
            license_path: "characters/quaternius/LICENSE.txt".to_string(),
            scene_name: "Scene".to_string(),
            animation_name: "Walk_Loop".to_string(),
            expected_animation_players: 1,
            scale: 1.0,
            yaw_degrees: 0.0,
            root_motion: false,
        }
    }
}

impl Fixture {
    fn to_ron(&self) -> String {
        format!(
            r#"(
    id: "{}",
    gltf_path: "{}",
    source_url: "{}",
    pack_version: "{}",
    downloaded_on: "{}",
    license: "{}",
    license_path: "{}",
    scene_name: "{}",
    animation_name: "{}",
    expected_animation_players: {},
    scale: {},
    yaw_degrees: {},
    root_motion: {},
)"#,
            self.id,
            self.gltf_path,
            self.source_url,
            self.pack_version,
            self.downloaded_on,
            self.license,
            self.license_path,
            self.scene_name,
            self.animation_name,
            self.expected_animation_players,
            self.scale,
            self.yaw_degrees,
            self.root_motion,
        )
    }
}

/// Writes a complete, self-consistent fixture tree:
/// `<root>/characters/quaternius/{model.glb, LICENSE.txt, character.ron, asset.lock.ron}`.
/// The lock is generated from the actual bytes of `MODEL_BYTES`, so it is
/// correct unless a test deliberately corrupts something afterward.
fn write_fixture(fixture: &Fixture) -> TempDir {
    let root = tempdir().unwrap();
    let asset_dir = root.path().join("characters/quaternius");
    fs::create_dir_all(&asset_dir).unwrap();
    fs::write(asset_dir.join("model.glb"), MODEL_BYTES).unwrap();
    fs::write(asset_dir.join("LICENSE.txt"), "CC0 1.0 Universal\n").unwrap();
    fs::write(asset_dir.join("character.ron"), fixture.to_ron()).unwrap();

    let sha256 = format!("{:x}", Sha256::digest(MODEL_BYTES));
    fs::write(
        asset_dir.join("asset.lock.ron"),
        format!(
            r#"(gltf_path: "{}", sha256: "{}", byte_size: {})"#,
            fixture.gltf_path,
            sha256,
            MODEL_BYTES.len()
        ),
    )
    .unwrap();

    root
}

fn asset_dir(root: &TempDir) -> PathBuf {
    root.path().join("characters/quaternius")
}

#[test]
fn loads_a_valid_character_contract() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);

    let config = load_character_config(root.path()).unwrap();

    assert_eq!(config.id, "fixture");
    assert_eq!(config.gltf_path, "characters/quaternius/model.glb");
    assert_eq!(config.scene_name, "Scene");
    assert_eq!(config.animation_name, "Walk_Loop");
    assert_eq!(config.expected_animation_players, 1);
    assert_eq!(config.scale, 1.0);
    assert!(!config.root_motion);
}

#[test]
fn rejects_a_changed_model() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    fs::write(
        asset_dir(&root).join("model.glb"),
        b"these bytes differ from the lock",
    )
    .unwrap();

    let result = load_character_config(root.path());

    assert!(matches!(result, Err(ConfigError::Integrity { .. })));
}

#[test]
fn rejects_missing_license_file() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    fs::remove_file(asset_dir(&root).join("LICENSE.txt")).unwrap();

    let result = load_character_config(root.path());

    assert!(matches!(result, Err(ConfigError::Read { .. })));
}

#[test]
fn rejects_empty_license_file() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    fs::write(asset_dir(&root).join("LICENSE.txt"), "").unwrap();

    let result = load_character_config(root.path());

    assert!(matches!(result, Err(ConfigError::EmptyLicense { .. })));
}

#[test]
fn rejects_nonpositive_scale() {
    let mut fixture = Fixture::default();
    fixture.scale = 0.0;
    let root = write_fixture(&fixture);

    let result = load_character_config(root.path());

    assert!(matches!(result, Err(ConfigError::InvalidScale(v)) if v == 0.0));
}

#[test]
fn rejects_negative_scale() {
    let mut fixture = Fixture::default();
    fixture.scale = -2.5;
    let root = write_fixture(&fixture);

    let result = load_character_config(root.path());

    assert!(matches!(result, Err(ConfigError::InvalidScale(v)) if v == -2.5));
}

#[test]
fn rejects_nonfinite_scale() {
    let mut fixture = Fixture::default();
    fixture.scale = f32::NAN;
    let root = write_fixture(&fixture);

    let result = load_character_config(root.path());

    assert!(matches!(result, Err(ConfigError::InvalidScale(v)) if v.is_nan()));
}

#[test]
fn rejects_infinite_scale() {
    let mut fixture = Fixture::default();
    fixture.scale = f32::INFINITY;
    let root = write_fixture(&fixture);

    let result = load_character_config(root.path());

    assert!(matches!(result, Err(ConfigError::InvalidScale(v)) if v.is_infinite()));
}

#[test]
fn rejects_zero_expected_animation_players() {
    let mut fixture = Fixture::default();
    fixture.expected_animation_players = 0;
    let root = write_fixture(&fixture);

    let result = load_character_config(root.path());

    assert!(matches!(result, Err(ConfigError::ZeroAnimationPlayers)));
}

#[test]
fn rejects_root_motion_enabled() {
    let mut fixture = Fixture::default();
    fixture.root_motion = true;
    let root = write_fixture(&fixture);

    let result = load_character_config(root.path());

    assert!(matches!(result, Err(ConfigError::RootMotionEnabled)));
}

#[test]
fn rejects_blank_scene_name() {
    let mut fixture = Fixture::default();
    fixture.scene_name = "   ".to_string();
    let root = write_fixture(&fixture);

    let result = load_character_config(root.path());

    assert!(matches!(
        result,
        Err(ConfigError::BlankField {
            field: "scene_name"
        })
    ));
}

#[test]
fn rejects_blank_animation_name() {
    let mut fixture = Fixture::default();
    fixture.animation_name = "".to_string();
    let root = write_fixture(&fixture);

    let result = load_character_config(root.path());

    assert!(matches!(
        result,
        Err(ConfigError::BlankField {
            field: "animation_name"
        })
    ));
}

#[test]
fn rejects_blank_id() {
    let mut fixture = Fixture::default();
    fixture.id = "".to_string();
    let root = write_fixture(&fixture);

    let result = load_character_config(root.path());

    assert!(matches!(
        result,
        Err(ConfigError::BlankField { field: "id" })
    ));
}

#[test]
fn rejects_blank_source_url() {
    let mut fixture = Fixture::default();
    fixture.source_url = "  ".to_string();
    let root = write_fixture(&fixture);

    let result = load_character_config(root.path());

    assert!(matches!(
        result,
        Err(ConfigError::BlankField {
            field: "source_url"
        })
    ));
}

#[test]
fn rejects_blank_license() {
    let mut fixture = Fixture::default();
    fixture.license = "".to_string();
    let root = write_fixture(&fixture);

    let result = load_character_config(root.path());

    assert!(matches!(
        result,
        Err(ConfigError::BlankField { field: "license" })
    ));
}

#[test]
fn rejects_blank_pack_version() {
    let mut fixture = Fixture::default();
    fixture.pack_version = "".to_string();
    let root = write_fixture(&fixture);

    let result = load_character_config(root.path());

    assert!(matches!(
        result,
        Err(ConfigError::BlankField {
            field: "pack_version"
        })
    ));
}

#[test]
fn rejects_blank_downloaded_on() {
    let mut fixture = Fixture::default();
    fixture.downloaded_on = "".to_string();
    let root = write_fixture(&fixture);

    let result = load_character_config(root.path());

    assert!(matches!(
        result,
        Err(ConfigError::BlankField {
            field: "downloaded_on"
        })
    ));
}

#[test]
fn rejects_absolute_gltf_path() {
    let mut fixture = Fixture::default();
    fixture.gltf_path = "C:/Windows/System32/evil.glb".to_string();
    let root = write_fixture(&fixture);

    let result = load_character_config(root.path());

    assert!(matches!(
        result,
        Err(ConfigError::InvalidPath {
            field: "gltf_path",
            ..
        })
    ));
}

#[test]
fn rejects_path_traversal_gltf_path() {
    let mut fixture = Fixture::default();
    fixture.gltf_path = "../../../../etc/model.glb".to_string();
    let root = write_fixture(&fixture);

    let result = load_character_config(root.path());

    assert!(matches!(
        result,
        Err(ConfigError::InvalidPath {
            field: "gltf_path",
            ..
        })
    ));
}

#[test]
fn rejects_absolute_license_path() {
    let mut fixture = Fixture::default();
    fixture.license_path = "/etc/LICENSE.txt".to_string();
    let root = write_fixture(&fixture);

    let result = load_character_config(root.path());

    assert!(matches!(
        result,
        Err(ConfigError::InvalidPath {
            field: "license_path",
            ..
        })
    ));
}

#[test]
fn rejects_path_traversal_license_path() {
    let mut fixture = Fixture::default();
    fixture.license_path = "characters/quaternius/../../../LICENSE.txt".to_string();
    let root = write_fixture(&fixture);

    let result = load_character_config(root.path());

    assert!(matches!(
        result,
        Err(ConfigError::InvalidPath {
            field: "license_path",
            ..
        })
    ));
}

#[test]
fn rejects_lock_config_gltf_path_mismatch() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    let sha256 = format!("{:x}", Sha256::digest(MODEL_BYTES));
    fs::write(
        asset_dir(&root).join("asset.lock.ron"),
        format!(
            r#"(gltf_path: "characters/quaternius/other.glb", sha256: "{}", byte_size: {})"#,
            sha256,
            MODEL_BYTES.len()
        ),
    )
    .unwrap();

    let result = load_character_config(root.path());

    assert!(matches!(result, Err(ConfigError::LockPathMismatch { .. })));
}

#[test]
fn rejects_malformed_config_ron() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    fs::write(
        asset_dir(&root).join("character.ron"),
        "not valid ron( at all",
    )
    .unwrap();

    let result = load_character_config(root.path());

    assert!(matches!(result, Err(ConfigError::Parse { .. })));
}

#[test]
fn rejects_malformed_lock_ron() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    fs::write(
        asset_dir(&root).join("asset.lock.ron"),
        "not valid ron( at all",
    )
    .unwrap();

    let result = load_character_config(root.path());

    assert!(matches!(result, Err(ConfigError::Parse { .. })));
}

#[test]
fn rejects_missing_config_file() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    fs::remove_file(asset_dir(&root).join("character.ron")).unwrap();

    let result = load_character_config(root.path());

    assert!(matches!(result, Err(ConfigError::Read { .. })));
}

#[test]
fn rejects_missing_lock_file() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    fs::remove_file(asset_dir(&root).join("asset.lock.ron")).unwrap();

    let result = load_character_config(root.path());

    assert!(matches!(result, Err(ConfigError::Read { .. })));
}

#[test]
fn rejects_missing_model_file() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    fs::remove_file(asset_dir(&root).join("model.glb")).unwrap();

    let result = load_character_config(root.path());

    assert!(matches!(result, Err(ConfigError::Read { .. })));
}

/// Validates the real checked-in Quaternius contract, run relative to the
/// crate root (Cargo sets the integration test binary's working directory
/// there), never a fixture.
#[test]
fn validates_the_real_quaternius_contract() {
    let config = load_character_config(Path::new("assets"))
        .expect("the checked-in Quaternius contract must be valid");

    assert_eq!(
        config.id,
        "quaternius-universal-animation-library-v3-standard"
    );
    assert_eq!(config.gltf_path, "characters/quaternius/UAL1_Standard.glb");
    assert_eq!(config.license, "CC0-1.0");
    assert_eq!(config.scene_name, "Scene");
    assert_eq!(config.animation_name, "Walk_Loop");
    assert_eq!(config.expected_animation_players, 1);
    assert_eq!(config.scale, 1.0);
    assert_eq!(config.yaw_degrees, 180.0);
    assert!(!config.root_motion);
}
