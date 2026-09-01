//! Contract tests for the runtime character-selection API.
//!
//! These cover the state-independent decisions the Bevy runtime delegates to
//! pure functions: exact named-asset matching, the actionable content of each
//! rejection, the discovered animation-player count, and the manifest-driven
//! spawn transform. Bevy's own asset loading is verified by the release smoke
//! run recorded in `docs/validation/`, not re-implemented here.

use std::{fs, path::Path};

use bevy::{
    MinimalPlugins,
    animation::AnimationPlayer,
    app::{App, AppExit},
    asset::{AssetApp, AssetPlugin},
    ecs::{hierarchy::ChildOf, system::RunSystemOnce},
    input::ButtonInput,
    prelude::{
        AnimationGraph, AnimationGraphHandle, AnimationTransitions, Entity, Handle,
        InheritedVisibility, KeyCode, State, Transform, Vec3, ViewVisibility, Visibility, With,
    },
    state::app::{AppExtStates, StatesPlugin},
};
use bevy_concept_world::{
    add_runtime_plugins,
    character::{
        CharacterAssetCatalog, CharacterPlugin, CharacterSelection, CharacterVariant,
        DiscoveredGltf, Humanoid, PhaseSyncError, PreparedCharacterCatalog, PreparedVariant,
        SelectionError, VariantHierarchyReady, VariantReadiness, begin_loading,
        character_transform, phase_synchronized_playback_speeds, spawn_character,
        validate_animation_players, validate_named_assets,
    },
    config::load_character_catalog,
    diagnostics::{
        ControlIntents, DiagnosticsPlugin, character_status_lines, control_help_lines,
        control_intents, handle_controls,
    },
    inspection::InspectionPlugin,
    locomotion::{HumanoidController, LocomotionPlugin, MovementInput, OrbitCamera},
    perf::PerformancePlugin,
    state::FailureReport,
    state::PrototypeState,
};

fn discovered(scenes: &[&str], animations: &[&str]) -> DiscoveredGltf {
    DiscoveredGltf {
        scenes: scenes.iter().map(|name| (*name).to_string()).collect(),
        animations: animations.iter().map(|name| (*name).to_string()).collect(),
    }
}

fn pressed(key: KeyCode) -> ButtonInput<KeyCode> {
    let mut keys = ButtonInput::default();
    keys.press(key);
    keys
}

#[test]
fn phase_sync_guard_adjusts_genuinely_unequal_clip_durations() {
    let reference_duration: f32 = 4.0 / 3.0;
    let technician_duration: f32 = 2.0;

    let speeds =
        phase_synchronized_playback_speeds(Some(reference_duration), Some(technician_duration))
            .expect("both real clip durations are valid");

    assert_eq!(speeds.reference, 1.0);
    assert!(
        (speeds.technician_man - technician_duration / reference_duration).abs() <= f32::EPSILON
    );
    let reference_cycle = reference_duration / speeds.reference;
    let technician_cycle = technician_duration / speeds.technician_man;
    assert!((reference_cycle - technician_cycle).abs() <= f32::EPSILON);
}

