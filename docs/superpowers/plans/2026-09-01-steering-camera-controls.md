# Steering and Camera Controls Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add responsive arrow-key humanoid steering, a smooth Down-arrow 180-degree turnaround, Q/E camera orbit, and clamped mouse-wheel zoom.

**Architecture:** A new `locomotion` plugin owns movement state, pure steering/turnaround math, and the orbit camera update. The existing character loader marks the validated humanoid root, while the inspection scene supplies the camera's initial orbit state. Systems run only in `PrototypeState::Running`, with camera follow ordered after character movement.

**Tech Stack:** Rust 1.98, Bevy 0.19.1 ECS/input/transform APIs, Cargo integration tests

---

## File map

| File | Responsibility |
|---|---|
| `src/locomotion.rs` | Pure steering math, humanoid controller, keyboard movement, orbit camera, and zoom |
| `src/lib.rs` | Export the locomotion module |
| `src/main.rs` | Register `LocomotionPlugin` |
| `src/character.rs` | Mark the spawned root with `Humanoid` and its controller |
| `src/inspection.rs` | Attach orbit state to the inspection camera and enlarge the ground |
| `src/diagnostics.rs` | Show the new controls and turnaround state |
| `tests/locomotion_contract.rs` | Pure and App-level movement/camera contract tests |
| `tests/app_contract.rs` | Verify plugin registration and preserved controls |
| `README.md` | Document steering, orbit, zoom, and verification |

### Task 1: Pure locomotion and camera math

**Files:**
- Create: `src/locomotion.rs`
- Modify: `src/lib.rs`
- Create: `tests/locomotion_contract.rs`

- [ ] **Step 1: Write failing steering and turnaround tests**

```rust
use std::f32::consts::{FRAC_PI_2, PI};
use std::time::Duration;
use bevy::prelude::Vec3;
use bevy_concept_world::locomotion::{
    Turnaround, advance_heading, forward_delta, normalize_angle,
};

const EPSILON: f32 = 1.0e-5;

#[test]
fn straight_motion_follows_bevy_forward() {
    assert!(
        forward_delta(0.0, 1.5, 2.0)
            .abs_diff_eq(Vec3::new(0.0, 0.0, -3.0), EPSILON)
    );
}

#[test]
fn left_and_right_steering_are_symmetric() {
    let left = advance_heading(0.0, 1.0, 1.0);
    let right = advance_heading(0.0, -1.0, 1.0);
    assert!((left + right).abs() < EPSILON);
}

#[test]
fn heading_is_normalized_after_repeated_turns() {
    let heading = normalize_angle(9.0 * PI);
    assert!((-PI..=PI).contains(&heading));
}

#[test]
fn turnaround_reaches_the_exact_opposite_heading() {
    let mut turn = Turnaround::new(FRAC_PI_2);
    let first = turn.advance(Duration::from_millis(375));
    assert!(!first.complete);
    let last = turn.advance(Duration::from_millis(375));
    assert!(last.complete);
    assert!((normalize_angle(last.heading - (-FRAC_PI_2))).abs() < EPSILON);
}
```

- [ ] **Step 2: Run the tests and verify RED**

Run:

```powershell
cargo test --test locomotion_contract
```

Expected: compilation fails because `bevy_concept_world::locomotion` does not
exist.

- [ ] **Step 3: Implement the minimal pure movement model**

Create `src/locomotion.rs` with these public contracts:

