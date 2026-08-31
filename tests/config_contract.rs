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
            escape(&self.id),
            escape(&self.gltf_path),
            escape(&self.source_url),
            escape(&self.pack_version),
            escape(&self.downloaded_on),
            escape(&self.license),
            escape(&self.license_path),
            escape(&self.scene_name),
            escape(&self.animation_name),
            self.expected_animation_players,
            self.scale,
            self.yaw_degrees,
            self.root_motion,
        )
    }
}

/// Escapes a value for a RON string literal, so a fixture can carry a
/// Windows-style path such as `..\..\evil.glb` without the backslashes being
/// read as escape sequences.
fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
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
            escape(&fixture.gltf_path),
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

/// Overwrites the fixture lock with an explicit digest and size, so tests can
/// exercise malformed, uppercase, and mismatched locks.
fn write_lock(root: &TempDir, gltf_path: &str, sha256: &str, byte_size: usize) {
    let gltf_path = escape(gltf_path);
    fs::write(
        asset_dir(root).join("asset.lock.ron"),
        format!(r#"(gltf_path: "{gltf_path}", sha256: "{sha256}", byte_size: {byte_size})"#),
    )
    .unwrap();
}

fn model_sha256() -> String {
    format!("{:x}", Sha256::digest(MODEL_BYTES))
}

/// Creates a file symlink, returning `false` when the platform refuses (an
/// unprivileged Windows session without Developer Mode). Callers skip rather
/// than fail so the suite stays green on stock Windows checkouts.
fn try_symlink_file(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, link).is_ok()
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, link);
        false
    }
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
fn rejects_a_model_whose_bytes_changed_at_the_same_size() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    let mut changed = MODEL_BYTES.to_vec();
    let last = changed.len() - 1;
    changed[last] ^= 0xff;
    assert_eq!(changed.len(), MODEL_BYTES.len());
    fs::write(asset_dir(&root).join("model.glb"), &changed).unwrap();

    let error = load_character_config(root.path()).unwrap_err();

    match error {
        ConfigError::Integrity {
            expected_hash,
            expected_size,
            actual_hash,
            actual_size,
            ..
        } => {
            assert_eq!(
                expected_size, actual_size,
                "this fixture changes bytes without changing size"
            );
            assert_ne!(expected_hash, actual_hash);
            assert_eq!(expected_hash, model_sha256());
        }
        other => panic!("expected an integrity mismatch, got {other}"),
    }
}

#[test]
fn rejects_a_model_whose_size_changed_behind_the_same_prefix() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    let mut extended = MODEL_BYTES.to_vec();
    extended.extend_from_slice(b" plus trailing bytes");
    fs::write(asset_dir(&root).join("model.glb"), &extended).unwrap();

    let error = load_character_config(root.path()).unwrap_err();

    match error {
        ConfigError::Integrity {
            expected_size,
            actual_size,
            expected_hash,
            actual_hash,
            ..
        } => {
            assert_eq!(expected_size, MODEL_BYTES.len() as u64);
            assert_eq!(actual_size, extended.len() as u64);
            assert_ne!(expected_hash, actual_hash);
        }
        other => panic!("expected an integrity mismatch, got {other}"),
    }
}

#[test]
fn accepts_an_uppercase_lock_digest() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    write_lock(
        &root,
        &fixture.gltf_path,
        &model_sha256().to_uppercase(),
        MODEL_BYTES.len(),
    );

    let config = load_character_config(root.path())
        .expect("a correct digest in uppercase hex must be accepted");

    assert_eq!(config.id, "fixture");
}

#[test]
fn rejects_a_lock_digest_that_is_too_short() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    write_lock(&root, &fixture.gltf_path, "deadbeef", MODEL_BYTES.len());

    let error = load_character_config(root.path()).unwrap_err();

    match error {
        ConfigError::InvalidDigest { value, .. } => assert_eq!(value, "deadbeef"),
        other => panic!("expected a malformed digest error, got {other}"),
    }
}

#[test]
fn rejects_a_lock_digest_that_is_too_long() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    let too_long = format!("{}0", model_sha256());
    write_lock(&root, &fixture.gltf_path, &too_long, MODEL_BYTES.len());

    let error = load_character_config(root.path()).unwrap_err();

    match error {
        ConfigError::InvalidDigest { value, .. } => assert_eq!(value, too_long),
        other => panic!("expected a malformed digest error, got {other}"),
    }
}

