//! Contract tests for the runtime decisions that used to be buried inside
//! Bevy systems: where the asset root comes from, when a load has failed or
//! timed out, how the unattended-capture environment variable is parsed, when
//! a capture is provably on disk, and which exit code a failure produces.
//!
//! Every helper under test is the *same* function the application calls, so
//! these are not a parallel re-implementation. The two App-level tests build a
//! real `App`/`World` and exercise the real state transition and the real
//! hierarchy walk.

use std::{path::Path, path::PathBuf, time::Duration};

use bevy::{
    MinimalPlugins,
    animation::AnimationPlayer,
    app::AppExit,
    ecs::{hierarchy::ChildOf, system::RunSystemOnce},
    prelude::*,
    state::app::StatesPlugin,
};
use bevy_concept_world::{
    ASSET_ROOT_ENV, AssetRootError, AssetRootSource,
    character::{
        CatalogLoadOutcome, CharacterVariant, LoadOutcome, LoadPhase, SelectionError,
        VariantReadiness, check_animation_players, evaluate_catalog_load, evaluate_load, timed_out,
    },
    diagnostics::{CAPTURE_ENV, CaptureVerdict, parse_capture_seconds, verify_capture},
    resolve_asset_root_from,
    state::{FailureReport, PrototypeState, escape_exit, fail},
};

// --- asset-root resolution ------------------------------------------------

/// A predicate over an explicit set of directories, so resolution is tested
/// without touching the real filesystem or the real environment.
fn present(dirs: &[&str]) -> impl Fn(&Path) -> bool + use<> {
    let dirs: Vec<PathBuf> = dirs.iter().map(PathBuf::from).collect();
    move |path: &Path| dirs.iter().any(|dir| dir == path)
}

#[test]
fn the_env_override_wins_over_every_other_candidate() {
    let exists = present(&["/override", "/cwd/assets", "/exe/assets"]);

    let root = resolve_asset_root_from(
        Some(Path::new("/override")),
        Path::new("/cwd"),
        Some(Path::new("/exe/app")),
        &exists,
    )
    .unwrap();

    assert_eq!(root.path, PathBuf::from("/override"));
    assert_eq!(root.source, AssetRootSource::Override);
}

#[test]
fn an_env_override_that_does_not_exist_is_a_loud_error_not_a_fallback() {
    let exists = present(&["/cwd/assets"]);

    let error = resolve_asset_root_from(
        Some(Path::new("/nowhere")),
        Path::new("/cwd"),
        Some(Path::new("/exe/app")),
        &exists,
    )
    .unwrap_err();

    match error {
        AssetRootError::OverrideMissing { env, path } => {
            assert_eq!(env, ASSET_ROOT_ENV);
            assert_eq!(path, PathBuf::from("/nowhere"));
        }
        other => panic!("expected OverrideMissing, got {other}"),
    }
}

#[test]
fn the_working_directory_is_used_when_it_holds_assets() {
    // This is the `cargo run` case: the working directory is the crate root.
    let exists = present(&["/cwd/assets", "/exe/assets"]);

    let root = resolve_asset_root_from(
        None,
        Path::new("/cwd"),
        Some(Path::new("/exe/app")),
        &exists,
    )
    .unwrap();

    assert_eq!(root.path, PathBuf::from("/cwd/assets"));
    assert_eq!(root.source, AssetRootSource::WorkingDirectory);
}

#[test]
fn a_copied_binary_finds_the_assets_beside_it() {
    let exists = present(&["/elsewhere/assets"]);

    let root = resolve_asset_root_from(
        None,
        Path::new("/some/other/cwd"),
        Some(Path::new("/elsewhere/app.exe")),
        &exists,
    )
    .unwrap();

    assert_eq!(root.path, PathBuf::from("/elsewhere/assets"));
    assert_eq!(root.source, AssetRootSource::ExecutableDirectory);
}

#[test]
fn a_target_release_binary_finds_the_repository_assets_above_it() {
    let exists = present(&["/repo/assets"]);

    let root = resolve_asset_root_from(
        None,
        Path::new("/some/other/cwd"),
        Some(Path::new("/repo/target/release/app.exe")),
        &exists,
    )
    .unwrap();

    assert_eq!(root.path, PathBuf::from("/repo/assets"));
    assert_eq!(root.source, AssetRootSource::ExecutableDirectory);
}

