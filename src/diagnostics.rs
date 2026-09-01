//! On-screen diagnostics, the prototype's keyboard controls, and the
//! unattended screenshot capture used on hosts with no desktop session.
//!
//! The overlay must keep working when the character manifest never loaded, so
//! everything it reads is optional or always present.
//!
//! Every wall-clock decision here is made on [`Time<Real>`]. A software
//! rasterizer can take seconds per frame and Bevy's default `Time` is the
//! virtual clock, which is affected by pausing and by relative speed; a
//! capture budget expressed in virtual time is not a wall-clock budget.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    state::state::StateTransitionEvent,
};
use thiserror::Error;

use crate::{
    character::Humanoid,
    config::CharacterConfig,
    locomotion::{HumanoidController, movement_status_line},
    state::{FailureReport, PrototypeState, escape_exit},
};

/// Where `P` and the unattended run write the visual acceptance screenshot,
/// relative to the current working directory.
pub const SCREENSHOT_PATH: &str = "docs/validation/humanoid-walk.png";

/// Set to a number of seconds to run unattended: the application waits that
/// long in `Running`, writes [`SCREENSHOT_PATH`], confirms the file is really
/// on disk and non-empty, and exits. This exists because a validation host
/// without an interactive desktop session cannot press `P`.
///
/// A malformed value is a fatal error, never a silent fall back to attended
/// mode: a scripted run that quietly became interactive would hang forever and
/// then be reported as an infrastructure timeout instead of an operator typo.
pub const CAPTURE_ENV: &str = "HUMANOID_WALK_CAPTURE_SECONDS";

/// Largest accepted capture delay: one day. Anything above this is a typo
/// (a millisecond value, or a stray exponent), not a request.
const MAX_CAPTURE_SECONDS: f64 = 86_400.0;

/// How long the unattended run waits for the screenshot to reach disk. A
/// software renderer can take several seconds per frame, and the capture needs
/// a few frames to be rendered, read back, and written.
pub const CAPTURE_GRACE: Duration = Duration::from_secs(240);

/// How long the unattended run waits to reach `Running` before giving up.
pub const CAPTURE_TIMEOUT: Duration = Duration::from_secs(180);

/// A [`CAPTURE_ENV`] value that cannot be honoured.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{env} is set to '{raw}', which is not a usable delay: {reason}")]
pub struct CaptureEnvError {
    pub env: &'static str,
    pub raw: String,
    pub reason: &'static str,
}

impl CaptureEnvError {
    fn new(raw: &str, reason: &'static str) -> Self {
        Self {
            env: CAPTURE_ENV,
            raw: raw.to_string(),
            reason,
        }
    }
}

/// Parses [`CAPTURE_ENV`].
///
/// `None` (unset) leaves unattended mode off. Anything else must be a finite,
/// non-negative number of seconds no larger than a day; every other value is
/// an error, including an empty or whitespace-only string.
pub fn parse_capture_seconds(raw: Option<&str>) -> Result<Option<Duration>, CaptureEnvError> {
    let Some(raw) = raw else {
        return Ok(None);
    };

    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(CaptureEnvError::new(raw, "it is empty"));
    }

    let seconds: f64 = trimmed
        .parse()
        .map_err(|_| CaptureEnvError::new(raw, "it is not a number of seconds"))?;

    if !seconds.is_finite() {
        return Err(CaptureEnvError::new(raw, "it is not finite"));
    }
    if seconds < 0.0 {
        return Err(CaptureEnvError::new(raw, "it is negative"));
    }
    if seconds > MAX_CAPTURE_SECONDS {
        return Err(CaptureEnvError::new(
            raw,
            "it is longer than the one-day maximum",
        ));
    }

    Ok(Some(Duration::from_secs_f64(seconds)))
}

/// Reads [`CAPTURE_ENV`] from the real process environment.
pub fn capture_seconds_from_env() -> Result<Option<Duration>, CaptureEnvError> {
    let raw = std::env::var(CAPTURE_ENV).ok();
    parse_capture_seconds(raw.as_deref())
}

