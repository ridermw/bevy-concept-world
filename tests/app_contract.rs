//! Contract tests for the runtime character-selection API.
//!
//! These cover the state-independent decisions the Bevy runtime delegates to
//! pure functions: exact named-asset matching, the actionable content of each
//! rejection, the discovered animation-player count, and the manifest-driven
//! spawn transform. Bevy's own asset loading is verified by the release smoke
//! run recorded in `docs/validation/`, not re-implemented here.

use std::{fs, path::Path};

use bevy::prelude::Vec3;
use bevy_concept_world::{
    character::{
        DiscoveredGltf, SelectionError, character_transform, validate_animation_players,
        validate_named_assets,
    },
    config::load_character_config,
    diagnostics::control_help_lines,
    state::FailureReport,
};

fn discovered(scenes: &[&str], animations: &[&str]) -> DiscoveredGltf {
    DiscoveredGltf {
        scenes: scenes.iter().map(|name| (*name).to_string()).collect(),
        animations: animations.iter().map(|name| (*name).to_string()).collect(),
    }
}

// --- exact named-asset matching -------------------------------------------

#[test]
fn accepts_the_required_scene_and_clip() {
    let found = discovered(&["Scene"], &["Idle_Loop", "Walk_Loop"]);

    assert_eq!(validate_named_assets(&found, "Scene", "Walk_Loop"), Ok(()));
}

#[test]
fn rejects_a_missing_scene() {
    let found = discovered(&["Collection"], &["Walk_Loop"]);

    assert!(matches!(
        validate_named_assets(&found, "Scene", "Walk_Loop"),
        Err(SelectionError::MissingScene { .. })
    ));
}

#[test]
fn rejects_a_renamed_walk_clip() {
    let found = discovered(&["Scene"], &["Walk"]);

    assert!(matches!(
        validate_named_assets(&found, "Scene", "Walk_Loop"),
        Err(SelectionError::MissingAnimation { .. })
    ));
}

#[test]
fn scene_matching_is_exact_not_a_prefix_or_substring() {
    let found = discovered(&["Scene.001", "MyScene"], &["Walk_Loop"]);

    assert!(matches!(
        validate_named_assets(&found, "Scene", "Walk_Loop"),
        Err(SelectionError::MissingScene { .. })
    ));
}

#[test]
fn animation_matching_is_exact_not_a_prefix_or_substring() {
    let found = discovered(&["Scene"], &["Walk_Loop_B", "Fast_Walk_Loop"]);

    assert!(matches!(
        validate_named_assets(&found, "Scene", "Walk_Loop"),
        Err(SelectionError::MissingAnimation { .. })
    ));
}

#[test]
fn a_missing_scene_is_reported_before_a_missing_clip() {
    let found = discovered(&["Collection"], &["Walk"]);

    assert!(matches!(
        validate_named_assets(&found, "Scene", "Walk_Loop"),
        Err(SelectionError::MissingScene { .. })
    ));
}

// --- actionable error content ---------------------------------------------

#[test]
fn reports_the_expected_and_discovered_scene_names() {
    let found = discovered(&["Collection", "Rig"], &["Walk_Loop"]);
    let error = validate_named_assets(&found, "Scene", "Walk_Loop").unwrap_err();
    let message = error.to_string();

    assert!(message.contains("Scene"), "{message}");
    assert!(message.contains("Collection"), "{message}");
    assert!(message.contains("Rig"), "{message}");
}

#[test]
fn reports_the_expected_and_discovered_animation_names() {
    let found = discovered(&["Scene"], &["Idle_Loop", "Jog_Fwd_Loop"]);
    let error = validate_named_assets(&found, "Scene", "Walk_Loop").unwrap_err();
    let message = error.to_string();

    assert!(message.contains("Walk_Loop"), "{message}");
    assert!(message.contains("Idle_Loop"), "{message}");
    assert!(message.contains("Jog_Fwd_Loop"), "{message}");
}

#[test]
fn reports_that_nothing_was_discovered_when_the_gltf_is_empty() {
    let found = discovered(&[], &[]);
    let error = validate_named_assets(&found, "Scene", "Walk_Loop").unwrap_err();

    assert!(error.to_string().contains("none"), "{error}");
}

// --- discovered animation-player count ------------------------------------

#[test]
fn accepts_the_expected_animation_player_count() {
    assert_eq!(validate_animation_players(1, 1), Ok(()));
}

#[test]
fn rejects_a_scene_with_no_animation_player() {
    let error = validate_animation_players(1, 0).unwrap_err();

    assert_eq!(
        error,
        SelectionError::AnimationPlayerCount {
            expected: 1,
            actual: 0,
        }
    );
    let message = error.to_string();
    assert!(message.contains('1'), "{message}");
    assert!(message.contains('0'), "{message}");
}

#[test]
fn rejects_a_scene_with_more_animation_players_than_expected() {
    let error = validate_animation_players(1, 2).unwrap_err();

    assert_eq!(
        error,
        SelectionError::AnimationPlayerCount {
            expected: 1,
            actual: 2,
        }
    );
    assert!(error.to_string().contains('2'), "{error}");
}

// --- manifest-driven spawn transform --------------------------------------