#[test]
fn phase_synchronization_rejects_unusable_clip_durations() {
    assert!(matches!(
        phase_synchronized_playback_speeds(None, Some(2.0)),
        Err(PhaseSyncError::MissingDuration {
            variant: CharacterVariant::Reference
        })
    ));
    assert!(matches!(
        phase_synchronized_playback_speeds(Some(4.0 / 3.0), Some(0.0)),
        Err(PhaseSyncError::NonPositiveDuration {
            variant: CharacterVariant::TechnicianMan,
            duration: 0.0
        })
    ));
    assert!(matches!(
        phase_synchronized_playback_speeds(Some(f32::NAN), Some(2.0)),
        Err(PhaseSyncError::NonFiniteDuration {
            variant: CharacterVariant::Reference,
            ..
        })
    ));
    assert!(matches!(
        phase_synchronized_playback_speeds(Some(f32::MIN_POSITIVE), Some(f32::MAX)),
        Err(PhaseSyncError::NonFinitePlaybackSpeed {
            variant: CharacterVariant::TechnicianMan,
            ..
        })
    ));
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
fn glb_chunks(bytes: &[u8]) -> (serde_json::Value, &[u8]) {
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

    let document =
        serde_json::from_slice(&bytes[20..end]).expect("GLB JSON chunk must be valid JSON");
    assert!(
        end + 8 <= bytes.len(),
        "GLB is too short to contain a binary chunk header"
    );
    let binary_length = u32::from_le_bytes(bytes[end..end + 4].try_into().unwrap()) as usize;
    assert_eq!(
        &bytes[end + 4..end + 8],
        b"BIN\0",
        "second GLB chunk must be binary"
    );
    let binary_end = end + 8 + binary_length;
    assert!(
        binary_end <= bytes.len(),
        "declared binary chunk runs past the file"
    );

    (document, &bytes[end + 8..binary_end])
}

fn glb_json(bytes: &[u8]) -> serde_json::Value {
    glb_chunks(bytes).0
}

fn scalar_f32_accessor(
    document: &serde_json::Value,
    binary: &[u8],
    accessor_index: usize,
) -> Vec<f32> {
    let accessor = &document["accessors"][accessor_index];
    assert_eq!(accessor["componentType"], 5126, "accessor must contain f32");
    assert_eq!(accessor["type"], "SCALAR", "accessor must be scalar");

    let buffer_view_index = accessor["bufferView"]
        .as_u64()
        .expect("accessor must reference a buffer view") as usize;
    let buffer_view = &document["bufferViews"][buffer_view_index];
    let count = accessor["count"]
        .as_u64()
        .expect("accessor must declare a count") as usize;
    let start = buffer_view["byteOffset"].as_u64().unwrap_or(0) as usize
        + accessor["byteOffset"].as_u64().unwrap_or(0) as usize;
    let stride = buffer_view["byteStride"].as_u64().unwrap_or(4) as usize;

    (0..count)
        .map(|index| {
            let offset = start + index * stride;
            let bytes: [u8; 4] = binary[offset..offset + 4]
                .try_into()
                .expect("f32 accessor value must fit in the binary chunk");
            f32::from_le_bytes(bytes)
        })
        .collect()
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

fn srgb_channel_to_linear(channel: u8) -> f64 {
    let channel = f64::from(channel) / 255.0;
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

fn assert_close(actual: f64, expected: f64, context: &str) {
    assert!(
        (actual - expected).abs() <= 1.0e-6,
        "{context}: expected {expected}, got {actual}"
    );
}

#[test]
fn the_checked_in_glb_really_declares_the_manifest_scene_and_clip() {
    let asset_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let config = load_character_catalog(&asset_root)
        .expect("the checked-in catalog must load")
        .reference;
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
    let config = load_character_catalog(&asset_root)
        .expect("the checked-in catalog must load")
        .reference;
    let bytes = fs::read(asset_root.join(&config.gltf_path)).expect("the locked GLB must exist");
    let document = glb_json(&bytes);

    let skins = document["skins"].as_array().map_or(0, Vec::len);
    assert_eq!(
        skins, config.expected_animation_players,
        "the manifest expects one animation player per animated skeleton"
    );
}

#[test]
fn the_midcreek_technician_glb_preserves_its_scene_animation_skin_and_player_contract() {
    let asset_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let config = load_character_catalog(&asset_root)
        .expect("the checked-in catalog must load")
        .technician_man;
    let bytes = fs::read(asset_root.join(&config.gltf_path)).expect("the locked GLB must exist");
    let document = glb_json(&bytes);

    assert_eq!(
        names(&document, "scenes").as_slice(),
        std::slice::from_ref(&config.scene_name)
    );
    assert_eq!(
        names(&document, "animations")
            .iter()
            .filter(|name| *name == &config.animation_name)
            .count(),
        1,
        "the selected technician animation must retain its exact unique name"
    );

    let skins = document["skins"]
        .as_array()
        .expect("the technician GLB must declare skins");
    assert_eq!(skins.len(), 1, "the technician GLB must retain one skin");
    assert_eq!(
        skins[0]["name"], "MidcreekTechnicianRig",
        "the technician skin must retain its stable name"
    );

    let skinned_nodes = document["nodes"]
        .as_array()
        .expect("the technician GLB must declare nodes")
        .iter()
        .filter(|node| node.get("skin").is_some())
        .count();
    assert_eq!(
        skinned_nodes, config.expected_animation_players,
        "each skinned hierarchy produces one expected AnimationPlayer"
    );
}

#[test]
fn the_midcreek_technician_walk_loop_sampler_times_start_at_zero() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/characters/midcreek/technician-man/technician-man.glb");
    let bytes = fs::read(path).expect("the generated technician GLB must exist");
    let (document, binary) = glb_chunks(&bytes);
    let walk = document["animations"]
        .as_array()
        .expect("the technician GLB must declare animations")
        .iter()
        .find(|animation| animation["name"] == "Walk_Loop")
        .expect("the technician GLB must declare Walk_Loop");

    let samplers = walk["samplers"]
        .as_array()
        .expect("Walk_Loop must declare samplers");
    assert!(!samplers.is_empty(), "Walk_Loop must have sampler inputs");
    for (index, sampler) in samplers.iter().enumerate() {
        let input = sampler["input"]
            .as_u64()
            .expect("animation sampler must reference an input accessor")
            as usize;
        let timestamps = scalar_f32_accessor(&document, binary, input);
        let earliest = timestamps.iter().copied().fold(f32::INFINITY, f32::min);
        let latest = timestamps.iter().copied().fold(f32::NEG_INFINITY, f32::max);

        assert!(
            earliest.abs() <= 1.0e-6,
            "Walk_Loop sampler {index} must start at zero, got {earliest}"
        );
        assert!(
            (latest - 4.0 / 3.0).abs() <= 1.0e-5,
            "Walk_Loop sampler {index} must end at the true 4/3s cycle, got {latest}"
        );
    }
}

#[test]
fn the_midcreek_technician_glb_contains_the_required_visual_modules_only() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/characters/midcreek/technician-man/technician-man.glb");
    let document = glb_json(&fs::read(path).expect("the generated technician GLB must exist"));
    let nodes = names(&document, "nodes");
    let meshes = names(&document, "meshes");

    for required in [
        "HighVisibilityVest",
        "HardHatShell",
        "EarDefender_+1",
        "EarDefender_-1",
        "ToolBelt",
        "RoomyDenimThigh_l",
        "RoomyDenimThigh_r",
        "WorkBoot_l",
        "WorkBoot_r",
    ] {
        assert!(
            nodes.iter().any(|name| name == required),
            "missing required technician module {required:?}: {nodes:?}"
        );
    }
    assert!(
        !meshes.iter().any(|name| name == "Icosphere"),
        "the source GLB's unreferenced Icosphere must not leak into the technician export"
    );
}

#[test]
fn the_midcreek_technician_glb_preserves_the_cel_shift_material_palette() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/characters/midcreek/technician-man/technician-man.glb");
    let document = glb_json(&fs::read(path).expect("the generated technician GLB must exist"));
    let expected = [
        ("Midcreek_skin", [0xC9, 0x8F, 0x6A]),
        ("Midcreek_hair", [0x2B, 0x23, 0x20]),
        ("Midcreek_shirt", [0x55, 0x70, 0x7F]),
        ("Midcreek_denim", [0x4A, 0x64, 0x85]),
        ("Midcreek_vest", [0xC8, 0xD9, 0x4A]),
        ("Midcreek_trim", [0xE8, 0x76, 0x3A]),
        ("Midcreek_silver", [0xD6, 0xDB, 0xE0]),
        ("Midcreek_hard_hat", [0x2C, 0x6F, 0xB8]),
        ("Midcreek_boots", [0x3A, 0x31, 0x28]),
        ("Midcreek_belt", [0x30, 0x2A, 0x25]),
        ("Midcreek_tools", [0xC6, 0x78, 0x2D]),
        ("Midcreek_defenders", [0x30, 0x36, 0x3B]),
        ("Midcreek_eyes", [0x23, 0x28, 0x2D]),
    ];
    let materials = document["materials"]
        .as_array()
        .expect("the technician GLB must declare materials");
    let midcreek_materials = materials
        .iter()
        .filter(|material| {
            material["name"]
                .as_str()
                .is_some_and(|name| name.starts_with("Midcreek_"))
        })
        .collect::<Vec<_>>();

    assert_eq!(
        midcreek_materials.len(),
        expected.len(),
        "the GLB must contain exactly the expected named Midcreek materials"
    );

    for (name, srgb) in expected {
        let material = midcreek_materials
            .iter()
            .find(|material| material["name"] == name)
            .unwrap_or_else(|| panic!("missing expected technician material {name}"));
        let pbr = &material["pbrMetallicRoughness"];
        let factors = pbr["baseColorFactor"]
            .as_array()
            .unwrap_or_else(|| panic!("{name} must declare a baseColorFactor"));
        assert_eq!(factors.len(), 4, "{name} must export an RGBA factor");

        for channel in 0..3 {
            assert_close(
                factors[channel]
                    .as_f64()
                    .unwrap_or_else(|| panic!("{name} channel {channel} must be numeric")),
                srgb_channel_to_linear(srgb[channel]),
                &format!("{name} base color channel {channel}"),
            );
        }
        assert_close(
            factors[3]
                .as_f64()
                .unwrap_or_else(|| panic!("{name} alpha must be numeric")),
            1.0,
            &format!("{name} alpha"),
        );
        assert_close(
            pbr["metallicFactor"]
                .as_f64()
                .unwrap_or_else(|| panic!("{name} must declare metallicFactor")),
            0.0,
            &format!("{name} metallic factor"),
        );
        assert_close(
            pbr["roughnessFactor"]
                .as_f64()
                .unwrap_or_else(|| panic!("{name} must declare roughnessFactor")),
            0.9,
            &format!("{name} roughness factor"),
        );
    }
}

