use std::{
    f32::consts::{PI, TAU},
    time::Duration,
};

use bevy::prelude::Vec3;

/// The walking speed used by the steering-controls prototype.
pub const FORWARD_SPEED: f32 = 1.5;

/// The maximum heading change rate in radians per second.
pub const STEERING_RATE: f32 = PI / 2.0;

/// A full turnaround takes exactly three quarters of a second.
pub const TURNAROUND_DURATION: Duration = Duration::from_millis(750);

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
        heading.is_finite() && speed.is_finite() && seconds.is_finite() && seconds >= 0.0,
        "forward_delta requires finite inputs and non-negative seconds"
    );

    let distance = speed * seconds;
    let (sin, cos) = heading.sin_cos();
    Vec3::new(sin * distance, 0.0, -cos * distance)
}

/// The state reported by a turnaround step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurnStep {
    pub heading: f32,
    pub complete: bool,
}

/// A smooth 180-degree turn from a normalized starting heading.
#[derive(Debug, Clone, PartialEq)]
pub struct Turnaround {
    pub start: f32,
    pub elapsed: Duration,
}

impl Turnaround {
    /// Creates a new turnaround from the provided starting heading.
    pub fn new(start: f32) -> Self {
        Self {
            start: normalize_angle(start),
            elapsed: Duration::ZERO,
        }
    }

    fn target_heading(&self) -> f32 {
        normalize_angle(self.start + PI)
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

        let progress = self.elapsed.as_secs_f32() / TURNAROUND_DURATION.as_secs_f32();
        let eased = progress * progress * (3.0 - 2.0 * progress);
        let heading_delta = normalize_angle(self.target_heading() - self.start);

        TurnStep {
            heading: normalize_angle(self.start + heading_delta * eased),
            complete: false,
        }
    }
}
