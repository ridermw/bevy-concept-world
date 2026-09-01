//! On-screen diagnostics and the prototype's three keyboard controls.
//!
//! The overlay must keep working when the character manifest never loaded, so
//! everything it reads is optional or always present.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    state::state::StateTransitionEvent,
};

use crate::{
    config::CharacterConfig,
    state::{FailureReport, PrototypeState},
};

/// Where `P` writes the visual acceptance screenshot, relative to the crate
/// root so the working directory does not matter.
const SCREENSHOT_PATH: &str = "docs/validation/humanoid-walk.png";

/// Set to a whole number of seconds to run unattended: the application waits
/// that long in `Running`, writes [`SCREENSHOT_PATH`], and exits. This exists
/// because a validation host without an interactive desktop session cannot
/// press `P`.
const CAPTURE_ENV: &str = "HUMANOID_WALK_CAPTURE_SECONDS";

/// How long the unattended run waits for the screenshot to reach disk. A
/// software renderer can take several seconds per frame, and the capture needs
/// a few frames to be rendered, read back, and written.
const CAPTURE_GRACE: Duration = Duration::from_secs(240);

/// How long the unattended run waits to reach `Running` before giving up.
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(180);

pub struct DiagnosticsPlugin;

impl Plugin for DiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_overlay)
            .add_systems(Update, (log_transitions, update_overlay, handle_controls))
            .add_systems(OnEnter(PrototypeState::Failed), log_failure);

        if let Some(capture) = UnattendedCapture::from_env() {
            info!(
                "unattended capture enabled: {:?} after reaching Running",
                capture.delay
            );
            app.insert_resource(capture)
                .add_systems(Update, unattended_capture);
        }
    }
}

#[derive(Component)]
struct StatusText;

fn screenshot_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(SCREENSHOT_PATH)
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

fn update_overlay(
    state: Res<State<PrototypeState>>,
    config: Option<Res<CharacterConfig>>,
    report: Res<FailureReport>,
    clips: Res<Assets<AnimationClip>>,
    graphs: Res<Assets<AnimationGraph>>,
    players: Query<(&AnimationPlayer, &AnimationGraphHandle)>,
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

    lines.push(format!("Animation players: {}", players.iter().count()));
    for (player, graph_handle) in &players {
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

    if report.is_recorded() {
        lines.push(String::new());
        lines.push(format!("FAILED: {}", report.to_display_string()));
    }

    lines.push(String::new());
    lines.push("Space: pause/resume   P: screenshot   Esc: exit".to_string());

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
    mut players: Query<&mut AnimationPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }

    if keys.just_pressed(KeyCode::Space) {
        for mut player in &mut players {
            if player.all_paused() {
                player.resume_all();
            } else {
                player.pause_all();
            }
        }
    }

    if keys.just_pressed(KeyCode::KeyP) {
        let path = screenshot_path();
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
/// and then `Escape`, but without a desktop session.
#[derive(Resource, Debug)]
struct UnattendedCapture {
    delay: Duration,
    requested_at: Option<Duration>,
}

impl UnattendedCapture {
    fn from_env() -> Option<Self> {
        let seconds: u64 = std::env::var(CAPTURE_ENV).ok()?.trim().parse().ok()?;
        Some(Self {
            delay: Duration::from_secs(seconds),
            requested_at: None,
        })
    }
}

fn unattended_capture(
    mut commands: Commands,
    time: Res<Time>,
    state: Res<State<PrototypeState>>,
    mut capture: ResMut<UnattendedCapture>,
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed();

    if let Some(requested_at) = capture.requested_at {
        if now.saturating_sub(requested_at) >= CAPTURE_GRACE {
            info!("unattended capture finished; exiting");
            exit.write(AppExit::Success);
        }
        return;
    }

    match state.get() {
        PrototypeState::Failed => {
            error!("unattended run reached Failed; no screenshot captured");
            exit.write(AppExit::error());
        }
        PrototypeState::Running if now >= capture.delay => {
            let path = screenshot_path();
            info!("unattended capture: writing {}", path.display());
            capture.requested_at = Some(now);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
        }
        _ if now >= capture.delay + CAPTURE_TIMEOUT => {
            error!(
                "unattended run never reached Running (state: {:?}); no screenshot captured",
                state.get()
            );
            exit.write(AppExit::error());
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
