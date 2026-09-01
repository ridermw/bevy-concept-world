//! Loading, contract validation, and animation of the humanoid character.
//!
//! The pure functions at the top of this module make every selection and
//! progress decision and are exercised directly by `tests/app_contract.rs` and
//! `tests/runtime_contract.rs`. The Bevy systems below only gather *real*
//! observations — the load states the `AssetServer` actually reports, the
//! names the loaded `Gltf` actually declares, and the `AnimationPlayer`
//! entities the spawned hierarchy actually contains — and hand them to those
//! functions. Nothing here substitutes an expected value for a discovered one.
//!
//! The four states are genuinely sequential. `Loading` polls the asset server
//! and, when the glTF and every dependency are loaded and the manifest's names
//! are confirmed, *prepares* handles and enters `Validating`. Nothing is
//! spawned in that frame, so `Validating` is always observable in the state
//! log. `OnEnter(Validating)` spawns the scene and attaches the observer that
//! discovers the real `AnimationPlayer` entities; only that observer may
//! request `Running`.

use std::time::Duration;

use bevy::{
    asset::{DependencyLoadState, LoadState, RecursiveDependencyLoadState},
    prelude::*,
    world_serialization::WorldInstanceReady,
};
use thiserror::Error;

use crate::{
    config::CharacterConfig,
    locomotion::HumanoidController,
    state::{FailureReport, PrototypeState, fail},
};

/// How long `Loading` may make no progress before the run fails. Measured on
/// [`Time<Real>`] so a stalled or throttled render loop cannot stretch it.
pub const LOADING_TIMEOUT: Duration = Duration::from_secs(180);

/// How long `Validating` may wait for [`WorldInstanceReady`] before the run
/// fails. Also measured on [`Time<Real>`].
pub const VALIDATING_TIMEOUT: Duration = Duration::from_secs(180);

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

/// Whether a wall-clock deadline has been reached.
///
/// The comparison is inclusive so a zero timeout means "one chance", and so a
/// deadline that lands exactly on a frame boundary is not skipped.
pub fn timed_out(elapsed: Duration, timeout: Duration) -> bool {
    elapsed >= timeout
}

/// One asset's progress, flattened from Bevy's three separate load-state
/// enums so the decision below can be written and tested once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadPhase {
    /// Not started, or still in flight.
    Pending,
    /// Finished successfully.
    Ready,
    /// Finished unsuccessfully. Terminal.
    Failed,
}

/// What the `Loading` state should do this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadOutcome {
    /// Keep waiting.
    Waiting,
    /// The root asset and every dependency are loaded.
    Ready,
    /// Something in the dependency tree failed to load.
    Failed,
    /// Nothing failed, but the load did not finish inside its budget.
    TimedOut,
}

/// Decides whether a load has finished, failed, or run out of time.
///
/// All three of Bevy's load states are consulted, not just the root's: a glTF
/// whose buffer or image dependency fails leaves the root in `Loaded` forever
/// while `is_loaded_with_dependencies` stays false, which is an infinite
/// `Loading` state rather than a diagnosable failure.
///
/// A `Failed` phase outranks the timeout, because the loader error is the
/// actionable cause and the timeout is only its symptom.
pub fn evaluate_load(
    root: LoadPhase,
    direct_dependencies: LoadPhase,
    recursive_dependencies: LoadPhase,
    elapsed: Duration,
    timeout: Duration,
) -> LoadOutcome {
    let phases = [root, direct_dependencies, recursive_dependencies];

    if phases.contains(&LoadPhase::Failed) {
        return LoadOutcome::Failed;
    }
    if phases.iter().all(|phase| *phase == LoadPhase::Ready) {
        return LoadOutcome::Ready;
    }
    if timed_out(elapsed, timeout) {
        return LoadOutcome::TimedOut;
    }
    LoadOutcome::Waiting
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

/// Walks the real spawned hierarchy under `root` and returns the
/// `AnimationPlayer` entities it actually contains, or the mismatch against
/// the manifest's expectation.
///
/// This is the only route to `Running`: there is no path that assumes a
/// player exists. `is_player` is supplied by the caller so the traversal can
/// be driven from a system or from a test `World` without duplicating it.
pub fn check_animation_players(
    root: Entity,
    expected: usize,
    children: &Query<&Children>,
    is_player: impl Fn(Entity) -> bool,
) -> Result<Vec<Entity>, SelectionError> {
    let found: Vec<Entity> = children
        .iter_descendants(root)
        .filter(|entity| is_player(*entity))
        .collect();

    validate_animation_players(expected, found.len())?;
    Ok(found)
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

/// Marks the validated humanoid root so runtime systems can target it.
#[derive(Component)]
pub struct Humanoid;

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
        )
        .add_systems(
            OnEnter(PrototypeState::Validating),
            spawn_character
                .run_if(resource_exists::<CharacterConfig>)
                .run_if(resource_exists::<PreparedCharacter>),
        )
        .add_systems(
            Update,
            poll_validating
                .run_if(in_state(PrototypeState::Validating))
                .run_if(resource_exists::<ValidatingStartedAt>),
        );
    }
}

