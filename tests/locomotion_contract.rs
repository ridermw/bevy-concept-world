use std::{
    f32::consts::{FRAC_PI_2, PI},
    time::Duration,
};

use bevy::{prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use bevy_concept_world::{
    character::{Humanoid, character_transform},
    config::CharacterConfig,
    locomotion::{
        FORWARD_SPEED, HumanoidController, LocomotionPlugin, MovementInput, STEERING_RATE,
        Turnaround, advance_heading, forward_delta, movement_input_from_keys, normalize_angle,
        steered_delta,
    },
    state::PrototypeState,
};

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

fn assert_chunk_invariant_for_steering(start_heading: f32, steering: f32) {
    let single_heading = advance_heading(start_heading, steering, 1.0);
    let single_translation = steered_delta(start_heading, steering, FORWARD_SPEED, 1.0);

    let repeated = (0..60).fold((start_heading, Vec3::ZERO), |(heading, translation), _| {
        let seconds = 1.0 / 60.0;
        let update_heading = advance_heading(heading, steering, seconds);
        let update_translation = steered_delta(heading, steering, FORWARD_SPEED, seconds);
        (update_heading, translation + update_translation)
    });

    assert!(
        (single_heading - repeated.0).abs() <= 2.0e-6,
        "expected chunk-invariant heading, single={single_heading}, repeated={}",
        repeated.0
    );
    assert!(
        single_translation.distance(repeated.1) <= 2.0e-6,
        "expected chunk-invariant translation, single={:?}, repeated={:?}",
        single_translation,
        repeated.1
    );
}

fn assert_rotation_close(actual: Quat, expected: Quat) {
    assert_vec3_close(actual * Vec3::X, expected * Vec3::X);
    assert_vec3_close(actual * Vec3::Y, expected * Vec3::Y);
    assert_vec3_close(actual * Vec3::Z, expected * Vec3::Z);
}

fn test_character_config() -> CharacterConfig {
    CharacterConfig {
        id: "test-humanoid".into(),
        gltf_path: "characters/quaternius/humanoid.glb".into(),
        source_url: "https://example.invalid/humanoid".into(),
        pack_version: "test".into(),
        downloaded_on: "2026-09-01".into(),
        license: "CC0".into(),
        license_path: "characters/quaternius/LICENSE.txt".into(),
        scene_name: "Scene".into(),
        animation_name: "Walk_Loop".into(),
        expected_animation_players: 1,
        scale: 0.5,
        yaw_degrees: 180.0,
        root_motion: false,
    }
}

fn locomotion_app(initial_state: PrototypeState) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(StatesPlugin)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            250,
        )))
        .insert_resource(ButtonInput::<KeyCode>::default())
        .insert_resource(test_character_config())
        .add_plugins(LocomotionPlugin)
        .insert_state(initial_state);
    app
}

fn enter(app: &mut App, state: PrototypeState) {
    app.world_mut()
        .resource_mut::<NextState<PrototypeState>>()
        .set(state);
    app.update();
}

#[test]
fn straight_motion_at_heading_zero_moves_along_negative_z() {
    assert_vec3_close(forward_delta(0.0, 1.5, 2.0), Vec3::new(0.0, 0.0, -3.0));
}

#[test]
fn forward_motion_matches_bevy_yaw_at_positive_quarter_turn() {
    let heading = FRAC_PI_2;
    let distance = 3.0;

    assert_vec3_close(
        forward_delta(heading, 1.5, 2.0),
        Quat::from_rotation_y(heading) * -Vec3::Z * distance,
    );
}