#[test]
fn rejects_a_lock_digest_with_non_hex_characters() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    let non_hex = "z".repeat(64);
    write_lock(&root, &fixture.gltf_path, &non_hex, MODEL_BYTES.len());

    let error = load_character_config(root.path()).unwrap_err();

    match error {
        ConfigError::InvalidDigest { value, .. } => assert_eq!(value, non_hex),
        other => panic!("expected a malformed digest error, got {other}"),
    }
}

#[test]
fn rejects_a_lock_digest_with_non_ascii_characters() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    // 64 characters, but not 64 ASCII hex bytes.
    let non_ascii = format!("{}é", &model_sha256()[..63]);
    write_lock(&root, &fixture.gltf_path, &non_ascii, MODEL_BYTES.len());

    let error = load_character_config(root.path()).unwrap_err();

    match error {
        ConfigError::InvalidDigest { value, .. } => assert_eq!(value, non_ascii),
        other => panic!("expected a malformed digest error, got {other}"),
    }
}

#[test]
fn rejects_missing_license_file() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    fs::remove_file(asset_dir(&root).join("LICENSE.txt")).unwrap();

    let error = load_character_config(root.path()).unwrap_err();

    match error {
        ConfigError::Read { path, .. } => {
            assert_eq!(path, asset_dir(&root).join("LICENSE.txt"));
        }
        other => panic!("expected a missing-license read error, got {other}"),
    }
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

/// Every absolute syntax must be rejected on every host, not only on the OS
/// whose `Path` implementation happens to understand it.
#[test]
fn rejects_every_absolute_path_syntax_on_every_platform() {
    let cases = [
        "C:/Windows/System32/evil.glb",
        "C:\\Windows\\System32\\evil.glb",
        "c:/windows/evil.glb",
        "C:evil.glb",
        "/etc/evil.glb",
        "\\etc\\evil.glb",
        "//server/share/evil.glb",
        "\\\\server\\share\\evil.glb",
    ];

    for case in cases {
        let mut fixture = Fixture::default();
        fixture.gltf_path = case.to_string();
        let root = write_fixture(&fixture);

        let result = load_character_config(root.path());

        assert!(
            matches!(
                result,
                Err(ConfigError::InvalidPath {
                    field: "gltf_path",
                    ..
                })
            ),
            "gltf_path {case:?} must be rejected as absolute, got {result:?}"
        );
    }
}

/// `..` must be caught whichever separator spells it, because a manifest is
/// authored once and loaded on every platform.
#[test]
fn rejects_every_traversal_separator_on_every_platform() {
    let cases = [
        "../../etc/evil.glb",
        "..\\..\\etc\\evil.glb",
        "characters/quaternius/../../../evil.glb",
        "characters\\quaternius\\..\\..\\..\\evil.glb",
        "characters/quaternius\\..\\evil.glb",
    ];

    for case in cases {
        let mut fixture = Fixture::default();
        fixture.license_path = case.to_string();
        let root = write_fixture(&fixture);

        let result = load_character_config(root.path());

        assert!(
            matches!(
                result,
                Err(ConfigError::InvalidPath {
                    field: "license_path",
                    ..
                })
            ),
            "license_path {case:?} must be rejected as traversing, got {result:?}"
        );
    }
}

#[test]
fn rejects_a_model_symlinked_outside_the_asset_root() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    let outside = tempdir().unwrap();
    let outside_model = outside.path().join("model.glb");
    // Identical bytes, so only containment — never the digest — can reject it.
    fs::write(&outside_model, MODEL_BYTES).unwrap();
    let inside_model = asset_dir(&root).join("model.glb");
    fs::remove_file(&inside_model).unwrap();

    if !try_symlink_file(&outside_model, &inside_model) {
        eprintln!("skipping: this session cannot create file symlinks");
        return;
    }

    let error = load_character_config(root.path()).unwrap_err();

    match error {
        ConfigError::EscapesAssetRoot {
            field,
            root: reported_root,
            resolved,
        } => {
            assert_eq!(field, "gltf_path");
            assert!(!resolved.starts_with(&reported_root));
            assert!(resolved.ends_with("model.glb"));
        }
        other => panic!("expected an asset-root escape error, got {other}"),
    }
}