/// What the file on disk says about a requested capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureVerdict {
    /// Not there yet, and there is still time.
    Waiting,
    /// A non-empty file exists: this is the only success.
    Written,
    /// The grace period expired with no file at all.
    Missing,
    /// The grace period expired with a zero-byte file.
    Empty,
}

/// Decides whether a requested capture has succeeded, is still pending, or has
/// failed.
///
/// `file_len` is `None` when the target path does not exist. Success requires
/// a real, non-empty file: the screenshot pipeline is asynchronous and its
/// observer swallows encode and I/O errors into the log, so "the request was
/// made" is not evidence that an image exists.
pub fn verify_capture(file_len: Option<u64>, elapsed: Duration, grace: Duration) -> CaptureVerdict {
    if file_len.is_some_and(|len| len > 0) {
        return CaptureVerdict::Written;
    }
    if elapsed < grace {
        return CaptureVerdict::Waiting;
    }
    match file_len {
        Some(_) => CaptureVerdict::Empty,
        None => CaptureVerdict::Missing,
    }
}

/// The overlay, the controls, and — when [`Self::capture`] is set — the
/// unattended capture run.
pub struct DiagnosticsPlugin {
    /// How long to wait in `Running` before capturing, or `None` for the
    /// ordinary attended run.
    pub capture: Option<Duration>,
}

impl DiagnosticsPlugin {
    /// The attended configuration: overlay and keyboard controls only.
    pub fn attended() -> Self {
        Self { capture: None }
    }

    /// The unattended configuration used by a scripted validation run.
    pub fn unattended(delay: Duration) -> Self {
        Self {
            capture: Some(delay),
        }
    }
}

impl Plugin for DiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_overlay)
            .add_systems(Update, (log_transitions, update_overlay, handle_controls))
            .add_systems(OnEnter(PrototypeState::Failed), log_failure);

        if let Some(delay) = self.capture {
            info!("unattended capture enabled: {delay:?} after reaching Running");
            app.insert_resource(UnattendedCapture::new(delay))
                .add_systems(Update, unattended_capture);
        }
    }
}

#[derive(Component)]
struct StatusText;

const CONTROL_HELP_LINES: [&str; 2] = [
    "Arrows: walk/steer/turn around   Q/E: orbit   Wheel: zoom",
    "Space: pause/resume   P: screenshot   Esc: exit",
];

/// The concise control help shown in the overlay.
pub fn control_help_lines() -> [&'static str; 2] {
    CONTROL_HELP_LINES
}

/// The actions the controls system should perform this frame.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlIntents {
    /// Exit the app with the given code.
    pub exit: Option<AppExit>,
    /// Toggle pause on every animation player.
    pub toggle_pause: bool,
    /// Request a screenshot from the primary window.
    pub screenshot: bool,
}

/// Interprets the control keys from the current frame.
pub fn control_intents(keys: &ButtonInput<KeyCode>, state: PrototypeState) -> ControlIntents {
    ControlIntents {
        exit: keys
            .just_pressed(KeyCode::Escape)
            .then(|| escape_exit(state)),
        toggle_pause: keys.just_pressed(KeyCode::Space),
        screenshot: keys.just_pressed(KeyCode::KeyP),
    }
}

/// The capture target. Kept relative to the working directory so a scripted
/// run writes into the repository it was launched from.
pub fn screenshot_path() -> PathBuf {
    PathBuf::from(SCREENSHOT_PATH)
}

/// Creates the capture's parent directory. Bevy's `save_to_disk` observer does
/// not create it, and a missing directory would otherwise surface only as a
/// logged I/O error long after the request.
fn prepare_capture_target(path: &Path) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }
    // A stale image from an earlier run must never be mistaken for this run's
    // output, so the target is removed before the request is made.
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("could not remove {}: {error}", path.display())),
    }
}

fn spawn_overlay(mut commands: Commands) {
    commands.spawn((
        Name::new("Diagnostics overlay"),
        StatusText,
        Text::new("Starting humanoid prototype..."),
        TextFont {
            font_size: FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            left: px(12),
            ..default()
        },
    ));
}