#[test]
fn resolution_fails_with_every_candidate_it_looked_at() {
    let exists = present(&[]);

    let error = resolve_asset_root_from(
        None,
        Path::new("/cwd"),
        Some(Path::new("/repo/target/release/app.exe")),
        &exists,
    )
    .unwrap_err();

    match error {
        AssetRootError::NotFound { candidates } => {
            assert!(
                candidates.contains(&PathBuf::from("/cwd/assets")),
                "{candidates:?}"
            );
            assert!(
                candidates.contains(&PathBuf::from("/repo/target/release/assets")),
                "{candidates:?}"
            );
            assert!(
                candidates.contains(&PathBuf::from("/repo/assets")),
                "{candidates:?}"
            );
        }
        other => panic!("expected NotFound, got {other}"),
    }
}

#[test]
fn resolution_still_reports_candidates_when_the_executable_is_unknown() {
    let error = resolve_asset_root_from(None, Path::new("/cwd"), None, &present(&[])).unwrap_err();

    let message = error.to_string();
    // Rendered with the host separator, so the two components are asserted
    // rather than a hard-coded Unix path.
    assert!(message.contains("cwd"), "{message}");
    assert!(message.contains("assets"), "{message}");
}

// --- load-state evaluation ------------------------------------------------

const TIMEOUT: Duration = Duration::from_secs(60);

#[test]
fn loading_is_only_ready_when_the_root_and_both_dependency_levels_are_loaded() {
    assert_eq!(
        evaluate_load(
            LoadPhase::Ready,
            LoadPhase::Ready,
            LoadPhase::Ready,
            Duration::ZERO,
            TIMEOUT
        ),
        LoadOutcome::Ready
    );
    assert_eq!(
        evaluate_load(
            LoadPhase::Ready,
            LoadPhase::Ready,
            LoadPhase::Pending,
            Duration::ZERO,
            TIMEOUT
        ),
        LoadOutcome::Waiting
    );
}

#[test]
fn a_failed_direct_dependency_fails_the_load() {
    assert_eq!(
        evaluate_load(
            LoadPhase::Ready,
            LoadPhase::Failed,
            LoadPhase::Pending,
            Duration::ZERO,
            TIMEOUT
        ),
        LoadOutcome::Failed
    );
}

#[test]
fn a_failed_recursive_dependency_fails_the_load() {
    assert_eq!(
        evaluate_load(
            LoadPhase::Ready,
            LoadPhase::Ready,
            LoadPhase::Failed,
            Duration::ZERO,
            TIMEOUT
        ),
        LoadOutcome::Failed
    );
}

#[test]
fn a_failed_root_fails_the_load() {
    assert_eq!(
        evaluate_load(
            LoadPhase::Failed,
            LoadPhase::Pending,
            LoadPhase::Pending,
            Duration::ZERO,
            TIMEOUT
        ),
        LoadOutcome::Failed
    );
}

#[test]
fn a_failure_is_reported_even_after_the_timeout_has_elapsed() {
    // The actionable cause is the loader error, never the timeout it caused.
    assert_eq!(
        evaluate_load(
            LoadPhase::Failed,
            LoadPhase::Failed,
            LoadPhase::Failed,
            Duration::from_secs(600),
            TIMEOUT
        ),
        LoadOutcome::Failed
    );
}

#[test]
fn a_load_that_never_finishes_times_out_instead_of_waiting_forever() {
    assert_eq!(
        evaluate_load(
            LoadPhase::Pending,
            LoadPhase::Pending,
            LoadPhase::Pending,
            TIMEOUT,
            TIMEOUT
        ),
        LoadOutcome::TimedOut
    );
}

#[test]
fn a_completed_load_is_ready_even_if_it_took_longer_than_the_timeout() {
    assert_eq!(
        evaluate_load(
            LoadPhase::Ready,
            LoadPhase::Ready,
            LoadPhase::Ready,
            Duration::from_secs(600),
            TIMEOUT
        ),
        LoadOutcome::Ready
    );
}

#[test]
fn a_deadline_is_reached_only_at_or_after_the_limit() {
    assert!(!timed_out(Duration::from_millis(59_999), TIMEOUT));
    assert!(timed_out(TIMEOUT, TIMEOUT));
    assert!(timed_out(Duration::from_secs(61), TIMEOUT));
}

