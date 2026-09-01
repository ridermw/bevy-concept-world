//! Loading, contract validation, and animation of the humanoid character.
//!
//! The pure functions at the top of this module make every selection decision
//! and are exercised directly by `tests/app_contract.rs`. The Bevy systems
//! below only gather *real* observations — the names the loaded `Gltf`
//! actually declares and the `AnimationPlayer` entities the spawned hierarchy
//! actually contains — and hand them to those functions. Nothing here
//! substitutes an expected value for a discovered one.

use std::time::Duration;

use bevy::{asset::LoadState, prelude::*, world_serialization::WorldInstanceReady};
use thiserror::Error;

use crate::{
    config::CharacterConfig,
    state::{FailureReport, PrototypeState, fail},
};

/// The named sub-assets a loaded glTF actually declares.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscoveredGltf {
    /// Every named scene in the file.
    pub scenes: Vec<String>,
    /// Every named animation clip in the file.
    pub animations: Vec<String>,
}

/// A mismatch between the checked-in manifest and the real asset.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SelectionError {
    #[error("glTF declares no scene named '{expected}'; discovered scenes: {}", format_names(.discovered))]
    MissingScene {
        expected: String,
        discovered: Vec<String>,
    },

    #[error("glTF declares no animation named '{expected}'; discovered animations: {}", format_names(.discovered))]
    MissingAnimation {
        expected: String,
        discovered: Vec<String>,
    },

    #[error(
        "expected {expected} AnimationPlayer entities in the spawned scene, discovered {actual}"
    )]
    AnimationPlayerCount { expected: usize, actual: usize },
}

fn format_names(names: &[String]) -> String {
    if names.is_empty() {
        "none".to_string()
    } else {
        names.join(", ")
    }
}

/// Confirms the asset declares the exact scene and animation names the
/// manifest requires. Matching is exact: a prefix, suffix, or `.001` duplicate
/// is a failure, never a silent substitution.
pub fn validate_named_assets(
    discovered: &DiscoveredGltf,
    scene: &str,
    animation: &str,
) -> Result<(), SelectionError> {
    if !discovered.scenes.iter().any(|name| name == scene) {
        return Err(SelectionError::MissingScene {
            expected: scene.to_string(),
            discovered: discovered.scenes.clone(),
        });
    }
    if !discovered.animations.iter().any(|name| name == animation) {
        return Err(SelectionError::MissingAnimation {
            expected: animation.to_string(),
            discovered: discovered.animations.clone(),
        });
    }
    Ok(())
}

/// Confirms the spawned hierarchy produced exactly the number of
/// `AnimationPlayer` entities the manifest expects. `actual` must always be a
/// count taken from the live world.
pub fn validate_animation_players(expected: usize, actual: usize) -> Result<(), SelectionError> {
    if expected == actual {
        Ok(())
    } else {
        Err(SelectionError::AnimationPlayerCount { expected, actual })
    }
}

/// The deterministic spawn transform described by the manifest.
///
/// The yaw correction turns the model's authored facing onto Bevy's forward
/// axis (`-Z`); the scale correction converts the pack's units to meters.
pub fn character_transform(scale: f32, yaw_degrees: f32) -> Transform {
    Transform::from_scale(Vec3::splat(scale))
        .with_rotation(Quat::from_rotation_y(yaw_degrees.to_radians()))
}

/// Loads the manifest's glTF, validates it against the manifest, spawns it,
/// and loops the named walk clip on every discovered `AnimationPlayer`.
pub struct CharacterPlugin;

impl Plugin for CharacterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(PrototypeState::Loading),
            begin_loading.run_if(resource_exists::<CharacterConfig>),
        )
        .add_systems(
            Update,
            poll_loading
                .run_if(in_state(PrototypeState::Loading))
                .run_if(resource_exists::<CharacterConfig>)
                .run_if(resource_exists::<CharacterAsset>),
        );
    }
}

/// Handle to the root `Gltf` asset. Held so the load is not dropped.
#[derive(Resource, Debug)]
struct CharacterAsset(Handle<Gltf>);

/// Carried by the spawned scene root until its `AnimationPlayer` entities have
/// been discovered and wired up.
#[derive(Component)]
struct PendingCharacter {
    graph: Handle<AnimationGraph>,
    node: AnimationNodeIndex,
}

fn begin_loading(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    config: Res<CharacterConfig>,
) {
    info!("loading character glTF: {}", config.gltf_path);
    commands.insert_resource(CharacterAsset(asset_server.load(config.gltf_path.clone())));
}

