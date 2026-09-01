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
        AnimationGraph, AnimationGraphHandle, AnimationTransitions, Entity, Handle, KeyCode, State,
        Transform, Vec3, Visibility, With,
    },
    state::app::{AppExtStates, StatesPlugin},
};
use bevy_concept_world::{
    add_runtime_plugins,
    character::{
        CharacterAssetCatalog, CharacterPlugin, CharacterSelection, CharacterVariant,
        DiscoveredGltf, Humanoid, PreparedCharacterCatalog, PreparedVariant, SelectionError,
        VariantHierarchyReady, VariantReadiness, begin_loading, character_transform,
        spawn_character, validate_animation_players, validate_named_assets,
    },
    config::{load_character_catalog, load_character_config},
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
fn production_bootstrap_loads_the_complete_character_catalog() {
    let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main.rs"))
        .expect("main.rs must be readable");

    assert!(
        source.contains("load_character_catalog"),
        "production bootstrap must validate every advertised character contract"
    );
    assert!(
        source.contains("insert_resource(catalog)"),
        "production bootstrap must insert the complete validated catalog"
    );
    assert!(
        !source.contains("insert_resource(config)"),
        "production bootstrap must not retain the legacy single-character resource"
    );
}

#[test]
fn production_loader_prepares_both_variants_before_validating() {
    let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/character.rs"))
        .expect("character.rs must be readable");
    let compact: String = source.split_whitespace().collect();

    assert!(
        compact.contains("evaluate_catalog_load(observations.iter()"),
        "the runtime must aggregate both real load observations"
    );
    assert!(
        compact.contains("PreparedCharacterCatalog::new(reference,technician_man)"),
        "the runtime must prepare both scenes and graphs before Validating"
    );
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
        PreparedVariant::new(Handle::default(), Handle::default(), node),
        PreparedVariant::new(Handle::default(), Handle::default(), node),
    );

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(catalog.clone())
        .insert_resource(prepared)
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
        PreparedVariant::new(Handle::default(), Handle::default(), node),
        PreparedVariant::new(Handle::default(), Handle::default(), node),
    );

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(StatesPlugin)
        .insert_resource(catalog)
        .insert_resource(prepared)
        .init_resource::<FailureReport>()
        .insert_state(PrototypeState::Validating);
    app.world_mut()
        .run_system_once(spawn_character)
        .expect("the character hierarchy must spawn");
    app
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