#[test]
fn the_overlay_exposes_the_updated_control_help_lines() {
    assert_eq!(
        control_help_lines(),
        [
            "Arrows: walk/steer/turn around   Q/E: orbit   Wheel: zoom",
            "Tab: switch model   Space: pause/resume",
            "P: screenshot   Esc: exit",
        ]
    );
}

#[test]
fn the_overlay_reports_active_model_and_each_variants_readiness() {
    let asset_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let catalog = load_character_catalog(&asset_root).expect("the checked-in catalog must load");
    let selection = CharacterSelection::default();
    let mut readiness = VariantReadiness::default();
    readiness.mark_ready(CharacterVariant::Reference, 1);
    readiness.mark_ready(CharacterVariant::TechnicianMan, 1);

    assert_eq!(
        character_status_lines(&catalog, &selection, &readiness),
        [
            "Active model: Quaternius reference",
            "Quaternius reference: ready, players 1/1",
            "Midcreek technician - man: ready, players 1/1",
        ]
    );
}

#[test]
fn escape_maps_to_a_success_exit_while_the_run_is_healthy() {
    let keys = pressed(KeyCode::Escape);

    assert_eq!(
        control_intents(&keys, PrototypeState::Running),
        ControlIntents {
            exit: Some(AppExit::Success),
            ..Default::default()
        }
    );
}

