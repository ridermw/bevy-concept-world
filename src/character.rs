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
    config::{CharacterCatalog, CharacterConfig},
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

/// A visual model that can be shown on the shared humanoid root.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CharacterVariant {
    Reference,
    TechnicianMan,
}

impl CharacterVariant {
    pub const ALL: [Self; 2] = [Self::Reference, Self::TechnicianMan];

    pub fn label(self) -> &'static str {
        match self {
            Self::Reference => "Quaternius reference",
            Self::TechnicianMan => "Midcreek technician - man",
        }
    }

    pub fn config(self, catalog: &CharacterCatalog) -> &CharacterConfig {
        match self {
            Self::Reference => &catalog.reference,
            Self::TechnicianMan => &catalog.technician_man,
        }
    }
}

/// Playback speeds that keep both resident walk loops on the reference clip's
/// cycle duration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseSynchronizedPlaybackSpeeds {
    pub reference: f32,
    pub technician_man: f32,
}

/// Why two animation clips cannot be kept at the same normalized gait phase.
#[derive(Debug, Error, Clone, Copy, PartialEq)]
pub enum PhaseSyncError {
    #[error("{} animation duration is unavailable", .variant.label())]
    MissingDuration { variant: CharacterVariant },

    #[error("{} animation duration must be positive, got {duration}", .variant.label())]
    NonPositiveDuration {
        variant: CharacterVariant,
        duration: f32,
    },

    #[error("{} animation duration must be finite, got {duration}", .variant.label())]
    NonFiniteDuration {
        variant: CharacterVariant,
        duration: f32,
    },

    #[error("{} playback speed must be finite, got {speed}", .variant.label())]
    NonFinitePlaybackSpeed {
        variant: CharacterVariant,
        speed: f32,
    },
}

/// Uses the reference clip duration as the common cycle duration.
///
/// Each speed is `own_duration / reference_duration`, so elapsed wall time
/// advances both clips through the same normalized phase.
pub fn phase_synchronized_playback_speeds(
    reference_duration: Option<f32>,
    technician_duration: Option<f32>,
) -> Result<PhaseSynchronizedPlaybackSpeeds, PhaseSyncError> {
    fn validate(variant: CharacterVariant, duration: Option<f32>) -> Result<f32, PhaseSyncError> {
        let Some(duration) = duration else {
            return Err(PhaseSyncError::MissingDuration { variant });
        };
        if !duration.is_finite() {
            return Err(PhaseSyncError::NonFiniteDuration { variant, duration });
        }
        if duration <= 0.0 {
            return Err(PhaseSyncError::NonPositiveDuration { variant, duration });
        }
        Ok(duration)
    }

    let reference = validate(CharacterVariant::Reference, reference_duration)?;
    let technician = validate(CharacterVariant::TechnicianMan, technician_duration)?;
    let technician_speed = technician / reference;
    if !technician_speed.is_finite() {
        return Err(PhaseSyncError::NonFinitePlaybackSpeed {
            variant: CharacterVariant::TechnicianMan,
            speed: technician_speed,
        });
    }

    Ok(PhaseSynchronizedPlaybackSpeeds {
        reference: 1.0,
        technician_man: technician_speed,
    })
}

/// Aggregate load decision for both advertised character variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogLoadOutcome {
    Waiting,
    Ready,
    Failed(CharacterVariant),
    TimedOut(CharacterVariant),
}

/// Combines per-variant load outcomes without losing which model failed.
pub fn evaluate_catalog_load(
    outcomes: impl IntoIterator<Item = (CharacterVariant, LoadOutcome)>,
) -> CatalogLoadOutcome {
    let outcomes: Vec<_> = outcomes.into_iter().collect();

    if let Some((variant, _)) = outcomes
        .iter()
        .find(|(_, outcome)| *outcome == LoadOutcome::Failed)
    {
        return CatalogLoadOutcome::Failed(*variant);
    }
    if let Some((variant, _)) = outcomes
        .iter()
        .find(|(_, outcome)| *outcome == LoadOutcome::TimedOut)
    {
        return CatalogLoadOutcome::TimedOut(*variant);
    }
    if outcomes
        .iter()
        .all(|(_, outcome)| *outcome == LoadOutcome::Ready)
    {
        CatalogLoadOutcome::Ready
    } else {
        CatalogLoadOutcome::Waiting
    }
}