#[test]
fn forward_motion_matches_bevy_yaw_at_negative_quarter_turn() {
    let heading = -FRAC_PI_2;
    let distance = 3.0;

    assert_vec3_close(
        forward_delta(heading, 1.5, 2.0),
        Quat::from_rotation_y(heading) * -Vec3::Z * distance,
    );
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
fn turnarounds_follow_a_consistent_positive_half_turn_near_wrap_boundaries() {
    for start in [-2.282_367_2, -2.28] {
        let mut turnaround = Turnaround::new(start);
        let halfway = turnaround.advance(Duration::from_millis(375));

        assert_close(normalize_angle(halfway.heading - start), FRAC_PI_2);
        assert!(!halfway.complete);
    }
}

#[test]
fn heading_advance_is_frame_rate_independent() {
    let start: f32 = 0.25;
    let steering: f32 = 0.75;
    let one_step = advance_heading(start, steering, 1.0);

    let ten_steps = (0..10).fold(start, |heading, _| advance_heading(heading, steering, 0.1));

    assert_close(one_step, ten_steps);
}

#[test]
fn down_starts_only_one_turnaround_until_it_finishes() {
    let mut controller = HumanoidController::default();
    controller.update(
        MovementInput {
            turnaround_pressed: true,
            turnaround_held: true,
            ..default()
        },
        Duration::ZERO,
    );
    let first = controller
        .turnaround()
        .expect("the first Down press must start a turnaround");

    controller.update(
        MovementInput {
            turnaround_pressed: true,
            turnaround_held: true,
            ..default()
        },
        Duration::ZERO,
    );
    assert_eq!(controller.turnaround(), Some(first));

    let completed = controller.update(
        MovementInput {
            turnaround_held: true,
            ..default()
        },
        Duration::from_millis(750),
    );
    assert!(!completed.turning_around);
    assert!(controller.turnaround().is_none());

    let after = controller.update(
        MovementInput {
            turnaround_held: true,
            ..default()
        },
        Duration::from_millis(125),
    );
    assert!(!after.turning_around);
    assert_vec3_close(
        after.translation,
        forward_delta(after.heading, FORWARD_SPEED, 0.125),
    );
    assert!(controller.turnaround().is_none());
}

#[test]
fn releasing_down_stops_translation_without_cancelling_the_turn() {
    let mut controller = HumanoidController::default();
    controller.update(
        MovementInput {
            turnaround_pressed: true,
            turnaround_held: true,
            ..default()
        },
        Duration::from_millis(100),
    );

    let update = controller.update(MovementInput::default(), Duration::from_millis(100));

    assert_eq!(update.translation, Vec3::ZERO);
    assert!(update.turning_around);
    assert!(controller.turnaround().is_some());
}

#[test]
fn held_down_applies_turn_remainder_then_walks_straight_without_requeueing() {
    let mut overrun = HumanoidController::default();
    overrun.update(
        MovementInput {
            turnaround_pressed: true,
            turnaround_held: true,
            ..default()
        },
        Duration::from_millis(700),
    );

    let mut split = overrun;
    let turning_tail = split.update(
        MovementInput {
            turnaround_held: true,
            ..default()
        },
        Duration::from_millis(50),
    );
    assert!(!turning_tail.turning_around);
    let straight_tail = split.update(
        MovementInput {
            forward: true,
            turnaround_held: true,
            ..default()
        },
        Duration::from_millis(50),
    );
    assert!(split.turnaround().is_none());

    let overrun_update = overrun.update(
        MovementInput {
            turnaround_held: true,
            ..default()
        },
        Duration::from_millis(100),
    );

    assert!(!overrun_update.turning_around);
    assert!(overrun.turnaround().is_none());
    assert_close(overrun_update.heading, turning_tail.heading);
    assert_vec3_close(
        overrun_update.translation,
        turning_tail.translation + straight_tail.translation,
    );

    let continued = overrun.update(
        MovementInput {
            turnaround_held: true,
            ..default()
        },
        Duration::from_millis(25),
    );
    assert!(!continued.turning_around);
    assert_vec3_close(
        continued.translation,
        forward_delta(continued.heading, FORWARD_SPEED, 0.025),
    );
    assert!(overrun.turnaround().is_none());
}

#[test]
fn left_and_right_mapping_are_symmetric_and_cancel_when_both_are_held() {
    let mut up = ButtonInput::<KeyCode>::default();
    up.press(KeyCode::ArrowUp);
    let up = movement_input_from_keys(&up);

    let mut left = ButtonInput::<KeyCode>::default();
    left.press(KeyCode::ArrowLeft);
    let left = movement_input_from_keys(&left);

    let mut right = ButtonInput::<KeyCode>::default();
    right.press(KeyCode::ArrowRight);
    let right = movement_input_from_keys(&right);

    let mut both = ButtonInput::<KeyCode>::default();
    both.press(KeyCode::ArrowLeft);
    both.press(KeyCode::ArrowRight);
    let both = movement_input_from_keys(&both);

    assert!(up.forward);
    assert_close(up.steering, 0.0);

    assert!(left.forward);
    assert!(right.forward);
    assert!(both.forward);
    assert_close(left.steering.abs(), 1.0);
    assert_close(right.steering.abs(), 1.0);
    assert_close(left.steering, -right.steering);
    assert_close(both.steering, 0.0);
}

#[test]
fn normal_left_steering_matches_the_same_arc_across_frame_chunking() {
    let mut single_step = HumanoidController::default();
    let single = single_step.update(
        MovementInput {
            forward: true,
            steering: 1.0,
            ..default()
        },
        Duration::from_secs(1),
    );

    let mut repeated = HumanoidController::default();
    let repeated_translation = (0..10).fold(Vec3::ZERO, |translation, _| {
        let update = repeated.update(
            MovementInput {
                forward: true,
                steering: 1.0,
                ..default()
            },
            Duration::from_millis(100),
        );
        translation + update.translation
    });

    let radius = FORWARD_SPEED / STEERING_RATE;
    assert_close(single.heading, FRAC_PI_2);
    assert_close(single.heading, repeated.heading());
    assert_vec3_close(single.translation, Vec3::new(-radius, 0.0, -radius));
    assert_vec3_close(single.translation, repeated_translation);
    assert!(!single.turning_around);
}

#[test]
fn normal_right_steering_matches_the_same_arc_across_frame_chunking() {
    let mut single_step = HumanoidController::default();
    let single = single_step.update(
        MovementInput {
            forward: true,
            steering: -1.0,
            ..default()
        },
        Duration::from_secs(1),
    );

    let mut repeated = HumanoidController::default();
    let repeated_translation = (0..10).fold(Vec3::ZERO, |translation, _| {
        let update = repeated.update(
            MovementInput {
                forward: true,
                steering: -1.0,
                ..default()
            },
            Duration::from_millis(100),
        );
        translation + update.translation
    });

    let radius = FORWARD_SPEED / STEERING_RATE;
    assert_close(single.heading, -FRAC_PI_2);
    assert_close(single.heading, repeated.heading());
    assert_vec3_close(single.translation, Vec3::new(radius, 0.0, -radius));
    assert_vec3_close(single.translation, repeated_translation);
    assert!(!single.turning_around);
}

#[test]
fn zero_steering_matches_straight_motion_across_frame_chunking() {
    let mut single_step = HumanoidController::default();
    let single = single_step.update(
        MovementInput {
            forward: true,
            ..default()
        },
        Duration::from_secs(1),
    );

    let mut repeated = HumanoidController::default();
    let repeated_translation = (0..10).fold(Vec3::ZERO, |translation, _| {
        let update = repeated.update(
            MovementInput {
                forward: true,
                ..default()
            },
            Duration::from_millis(100),
        );
        translation + update.translation
    });

    assert_close(single.heading, repeated.heading());
    assert_vec3_close(single.translation, repeated_translation);
    assert_vec3_close(single.translation, forward_delta(0.0, FORWARD_SPEED, 1.0));
    assert!(!single.turning_around);
}

#[test]
fn tiny_positive_steering_remains_chunk_invariant() {
    let start_heading = 0.25;
    assert_chunk_invariant_for_steering(start_heading, 1.0e-6);
}

#[test]
fn tiny_negative_steering_remains_chunk_invariant() {
    let start_heading = 0.25;
    assert_chunk_invariant_for_steering(start_heading, -1.0e-6);
}

#[test]
fn slightly_larger_positive_steering_remains_chunk_invariant() {
    let start_heading = 0.25;
    assert_chunk_invariant_for_steering(start_heading, 1.0e-5);
}

#[test]
fn slightly_larger_negative_steering_remains_chunk_invariant() {
    let start_heading = 0.25;
    assert_chunk_invariant_for_steering(start_heading, -1.0e-5);
}

#[test]
fn active_turnarounds_ignore_normal_steering() {
    let mut controller = HumanoidController::default();
    controller.update(
        MovementInput {
            turnaround_pressed: true,
            turnaround_held: true,
            ..default()
        },
        Duration::from_millis(100),
    );

    let mut steer_left = controller;
    let mut steer_right = controller;

    let left = steer_left.update(
        MovementInput {
            forward: true,
            steering: -1.0,
            turnaround_held: true,
            ..default()
        },
        Duration::from_millis(125),
    );
    let right = steer_right.update(
        MovementInput {
            forward: true,
            steering: 1.0,
            turnaround_held: true,
            ..default()
        },
        Duration::from_millis(125),
    );

    assert_close(left.heading, right.heading);
    assert_vec3_close(left.translation, right.translation);
    assert_eq!(steer_left.turnaround(), steer_right.turnaround());
}

#[test]
fn turnaround_translation_is_frame_rate_independent_while_down_is_held() {
    let mut single_step = HumanoidController::default();
    let single = single_step.update(
        MovementInput {
            turnaround_pressed: true,
            turnaround_held: true,
            ..default()
        },
        Duration::from_millis(750),
    );

    let mut repeated = HumanoidController::default();
    let stepped = (0..10).fold(Vec3::ZERO, |translation, step| {
        let update = repeated.update(
            MovementInput {
                turnaround_pressed: step == 0,
                turnaround_held: true,
                ..default()
            },
            Duration::from_millis(75),
        );
        translation + update.translation
    });

    assert_vec3_close(single.translation, stepped);
}

#[test]
fn transforms_change_only_while_running() {
    let mut app = locomotion_app(PrototypeState::Loading);
    let base = character_transform(0.5, 180.0);
    let untouched = Transform::from_xyz(3.0, 2.0, 1.0);
    let humanoid = app
        .world_mut()
        .spawn((
            Humanoid,
            HumanoidController::default(),
            base,
            GlobalTransform::default(),
        ))
        .id();
    let untouched_entity = app
        .world_mut()
        .spawn((untouched, GlobalTransform::default()))
        .id();

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ArrowLeft);

    app.update();

    let loading_transform = app
        .world()
        .entity(humanoid)
        .get::<Transform>()
        .cloned()
        .expect("humanoid root must still have a transform");
    assert_eq!(loading_transform, base);

    enter(&mut app, PrototypeState::Running);
    app.update();

    let running_transform = app
        .world()
        .entity(humanoid)
        .get::<Transform>()
        .cloned()
        .expect("humanoid root must still have a transform");
    let controller = app
        .world()
        .entity(humanoid)
        .get::<HumanoidController>()
        .expect("the locomotion system must keep the controller on the humanoid");

    assert_ne!(running_transform.translation, base.translation);
    assert_ne!(running_transform.rotation, base.rotation);
    assert_eq!(running_transform.scale, base.scale);
    assert_rotation_close(
        running_transform.rotation,
        Quat::from_rotation_y(controller.heading()) * base.rotation,
    );

    let untouched_after = app
        .world()
        .entity(untouched_entity)
        .get::<Transform>()
        .cloned()
        .expect("unrelated transforms must remain readable");
    assert_eq!(untouched_after, untouched);
}
