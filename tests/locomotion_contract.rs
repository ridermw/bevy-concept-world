use std::{
    f32::consts::{FRAC_PI_2, PI},
    time::Duration,
};

use bevy::prelude::Vec3;
use bevy_concept_world::locomotion::{Turnaround, advance_heading, forward_delta, normalize_angle};

fn assert_vec3_close(actual: Vec3, expected: Vec3) {
    assert!(
        actual.distance(expected) <= 1e-6,
        "expected {expected:?}, got {actual:?}"
    );
}

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 1e-6,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn straight_motion_at_heading_zero_moves_along_negative_z() {
    assert_vec3_close(forward_delta(0.0, 1.5, 2.0), Vec3::new(0.0, 0.0, -3.0));
}

#[test]
fn left_and_right_heading_changes_are_symmetric() {
    let start: f32 = 0.2;
    let seconds: f32 = 0.25;
    let left = advance_heading(start, -1.0, seconds);
    let right = advance_heading(start, 1.0, seconds);

    assert_close((start - left).abs(), (right - start).abs());
    assert_close(left + right, 2.0 * start);
}

#[test]
fn normalize_angle_stays_within_the_documented_bounds() {
    for angle in [
        -10.0 * PI,
        -PI,
        -1.0,
        0.0,
        1.0,
        PI,
        10.0 * PI,
        123_456.79,
        -123_456.79,
    ] {
        let normalized = normalize_angle(angle);
        assert!(normalized.is_finite(), "{normalized}");
        assert!(
            (-PI..=PI).contains(&normalized),
            "{angle} normalized to {normalized}"
        );
    }
}

#[test]
fn a_turnaround_is_incomplete_after_one_half_and_complete_after_the_other_half() {
    let mut turnaround = Turnaround::new(FRAC_PI_2);

    let first = turnaround.advance(Duration::from_millis(375));
    assert!(!first.complete);

    let second = turnaround.advance(Duration::from_millis(375));
    assert!(second.complete);
    assert_close(second.heading, -FRAC_PI_2);
}

#[test]
fn heading_advance_is_frame_rate_independent() {
    let start: f32 = 0.25;
    let steering: f32 = 0.75;
    let one_step = advance_heading(start, steering, 1.0);

    let ten_steps = (0..10).fold(start, |heading, _| advance_heading(heading, steering, 0.1));

    assert_close(one_step, ten_steps);
}
