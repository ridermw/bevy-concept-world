//! Contract tests for the performance-baseline diagnostics.
//!
//! The baseline exists to make the deferred GPU-host performance gate
//! actionable: a run on a machine with a real adapter must be able to read
//! startup time, entity count, mesh and material counts, and decoded texture
//! bytes straight out of its own log, without anyone estimating anything.
//!
//! Everything here is either a pure function the application itself calls, or
//! a real `App` exercising the real plugin. The formatting is asserted
//! exactly, because the documented "look for this line" instruction is only
//! useful if the line really has that shape.

use std::time::Duration;

use bevy::{
    MinimalPlugins,
    asset::AssetPlugin,
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
    state::app::StatesPlugin,
};
use bevy_concept_world::{
    perf::{
        BASELINE_LOG_PREFIX, BaselineSnapshot, FRAME_TIME_LOG_INTERVAL, ImageBytes,
        PerformancePlugin, RunningBaseline, format_bytes, summarize_image_bytes,
    },
    state::PrototypeState,
};

// --- byte formatting ------------------------------------------------------

#[test]
fn small_totals_are_reported_in_plain_bytes() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(1), "1 B");
    assert_eq!(format_bytes(1023), "1023 B");
}

#[test]
fn a_kibibyte_is_the_first_scaled_unit() {
    assert_eq!(format_bytes(1024), "1.00 KiB");
    assert_eq!(format_bytes(1536), "1.50 KiB");
}

#[test]
fn larger_totals_scale_through_mebibytes_and_gibibytes() {
    assert_eq!(format_bytes(1024 * 1024), "1.00 MiB");
    assert_eq!(format_bytes(3 * 1024 * 1024 + 512 * 1024), "3.50 MiB");
    assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GiB");
}

#[test]
fn the_largest_representable_total_does_not_panic_or_overflow_its_unit() {
    // A texture budget can never reach this, but a formatter that panics on
    // the extreme is a formatter that can take down the run it was added to
    // observe.
    let text = format_bytes(u64::MAX);
    assert!(text.ends_with(" GiB"), "{text}");
}

// --- decoded image accounting ---------------------------------------------

#[test]
fn no_images_summarize_to_all_zeros() {
    assert_eq!(summarize_image_bytes([]), ImageBytes::default());
}

#[test]
fn decoded_image_bytes_are_summed_across_every_image() {
    let summary = summarize_image_bytes([Some(16), Some(32), Some(64)]);

    assert_eq!(
        summary,
        ImageBytes {
            images: 3,
            decoded_bytes: 112,
            without_cpu_data: 0,
        }
    );
}

#[test]
fn an_image_with_no_cpu_side_data_is_counted_separately_rather_than_as_zero() {
    // `Image::data` is `None` for a GPU-only texture. Folding those silently
    // into the total would understate the figure without saying so, and the
    // whole point of this line is that it can be trusted.
    let summary = summarize_image_bytes([Some(100), None, Some(28), None]);

    assert_eq!(
        summary,
        ImageBytes {
            images: 4,
            decoded_bytes: 128,
            without_cpu_data: 2,
        }
    );
}

#[test]
fn the_running_total_is_widened_so_large_textures_cannot_wrap() {
    let huge = usize::MAX;
    let summary = summarize_image_bytes([Some(huge), Some(huge)]);

    assert_eq!(summary.images, 2);
    assert_eq!(summary.decoded_bytes, (huge as u64).saturating_mul(2));
}

// --- the baseline log line ------------------------------------------------

fn snapshot() -> BaselineSnapshot {
    BaselineSnapshot {
        startup_elapsed: Duration::from_millis(1234),
        entities: 118,
        meshes: 8,
        materials: 6,
        images: ImageBytes {
            images: 4,
            decoded_bytes: 3 * 1024 * 1024,
            without_cpu_data: 1,
        },
    }
}

#[test]
fn the_baseline_line_reports_every_number_the_design_asks_for() {
    assert_eq!(
        snapshot().to_log_line(),
        "performance baseline: startup_to_running=1.234s entities=118 meshes=8 \
         standard_materials=6 images=4 decoded_image_bytes=3145728 (3.00 MiB) \
         images_without_cpu_data=1"
    );
}

#[test]
fn the_baseline_line_starts_with_the_documented_prefix() {
    assert!(
        snapshot().to_log_line().starts_with(BASELINE_LOG_PREFIX),
        "the README tells operators to grep for {BASELINE_LOG_PREFIX:?}"
    );
}

#[test]
fn startup_time_keeps_millisecond_resolution() {
    let mut snapshot = snapshot();
    snapshot.startup_elapsed = Duration::from_micros(7_500);

    assert!(
        snapshot.to_log_line().contains("startup_to_running=0.008s"),
        "{}",
        snapshot.to_log_line()
    );
}