#[test]
fn rejects_a_license_symlinked_outside_the_asset_root() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    let outside = tempdir().unwrap();
    let outside_license = outside.path().join("LICENSE.txt");
    fs::write(&outside_license, "CC0 1.0 Universal\n").unwrap();
    let inside_license = asset_dir(&root).join("LICENSE.txt");
    fs::remove_file(&inside_license).unwrap();

    if !try_symlink_file(&outside_license, &inside_license) {
        eprintln!("skipping: this session cannot create file symlinks");
        return;
    }

    let error = load_character_config(root.path()).unwrap_err();

    match error {
        ConfigError::EscapesAssetRoot {
            field,
            root: reported_root,
            resolved,
        } => {
            assert_eq!(field, "license_path");
            assert!(!resolved.starts_with(&reported_root));
            assert!(resolved.ends_with("LICENSE.txt"));
        }
        other => panic!("expected an asset-root escape error, got {other}"),
    }
}

#[test]
fn rejects_config_ron_that_is_not_utf8() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    let mut bytes = fixture.to_ron().into_bytes();
    // 0xff can never begin a valid UTF-8 sequence.
    bytes.insert(0, 0xff);
    fs::write(asset_dir(&root).join("character.ron"), &bytes).unwrap();

    let error = load_character_config(root.path()).unwrap_err();

    match error {
        ConfigError::Utf8 { path, .. } => {
            assert_eq!(path, asset_dir(&root).join("character.ron"));
        }
        other => panic!("expected a UTF-8 error, got {other}"),
    }
}

#[test]
fn rejects_lock_ron_that_is_not_utf8() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    fs::write(
        asset_dir(&root).join("asset.lock.ron"),
        [b'(', 0x80, 0xfe, b')'],
    )
    .unwrap();

    let error = load_character_config(root.path()).unwrap_err();

    match error {
        ConfigError::Utf8 { path, .. } => {
            assert_eq!(path, asset_dir(&root).join("asset.lock.ron"));
        }
        other => panic!("expected a UTF-8 error, got {other}"),
    }
}

#[test]
fn rejects_config_ron_missing_a_required_field() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    let without_animation_name = fixture
        .to_ron()
        .lines()
        .filter(|line| !line.trim_start().starts_with("animation_name:"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!without_animation_name.contains("animation_name"));
    fs::write(
        asset_dir(&root).join("character.ron"),
        &without_animation_name,
    )
    .unwrap();

    let error = load_character_config(root.path()).unwrap_err();

    match error {
        ConfigError::Parse { path, source } => {
            assert_eq!(path, asset_dir(&root).join("character.ron"));
            assert!(
                source.to_string().contains("animation_name"),
                "the parse error must name the omitted field, got {source}"
            );
        }
        other => panic!("expected a parse error for the omitted field, got {other}"),
    }
}

#[test]
fn rejects_lock_ron_missing_a_required_field() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    fs::write(
        asset_dir(&root).join("asset.lock.ron"),
        format!(r#"(gltf_path: "{}", byte_size: 1)"#, fixture.gltf_path),
    )
    .unwrap();

    let error = load_character_config(root.path()).unwrap_err();

    match error {
        ConfigError::Parse { path, source } => {
            assert_eq!(path, asset_dir(&root).join("asset.lock.ron"));
            assert!(
                source.to_string().contains("sha256"),
                "the parse error must name the omitted field, got {source}"
            );
        }
        other => panic!("expected a parse error for the omitted field, got {other}"),
    }
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

    let error = load_character_config(root.path()).unwrap_err();

    match error {
        ConfigError::Read { path, .. } => {
            assert_eq!(path, asset_dir(&root).join("character.ron"));
        }
        other => panic!("expected a missing-manifest read error, got {other}"),
    }
}

#[test]
fn rejects_missing_lock_file() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    fs::remove_file(asset_dir(&root).join("asset.lock.ron")).unwrap();

    let error = load_character_config(root.path()).unwrap_err();

    match error {
        ConfigError::Read { path, .. } => {
            assert_eq!(path, asset_dir(&root).join("asset.lock.ron"));
        }
        other => panic!("expected a missing-lock read error, got {other}"),
    }
}

#[test]
fn rejects_missing_model_file() {
    let fixture = Fixture::default();
    let root = write_fixture(&fixture);
    fs::remove_file(asset_dir(&root).join("model.glb")).unwrap();

    let error = load_character_config(root.path()).unwrap_err();

    match error {
        ConfigError::Read { path, .. } => {
            assert_eq!(path, asset_dir(&root).join("model.glb"));
        }
        other => panic!("expected a missing-model read error, got {other}"),
    }
}

/// Validates the real checked-in Quaternius contract, resolved from the crate
/// manifest directory rather than the process working directory, never a
/// fixture.
#[test]
fn validates_the_real_quaternius_contract() {
    let asset_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");

    let config = load_character_config(&asset_root)
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