#[test]
fn escape_maps_to_an_error_exit_after_a_failed_run() {
    let keys = pressed(KeyCode::Escape);

    assert_eq!(
        control_intents(&keys, PrototypeState::Failed),
        ControlIntents {
            exit: Some(AppExit::error()),
            ..Default::default()
        }
    );
}

#[test]
fn space_maps_to_a_pause_toggle_intent() {
    let keys = pressed(KeyCode::Space);

    assert_eq!(
        control_intents(&keys, PrototypeState::Running),
        ControlIntents {
            toggle_pause: true,
            ..Default::default()
        }
    );
}

#[test]
fn pause_and_resume_apply_to_both_resident_animation_players() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(StatesPlugin)
        .insert_resource(ButtonInput::<KeyCode>::default())
        .init_resource::<CharacterSelection>()
        .init_resource::<FailureReport>()
        .insert_state(PrototypeState::Running);

    let (_, node) = AnimationGraph::from_clip(Handle::default());
    let mut reference = AnimationPlayer::default();
    reference.play(node).repeat();
    let mut technician = AnimationPlayer::default();
    technician.play(node).repeat();
    let players = [
        app.world_mut().spawn(reference).id(),
        app.world_mut().spawn(technician).id(),
    ];

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Space);
    app.world_mut()
        .run_system_once(handle_controls)
        .expect("the controls system must pause both players");
    assert!(players.iter().all(|entity| {
        app.world()
            .entity(*entity)
            .get::<AnimationPlayer>()
            .unwrap()
            .all_paused()
    }));

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Space);
    app.world_mut()
        .run_system_once(handle_controls)
        .expect("the controls system must resume both players");
    assert!(players.iter().all(|entity| {
        !app.world()
            .entity(*entity)
            .get::<AnimationPlayer>()
            .unwrap()
            .all_paused()
    }));
}