#[test]
fn a_scene_with_no_textures_still_reports_a_real_zero() {
    let mut snapshot = snapshot();
    snapshot.images = ImageBytes::default();

    let line = snapshot.to_log_line();
    assert!(line.contains("images=0"), "{line}");
    assert!(line.contains("decoded_image_bytes=0 (0 B)"), "{line}");
    assert!(line.contains("images_without_cpu_data=0"), "{line}");
}

// --- App-level: the real plugin -------------------------------------------

fn perf_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(StatesPlugin)
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_asset::<Image>()
        .add_plugins(PerformancePlugin)
        .insert_state(PrototypeState::Loading);
    app
}

fn enter(app: &mut App, state: PrototypeState) {
    app.world_mut()
        .resource_mut::<NextState<PrototypeState>>()
        .set(state);
    app.update();
}

#[test]
fn frame_time_diagnostics_are_registered_so_the_gpu_host_can_read_them() {
    let mut app = perf_app();
    app.update();

    let store = app.world().resource::<DiagnosticsStore>();
    for path in [
        FrameTimeDiagnosticsPlugin::FRAME_TIME,
        FrameTimeDiagnosticsPlugin::FPS,
        FrameTimeDiagnosticsPlugin::FRAME_COUNT,
    ] {
        assert!(store.get(&path).is_some(), "{path} must be registered");
    }
}

#[test]
fn the_periodic_frame_time_log_is_slow_enough_not_to_flood_the_transcript() {
    // A one-second default would bury the state and capture lines the smoke
    // test transcript is read for.
    assert!(
        FRAME_TIME_LOG_INTERVAL >= Duration::from_secs(5),
        "{FRAME_TIME_LOG_INTERVAL:?}"
    );
}

#[test]
fn no_baseline_is_recorded_before_the_run_reaches_running() {
    let mut app = perf_app();
    for _ in 0..4 {
        app.update();
    }
    enter(&mut app, PrototypeState::Validating);

    assert_eq!(app.world().resource::<RunningBaseline>().get(), None);
}

#[test]
fn reaching_running_records_a_baseline_from_the_real_world() {
    let mut app = perf_app();
    app.update();

    app.world_mut().spawn_empty();
    app.world_mut().spawn_empty();
    let before = app.world().entities().count_spawned() as usize;

    enter(&mut app, PrototypeState::Running);

    let baseline = app
        .world()
        .resource::<RunningBaseline>()
        .get()
        .expect("entering Running must record the baseline");
    assert!(baseline.entities >= before, "{baseline:?}");
    assert!(
        baseline.startup_elapsed > Duration::ZERO,
        "startup time must come from the real clock, got {:?}",
        baseline.startup_elapsed
    );
}

#[test]
fn the_baseline_counts_the_meshes_materials_and_decoded_texture_bytes_in_the_world() {
    let mut app = perf_app();
    app.update();

    let mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Mesh::from(Cuboid::default()));
    let material = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial::default());
    let image = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::default());
    let expected_bytes = app
        .world()
        .resource::<Assets<Image>>()
        .iter()
        .filter_map(|(_, image)| image.data.as_ref())
        .map(|data| data.len() as u64)
        .sum::<u64>();

    enter(&mut app, PrototypeState::Running);

    let baseline = app.world().resource::<RunningBaseline>().get().unwrap();
    assert_eq!(baseline.meshes, 1);
    assert_eq!(baseline.materials, 1);
    assert_eq!(baseline.images.images, 1);
    assert_eq!(baseline.images.decoded_bytes, expected_bytes);

    // The handles are held until here on purpose: a dropped handle frees the
    // asset before the baseline is taken, and the counts would read zero.
    drop((mesh, material, image));
}

#[test]
fn the_baseline_is_recorded_once_and_never_overwritten_by_a_later_entry() {
    // The state machine does not re-enter `Running`, but a one-time line that
    // silently re-fires would turn a baseline into noise, and a later reading
    // would no longer be the startup measurement it claims to be.
    let mut app = perf_app();
    app.update();

    enter(&mut app, PrototypeState::Running);
    let first = app.world().resource::<RunningBaseline>().get().unwrap();

    app.world_mut().spawn_empty();
    enter(&mut app, PrototypeState::Validating);
    enter(&mut app, PrototypeState::Running);
    let second = app.world().resource::<RunningBaseline>().get().unwrap();

    assert_eq!(first, second);
}

#[test]
fn a_failed_run_never_reports_a_startup_baseline() {
    let mut app = perf_app();
    app.update();

    enter(&mut app, PrototypeState::Failed);
    for _ in 0..4 {
        app.update();
    }

    assert_eq!(app.world().resource::<RunningBaseline>().get(), None);
}