#[allow(clippy::too_many_arguments)]
fn update_overlay(
    state: Res<State<PrototypeState>>,
    config: Option<Res<CharacterConfig>>,
    report: Res<FailureReport>,
    clips: Res<Assets<AnimationClip>>,
    graphs: Res<Assets<AnimationGraph>>,
    controllers: Query<&HumanoidController, With<Humanoid>>,
    // Every player in the world, whether or not a graph was attached to it. A
    // count taken only from graph-carrying players would report the expected
    // number by construction and hide the exact defect — a discovered player
    // that was never wired up — that this overlay exists to expose.
    players: Query<&AnimationPlayer>,
    graph_players: Query<(&AnimationPlayer, &AnimationGraphHandle)>,
    mut overlay: Query<&mut Text, With<StatusText>>,
) {
    let mut lines = vec![format!("State: {:?}", state.get())];

    match config.as_deref() {
        Some(config) => {
            lines.push(format!("Asset: {}", config.gltf_path));
            lines.push(format!(
                "Scene: {}   Clip: {}",
                config.scene_name, config.animation_name
            ));
        }
        None => lines.push("Asset: <manifest unavailable>".to_string()),
    }

    let total = players.iter().count();
    let wired = graph_players.iter().count();
    lines.push(format!(
        "Animation players: {total} ({wired} with an animation graph)"
    ));
    for (player, graph_handle) in &graph_players {
        for (node, active) in player.playing_animations() {
            let duration = graphs
                .get(&graph_handle.0)
                .and_then(|graph| graph.get(*node))
                .and_then(|node| match &node.node_type {
                    AnimationNodeType::Clip(clip) => clips.get(clip),
                    _ => None,
                })
                .map_or(f32::NAN, AnimationClip::duration);
            lines.push(format!(
                "  clip {:.2}s  speed {:.2}x  {}",
                duration,
                active.speed(),
                if active.is_paused() {
                    "paused"
                } else {
                    "playing"
                },
            ));
        }
    }

    if let Some(line) = controllers
        .iter()
        .find_map(|controller| movement_status_line(controller.turning_around()))
    {
        lines.push(line.to_string());
    }

    if report.is_recorded() {
        lines.push(String::new());
        lines.push(format!("FAILED: {}", report.to_display_string()));
    }

    lines.push(String::new());
    lines.extend(control_help_lines().into_iter().map(str::to_string));

    let text = lines.join("\n");
    for mut overlay in &mut overlay {
        if overlay.0 != text {
            overlay.0 = text.clone();
        }
    }
}