```rust
use std::{
    f32::consts::{PI, TAU},
    time::Duration,
};
use bevy::prelude::*;

pub const FORWARD_SPEED: f32 = 1.5;
pub const STEERING_RATE: f32 = PI / 2.0;
pub const TURNAROUND_DURATION: Duration = Duration::from_millis(750);

pub fn normalize_angle(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

pub fn advance_heading(heading: f32, steering: f32, seconds: f32) -> f32 {
    normalize_angle(heading + steering.clamp(-1.0, 1.0) * STEERING_RATE * seconds.max(0.0))
}

pub fn forward_delta(heading: f32, speed: f32, seconds: f32) -> Vec3 {
    Quat::from_rotation_y(heading) * -Vec3::Z * speed.max(0.0) * seconds.max(0.0)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurnStep {
    pub heading: f32,
    pub complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Turnaround {
    start: f32,
    elapsed: Duration,
}

impl Turnaround {
    pub fn new(start: f32) -> Self {
        Self {
            start: normalize_angle(start),
            elapsed: Duration::ZERO,
        }
    }

    pub fn advance(&mut self, delta: Duration) -> TurnStep {
        self.elapsed = self.elapsed.saturating_add(delta).min(TURNAROUND_DURATION);
        let linear = self.elapsed.as_secs_f32() / TURNAROUND_DURATION.as_secs_f32();
        let eased = linear * linear * (3.0 - 2.0 * linear);
        TurnStep {
            heading: normalize_angle(self.start + PI * eased),
            complete: self.elapsed == TURNAROUND_DURATION,
        }
    }
}
```

Export it from `src/lib.rs`:

```rust
pub mod locomotion;
```

- [ ] **Step 4: Run the tests and verify GREEN**

Run:

```powershell
cargo test --test locomotion_contract
```

Expected: all Task 1 tests pass.

- [ ] **Step 5: Commit**

```powershell
git add src\lib.rs src\locomotion.rs tests\locomotion_contract.rs
git commit -m "feat: add locomotion math"
```

### Task 2: Wire keyboard steering to the humanoid root

**Files:**
- Modify: `src/locomotion.rs`
- Modify: `src/character.rs`
- Modify: `src/main.rs`
- Modify: `tests/locomotion_contract.rs`

- [ ] **Step 1: Add failing controller-state tests**

Append:

```rust
use bevy_concept_world::locomotion::{HumanoidController, MovementInput};

#[test]
fn down_starts_only_one_turnaround_until_it_finishes() {
    let mut controller = HumanoidController::default();
    controller.update(
        MovementInput { turnaround_pressed: true, turnaround_held: true, ..default() },
        Duration::ZERO,
    );
    let first = controller.turnaround();
    controller.update(
        MovementInput { turnaround_pressed: true, turnaround_held: true, ..default() },
        Duration::ZERO,
    );
    assert_eq!(controller.turnaround(), first);
}

#[test]
fn releasing_down_stops_translation_without_cancelling_the_turn() {
    let mut controller = HumanoidController::default();
    controller.update(
        MovementInput { turnaround_pressed: true, turnaround_held: true, ..default() },
        Duration::from_millis(100),
    );
    let update = controller.update(MovementInput::default(), Duration::from_millis(100));
    assert_eq!(update.translation, Vec3::ZERO);
    assert!(controller.turnaround().is_some());
}
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```powershell
cargo test --test locomotion_contract
```

Expected: compilation fails because `HumanoidController` and `MovementInput`
do not exist.

- [ ] **Step 3: Implement controller state and plugin wiring**

Add to `src/locomotion.rs`:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MovementInput {
    pub forward: bool,
    pub steering: f32,
    pub turnaround_pressed: bool,
    pub turnaround_held: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MovementUpdate {
    pub heading: f32,
    pub translation: Vec3,
    pub turning_around: bool,
}

#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct HumanoidController {
    heading: f32,
    turnaround: Option<Turnaround>,
}

impl HumanoidController {
    pub fn turnaround(&self) -> Option<Turnaround> {
        self.turnaround
    }

    pub fn update(&mut self, input: MovementInput, delta: Duration) -> MovementUpdate {
        if input.turnaround_pressed && self.turnaround.is_none() {
            self.turnaround = Some(Turnaround::new(self.heading));
        }

        if let Some(turnaround) = &mut self.turnaround {
            let step = turnaround.advance(delta);
            self.heading = step.heading;
            if step.complete {
                self.turnaround = None;
            }
            return MovementUpdate {
                heading: self.heading,
                translation: if input.turnaround_held {
                    forward_delta(self.heading, FORWARD_SPEED, delta.as_secs_f32())
                } else {
                    Vec3::ZERO
                },
                turning_around: self.turnaround.is_some(),
            };
        }

        self.heading = advance_heading(self.heading, input.steering, delta.as_secs_f32());
        MovementUpdate {
            heading: self.heading,
            translation: if input.forward {
                forward_delta(self.heading, FORWARD_SPEED, delta.as_secs_f32())
            } else {
                Vec3::ZERO
            },
            turning_around: false,
        }
    }
}
```