#[test]
fn a_yaw_of_180_degrees_turns_the_model_to_face_bevy_forward() {
    let transform = character_transform(1.0, 180.0);

    // The Quaternius rig faces its own local +Z; Bevy's forward is -Z.
    let facing = transform.rotation * Vec3::Z;
    assert!(
        facing.abs_diff_eq(-Vec3::Z, 1.0e-5),
        "expected the rig to face -Z, got {facing:?}"
    );
    assert!(
        (transform.rotation * Vec3::Y).abs_diff_eq(Vec3::Y, 1.0e-5),
        "a yaw correction must not tip the rig over"
    );
}

#[test]
fn a_yaw_of_zero_degrees_leaves_the_model_unrotated() {
    let transform = character_transform(1.0, 0.0);

    assert!((transform.rotation * Vec3::Z).abs_diff_eq(Vec3::Z, 1.0e-5));
}

#[test]
fn the_manifest_scale_is_applied_uniformly() {
    let transform = character_transform(0.5, 180.0);

    assert!(transform.scale.abs_diff_eq(Vec3::splat(0.5), 1.0e-6));
    assert_eq!(transform.translation, Vec3::ZERO);
}

// --- fatal failures are persistent ----------------------------------------

#[test]
fn a_fresh_failure_report_holds_nothing() {
    let report = FailureReport::default();

    assert!(!report.is_recorded());
    assert_eq!(report.to_display_string(), "");
}

#[test]
fn the_first_recorded_failure_is_the_one_that_is_kept() {
    let mut report = FailureReport::default();

    assert!(report.record("root cause", vec!["expected 1, got 0".into()]));
    assert!(!report.record("later symptom", vec!["nothing to animate".into()]));

    assert_eq!(report.summary, "root cause");
    assert_eq!(report.details, vec!["expected 1, got 0".to_string()]);
}

#[test]
fn a_recorded_failure_displays_its_summary_and_every_detail() {
    let report = FailureReport::new(
        "Character glTF does not match the manifest",
        vec!["asset path: a.glb".into(), "discovered: Walk".into()],
    );
    let shown = report.to_display_string();

    assert!(
        shown.contains("Character glTF does not match the manifest"),
        "{shown}"
    );
    assert!(shown.contains("asset path: a.glb"), "{shown}");
    assert!(shown.contains("discovered: Walk"), "{shown}");
}

// --- the real checked-in humanoid -----------------------------------------

/// Extracts the JSON chunk of a binary glTF container.
///
/// This reads the GLB envelope only — it deliberately does not re-implement
/// Bevy's loader, meshes, skins, or animation sampling. It exists so the
/// manifest's `scene_name` and `animation_name` can be checked against the
/// bytes actually checked in, rather than against fabricated metadata.
fn glb_json(bytes: &[u8]) -> serde_json::Value {
    assert!(bytes.len() >= 20, "GLB is too short to contain a header");
    assert_eq!(&bytes[0..4], b"glTF", "missing GLB magic");
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
        2,
        "expected glTF container version 2"
    );

    let chunk_length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    assert_eq!(&bytes[16..20], b"JSON", "first GLB chunk must be JSON");
    let end = 20 + chunk_length;
    assert!(end <= bytes.len(), "declared JSON chunk runs past the file");

    serde_json::from_slice(&bytes[20..end]).expect("GLB JSON chunk must be valid JSON")
}

fn names(document: &serde_json::Value, key: &str) -> Vec<String> {
    document[key]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry["name"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn the_checked_in_glb_really_declares_the_manifest_scene_and_clip() {
    let asset_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let config = load_character_config(&asset_root).expect("the checked-in contract must load");
    let bytes = fs::read(asset_root.join(&config.gltf_path)).expect("the locked GLB must exist");
    let document = glb_json(&bytes);

    let found = DiscoveredGltf {
        scenes: names(&document, "scenes"),
        animations: names(&document, "animations"),
    };

    assert_eq!(
        validate_named_assets(&found, &config.scene_name, &config.animation_name),
        Ok(()),
        "scenes: {:?}, animations: {:?}",
        found.scenes,
        found.animations
    );
}

#[test]
fn the_checked_in_glb_has_one_skin_matching_the_expected_animation_player_count() {
    let asset_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let config = load_character_config(&asset_root).expect("the checked-in contract must load");
    let bytes = fs::read(asset_root.join(&config.gltf_path)).expect("the locked GLB must exist");
    let document = glb_json(&bytes);

    let skins = document["skins"].as_array().map_or(0, Vec::len);
    assert_eq!(
        skins, config.expected_animation_players,
        "the manifest expects one animation player per animated skeleton"
    );
}

#[test]
fn the_overlay_exposes_the_updated_control_help_lines() {
    assert_eq!(
        control_help_lines(),
        [
            "Arrows: walk/steer/turn around   Q/E: orbit   Wheel: zoom",
            "Space: pause/resume   P: screenshot   Esc: exit",
        ]
    );
}

#[test]
fn main_still_registers_the_locomotion_plugin() {
    let main_rs = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs"));

    assert!(main_rs.contains("LocomotionPlugin"), "{main_rs}");
}

#[test]
fn legacy_space_p_and_escape_handling_remains_in_the_controls_system() {
    let diagnostics = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/diagnostics.rs"));

    assert!(diagnostics.contains("KeyCode::Space"), "{diagnostics}");
    assert!(diagnostics.contains("KeyCode::KeyP"), "{diagnostics}");
    assert!(diagnostics.contains("KeyCode::Escape"), "{diagnostics}");
}