/// Player readiness discovered from each spawned visual hierarchy.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct VariantReadiness {
    reference_players: Option<usize>,
    technician_players: Option<usize>,
}

impl VariantReadiness {
    pub fn mark_ready(&mut self, variant: CharacterVariant, players: usize) {
        match variant {
            CharacterVariant::Reference => self.reference_players = Some(players),
            CharacterVariant::TechnicianMan => self.technician_players = Some(players),
        }
    }

    pub fn players(&self, variant: CharacterVariant) -> Option<usize> {
        match variant {
            CharacterVariant::Reference => self.reference_players,
            CharacterVariant::TechnicianMan => self.technician_players,
        }
    }

    pub fn all_ready(&self) -> bool {
        self.reference_players.is_some() && self.technician_players.is_some()
    }
}

/// The currently visible visual model.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterSelection {
    active: CharacterVariant,
}

impl Default for CharacterSelection {
    fn default() -> Self {
        Self {
            active: CharacterVariant::Reference,
        }
    }
}

impl CharacterSelection {
    pub fn active(&self) -> CharacterVariant {
        self.active
    }

    pub fn toggle(&mut self) {
        self.active = match self.active {
            CharacterVariant::Reference => CharacterVariant::TechnicianMan,
            CharacterVariant::TechnicianMan => CharacterVariant::Reference,
        };
    }
}

impl Plugin for CharacterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CharacterSelection>()
            .init_resource::<VariantReadiness>()
            .init_resource::<ValidatedVariants>()
            .add_systems(
                OnEnter(PrototypeState::Loading),
                begin_loading.run_if(resource_exists::<CharacterCatalog>),
            )
            .add_systems(
                Update,
                poll_loading
                    .run_if(in_state(PrototypeState::Loading))
                    .run_if(resource_exists::<CharacterCatalog>)
                    .run_if(resource_exists::<CharacterAssetCatalog>),
            )
            .add_systems(
                OnEnter(PrototypeState::Validating),
                spawn_character
                    .run_if(resource_exists::<CharacterCatalog>)
                    .run_if(resource_exists::<PreparedCharacterCatalog>),
            )
            .add_systems(
                Update,
                poll_validating.run_if(in_state(PrototypeState::Validating)),
            );
    }
}

/// Handles to both root `Gltf` assets. Held so neither load is dropped.
#[derive(Resource, Debug)]
pub struct CharacterAssetCatalog {
    reference: Handle<Gltf>,
    technician_man: Handle<Gltf>,
}

impl CharacterAssetCatalog {
    pub fn handle(&self, variant: CharacterVariant) -> &Handle<Gltf> {
        match variant {
            CharacterVariant::Reference => &self.reference,
            CharacterVariant::TechnicianMan => &self.technician_man,
        }
    }
}

/// Wall-clock instant, on [`Time<Real>`], at which the load was requested.
#[derive(Resource, Debug)]
struct LoadingStartedAt(Duration);

/// Wall-clock instant, on [`Time<Real>`], at which `Validating` was entered.
#[derive(Resource, Debug)]
struct ValidatingStartedAt(Duration);

/// Prepared scene and animation handles for one visual variant.
#[derive(Debug, Clone)]
pub struct PreparedVariant {
    scene: Handle<bevy::world_serialization::WorldAsset>,
    graph: Handle<AnimationGraph>,
    node: AnimationNodeIndex,
    duration: f32,
}

impl PreparedVariant {
    pub fn new(
        scene: Handle<bevy::world_serialization::WorldAsset>,
        graph: Handle<AnimationGraph>,
        node: AnimationNodeIndex,
        duration: f32,
    ) -> Self {
        Self {
            scene,
            graph,
            node,
            duration,
        }
    }
}