#[test]
fn p_maps_to_a_screenshot_intent() {
    let keys = pressed(KeyCode::KeyP);

    assert_eq!(
        control_intents(&keys, PrototypeState::Running),
        ControlIntents {
            screenshot: true,
            ..Default::default()
        }
    );
}

#[test]
fn tab_maps_to_a_model_toggle_without_replacing_the_screenshot_control() {
    let tab = pressed(KeyCode::Tab);
    let p = pressed(KeyCode::KeyP);

    assert_eq!(
        control_intents(&tab, PrototypeState::Running),
        ControlIntents {
            toggle_model: true,
            ..Default::default()
        }
    );
    assert_eq!(
        control_intents(&p, PrototypeState::Running),
        ControlIntents {
            screenshot: true,
            ..Default::default()
        }
    );
}

#[test]
fn model_selection_starts_on_the_reference_and_cycles_both_variants() {
    let mut selection = CharacterSelection::default();

    assert_eq!(selection.active(), CharacterVariant::Reference);
    selection.toggle();
    assert_eq!(selection.active(), CharacterVariant::TechnicianMan);
    selection.toggle();
    assert_eq!(selection.active(), CharacterVariant::Reference);
}

#[test]
fn tab_changes_only_selection_and_variant_visibility() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(StatesPlugin)
        .insert_resource(ButtonInput::<KeyCode>::default())
        .init_resource::<CharacterSelection>()
        .init_resource::<FailureReport>()
        .insert_state(PrototypeState::Running);

    let mut controller = HumanoidController::default();
    controller.update(
        MovementInput {
            turnaround_pressed: true,
            turnaround_held: true,
            ..Default::default()
        },
        std::time::Duration::from_millis(125),
    );
    let parent_transform = Transform::from_xyz(3.0, 0.0, -2.0)
        .with_rotation(bevy::prelude::Quat::from_rotation_y(0.7));
    let parent = app
        .world_mut()
        .spawn((Humanoid, controller, parent_transform))
        .id();
    let reference_transform = character_transform(0.5, 180.0);
    let technician_transform = character_transform(1.0, 180.0);
    let reference = app
        .world_mut()
        .spawn((
            ChildOf(parent),
            CharacterVariant::Reference,
            reference_transform,
            Visibility::Inherited,
        ))
        .id();
    let technician = app
        .world_mut()
        .spawn((
            ChildOf(parent),
            CharacterVariant::TechnicianMan,
            technician_transform,
            Visibility::Hidden,
        ))
        .id();
    let (_, node) = AnimationGraph::from_clip(Handle::default());
    let mut reference_animation = AnimationPlayer::default();
    reference_animation.play(node).repeat().set_seek_time(0.35);
    let mut technician_animation = AnimationPlayer::default();
    technician_animation.play(node).repeat().set_seek_time(0.35);
    let reference_player = app.world_mut().spawn(reference_animation).id();
    let technician_player = app.world_mut().spawn(technician_animation).id();
    let orbit = OrbitCamera {
        yaw: 0.4,
        pitch: 0.2,
        current_distance: 4.0,
        target_distance: 5.0,
        target_height: 0.95,
    };
    let orbit_transform = Transform::from_xyz(4.0, 2.0, 3.0);
    let camera = app.world_mut().spawn((orbit, orbit_transform)).id();

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Tab);
    app.world_mut()
        .run_system_once(handle_controls)
        .expect("the real controls system must run");

    assert_eq!(
        app.world().resource::<CharacterSelection>().active(),
        CharacterVariant::TechnicianMan
    );
    assert_eq!(
        *app.world().entity(reference).get::<Visibility>().unwrap(),
        Visibility::Hidden
    );
    assert_eq!(
        *app.world().entity(technician).get::<Visibility>().unwrap(),
        Visibility::Inherited
    );
    assert_eq!(
        *app.world().entity(parent).get::<Transform>().unwrap(),
        parent_transform
    );
    assert_eq!(
        *app.world()
            .entity(parent)
            .get::<HumanoidController>()
            .unwrap(),
        controller
    );
    assert_eq!(
        *app.world().entity(reference).get::<Transform>().unwrap(),
        reference_transform
    );
    assert_eq!(
        *app.world().entity(technician).get::<Transform>().unwrap(),
        technician_transform
    );
    assert_eq!(
        *app.world().entity(camera).get::<OrbitCamera>().unwrap(),
        orbit
    );
    assert_eq!(
        *app.world().entity(camera).get::<Transform>().unwrap(),
        orbit_transform
    );
    for player in [reference_player, technician_player] {
        let seek_time = app
            .world()
            .entity(player)
            .get::<AnimationPlayer>()
            .unwrap()
            .playing_animations()
            .next()
            .unwrap()
            .1
            .seek_time();
        assert_eq!(seek_time, 0.35);
    }
}