#[allow(clippy::too_many_arguments)]
fn poll_loading(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    asset: Res<CharacterAsset>,
    config: Res<CharacterConfig>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut next_state: ResMut<NextState<PrototypeState>>,
    mut report: ResMut<FailureReport>,
    mut spawned: Local<bool>,
) {
    // A queued state transition is only applied at the next `StateTransition`
    // run, so this guard is what actually keeps the character from being
    // spawned twice.
    if *spawned {
        return;
    }

    if let Some(LoadState::Failed(error)) = asset_server.get_load_state(&asset.0) {
        *spawned = true;
        fail(
            &mut next_state,
            &mut report,
            "Character glTF failed to load",
            vec![
                format!("asset path: {}", config.gltf_path),
                format!("loader error: {error}"),
            ],
        );
        return;
    }

    if !asset_server.is_loaded_with_dependencies(&asset.0) {
        return;
    }

    let Some(gltf) = gltfs.get(&asset.0) else {
        *spawned = true;
        fail(
            &mut next_state,
            &mut report,
            "Character glTF reported loaded but is absent from Assets<Gltf>",
            vec![format!("asset path: {}", config.gltf_path)],
        );
        return;
    };

    // Sorted so a failure message is stable between runs; the underlying maps
    // are unordered.
    let mut scenes: Vec<String> = gltf.named_scenes.keys().map(ToString::to_string).collect();
    scenes.sort();
    let mut animations: Vec<String> = gltf
        .named_animations
        .keys()
        .map(ToString::to_string)
        .collect();
    animations.sort();
    let discovered = DiscoveredGltf { scenes, animations };

    if let Err(error) =
        validate_named_assets(&discovered, &config.scene_name, &config.animation_name)
    {
        *spawned = true;
        fail(
            &mut next_state,
            &mut report,
            "Character glTF does not match the manifest",
            vec![
                format!("asset path: {}", config.gltf_path),
                error.to_string(),
            ],
        );
        return;
    }

    // Both lookups are guaranteed by the validation immediately above, which
    // ran against the keys of these exact maps.
    let scene = gltf.named_scenes[config.scene_name.as_str()].clone();
    let clip = gltf.named_animations[config.animation_name.as_str()].clone();

    let (graph, node) = AnimationGraph::from_clip(clip);
    let graph = graphs.add(graph);

    *spawned = true;
    commands
        .spawn((
            Name::new("Humanoid"),
            WorldAssetRoot(scene),
            character_transform(config.scale, config.yaw_degrees),
            PendingCharacter { graph, node },
        ))
        .observe(start_animation);

    info!(
        "spawned scene '{}' with clip '{}'; validating spawned hierarchy",
        config.scene_name, config.animation_name
    );
    next_state.set(PrototypeState::Validating);
}

/// Runs once the spawned world instance exists, so the hierarchy can be
/// searched for the `AnimationPlayer` entities it really produced.
#[allow(clippy::too_many_arguments)]
fn start_animation(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    pending: Query<&PendingCharacter>,
    mut players: Query<&mut AnimationPlayer>,
    config: Res<CharacterConfig>,
    mut next_state: ResMut<NextState<PrototypeState>>,
    mut report: ResMut<FailureReport>,
    mut handled: Local<bool>,
) {
    if *handled {
        return;
    }
    let root = ready.entity;
    let Ok(pending) = pending.get(root) else {
        return;
    };
    *handled = true;

    let player_entities: Vec<Entity> = children
        .iter_descendants(root)
        .filter(|entity| players.contains(*entity))
        .collect();

    if let Err(error) =
        validate_animation_players(config.expected_animation_players, player_entities.len())
    {
        fail(
            &mut next_state,
            &mut report,
            "Spawned character scene does not match the manifest",
            vec![
                format!("scene: {}", config.scene_name),
                format!("asset path: {}", config.gltf_path),
                error.to_string(),
            ],
        );
        return;
    }

    for entity in player_entities {
        let Ok(mut player) = players.get_mut(entity) else {
            continue;
        };
        let mut transitions = AnimationTransitions::new();
        transitions
            .play(&mut player, pending.node, Duration::ZERO)
            .repeat();
        commands
            .entity(entity)
            .insert((AnimationGraphHandle(pending.graph.clone()), transitions));
    }

    commands.entity(root).remove::<PendingCharacter>();
    info!("looping '{}' on the humanoid", config.animation_name);
    next_state.set(PrototypeState::Running);
}