/// Prepared handles for every advertised visual variant.
#[derive(Resource, Debug, Clone)]
pub struct PreparedCharacterCatalog {
    reference: PreparedVariant,
    technician_man: PreparedVariant,
}

impl PreparedCharacterCatalog {
    pub fn new(reference: PreparedVariant, technician_man: PreparedVariant) -> Self {
        Self {
            reference,
            technician_man,
        }
    }
}

/// Carried by the spawned scene root until its `AnimationPlayer` entities have
/// been discovered and wired up.
#[derive(Component)]
struct PendingCharacter {
    graph: Handle<AnimationGraph>,
    node: AnimationNodeIndex,
    duration: f32,
}

#[derive(Clone)]
struct ValidatedVariant {
    root: Entity,
    players: Vec<Entity>,
    graph: Handle<AnimationGraph>,
    node: AnimationNodeIndex,
    duration: f32,
}

#[derive(Resource, Default)]
struct ValidatedVariants {
    reference: Option<ValidatedVariant>,
    technician_man: Option<ValidatedVariant>,
}

impl ValidatedVariants {
    fn insert(&mut self, variant: CharacterVariant, validated: ValidatedVariant) {
        match variant {
            CharacterVariant::Reference => self.reference = Some(validated),
            CharacterVariant::TechnicianMan => self.technician_man = Some(validated),
        }
    }

    fn both(&self) -> Option<[ValidatedVariant; 2]> {
        Some([self.reference.clone()?, self.technician_man.clone()?])
    }
}

/// Testable notification that one spawned variant hierarchy is ready to
/// validate. Production forwards Bevy's `WorldInstanceReady` event to this.
#[derive(EntityEvent, Debug, Clone, Copy, PartialEq, Eq)]
pub struct VariantHierarchyReady {
    pub entity: Entity,
}

fn forward_world_instance_ready(ready: On<WorldInstanceReady>, mut commands: Commands) {
    commands.trigger(VariantHierarchyReady {
        entity: ready.entity,
    });
}

pub fn begin_loading(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    catalog: Res<CharacterCatalog>,
    real: Res<Time<Real>>,
) {
    info!(
        "loading character glTFs: {}, {}",
        catalog.reference.gltf_path, catalog.technician_man.gltf_path
    );
    commands.insert_resource(CharacterAssetCatalog {
        reference: asset_server.load(catalog.reference.gltf_path.clone()),
        technician_man: asset_server.load(catalog.technician_man.gltf_path.clone()),
    });
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

struct VariantLoadObservation {
    variant: CharacterVariant,
    outcome: LoadOutcome,
    state_details: String,
    errors: Vec<String>,
    registered: bool,
}

fn observe_variant_load(
    asset_server: &AssetServer,
    assets: &CharacterAssetCatalog,
    variant: CharacterVariant,
    elapsed: Duration,
) -> VariantLoadObservation {
    let Some((root, direct, recursive)) = asset_server.get_load_states(assets.handle(variant))
    else {
        return VariantLoadObservation {
            variant,
            outcome: if timed_out(elapsed, LOADING_TIMEOUT) {
                LoadOutcome::TimedOut
            } else {
                LoadOutcome::Waiting
            },
            state_details: "the asset server reports no load state for the requested handle"
                .to_string(),
            errors: Vec::new(),
            registered: false,
        };
    };

    let (root_phase, root_error) = root_phase(&root);
    let (direct_phase, direct_error) = dependency_phase(&direct);
    let (recursive_phase, recursive_error) = recursive_dependency_phase(&recursive);
    let mut errors = Vec::new();
    for (what, error) in [
        ("root asset", root_error),
        ("direct dependency", direct_error),
        ("recursive dependency", recursive_error),
    ] {
        if let Some(error) = error {
            errors.push(format!("{what} loader error: {error}"));
        }
    }

    VariantLoadObservation {
        variant,
        outcome: evaluate_load(
            root_phase,
            direct_phase,
            recursive_phase,
            elapsed,
            LOADING_TIMEOUT,
        ),
        state_details: format!(
            "load states: root={root:?}, dependencies={direct:?}, \
             recursive dependencies={recursive:?}"
        ),
        errors,
        registered: true,
    }
}

fn prepare_variant(
    variant: CharacterVariant,
    config: &CharacterConfig,
    assets: &CharacterAssetCatalog,
    gltfs: &Assets<Gltf>,
    clips: &Assets<AnimationClip>,
    graphs: &mut Assets<AnimationGraph>,
) -> Result<PreparedVariant, (&'static str, Vec<String>)> {
    let Some(gltf) = gltfs.get(assets.handle(variant)) else {
        return Err((
            "Character glTF reported loaded but is absent from Assets<Gltf>",
            vec![
                format!("variant: {}", variant.label()),
                format!("asset path: {}", config.gltf_path),
            ],
        ));
    };

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
        return Err((
            "Character glTF does not match the manifest",
            vec![
                format!("variant: {}", variant.label()),
                format!("asset path: {}", config.gltf_path),
                error.to_string(),
            ],
        ));
    }

    let scene = gltf.named_scenes[config.scene_name.as_str()].clone();
    let clip = gltf.named_animations[config.animation_name.as_str()].clone();
    let Some(duration) = clips.get(&clip).map(AnimationClip::duration) else {
        return Err((
            "Character animation clip is unavailable after its glTF loaded",
            vec![
                format!("variant: {}", variant.label()),
                format!("clip: {}", config.animation_name),
                format!("asset path: {}", config.gltf_path),
            ],
        ));
    };
    let (graph, node) = AnimationGraph::from_clip(clip);

    Ok(PreparedVariant::new(
        scene,
        graphs.add(graph),
        node,
        duration,
    ))
}

