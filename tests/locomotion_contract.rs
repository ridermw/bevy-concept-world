use std::{
    f32::consts::{FRAC_PI_2, PI},
    time::Duration,
};

use bevy::{
    input::mouse::{AccumulatedMouseScroll, MouseScrollUnit},
    prelude::*,
    state::app::StatesPlugin,
    time::TimeUpdateStrategy,
};
use bevy_concept_world::{
    character::{Humanoid, character_transform},
    config::CharacterConfig,
    inspection::{CAMERA_POSITION, LOOK_AT},
    locomotion::{
        CAMERA_MAX_DISTANCE, CAMERA_MIN_DISTANCE, FORWARD_SPEED, HumanoidController,
        LocomotionPlugin, MovementInput, OrbitCamera, STEERING_RATE, Turnaround, advance_heading,
        forward_delta, movement_input_from_keys, normalize_angle, orbit_camera_transform,
        orbit_yaw, smooth_distance, steered_delta, zoom_target,
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

fn assert_close_within(actual: f32, expected: f32, tolerance: f32) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected} +/- {tolerance}, got {actual}"
    );
}

fn assert_normalize_angle_preserves_bits(angle: f32) {
    let normalized = normalize_angle(angle);
    assert_eq!(
        normalized.to_bits(),
        angle.to_bits(),
        "expected {angle} to remain bit-for-bit unchanged, got {normalized}"
    );
}