#[test]
fn stable_humanoid_root_owns_both_visual_variants() {
    let asset_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let catalog = load_character_catalog(&asset_root).expect("the checked-in catalog must load");
    let (_, node) = AnimationGraph::from_clip(Handle::default());
    let prepared = PreparedCharacterCatalog::new(
        PreparedVariant::new(Handle::default(), Handle::default(), node, 4.0 / 3.0),
        PreparedVariant::new(Handle::default(), Handle::default(), node, 4.0 / 3.0),
    );

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(catalog.clone())
        .insert_resource(prepared)
        .init_resource::<CharacterSelection>()
        .add_systems(bevy::app::Update, spawn_character);
    app.update();

    let world = app.world_mut();
    let humanoids: Vec<_> = world
        .query_filtered::<(Entity, &Transform, &HumanoidController), With<Humanoid>>()
        .iter(world)
        .map(|(entity, transform, controller)| (entity, *transform, *controller))
        .collect();
    assert_eq!(humanoids.len(), 1);
    let (humanoid, transform, _) = humanoids[0];
    assert_eq!(transform, Transform::IDENTITY);

    let variants: Vec<_> = world
        .query::<(Entity, &CharacterVariant, &ChildOf, &Transform, &Visibility)>()
        .iter(world)
        .map(|(entity, variant, parent, transform, visibility)| {
            (entity, *variant, parent.parent(), *transform, *visibility)
        })
        .collect();
    assert_eq!(variants.len(), 2);
    assert!(
        variants
            .iter()
            .all(|(_, _, parent, _, _)| *parent == humanoid)
    );

    let reference = variants
        .iter()
        .find(|(_, variant, _, _, _)| *variant == CharacterVariant::Reference)
        .expect("reference visual must be present");
    let technician = variants
        .iter()
        .find(|(_, variant, _, _, _)| *variant == CharacterVariant::TechnicianMan)
        .expect("technician visual must be present");

    assert_eq!(
        reference.3,
        character_transform(catalog.reference.scale, catalog.reference.yaw_degrees)
    );
    assert_eq!(
        technician.3,
        character_transform(
            catalog.technician_man.scale,
            catalog.technician_man.yaw_degrees
        )
    );
    assert_eq!(reference.4, Visibility::Inherited);
    assert_eq!(technician.4, Visibility::Hidden);
}