#[allow(clippy::too_many_arguments)]
fn poll_loading(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    assets: Res<CharacterAssetCatalog>,
    catalog: Res<CharacterCatalog>,
    started_at: Option<Res<LoadingStartedAt>>,
    real: Res<Time<Real>>,
    gltfs: Res<Assets<Gltf>>,
    clips: Res<Assets<AnimationClip>>,
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

    let observations: Vec<_> = CharacterVariant::ALL
        .into_iter()
        .map(|variant| observe_variant_load(&asset_server, &assets, variant, elapsed))
        .collect();

    match evaluate_catalog_load(
        observations
            .iter()
            .map(|observation| (observation.variant, observation.outcome)),
    ) {
        CatalogLoadOutcome::Waiting => return,
        CatalogLoadOutcome::Failed(variant) => {
            *prepared = true;
            let observation = observations
                .iter()
                .find(|observation| observation.variant == variant)
                .expect("failed variant came from the observations");
            let config = variant.config(&catalog);
            let mut details = vec![
                format!("variant: {}", variant.label()),
                format!("asset path: {}", config.gltf_path),
            ];
            details.extend(observation.errors.iter().cloned());
            details.push(observation.state_details.clone());
            fail(
                &mut next_state,
                &mut report,
                "Character glTF failed to load",
                details,
            );
            return;
        }
        CatalogLoadOutcome::TimedOut(variant) => {
            *prepared = true;
            let observation = observations
                .iter()
                .find(|observation| observation.variant == variant)
                .expect("timed-out variant came from the observations");
            let config = variant.config(&catalog);
            let summary = if observation.registered {
                "Character glTF did not finish loading in time"
            } else {
                "Character glTF never started loading"
            };
            fail(
                &mut next_state,
                &mut report,
                summary,
                vec![
                    format!("variant: {}", variant.label()),
                    format!("asset path: {}", config.gltf_path),
                    format!(
                        "waited: {:.1}s (limit {:.0}s, real time)",
                        elapsed.as_secs_f32(),
                        LOADING_TIMEOUT.as_secs_f32()
                    ),
                    observation.state_details.clone(),
                ],
            );
            return;
        }
        CatalogLoadOutcome::Ready => {}
    }

    let reference = match prepare_variant(
        CharacterVariant::Reference,
        &catalog.reference,
        &assets,
        &gltfs,
        &clips,
        &mut graphs,
    ) {
        Ok(prepared) => prepared,
        Err((summary, details)) => {
            *prepared = true;
            fail(&mut next_state, &mut report, summary, details);
            return;
        }
    };
    let technician_man = match prepare_variant(
        CharacterVariant::TechnicianMan,
        &catalog.technician_man,
        &assets,
        &gltfs,
        &clips,
        &mut graphs,
    ) {
        Ok(prepared) => prepared,
        Err((summary, details)) => {
            *prepared = true;
            fail(&mut next_state, &mut report, summary, details);
            return;
        }
    };

    *prepared = true;
    commands.insert_resource(PreparedCharacterCatalog::new(reference, technician_man));

    info!("both glTFs match their manifests; entering Validating");
    next_state.set(PrototypeState::Validating);
}