fn assert_chunk_invariant_for_steering(
    start_heading: f32,
    steering: f32,
    chunk_count: u32,
    chunk_millis: u64,
) {
    let chunk = Duration::from_millis(chunk_millis);
    let total = Duration::from_millis(chunk_millis * u64::from(chunk_count));
    let chunk_seconds = chunk.as_secs_f32();
    let total_seconds = total.as_secs_f32();

    let single_heading = advance_heading(start_heading, steering, total_seconds);
    let single_translation = steered_delta(start_heading, steering, FORWARD_SPEED, total_seconds);

    let repeated =
        (0..chunk_count).fold((start_heading, Vec3::ZERO), |(heading, translation), _| {
            let update_heading = advance_heading(heading, steering, chunk_seconds);
            let update_translation = steered_delta(heading, steering, FORWARD_SPEED, chunk_seconds);
            (update_heading, translation + update_translation)
        });

    let heading_error = normalize_angle(single_heading - repeated.0).abs();
    let repeated_heading_change = normalize_angle(repeated.0 - start_heading);
    let translation_error = single_translation.distance(repeated.1);

    assert!(
        repeated_heading_change != 0.0,
        "expected repeated heading to change from start, steering={steering}, start={start_heading}, got {repeated_heading_change}"
    );
    if steering.is_sign_positive() {
        assert!(
            repeated_heading_change > 0.0,
            "expected repeated heading to move positively for steering={steering}, start={start_heading}, got {repeated_heading_change}"
        );
    } else {
        assert!(
            repeated_heading_change < 0.0,
            "expected repeated heading to move negatively for steering={steering}, start={start_heading}, got {repeated_heading_change}"
        );
    }
    assert!(
        heading_error <= 2.0e-6,
        "expected chunk-invariant heading within 2.0e-6, single={single_heading}, repeated={}, error={heading_error}",
        repeated.0
    );
    assert!(
        translation_error <= 4.0e-6,
        "expected chunk-invariant translation within 4.0e-6, single={:?}, repeated={:?}, error={translation_error}",
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
        .insert_resource(AccumulatedMouseScroll::default())
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
fn normalize_angle_preserves_in_range_bits() {
    let just_below_pi = f32::from_bits(PI.to_bits() - 1);
    for angle in [
        -0.0,
        0.0,
        f32::from_bits(1),
        -f32::from_bits(1),
        0.25,
        -0.25,
        1.0e-30,
        -1.0e-30,
        just_below_pi,
    ] {
        assert_normalize_angle_preserves_bits(angle);
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
fn orbit_yaw_is_symmetric_and_frame_rate_independent() {
    let start = 0.35;

    let q_once = orbit_yaw(start, 1.0, 1.0);
    let e_once = orbit_yaw(start, -1.0, 1.0);
    let q_steps = (0..10).fold(start, |yaw, _| orbit_yaw(yaw, 1.0, 0.1));
    let e_steps = (0..10).fold(start, |yaw, _| orbit_yaw(yaw, -1.0, 0.1));

    let q_delta = normalize_angle(q_once - start);
    let e_delta = normalize_angle(e_once - start);

    assert_close(q_delta, FRAC_PI_2);
    assert_close(e_delta, -FRAC_PI_2);
    assert_close(q_delta, -e_delta);
    assert_close(q_once, q_steps);
    assert_close(e_once, e_steps);
}

#[test]
fn zoom_target_uses_scroll_direction_and_clamps_to_camera_bounds() {
    assert_close(zoom_target(4.0, 1.0), 3.25);
    assert_close(zoom_target(4.0, -1.0), 4.75);
    assert_close(zoom_target(2.0, 10.0), CAMERA_MIN_DISTANCE);
    assert_close(zoom_target(11.5, -10.0), CAMERA_MAX_DISTANCE);
}

#[test]
fn smooth_distance_converges_without_overshoot_and_is_chunk_stable() {
    let start = CAMERA_MAX_DISTANCE;
    let target = CAMERA_MIN_DISTANCE;
    let single = smooth_distance(start, target, 1.0);

    assert!(
        single < start,
        "expected {single} to move inward from {start}"
    );
    assert!(
        single > target,
        "expected {single} to stay above the target {target}"
    );

    let mut stepped = start;
    for _ in 0..60 {
        let next = smooth_distance(stepped, target, 1.0 / 60.0);
        assert!(next < stepped, "expected {next} to keep moving inward");
        assert!(
            next > target,
            "expected {next} to avoid overshooting the target {target}"
        );
        stepped = next;
    }

    assert_close_within(single, stepped, 2.0e-5);

    let zooming_out = smooth_distance(CAMERA_MIN_DISTANCE, CAMERA_MAX_DISTANCE, 0.25);
    assert!(zooming_out > CAMERA_MIN_DISTANCE);
    assert!(zooming_out < CAMERA_MAX_DISTANCE);
}

#[test]
fn orbit_camera_transform_translates_with_the_target_and_looks_at_target_height() {
    let orbit = OrbitCamera::from_position_and_focus(CAMERA_POSITION, LOOK_AT);
    let origin = orbit_camera_transform(Vec3::ZERO, orbit);
    let translated_target = Vec3::new(1.25, 0.0, -0.75);
    let translated = orbit_camera_transform(translated_target, orbit);

    assert_vec3_close(origin.translation, CAMERA_POSITION);
    assert_rotation_close(
        origin.rotation,
        Transform::from_translation(CAMERA_POSITION)
            .looking_at(LOOK_AT, Vec3::Y)
            .rotation,
    );
    assert_vec3_close(
        translated.translation - origin.translation,
        translated_target,
    );

    let facing_left =
        Transform::from_translation(translated_target).with_rotation(Quat::from_rotation_y(1.25));
    let facing_right =
        Transform::from_translation(translated_target).with_rotation(Quat::from_rotation_y(-0.75));
    let left_camera = orbit_camera_transform(facing_left.translation, orbit);
    let right_camera = orbit_camera_transform(facing_right.translation, orbit);
    assert_vec3_close(left_camera.translation, right_camera.translation);
    assert_rotation_close(left_camera.rotation, right_camera.rotation);

    let focus = translated_target + Vec3::Y * orbit.target_height;
    assert_vec3_close(
        translated.rotation * -Vec3::Z,
        (focus - translated.translation).normalize(),
    );
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
    assert_chunk_invariant_for_steering(start_heading, 1.0e-6, 60, 12);
}

#[test]
fn tiny_negative_steering_remains_chunk_invariant() {
    let start_heading = 0.25;
    assert_chunk_invariant_for_steering(start_heading, -1.0e-6, 60, 12);
}

#[test]
fn slightly_larger_positive_steering_remains_chunk_invariant() {
    let start_heading = 0.25;
    assert_chunk_invariant_for_steering(start_heading, 1.0e-5, 60, 12);
}

#[test]
fn slightly_larger_negative_steering_remains_chunk_invariant() {
    let start_heading = 0.25;
    assert_chunk_invariant_for_steering(start_heading, -1.0e-5, 60, 12);
}

#[test]
fn tiny_steering_remains_chunk_invariant_at_60_chunks() {
    assert_chunk_invariant_for_steering(0.0, 1.3e-6, 60, 12);
}

#[test]
fn tiny_steering_remains_chunk_invariant_at_144_chunks() {
    assert_chunk_invariant_for_steering(0.0, 1.3e-6, 144, 5);
}

#[test]
fn tiny_steering_remains_chunk_invariant_at_240_chunks() {
    assert_chunk_invariant_for_steering(0.0, 1.3e-6, 240, 3);
}

#[test]
fn full_steering_remains_chunk_invariant_at_240_chunks() {
    assert_chunk_invariant_for_steering(0.25, 1.0, 240, 3);
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

#[test]
fn orbit_camera_updates_only_while_running_and_follows_post_movement_target() {
    let mut app = locomotion_app(PrototypeState::Loading);
    let base = character_transform(0.5, 180.0);
    let humanoid = app
        .world_mut()
        .spawn((
            Humanoid,
            HumanoidController::default(),
            base,
            GlobalTransform::default(),
        ))
        .id();
    let orbit = OrbitCamera::from_position_and_focus(CAMERA_POSITION, LOOK_AT);
    let initial_camera = orbit_camera_transform(base.translation, orbit);
    let camera = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            orbit,
            initial_camera,
            GlobalTransform::default(),
        ))
        .id();

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ArrowUp);

    app.update();

    let loading_humanoid = app
        .world()
        .entity(humanoid)
        .get::<Transform>()
        .cloned()
        .expect("humanoid root must still have a transform");
    let loading_camera = app
        .world()
        .entity(camera)
        .get::<Transform>()
        .cloned()
        .expect("orbit camera must still have a transform");
    assert_eq!(loading_humanoid, base);
    assert_eq!(loading_camera, initial_camera);

    enter(&mut app, PrototypeState::Running);
    app.update();

    let running_humanoid = app
        .world()
        .entity(humanoid)
        .get::<Transform>()
        .cloned()
        .expect("humanoid root must still have a transform");
    let running_camera = app
        .world()
        .entity(camera)
        .get::<Transform>()
        .cloned()
        .expect("orbit camera must still have a transform");
    let orbit_state = *app
        .world()
        .entity(camera)
        .get::<OrbitCamera>()
        .expect("orbit camera state must stay attached");

    let humanoid_delta = running_humanoid.translation - base.translation;
    let expected_camera = orbit_camera_transform(running_humanoid.translation, orbit_state);

    assert_ne!(running_humanoid.translation, base.translation);
    assert_vec3_close(
        running_camera.translation - initial_camera.translation,
        humanoid_delta,
    );
    assert_vec3_close(running_camera.translation, expected_camera.translation);
    assert_rotation_close(running_camera.rotation, expected_camera.rotation);
}

#[test]
fn orbit_camera_converts_pixel_scroll_to_line_zoom() {
    let mut app = locomotion_app(PrototypeState::Running);
    let orbit = OrbitCamera::from_position_and_focus(CAMERA_POSITION, LOOK_AT);
    let _humanoid = app.world_mut().spawn((
        Humanoid,
        HumanoidController::default(),
        character_transform(0.5, 180.0),
        GlobalTransform::default(),
    ));
    let camera = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            orbit,
            orbit_camera_transform(Vec3::ZERO, orbit),
            GlobalTransform::default(),
        ))
        .id();

    *app.world_mut().resource_mut::<AccumulatedMouseScroll>() = AccumulatedMouseScroll {
        unit: MouseScrollUnit::Pixel,
        delta: Vec2::new(0.0, MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR),
    };

    app.update();

    let orbit_state = *app
        .world()
        .entity(camera)
        .get::<OrbitCamera>()
        .expect("orbit camera state must stay attached");
    assert_close(orbit_state.target_distance, orbit.target_distance - 0.75);
}
