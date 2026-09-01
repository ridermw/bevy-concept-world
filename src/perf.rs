//! Performance-baseline diagnostics.
//!
//! The design asks for debug and release startup time, steady-state frame time
//! for one humanoid, entity count, mesh and material count, and the sum of
//! decoded texture bytes. None of those can be taken on a software-rasterizer
//! host, so the job of this module is not to measure them here — it is to make
//! sure that whoever runs the binary on a GPU host gets every one of those
//! numbers out of the log without instrumenting anything or estimating
//! anything.
//!
//! Two things are emitted:
//!
//! 1. **Steady-state frame time**, from Bevy's own
//!    [`FrameTimeDiagnosticsPlugin`] and [`LogDiagnosticsPlugin`], filtered to
//!    `frame_time` and `fps` and throttled to [`FRAME_TIME_LOG_INTERVAL`] so
//!    it cannot bury the state and capture lines the smoke-test transcript is
//!    read for.
//! 2. **A single [`BASELINE_LOG_PREFIX`] line** on entering
//!    [`PrototypeState::Running`], carrying startup elapsed time and the four
//!    asset- and world-derived counts.
//!
//! Nothing here changes what the prototype renders or how fast it runs; it
//! only reports.

use std::time::Duration;

use bevy::{
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    platform::collections::HashSet,
    prelude::*,
};

use crate::state::PrototypeState;

/// How often the filtered frame-time diagnostic is written to the log.
///
/// Bevy's default is one second, which floods an unattended transcript. The
/// numbers wanted here are steady-state, so a slower cadence loses nothing.
pub const FRAME_TIME_LOG_INTERVAL: Duration = Duration::from_secs(5);

/// The exact start of the one-time baseline line. Documented in the README so
/// an operator can grep for it.
pub const BASELINE_LOG_PREFIX: &str = "performance baseline: ";

/// Decoded-texture accounting over every [`Image`] in the asset collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ImageBytes {
    /// How many images exist.
    pub images: usize,
    /// Sum of the CPU-side decoded byte lengths.
    pub decoded_bytes: u64,
    /// How many images carry no CPU-side data at all.
    ///
    /// Reported separately rather than folded in as zero: a GPU-only texture
    /// still occupies memory, and silently understating the total would make
    /// the figure untrustworthy in exactly the direction that matters.
    pub without_cpu_data: usize,
}

/// Sums decoded image sizes.
///
/// Each item is the CPU-side byte length of one image, or `None` when that
/// image has no CPU-side data. Accumulated in `u64` and saturating, so a
/// pathological total reports a ceiling instead of wrapping to a small,
/// plausible-looking number.
pub fn summarize_image_bytes<I>(sizes: I) -> ImageBytes
where
    I: IntoIterator<Item = Option<usize>>,
{
    let mut summary = ImageBytes::default();
    for size in sizes {
        summary.images += 1;
        match size {
            Some(len) => {
                summary.decoded_bytes = summary.decoded_bytes.saturating_add(len as u64);
            }
            None => summary.without_cpu_data += 1,
        }
    }
    summary
}

/// Renders a byte count as a human-readable binary-prefixed size.
///
/// The exact byte total is always logged beside this, so this is a reading
/// aid, never the only record of the number.
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [(&str, u64); 3] = [("GiB", 1 << 30), ("MiB", 1 << 20), ("KiB", 1 << 10)];

    for (unit, scale) in UNITS {
        if bytes >= scale {
            return format!("{:.2} {unit}", bytes as f64 / scale as f64);
        }
    }
    format!("{bytes} B")
}

/// Renders a duration as seconds with millisecond resolution.
///
/// Done in integer nanoseconds, rounded to the nearest millisecond, because
/// `{:.3}` over `as_secs_f64` truncates values such as 7.5 ms down to 7 ms:
/// the nearest `f64` to 0.0075 is below it.
fn format_seconds(duration: Duration) -> String {
    let millis = (duration.as_nanos() + 500_000) / 1_000_000;
    format!("{}.{:03}s", millis / 1000, millis % 1000)
}

/// The one-time measurement taken when the prototype reaches `Running`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BaselineSnapshot {
    /// Real time from application startup to entering `Running`. Which build
    /// profile this describes is the caller's to record; the same line is
    /// emitted by a debug and a release run.
    pub startup_elapsed: Duration,
    /// Live entities in the main world.
    pub entities: usize,
    /// `Mesh` assets held by the asset server.
    pub meshes: usize,
    /// `StandardMaterial` assets held by the asset server.
    pub materials: usize,
    /// Decoded texture accounting.
    pub images: ImageBytes,
}

impl BaselineSnapshot {
    /// The single log line this whole module exists to produce.
    pub fn to_log_line(&self) -> String {
        format!(
            "{BASELINE_LOG_PREFIX}startup_to_running={} entities={} meshes={} \
             standard_materials={} images={} decoded_image_bytes={} ({}) \
             images_without_cpu_data={}",
            format_seconds(self.startup_elapsed),
            self.entities,
            self.meshes,
            self.materials,
            self.images.images,
            self.images.decoded_bytes,
            format_bytes(self.images.decoded_bytes),
            self.images.without_cpu_data,
        )
    }
}

/// Holds the baseline once it has been taken, so it is taken exactly once and
/// so a test can assert on it without parsing the log.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct RunningBaseline(Option<BaselineSnapshot>);

impl RunningBaseline {
    /// The recorded baseline, or `None` if the run never reached `Running`.
    pub fn get(&self) -> Option<BaselineSnapshot> {
        self.0
    }

    /// True once a baseline has been taken.
    pub fn is_recorded(&self) -> bool {
        self.0.is_some()
    }
}

/// Frame-time logging plus the one-time `Running` baseline.
pub struct PerformancePlugin;

impl Plugin for PerformancePlugin {
    fn build(&self, app: &mut App) {
        // `FrameTimeDiagnosticsPlugin` registers the diagnostics — and the
        // store itself — so it must be built before the logger that reads it.
        app.add_plugins(FrameTimeDiagnosticsPlugin::default())
            .add_plugins(LogDiagnosticsPlugin {
                debug: false,
                wait_duration: FRAME_TIME_LOG_INTERVAL,
                filter: Some(HashSet::from_iter([
                    FrameTimeDiagnosticsPlugin::FRAME_TIME,
                    FrameTimeDiagnosticsPlugin::FPS,
                ])),
            })
            .init_resource::<RunningBaseline>()
            .add_systems(OnEnter(PrototypeState::Running), record_baseline);
    }
}

fn record_baseline(
    real: Res<Time<Real>>,
    entities: Query<Entity>,
    meshes: Res<Assets<Mesh>>,
    materials: Res<Assets<StandardMaterial>>,
    images: Res<Assets<Image>>,
    mut baseline: ResMut<RunningBaseline>,
) {
    // A second reading would no longer be the startup measurement it claims
    // to be, so the first one wins and nothing further is logged.
    if baseline.is_recorded() {
        return;
    }

    let snapshot = BaselineSnapshot {
        startup_elapsed: real.elapsed(),
        entities: entities.iter().count(),
        meshes: meshes.len(),
        materials: materials.len(),
        images: summarize_image_bytes(
            images
                .iter()
                .map(|(_, image)| image.data.as_ref().map(Vec::len)),
        ),
    };

    info!("{}", snapshot.to_log_line());
    baseline.0 = Some(snapshot);
}