/// Spawns the validated scene. This runs on `OnEnter(Validating)`, one frame
/// after the transition was requested, so `Validating` is a state the run
/// really occupies and really logs.
pub fn spawn_character(
    mut commands: Commands,
    catalog: Res<CharacterCatalog>,
    prepared: Res<PreparedCharacterCatalog>,
    real: Res<Time<Real>>,
) {
    commands.insert_resource(ValidatingStartedAt(real.elapsed()));
    commands.insert_resource(VariantReadiness::default());
    commands.insert_resource(ValidatedVariants::default());
    let humanoid = commands
        .spawn((
            Name::new("Humanoid"),
            Humanoid,
            HumanoidController::default(),
            Transform::IDENTITY,
            Visibility::Inherited,
        ))
        .id();

    for (variant, config, prepared, visibility) in [
        (
            CharacterVariant::Reference,
            &catalog.reference,
            &prepared.reference,
            Visibility::Inherited,
        ),
        (
            CharacterVariant::TechnicianMan,
            &catalog.technician_man,
            &prepared.technician_man,
            Visibility::Hidden,
        ),
    ] {
        commands
            .spawn((
                Name::new(variant.label()),
                variant,
                ChildOf(humanoid),
                bevy::world_serialization::WorldAssetRoot(prepared.scene.clone()),
                character_transform(config.scale, config.yaw_degrees),
                visibility,
                PendingCharacter {
                    graph: prepared.graph.clone(),
                    node: prepared.node,
                    duration: prepared.duration,
                },
            ))
            .observe(forward_world_instance_ready)
            .observe(start_animation);
    }

    info!("spawned both character scenes; validating their hierarchies");
}

/// Fails the run if the spawned world instance never becomes ready. Without
/// this the application would sit in `Validating` forever, looking healthy.
fn poll_validating(
    started_at: Option<Res<ValidatingStartedAt>>,
    real: Res<Time<Real>>,
    catalog: Res<CharacterCatalog>,
    readiness: Res<VariantReadiness>,
    mut next_state: ResMut<NextState<PrototypeState>>,
    mut report: ResMut<FailureReport>,
    mut reported: Local<bool>,
) {
    if *reported {
        return;
    }
    let Some(started_at) = started_at else {
        *reported = true;
        fail(
            &mut next_state,
            &mut report,
            "Character validation watchdog has no start marker",
            vec![
                "Validating was entered without recording its wall-clock start time".to_string(),
                "the runtime cannot enforce the spawned-scene readiness timeout".to_string(),
            ],
        );
        return;
    };
    let elapsed = real.elapsed().saturating_sub(started_at.0);
    if !timed_out(elapsed, VALIDATING_TIMEOUT) {
        return;
    }

    *reported = true;
    let mut details = vec![format!(
        "waited: {:.1}s (limit {:.0}s, real time) with no completed validation",
        elapsed.as_secs_f32(),
        VALIDATING_TIMEOUT.as_secs_f32()
    )];
    for variant in CharacterVariant::ALL {
        if readiness.players(variant).is_none() {
            let config = variant.config(&catalog);
            details.push(format!(
                "{} still pending: scene '{}' from {}",
                variant.label(),
                config.scene_name,
                config.gltf_path
            ));
        }
    }

    fail(
        &mut next_state,
        &mut report,
        "Spawned character scenes never both became ready",
        details,
    );
}