Add a public marker in `src/character.rs` and include it on the root:

```rust
#[derive(Component)]
pub struct Humanoid;

commands.spawn((
    Name::new("Humanoid"),
    Humanoid,
    HumanoidController::default(),
    // existing scene root, transform, and pending character
));
```

Add `LocomotionPlugin` with an `Update` system gated on
`in_state(PrototypeState::Running)`. Convert keyboard state to
`MovementInput`, call `controller.update`, add its translation, and set:

```rust
transform.rotation =
    Quat::from_rotation_y(update.heading) * character_transform(config.scale, config.yaw_degrees).rotation;
```

Register `LocomotionPlugin` in `src/main.rs` after `CharacterPlugin`.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run:

```powershell
cargo test --test locomotion_contract --test app_contract
```

Expected: all locomotion and application contract tests pass.

- [ ] **Step 5: Commit**

```powershell
git add src\character.rs src\locomotion.rs src\main.rs tests\locomotion_contract.rs
git commit -m "feat: steer the walking humanoid"
```

### Task 3: Add Q/E orbit and mouse-wheel zoom

**Files:**
- Modify: `src/locomotion.rs`
- Modify: `src/inspection.rs`
- Modify: `tests/locomotion_contract.rs`

- [ ] **Step 1: Add failing orbit and zoom tests**

Append:

```rust
use bevy_concept_world::locomotion::{
    CAMERA_MAX_DISTANCE, CAMERA_MIN_DISTANCE, OrbitCamera,
    orbit_yaw, zoom_target, smooth_distance,
};

#[test]
fn q_and_e_orbit_in_opposite_directions() {
    assert_eq!(orbit_yaw(0.0, 1.0, 1.0), -orbit_yaw(0.0, -1.0, 1.0));
}

#[test]
fn wheel_up_zooms_in_and_distance_is_clamped() {
    assert!(zoom_target(4.0, 1.0) < 4.0);
    assert_eq!(zoom_target(CAMERA_MIN_DISTANCE, 100.0), CAMERA_MIN_DISTANCE);
    assert_eq!(zoom_target(CAMERA_MAX_DISTANCE, -100.0), CAMERA_MAX_DISTANCE);
}

#[test]
fn camera_distance_converges_without_overshoot() {
    let next = smooth_distance(8.0, 2.0, 0.1);
    assert!((2.0..8.0).contains(&next));
}
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```powershell
cargo test --test locomotion_contract
```

Expected: compilation fails because the orbit-camera API does not exist.

- [ ] **Step 3: Implement orbit state and camera update**

Add:

```rust
use bevy::input::mouse::AccumulatedMouseScroll;

pub const CAMERA_MIN_DISTANCE: f32 = 1.5;
pub const CAMERA_MAX_DISTANCE: f32 = 12.0;
const CAMERA_ORBIT_RATE: f32 = PI / 2.0;
const CAMERA_ZOOM_PER_LINE: f32 = 0.75;
const CAMERA_ZOOM_RESPONSE: f32 = 10.0;

pub fn orbit_yaw(yaw: f32, direction: f32, seconds: f32) -> f32 {
    normalize_angle(yaw + direction.clamp(-1.0, 1.0) * CAMERA_ORBIT_RATE * seconds.max(0.0))
}

