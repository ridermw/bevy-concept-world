//! The deterministic inspection scene.
//!
//! This scene exists to expose asset defects, not to resemble a finished
//! world. Every transform, light setting, and marker size below is fixed so
//! that two runs produce the same framing and screenshots stay comparable.

use bevy::{light::CascadeShadowConfigBuilder, prelude::*};

use crate::locomotion::{OrbitCamera, orbit_camera_transform};

/// Where the camera looks: roughly the chest height of a 1.8 m humanoid.
pub const LOOK_AT: Vec3 = Vec3::new(0.0, 0.95, 0.0);
/// Fixed inspection-camera position before orbit controls take over.
pub const CAMERA_POSITION: Vec3 = Vec3::new(2.6, 1.8, 3.8);
/// Half-extent of the square ground plane, in meters.
const GROUND_SIZE: f32 = 100.0;
/// Thickness of both reference markers, in meters.
const MARKER_THICKNESS: f32 = 0.05;

pub struct InspectionPlugin;

impl Plugin for InspectionPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 400.0,
            ..default()
        })
        .add_systems(Startup, setup);
    }
}

/// The one-meter vertical scale reference.
#[derive(Component)]
pub struct MeterMarker;

/// The marker pointing along Bevy's forward axis, `-Z`.
#[derive(Component)]
pub struct ForwardMarker;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let orbit_camera = OrbitCamera::from_position_and_focus(CAMERA_POSITION, LOOK_AT);
    commands.spawn((
        Name::new("Inspection camera"),
        Camera3d::default(),
        orbit_camera,
        orbit_camera_transform(Vec3::ZERO, orbit_camera),
    ));

    commands.spawn((
        Name::new("Key light"),
        DirectionalLight {
            illuminance: 10_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, -0.9, -0.8, 0.0)),
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 4.0,
            maximum_distance: 24.0,
            ..default()
        }
        .build(),
    ));

    commands.spawn((
        Name::new("Ground"),
        Mesh3d(meshes.add(Plane3d::default().mesh().size(GROUND_SIZE, GROUND_SIZE))),
        MeshMaterial3d(materials.add(Color::srgb(0.20, 0.22, 0.25))),
    ));

    // One meter tall, standing on the ground, beside the character.
    commands.spawn((
        Name::new("One-meter marker"),
        MeterMarker,
        Mesh3d(meshes.add(Cuboid::new(MARKER_THICKNESS, 1.0, MARKER_THICKNESS))),
        MeshMaterial3d(materials.add(Color::srgb(0.95, 0.80, 0.15))),
        Transform::from_xyz(-0.9, 0.5, 0.0),
    ));

    // One meter long, laid on the ground from the origin toward -Z, which is
    // the direction the humanoid is expected to face.
    let forward_material = materials.add(Color::srgb(0.20, 0.75, 0.95));
    commands.spawn((
        Name::new("Forward marker"),
        ForwardMarker,
        Mesh3d(meshes.add(Cuboid::new(MARKER_THICKNESS, 0.02, 1.0))),
        MeshMaterial3d(forward_material.clone()),
        Transform::from_xyz(0.0, 0.01, -0.5),
    ));
    // Arrow head at the far, -Z end.
    commands.spawn((
        Name::new("Forward marker tip"),
        ForwardMarker,
        Mesh3d(meshes.add(Cuboid::new(0.25, 0.02, MARKER_THICKNESS))),
        MeshMaterial3d(forward_material),
        Transform::from_xyz(0.0, 0.01, -0.9),
    ));
}