/// Handle to the root `Gltf` asset. Held so the load is not dropped.
#[derive(Resource, Debug)]
struct CharacterAsset(Handle<Gltf>);

/// Wall-clock instant, on [`Time<Real>`], at which the load was requested.
#[derive(Resource, Debug)]
struct LoadingStartedAt(Duration);

/// Wall-clock instant, on [`Time<Real>`], at which `Validating` was entered.
#[derive(Resource, Debug)]
struct ValidatingStartedAt(Duration);

/// Everything `Loading` proved and prepared, handed to `Validating` so nothing
/// is spawned in the frame the transition is requested.
#[derive(Resource, Debug)]
struct PreparedCharacter {
    scene: Handle<bevy::world_serialization::WorldAsset>,
    graph: Handle<AnimationGraph>,
    node: AnimationNodeIndex,
}

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
    real: Res<Time<Real>>,
) {
    info!("loading character glTF: {}", config.gltf_path);
    commands.insert_resource(CharacterAsset(asset_server.load(config.gltf_path.clone())));
    commands.insert_resource(LoadingStartedAt(real.elapsed()));
}

/// Flattens Bevy's root load state and returns the loader error, if any.
fn root_phase(state: &LoadState) -> (LoadPhase, Option<String>) {
    match state {
        LoadState::Loaded => (LoadPhase::Ready, None),
        LoadState::Failed(error) => (LoadPhase::Failed, Some(error.to_string())),
        LoadState::NotLoaded | LoadState::Loading => (LoadPhase::Pending, None),
    }
}

fn dependency_phase(state: &DependencyLoadState) -> (LoadPhase, Option<String>) {
    match state {
        DependencyLoadState::Loaded => (LoadPhase::Ready, None),
        DependencyLoadState::Failed(error) => (LoadPhase::Failed, Some(error.to_string())),
        DependencyLoadState::NotLoaded | DependencyLoadState::Loading => (LoadPhase::Pending, None),
    }
}

