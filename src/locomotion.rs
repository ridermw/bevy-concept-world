use std::{
    f32::consts::{PI, TAU},
    time::Duration,
};

use bevy::prelude::*;

use crate::{
    character::{Humanoid, character_transform},
    config::CharacterConfig,
    state::PrototypeState,
};

/// The walking speed used by the steering-controls prototype.
pub const FORWARD_SPEED: f32 = 1.5;

/// The maximum heading change rate in radians per second.
pub const STEERING_RATE: f32 = PI / 2.0;

/// A full turnaround takes exactly three quarters of a second.
pub const TURNAROUND_DURATION: Duration = Duration::from_millis(750);

/// Turnaround translation is integrated on this fixed timeline so it stays
/// consistent across different frame chunking.
const TURNAROUND_TRANSLATION_STEP: Duration = Duration::from_nanos(8_333_333);

/// The movement request derived from the current input state.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MovementInput {
    pub forward: bool,
    pub steering: f32,
    pub turnaround_pressed: bool,
    pub turnaround_held: bool,
}

/// The movement applied to the humanoid this frame.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MovementUpdate {
    pub heading: f32,
    pub translation: Vec3,
    pub turning_around: bool,
}

/// Normalizes an angle into the `[-PI, PI]` range.
///
/// The contract is finite input only; non-finite values are a programmer
/// error and are rejected loudly.
pub fn normalize_angle(angle: f32) -> f32 {
    assert!(
        angle.is_finite(),
        "normalize_angle requires finite input, got {angle}"
    );
    (angle + PI).rem_euclid(TAU) - PI
}

/// Advances a heading by steering input over a time span.
pub fn advance_heading(current: f32, steering: f32, seconds: f32) -> f32 {
    assert!(
        current.is_finite() && steering.is_finite() && seconds.is_finite() && seconds >= 0.0,
        "advance_heading requires finite inputs and non-negative seconds"
    );

    let steering = steering.clamp(-1.0, 1.0);
    normalize_angle(current + steering * STEERING_RATE * seconds)
}

/// Computes the displacement for a forward-facing move.
///
/// Heading zero points along Bevy's negative Z axis.
pub fn forward_delta(heading: f32, speed: f32, seconds: f32) -> Vec3 {
    assert!(
        heading.is_finite()
            && speed.is_finite()
            && speed >= 0.0
            && seconds.is_finite()
            && seconds >= 0.0,
        "forward_delta requires finite inputs and non-negative speed and seconds"
    );

    let distance = speed * seconds;
    Quat::from_rotation_y(heading) * -Vec3::Z * distance
}

/// Integrates a constant-speed, constant-steering move over one frame.
///
/// Heading zero points along Bevy's negative Z axis, and the displacement
/// follows the same convention as `Quat::from_rotation_y(heading) * -Vec3::Z`.
pub fn steered_delta(start_heading: f32, steering: f32, speed: f32, seconds: f32) -> Vec3 {
    assert!(
        start_heading.is_finite()
            && steering.is_finite()
            && speed.is_finite()
            && speed >= 0.0
            && seconds.is_finite()
            && seconds >= 0.0,
        "steered_delta requires finite inputs and non-negative speed and seconds"
    );

    let angular_velocity = steering.clamp(-1.0, 1.0) * STEERING_RATE;
    let sweep = angular_velocity * seconds;
    let half_sweep = sweep * 0.5;
    let sinc = if half_sweep == 0.0 {
        1.0
    } else {
        half_sweep.sin() / half_sweep
    };

    forward_delta(start_heading + half_sweep, speed, seconds) * sinc
}

/// The state reported by a turnaround step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurnStep {
    pub heading: f32,
    pub complete: bool,
}

/// A smooth 180-degree turn from a normalized starting heading.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Turnaround {
    pub start: f32,
    signed_delta: f32,
    pub elapsed: Duration,
}

fn eased_progress(progress: f32) -> f32 {
    progress * progress * (3.0 - 2.0 * progress)
}

fn duration_from_nanos(nanos: u128) -> Duration {
    Duration::from_nanos(
        nanos
            .try_into()
            .expect("turnaround integration durations fit within u64 nanoseconds"),
    )
}

impl Turnaround {
    /// Creates a new turnaround from the provided starting heading.
    pub fn new(start: f32) -> Self {
        Self {
            start: normalize_angle(start),
            signed_delta: PI,
            elapsed: Duration::ZERO,
        }
    }

    fn target_heading(&self) -> f32 {
        normalize_angle(self.start + self.signed_delta)
    }

    fn heading_at_elapsed(&self, elapsed: Duration) -> f32 {
        let elapsed = elapsed.min(TURNAROUND_DURATION);
        if elapsed >= TURNAROUND_DURATION {
            return self.target_heading();
        }

        let progress = elapsed.as_secs_f32() / TURNAROUND_DURATION.as_secs_f32();
        normalize_angle(self.start + self.signed_delta * eased_progress(progress))
    }