fn handle_controls(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<PrototypeState>>,
    mut players: Query<&mut AnimationPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    let intents = control_intents(&keys, *state.get());

    if let Some(app_exit) = intents.exit {
        exit.write(app_exit);
    }

    if intents.toggle_pause {
        for mut player in &mut players {
            if player.all_paused() {
                player.resume_all();
            } else {
                player.pause_all();
            }
        }
    }

    if intents.screenshot {
        let path = screenshot_path();
        if let Err(error) = prepare_capture_target(&path) {
            error!("cannot capture screenshot: {error}");
            return;
        }
        info!("capturing screenshot to {}", path.display());
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
}

/// Records every state change in the log, so a headless or remote run can be
/// verified from its output alone.
fn log_transitions(mut transitions: MessageReader<StateTransitionEvent<PrototypeState>>) {
    for transition in transitions.read() {
        info!(
            "prototype state: {:?} -> {:?}",
            transition.exited, transition.entered
        );
    }
}

/// Drives an unattended screenshot run. Identical in effect to pressing `P`
/// and then `Escape`, but without a desktop session — and, unlike a keypress,
/// it verifies its own output before reporting success.
#[derive(Resource, Debug)]
struct UnattendedCapture {
    delay: Duration,
    /// Real-time instant at which the plugin started counting.
    started_at: Option<Duration>,
    /// Real-time instant at which the screenshot was requested.
    requested_at: Option<Duration>,
    /// Set by the screenshot observer, purely so a failure can say whether the
    /// render pipeline produced an image at all.
    captured: bool,
    finished: bool,
}

impl UnattendedCapture {
    fn new(delay: Duration) -> Self {
        Self {
            delay,
            started_at: None,
            requested_at: None,
            captured: false,
            finished: false,
        }
    }
}

/// Observer attached to the capture entity. It records that the render
/// pipeline delivered an image; the file itself is still verified on disk.
fn note_capture(_captured: On<ScreenshotCaptured>, capture: Option<ResMut<UnattendedCapture>>) {
    if let Some(mut capture) = capture {
        capture.captured = true;
    }
}

fn fail_capture(
    exit: &mut MessageWriter<AppExit>,
    report: &mut FailureReport,
    summary: &str,
    details: Vec<String>,
) {
    report.record(summary, details);
    error!("{}", report.to_display_string());
    exit.write(AppExit::error());
}

#[allow(clippy::too_many_arguments)]
fn unattended_capture(
    mut commands: Commands,
    real: Res<Time<Real>>,
    state: Res<State<PrototypeState>>,
    mut capture: ResMut<UnattendedCapture>,
    mut report: ResMut<FailureReport>,
    mut exit: MessageWriter<AppExit>,
) {
    if capture.finished {
        return;
    }

    let now = real.elapsed();
    let started_at = *capture.started_at.get_or_insert(now);
    let elapsed = now.saturating_sub(started_at);
    let path = screenshot_path();

    if let Some(requested_at) = capture.requested_at {
        let waited = now.saturating_sub(requested_at);
        let file_len = std::fs::metadata(&path).ok().map(|meta| meta.len());

        match verify_capture(file_len, waited, CAPTURE_GRACE) {
            CaptureVerdict::Waiting => {}
            CaptureVerdict::Written => {
                capture.finished = true;
                info!(
                    "unattended capture verified: {} ({} bytes); exiting",
                    path.display(),
                    file_len.unwrap_or_default()
                );
                exit.write(AppExit::Success);
            }
            CaptureVerdict::Missing => {
                capture.finished = true;
                fail_capture(
                    &mut exit,
                    &mut report,
                    "Unattended capture produced no screenshot file",
                    vec![
                        format!("expected file: {}", path.display()),
                        format!(
                            "waited: {:.1}s (limit {:.0}s, real time)",
                            waited.as_secs_f32(),
                            CAPTURE_GRACE.as_secs_f32()
                        ),
                        format!(
                            "the render pipeline {} deliver a captured image",
                            if capture.captured { "did" } else { "did not" }
                        ),
                    ],
                );
            }
            CaptureVerdict::Empty => {
                capture.finished = true;
                fail_capture(
                    &mut exit,
                    &mut report,
                    "Unattended capture wrote an empty screenshot file",
                    vec![
                        format!("file: {} (0 bytes)", path.display()),
                        format!(
                            "waited: {:.1}s (limit {:.0}s, real time)",
                            waited.as_secs_f32(),
                            CAPTURE_GRACE.as_secs_f32()
                        ),
                    ],
                );
            }
        }
        return;
    }

    match state.get() {
        PrototypeState::Failed => {
            capture.finished = true;
            error!("unattended run reached Failed; no screenshot captured");
            exit.write(AppExit::error());
        }
        PrototypeState::Running if elapsed >= capture.delay => {
            if let Err(error) = prepare_capture_target(&path) {
                capture.finished = true;
                fail_capture(
                    &mut exit,
                    &mut report,
                    "Unattended capture could not prepare its output path",
                    vec![format!("target: {}", path.display()), error],
                );
                return;
            }
            info!("unattended capture: writing {}", path.display());
            capture.requested_at = Some(now);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path))
                .observe(note_capture);
        }
        _ if elapsed >= capture.delay + CAPTURE_TIMEOUT => {
            capture.finished = true;
            fail_capture(
                &mut exit,
                &mut report,
                "Unattended run never reached Running",
                vec![
                    format!("state: {:?}", state.get()),
                    format!(
                        "waited: {:.1}s (limit {:.0}s, real time)",
                        elapsed.as_secs_f32(),
                        (capture.delay + CAPTURE_TIMEOUT).as_secs_f32()
                    ),
                ],
            );
        }
        _ => {}
    }
}

fn log_failure(report: Res<FailureReport>) {
    if report.is_recorded() {
        error!("{}", report.to_display_string());
    } else {
        error!("entered Failed with no recorded failure detail");
    }
}