fn recursive_dependency_phase(state: &RecursiveDependencyLoadState) -> (LoadPhase, Option<String>) {
    match state {
        RecursiveDependencyLoadState::Loaded => (LoadPhase::Ready, None),
        RecursiveDependencyLoadState::Failed(error) => (LoadPhase::Failed, Some(error.to_string())),
        RecursiveDependencyLoadState::NotLoaded | RecursiveDependencyLoadState::Loading => {
            (LoadPhase::Pending, None)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn poll_loading(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    asset: Res<CharacterAsset>,
    config: Res<CharacterConfig>,
    started_at: Option<Res<LoadingStartedAt>>,
    real: Res<Time<Real>>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut next_state: ResMut<NextState<PrototypeState>>,
    mut report: ResMut<FailureReport>,
    mut prepared: Local<bool>,
) {
    // A queued state transition is only applied at the next `StateTransition`
    // run, so this guard is what actually keeps the work below from running
    // twice.
    if *prepared {
        return;
    }

    let started_at = started_at.map_or(Duration::ZERO, |started| started.0);
    let elapsed = real.elapsed().saturating_sub(started_at);

    // `get_load_states` returns `None` only until the asset server has
    // registered the handle, which is still legitimate progress.
    let Some((root, direct, recursive)) = asset_server.get_load_states(&asset.0) else {
        if timed_out(elapsed, LOADING_TIMEOUT) {
            *prepared = true;
            fail(
                &mut next_state,
                &mut report,
                "Character glTF never started loading",
                vec![
                    format!("asset path: {}", config.gltf_path),
                    format!("waited: {:.1}s", elapsed.as_secs_f32()),
                    "the asset server reports no load state for the requested handle".to_string(),
                ],
            );
        }
        return;
    };

    let (root_phase, root_error) = root_phase(&root);
    let (direct_phase, direct_error) = dependency_phase(&direct);
    let (recursive_phase, recursive_error) = recursive_dependency_phase(&recursive);

    match evaluate_load(
        root_phase,
        direct_phase,
        recursive_phase,
        elapsed,
        LOADING_TIMEOUT,
    ) {
        LoadOutcome::Waiting => return,
        LoadOutcome::Failed => {
            *prepared = true;
            let mut details = vec![format!("asset path: {}", config.gltf_path)];
            for (what, error) in [
                ("root asset", root_error),
                ("direct dependency", direct_error),
                ("recursive dependency", recursive_error),
            ] {
                if let Some(error) = error {
                    details.push(format!("{what} loader error: {error}"));
                }
            }
            details.push(format!(
                "load states: root={root:?}, dependencies={direct:?}, \
                 recursive dependencies={recursive:?}"
            ));
            fail(
                &mut next_state,
                &mut report,
                "Character glTF failed to load",
                details,
            );
            return;
        }
        LoadOutcome::TimedOut => {
            *prepared = true;
            fail(
                &mut next_state,
                &mut report,
                "Character glTF did not finish loading in time",
                vec![
                    format!("asset path: {}", config.gltf_path),
                    format!(
                        "waited: {:.1}s (limit {:.0}s, real time)",
                        elapsed.as_secs_f32(),
                        LOADING_TIMEOUT.as_secs_f32()
                    ),
                    format!(
                        "load states: root={root:?}, dependencies={direct:?}, \
                         recursive dependencies={recursive:?}"
                    ),
                ],
            );
            return;
        }
        LoadOutcome::Ready => {}
    }

    let Some(gltf) = gltfs.get(&asset.0) else {
        *prepared = true;
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
        *prepared = true;
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

    *prepared = true;
    commands.insert_resource(PreparedCharacter { scene, graph, node });

    info!(
        "glTF matches the manifest (scene '{}', clip '{}'); entering Validating",
        config.scene_name, config.animation_name
    );
    next_state.set(PrototypeState::Validating);
}

/// Spawns the validated scene. This runs on `OnEnter(Validating)`, one frame
/// after the transition was requested, so `Validating` is a state the run
/// really occupies and really logs.
fn spawn_character(
    mut commands: Commands,
    config: Res<CharacterConfig>,
    prepared: Res<PreparedCharacter>,
    real: Res<Time<Real>>,
) {
    commands.insert_resource(ValidatingStartedAt(real.elapsed()));
    commands
        .spawn((
            Name::new("Humanoid"),
            Humanoid,
            HumanoidController::default(),
            bevy::world_serialization::WorldAssetRoot(prepared.scene.clone()),
            character_transform(config.scale, config.yaw_degrees),
            PendingCharacter {
                graph: prepared.graph.clone(),
                node: prepared.node,
            },
        ))
        .observe(start_animation);

    info!(
        "spawned scene '{}'; validating the spawned hierarchy",
        config.scene_name
    );
}

/// Fails the run if the spawned world instance never becomes ready. Without
/// this the application would sit in `Validating` forever, looking healthy.
fn poll_validating(
    started_at: Res<ValidatingStartedAt>,
    real: Res<Time<Real>>,
    config: Res<CharacterConfig>,
    mut next_state: ResMut<NextState<PrototypeState>>,
    mut report: ResMut<FailureReport>,
    mut reported: Local<bool>,
) {
    if *reported {
        return;
    }
    let elapsed = real.elapsed().saturating_sub(started_at.0);
    if !timed_out(elapsed, VALIDATING_TIMEOUT) {
        return;
    }

    *reported = true;
    fail(
        &mut next_state,
        &mut report,
        "Spawned character scene never became ready",
        vec![
            format!("scene: {}", config.scene_name),
            format!("asset path: {}", config.gltf_path),
            format!(
                "waited: {:.1}s (limit {:.0}s, real time) with no WorldInstanceReady",
                elapsed.as_secs_f32(),
                VALIDATING_TIMEOUT.as_secs_f32()
            ),
        ],
    );
}

/// Runs once the spawned world instance exists, so the hierarchy can be
/// searched for the `AnimationPlayer` entities it really produced. This is the
/// only system that may request `Running`.
#[allow(clippy::too_many_arguments)]
fn start_animation(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    pending: Query<&PendingCharacter>,
    all_players: Query<(), With<AnimationPlayer>>,
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

    let player_entities = match check_animation_players(
        root,
        config.expected_animation_players,
        &children,
        |entity| all_players.contains(entity),
    ) {
        Ok(entities) => entities,
        Err(error) => {
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
    };

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