#[test]
fn initial_variant_visibility_follows_character_selection() {
    let asset_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let catalog = load_character_catalog(&asset_root).expect("the checked-in catalog must load");
    let (_, node) = AnimationGraph::from_clip(Handle::default());
    let prepared = PreparedCharacterCatalog::new(
        PreparedVariant::new(Handle::default(), Handle::default(), node, 4.0 / 3.0),
        PreparedVariant::new(Handle::default(), Handle::default(), node, 4.0 / 3.0),
    );

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(catalog)
        .insert_resource(prepared)
        .init_resource::<CharacterSelection>()
        .add_systems(bevy::app::Update, spawn_character);
    app.world_mut()
        .resource_mut::<CharacterSelection>()
        .toggle();
    app.update();

    let world = app.world_mut();
    let visibility: Vec<_> = world
        .query::<(&CharacterVariant, &Visibility)>()
        .iter(world)
        .map(|(variant, visibility)| (*variant, *visibility))
        .collect();
    assert_eq!(
        visibility,
        [
            (CharacterVariant::Reference, Visibility::Hidden),
            (CharacterVariant::TechnicianMan, Visibility::Inherited),
        ]
    );
}

#[test]
fn stable_humanoid_root_carries_visibility_hierarchy_components() {
    let asset_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let catalog = load_character_catalog(&asset_root).expect("the checked-in catalog must load");
    let (_, node) = AnimationGraph::from_clip(Handle::default());
    let prepared = PreparedCharacterCatalog::new(
        PreparedVariant::new(Handle::default(), Handle::default(), node, 4.0 / 3.0),
        PreparedVariant::new(Handle::default(), Handle::default(), node, 4.0 / 3.0),
    );

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(catalog)
        .insert_resource(prepared)
        .init_resource::<CharacterSelection>()
        .add_systems(bevy::app::Update, spawn_character);
    app.update();

    let world = app.world_mut();
    let humanoid = world
        .query_filtered::<Entity, With<Humanoid>>()
        .single(world)
        .expect("exactly one stable humanoid root must spawn");
    let root = world.entity(humanoid);

    assert!(
        root.contains::<Visibility>(),
        "the stable parent must participate in visibility propagation"
    );
    assert!(
        root.contains::<InheritedVisibility>(),
        "the stable parent must carry inherited visibility state"
    );
    assert!(
        root.contains::<ViewVisibility>(),
        "the stable parent must carry computed view visibility"
    );
}

#[test]
fn loading_retains_distinct_handles_for_both_advertised_variants() {
    let asset_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let catalog = load_character_catalog(&asset_root).expect("the checked-in catalog must load");

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin {
            file_path: asset_root.to_string_lossy().into_owned(),
            ..Default::default()
        })
        .init_asset::<bevy::prelude::Gltf>()
        .insert_resource(catalog)
        .add_systems(bevy::app::Update, begin_loading);
    app.update();

    let assets = app.world().resource::<CharacterAssetCatalog>();
    assert_ne!(
        assets.handle(CharacterVariant::Reference).id(),
        assets.handle(CharacterVariant::TechnicianMan).id()
    );
}

fn validating_character_app() -> App {
    let asset_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let catalog = load_character_catalog(&asset_root).expect("the checked-in catalog must load");
    let (_, node) = AnimationGraph::from_clip(Handle::default());
    let prepared = PreparedCharacterCatalog::new(
        PreparedVariant::new(Handle::default(), Handle::default(), node, 4.0 / 3.0),
        PreparedVariant::new(Handle::default(), Handle::default(), node, 4.0 / 3.0),
    );

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(StatesPlugin)
        .insert_resource(catalog)
        .insert_resource(prepared)
        .init_resource::<CharacterSelection>()
        .init_resource::<FailureReport>()
        .insert_state(PrototypeState::Validating);
    app.world_mut()
        .run_system_once(spawn_character)
        .expect("the character hierarchy must spawn");
    app
}

#[test]
fn validating_without_a_start_marker_fails_actionably_instead_of_hanging() {
    let asset_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let catalog = load_character_catalog(&asset_root).expect("the checked-in catalog must load");
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(StatesPlugin)
        .insert_resource(catalog)
        .init_resource::<FailureReport>()
        .insert_state(PrototypeState::Validating)
        .add_plugins(CharacterPlugin);

    app.update();
    app.update();

    assert_eq!(
        *app.world().resource::<State<PrototypeState>>().get(),
        PrototypeState::Failed,
        "a missing validation start marker must not leave the app in Validating"
    );
    let report = app.world().resource::<FailureReport>().to_display_string();
    assert!(report.contains("validation watchdog"), "{report}");
    assert!(report.contains("start marker"), "{report}");
}