#[test]
fn both_variants_must_be_ready_before_validation_can_begin() {
    assert_eq!(
        evaluate_catalog_load([
            (CharacterVariant::Reference, LoadOutcome::Ready),
            (CharacterVariant::TechnicianMan, LoadOutcome::Waiting),
        ]),
        CatalogLoadOutcome::Waiting
    );
}

#[test]
fn either_variant_failure_blocks_the_catalog() {
    assert_eq!(
        evaluate_catalog_load([
            (CharacterVariant::Reference, LoadOutcome::Ready),
            (CharacterVariant::TechnicianMan, LoadOutcome::Failed),
        ]),
        CatalogLoadOutcome::Failed(CharacterVariant::TechnicianMan)
    );
}

#[test]
fn either_variant_timeout_blocks_the_catalog() {
    assert_eq!(
        evaluate_catalog_load([
            (CharacterVariant::Reference, LoadOutcome::TimedOut),
            (CharacterVariant::TechnicianMan, LoadOutcome::Ready),
        ]),
        CatalogLoadOutcome::TimedOut(CharacterVariant::Reference)
    );
}

#[test]
fn both_spawned_variants_must_validate_before_running() {
    let mut readiness = VariantReadiness::default();

    readiness.mark_ready(CharacterVariant::Reference, 1);
    assert!(!readiness.all_ready());
    assert_eq!(readiness.players(CharacterVariant::Reference), Some(1));
    assert_eq!(readiness.players(CharacterVariant::TechnicianMan), None);

    readiness.mark_ready(CharacterVariant::TechnicianMan, 1);
    assert!(readiness.all_ready());
    assert_eq!(readiness.players(CharacterVariant::TechnicianMan), Some(1));
}

// --- unattended capture environment ---------------------------------------

#[test]
fn an_unset_capture_variable_leaves_unattended_mode_off() {
    assert_eq!(parse_capture_seconds(None), Ok(None));
}

#[test]
fn a_whole_number_of_seconds_enables_unattended_mode() {
    assert_eq!(
        parse_capture_seconds(Some(" 5 ")),
        Ok(Some(Duration::from_secs(5)))
    );
}

#[test]
fn zero_seconds_enables_unattended_mode_with_no_delay() {
    assert_eq!(
        parse_capture_seconds(Some("0")),
        Ok(Some(Duration::ZERO)),
        "0 is a legitimate 'capture as soon as Running' request"
    );
}

#[test]
fn a_fractional_delay_is_accepted() {
    assert_eq!(
        parse_capture_seconds(Some("1.5")),
        Ok(Some(Duration::from_millis(1500)))
    );
}

#[test]
fn a_malformed_capture_variable_is_a_loud_error_not_a_silent_disable() {
    for raw in ["", "   ", "abc", "5s", "5,0", "--1"] {
        let error =
            parse_capture_seconds(Some(raw)).expect_err(&format!("{raw:?} must be rejected"));
        assert_eq!(error.env, CAPTURE_ENV);
        assert_eq!(error.raw, raw);
    }
}

#[test]
fn a_negative_capture_delay_is_rejected() {
    let error = parse_capture_seconds(Some("-1")).unwrap_err();

    assert!(error.to_string().contains("-1"), "{error}");
}

#[test]
fn a_non_finite_capture_delay_is_rejected() {
    for raw in ["NaN", "nan", "inf", "-inf", "infinity"] {
        assert!(
            parse_capture_seconds(Some(raw)).is_err(),
            "{raw:?} must be rejected"
        );
    }
}

#[test]
fn an_absurd_capture_delay_is_rejected() {
    assert!(parse_capture_seconds(Some("1e300")).is_err());
}

// --- capture verification -------------------------------------------------

const GRACE: Duration = Duration::from_secs(30);

#[test]
fn a_capture_is_only_successful_once_a_nonempty_file_is_on_disk() {
    assert_eq!(
        verify_capture(Some(1024), Duration::ZERO, GRACE),
        CaptureVerdict::Written
    );
}

#[test]
fn a_capture_is_still_pending_while_the_file_is_absent_and_the_grace_holds() {
    assert_eq!(
        verify_capture(None, Duration::ZERO, GRACE),
        CaptureVerdict::Waiting
    );
}