    fn translation_over(&self, delta: Duration, speed: f32) -> Vec3 {
        let start = self.elapsed.min(TURNAROUND_DURATION);
        let end = self.elapsed.saturating_add(delta).min(TURNAROUND_DURATION);
        if end <= start {
            return Vec3::ZERO;
        }

        let step = TURNAROUND_TRANSLATION_STEP.as_nanos();
        let end_nanos = end.as_nanos();
        let mut cursor = start.as_nanos();
        let mut translation = Vec3::ZERO;

        while cursor < end_nanos {
            let next_grid = if cursor.is_multiple_of(step) {
                cursor + step
            } else {
                ((cursor / step) + 1) * step
            };
            let segment_end = end_nanos.min(next_grid);
            let midpoint = cursor + (segment_end - cursor) / 2;
            let span = duration_from_nanos(segment_end - cursor);
            let heading = self.heading_at_elapsed(duration_from_nanos(midpoint));

            translation += forward_delta(heading, speed, span.as_secs_f32());
            cursor = segment_end;
        }

        translation
    }

    /// Advances the turnaround and reports the current heading.
    pub fn advance(&mut self, delta: Duration) -> TurnStep {
        self.elapsed = self.elapsed.saturating_add(delta);

        if self.elapsed >= TURNAROUND_DURATION {
            self.elapsed = TURNAROUND_DURATION;
            return TurnStep {
                heading: self.target_heading(),
                complete: true,
            };
        }

        TurnStep {
            heading: self.heading_at_elapsed(self.elapsed),
            complete: false,
        }
    }
}

/// State carried by the humanoid root between movement updates.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct HumanoidController {
    heading: f32,
    turnaround: Option<Turnaround>,
    walking_after_turnaround: bool,
}

impl HumanoidController {
    /// Returns the current locomotion heading in radians.
    pub fn heading(&self) -> f32 {
        self.heading
    }

    /// Returns the active turnaround, if the humanoid is still turning.
    pub fn turnaround(&self) -> Option<Turnaround> {
        self.turnaround
    }

    /// True while a smooth turnaround is still in progress.
    pub fn turning_around(&self) -> bool {
        self.turnaround.is_some()
    }

    /// Advances the controller by one frame of input and time.
    pub fn update(&mut self, input: MovementInput, delta: Duration) -> MovementUpdate {
        if !input.turnaround_held {
            self.walking_after_turnaround = false;
        }

        if input.turnaround_pressed && self.turnaround.is_none() {
            self.turnaround = Some(Turnaround::new(self.heading));
        }

        if let Some(turnaround) = &mut self.turnaround {
            let turn_delta = TURNAROUND_DURATION
                .saturating_sub(turnaround.elapsed)
                .min(delta);
            let leftover_delta = delta.saturating_sub(turn_delta);
            let mut translation = if input.turnaround_held {
                turnaround.translation_over(turn_delta, FORWARD_SPEED)
            } else {
                Vec3::ZERO
            };
            let step = turnaround.advance(delta);
            self.heading = step.heading;
            if step.complete {
                self.turnaround = None;
                self.walking_after_turnaround = input.turnaround_held;
                if input.turnaround_held && !leftover_delta.is_zero() {
                    translation +=
                        forward_delta(self.heading, FORWARD_SPEED, leftover_delta.as_secs_f32());
                }
            }

            return MovementUpdate {
                heading: self.heading,
                translation,
                turning_around: self.turnaround.is_some(),
            };
        }

        let seconds = delta.as_secs_f32();
        let start_heading = self.heading;
        self.heading = advance_heading(start_heading, input.steering, seconds);
        MovementUpdate {
            heading: self.heading,
            translation: if input.forward || self.walking_after_turnaround {
                steered_delta(start_heading, input.steering, FORWARD_SPEED, seconds)
            } else {
                Vec3::ZERO
            },
            turning_around: false,
        }
    }
}

/// Converts the arrow-key state into a locomotion request.
pub fn movement_input_from_keys(keys: &ButtonInput<KeyCode>) -> MovementInput {
    let left = keys.pressed(KeyCode::ArrowLeft);
    let right = keys.pressed(KeyCode::ArrowRight);

    MovementInput {
        forward: keys.pressed(KeyCode::ArrowUp) || left || right,
        steering: match (left, right) {
            (true, false) => 1.0,
            (false, true) => -1.0,
            (true, true) | (false, false) => 0.0,
        },
        turnaround_pressed: keys.just_pressed(KeyCode::ArrowDown),
        turnaround_held: keys.pressed(KeyCode::ArrowDown),
    }
}

/// Applies locomotion to the validated humanoid root while the prototype runs.
pub struct LocomotionPlugin;

impl Plugin for LocomotionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            update_humanoid
                .run_if(in_state(PrototypeState::Running))
                .run_if(resource_exists::<CharacterConfig>),
        );
    }
}

fn update_humanoid(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    config: Res<CharacterConfig>,
    mut humanoid: Query<(&mut HumanoidController, &mut Transform), With<Humanoid>>,
) {
    let Ok((mut controller, mut transform)) = humanoid.single_mut() else {
        return;
    };

    let update = controller.update(movement_input_from_keys(&keys), time.delta());
    transform.rotation = Quat::from_rotation_y(update.heading)
        * character_transform(config.scale, config.yaw_degrees).rotation;
    transform.translation += update.translation;
}