pub fn zoom_target(distance: f32, wheel_lines: f32) -> f32 {
    (distance - wheel_lines * CAMERA_ZOOM_PER_LINE)
        .clamp(CAMERA_MIN_DISTANCE, CAMERA_MAX_DISTANCE)
}

pub fn smooth_distance(current: f32, target: f32, seconds: f32) -> f32 {
    let blend = 1.0 - (-CAMERA_ZOOM_RESPONSE * seconds.max(0.0)).exp();
    current + (target - current) * blend
}

#[derive(Component, Debug, Clone, Copy)]
pub struct OrbitCamera {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target_distance: f32,
    pub target_height: f32,
}
```

Attach `OrbitCamera` to the inspection camera using values derived from the
existing `CAMERA_POSITION` and `LOOK_AT`. Add a camera system after locomotion
that:

1. reads Q/E and `AccumulatedMouseScroll`;
2. updates yaw and target distance;
3. smooths current distance;
4. builds a spherical offset;
5. places the camera at `humanoid.translation + target + offset`;
6. calls `looking_at(target, Vec3::Y)`.

Increase `GROUND_SIZE` from `12.0` to `100.0`.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run:

```powershell
cargo test --test locomotion_contract
```

Expected: all locomotion contract tests pass.

- [ ] **Step 5: Commit**

```powershell
git add src\inspection.rs src\locomotion.rs tests\locomotion_contract.rs
git commit -m "feat: add orbit camera and zoom"
```

### Task 4: Diagnostics and documentation

**Files:**
- Modify: `src/diagnostics.rs`
- Modify: `tests/app_contract.rs`
- Modify: `README.md`

- [ ] **Step 1: Add failing source-contract assertions**

Add assertions that require the rendered controls:

```rust
assert!(diagnostics.contains("Arrows: walk/steer/turn around"));
assert!(diagnostics.contains("Q/E: orbit"));
assert!(diagnostics.contains("Wheel: zoom"));
```

Add assertions that `main.rs` registers `LocomotionPlugin` and that
`diagnostics.rs` still contains Space, P, and Escape handling.

- [ ] **Step 2: Run the test and verify RED**

Run:

```powershell
cargo test --test app_contract
```

Expected: assertions fail because the new help text is absent.

- [ ] **Step 3: Update overlay and README**

The overlay control footer becomes:

```text
Arrows: walk/steer/turn around   Q/E: orbit   Wheel: zoom
Space: pause/resume   P: screenshot   Esc: exit
```

When the controller has an active turnaround, append:

```text
Movement: turning around
```

Document every control, movement semantics, constants, and the fact that the
walk clip continues in place when no movement key is held.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run:

```powershell
cargo test --test app_contract --test locomotion_contract
```

Expected: both test binaries pass.

- [ ] **Step 5: Commit**

```powershell
git add README.md src\diagnostics.rs tests\app_contract.rs
git commit -m "docs: describe steering controls"
```

### Task 5: Full verification and publication

**Files:**
- Modify if necessary: `docs/validation/humanoid-smoke-test.md`

- [ ] **Step 1: Run formatting and lint gates**

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Expected: both exit successfully with no warnings.

- [ ] **Step 2: Run all tests and checks**

```powershell
cargo test
cargo check --all-targets
```

Expected: all tests pass and all targets check.

- [ ] **Step 3: Run a bounded release smoke**

```powershell
$env:HUMANOID_WALK_CAPTURE_SECONDS='3'
cargo run --release
Remove-Item Env:HUMANOID_WALK_CAPTURE_SECONDS
```

Expected: the application reaches `Running`, loops `Walk_Loop`, writes a
non-empty PNG, and exits successfully.

- [ ] **Step 4: Record the verification result**

Update `docs/validation/humanoid-smoke-test.md` with the new controls, automated
gate results, and the requirement to assess steering feel on hardware GPU.

- [ ] **Step 5: Commit and push**

```powershell
git add docs\validation\humanoid-smoke-test.md
git commit -m "test: verify steering controls"
git push origin main
```