/// Runs once the spawned world instance exists, so the hierarchy can be
/// searched for the `AnimationPlayer` entities it really produced. This is the
/// only system that may request `Running`.
#[allow(clippy::too_many_arguments)]
fn start_animation(
    ready: On<VariantHierarchyReady>,
    mut commands: Commands,
    children: Query<&Children>,
    pending: Query<(&PendingCharacter, &CharacterVariant)>,
    all_players: Query<(), With<AnimationPlayer>>,
    mut players: Query<&mut AnimationPlayer>,
    catalog: Res<CharacterCatalog>,
    mut readiness: ResMut<VariantReadiness>,
    mut validated_variants: ResMut<ValidatedVariants>,
    mut next_state: ResMut<NextState<PrototypeState>>,
    mut report: ResMut<FailureReport>,
) {
    let root = ready.entity;
    let Ok((pending, variant)) = pending.get(root) else {
        return;
    };
    let variant = *variant;
    if readiness.players(variant).is_some() {
        return;
    }
    let config = variant.config(&catalog);

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
                    format!("variant: {}", variant.label()),
                    format!("scene: {}", config.scene_name),
                    format!("asset path: {}", config.gltf_path),
                    error.to_string(),
                ],
            );
            return;
        }
    };

    readiness.mark_ready(variant, player_entities.len());
    validated_variants.insert(
        variant,
        ValidatedVariant {
            root,
            players: player_entities,
            graph: pending.graph.clone(),
            node: pending.node,
            duration: pending.duration,
        },
    );

    if !readiness.all_ready() {
        info!(
            "{} hierarchy validated; waiting for the other variant",
            variant.label()
        );
        return;
    }

    let Some(validated) = validated_variants.both() else {
        fail(
            &mut next_state,
            &mut report,
            "Validated character readiness is internally inconsistent",
            vec!["both variants were marked ready but their player records are incomplete".into()],
        );
        return;
    };

    let [reference, technician_man] = validated;
    let common_cycle_duration = reference.duration;
    let speeds = match phase_synchronized_playback_speeds(
        Some(common_cycle_duration),
        Some(technician_man.duration),
    ) {
        Ok(speeds) => speeds,
        Err(error) => {
            fail(
                &mut next_state,
                &mut report,
                "Character walk loops cannot be phase synchronized",
                vec![
                    error.to_string(),
                    format!(
                        "{} duration: {}s",
                        CharacterVariant::Reference.label(),
                        reference.duration
                    ),
                    format!(
                        "{} duration: {}s",
                        CharacterVariant::TechnicianMan.label(),
                        technician_man.duration
                    ),
                ],
            );
            return;
        }
    };

    for (validated, speed) in [
        (reference, speeds.reference),
        (technician_man, speeds.technician_man),
    ] {
        for entity in validated.players {
            let Ok(mut player) = players.get_mut(entity) else {
                fail(
                    &mut next_state,
                    &mut report,
                    "Validated AnimationPlayer disappeared before startup",
                    vec![format!("entity: {entity}")],
                );
                return;
            };
            let mut transitions = AnimationTransitions::new();
            transitions
                .play(&mut player, validated.node, Duration::ZERO)
                .repeat()
                .set_speed(speed);
            commands
                .entity(entity)
                .insert((AnimationGraphHandle(validated.graph.clone()), transitions));
        }
        commands.entity(validated.root).remove::<PendingCharacter>();
    }

    info!(
        "both character walk loops started at phase zero with a shared {:.4}s cycle \
         (reference {:.4}x, technician {:.4}x)",
        common_cycle_duration, speeds.reference, speeds.technician_man
    );
    next_state.set(PrototypeState::Running);
}