#[test]
fn a_zero_byte_capture_is_not_success_while_the_grace_holds() {
    assert_eq!(
        verify_capture(Some(0), Duration::ZERO, GRACE),
        CaptureVerdict::Waiting
    );
}

#[test]
fn an_absent_capture_file_fails_once_the_grace_expires() {
    assert_eq!(verify_capture(None, GRACE, GRACE), CaptureVerdict::Missing);
}

#[test]
fn a_zero_byte_capture_file_fails_once_the_grace_expires() {
    assert_eq!(verify_capture(Some(0), GRACE, GRACE), CaptureVerdict::Empty);
}

// --- exit codes -----------------------------------------------------------

#[test]
fn escaping_from_a_failed_run_exits_nonzero() {
    assert_eq!(escape_exit(PrototypeState::Failed), AppExit::error());
}

#[test]
fn escaping_from_any_healthy_state_exits_successfully() {
    for state in [
        PrototypeState::Loading,
        PrototypeState::Validating,
        PrototypeState::Running,
    ] {
        assert_eq!(escape_exit(state), AppExit::Success, "{state:?}");
    }
}

// --- App-level: the real state machine ------------------------------------

fn state_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(StatesPlugin)
        .init_resource::<FailureReport>()
        .insert_state(PrototypeState::Loading);
    app
}

#[test]
fn recording_a_failure_moves_the_real_app_into_the_terminal_failed_state() {
    let mut app = state_app();
    app.add_systems(
        Update,
        |mut next: ResMut<NextState<PrototypeState>>, mut report: ResMut<FailureReport>| {
            if !report.is_recorded() {
                fail(
                    &mut next,
                    &mut report,
                    "Character glTF failed to load",
                    vec!["asset path: a.glb".into()],
                );
            }
        },
    );

    app.update();
    app.update();

    assert_eq!(
        *app.world().resource::<State<PrototypeState>>().get(),
        PrototypeState::Failed
    );
    let report = app.world().resource::<FailureReport>();
    assert_eq!(report.summary, "Character glTF failed to load");
}

#[test]
fn the_app_never_leaves_the_failed_state_on_its_own() {
    let mut app = state_app();
    app.insert_state(PrototypeState::Failed);

    for _ in 0..8 {
        app.update();
    }

    assert_eq!(
        *app.world().resource::<State<PrototypeState>>().get(),
        PrototypeState::Failed
    );
}

// --- App-level: the real spawned-hierarchy walk ---------------------------

/// Builds a scene-like hierarchy `root -> mid -> leaf` and puts an
/// `AnimationPlayer` on `players` of those entities.
fn hierarchy_with_players(world: &mut World, player_depths: &[usize]) -> Entity {
    let root = world.spawn(Name::new("Humanoid")).id();
    let mut parent = root;
    for depth in 1..=3usize {
        let mut entity = world.spawn(ChildOf(parent));
        if player_depths.contains(&depth) {
            entity.insert(AnimationPlayer::default());
        }
        parent = entity.id();
    }
    root
}

fn run_check(world: &mut World, root: Entity, expected: usize) -> Result<usize, SelectionError> {
    world
        .run_system_once(
            move |children: Query<&Children>, players: Query<(), With<AnimationPlayer>>| {
                check_animation_players(root, expected, &children, |entity| {
                    players.contains(entity)
                })
                .map(|found| found.len())
            },
        )
        .expect("the check system must run")
}

#[test]
fn a_hierarchy_with_no_animation_player_is_rejected_so_running_is_unreachable() {
    let mut world = World::new();
    let root = hierarchy_with_players(&mut world, &[]);

    let error = run_check(&mut world, root, 1).unwrap_err();

    assert_eq!(
        error,
        SelectionError::AnimationPlayerCount {
            expected: 1,
            actual: 0
        }
    );
}

#[test]
fn a_deeply_nested_animation_player_is_really_discovered() {
    let mut world = World::new();
    let root = hierarchy_with_players(&mut world, &[3]);

    assert_eq!(run_check(&mut world, root, 1), Ok(1));
}

#[test]
fn every_animation_player_in_the_hierarchy_is_counted() {
    let mut world = World::new();
    let root = hierarchy_with_players(&mut world, &[1, 3]);

    let error = run_check(&mut world, root, 1).unwrap_err();

    assert_eq!(
        error,
        SelectionError::AnimationPlayerCount {
            expected: 1,
            actual: 2
        }
    );
}