fn variant_root(app: &mut App, wanted: CharacterVariant) -> Entity {
    let world = app.world_mut();
    world
        .query::<(Entity, &CharacterVariant)>()
        .iter(world)
        .find_map(|(entity, variant)| (*variant == wanted).then_some(entity))
        .expect("the requested visual root must exist")
}

#[test]
fn both_players_are_wired_and_started_only_after_both_variants_validate() {
    let mut app = validating_character_app();
    let reference_root = variant_root(&mut app, CharacterVariant::Reference);
    let technician_root = variant_root(&mut app, CharacterVariant::TechnicianMan);
    let reference_player = app
        .world_mut()
        .spawn((ChildOf(reference_root), AnimationPlayer::default()))
        .id();
    let technician_player = app
        .world_mut()
        .spawn((ChildOf(technician_root), AnimationPlayer::default()))
        .id();

    app.world_mut().trigger(VariantHierarchyReady {
        entity: reference_root,
    });
    assert_eq!(
        *app.world().resource::<State<PrototypeState>>().get(),
        PrototypeState::Validating
    );
    assert!(
        app.world()
            .entity(reference_player)
            .get::<AnimationPlayer>()
            .unwrap()
            .playing_animations()
            .next()
            .is_none(),
        "neither animation may start until both hierarchies validate"
    );

    app.world_mut().trigger(VariantHierarchyReady {
        entity: technician_root,
    });
    app.update();

    assert_eq!(
        *app.world().resource::<State<PrototypeState>>().get(),
        PrototypeState::Running
    );
    let readiness = app.world().resource::<VariantReadiness>();
    assert_eq!(readiness.players(CharacterVariant::Reference), Some(1));
    assert_eq!(readiness.players(CharacterVariant::TechnicianMan), Some(1));

    for entity in [reference_player, technician_player] {
        let entity_ref = app.world().entity(entity);
        assert!(entity_ref.contains::<AnimationGraphHandle>());
        assert!(entity_ref.contains::<AnimationTransitions>());
        let active = entity_ref
            .get::<AnimationPlayer>()
            .unwrap()
            .playing_animations()
            .next()
            .expect("both real players must be started");
        assert_eq!(active.1.seek_time(), 0.0);
        assert_eq!(active.1.speed(), 1.0);
    }
}

#[test]
fn a_player_count_failure_in_either_variant_prevents_running() {
    let mut app = validating_character_app();
    let reference_root = variant_root(&mut app, CharacterVariant::Reference);

    app.world_mut().trigger(VariantHierarchyReady {
        entity: reference_root,
    });
    app.update();

    assert_eq!(
        *app.world().resource::<State<PrototypeState>>().get(),
        PrototypeState::Failed
    );
    let report = app.world().resource::<FailureReport>();
    assert!(
        report.to_display_string().contains("Quaternius reference"),
        "{}",
        report.to_display_string()
    );
    assert!(
        report.to_display_string().contains("discovered 0"),
        "{}",
        report.to_display_string()
    );
}

#[test]
fn tab_does_not_switch_models_before_the_runtime_is_ready() {
    let tab = pressed(KeyCode::Tab);

    for state in [
        PrototypeState::Loading,
        PrototypeState::Validating,
        PrototypeState::Failed,
    ] {
        assert!(
            !control_intents(&tab, state).toggle_model,
            "Tab must be ignored in {state:?}"
        );
    }
}

#[test]
fn runtime_plugin_configuration_registers_the_core_runtime_plugins() {
    let mut app = App::new();
    add_runtime_plugins(&mut app, DiagnosticsPlugin::attended());

    assert!(app.is_plugin_added::<InspectionPlugin>());
    assert!(app.is_plugin_added::<CharacterPlugin>());
    assert!(app.is_plugin_added::<LocomotionPlugin>());
    assert!(app.is_plugin_added::<DiagnosticsPlugin>());
    assert!(app.is_plugin_added::<PerformancePlugin>());
}
