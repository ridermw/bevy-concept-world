# Steering and Camera Controls Design

**Date:** September 1, 2026  
**Status:** Approved design; implementation not started

## Purpose

Extend the humanoid walk prototype from a stationary animation viewer into a
small interactive locomotion demonstration. The user must be able to steer the
walking humanoid, reverse its travel direction, orbit the view, and zoom while
preserving the existing asset-validation and animation guarantees.

This remains an inspection prototype rather than a general character
controller. It does not add physics, collision detection, terrain navigation,
animation blending, or new animation assets.

## Controls

| Input | Behavior |
|---|---|
| Up Arrow | Walk straight ahead at the normal movement speed |
| Left Arrow | Walk forward while smoothly steering left |
| Right Arrow | Walk forward while smoothly steering right |
| Down Arrow | Start one smooth 180-degree turnaround, then walk in the new direction while held |
| Q | Orbit the inspection camera left around the humanoid |
| E | Orbit the inspection camera right around the humanoid |
| Mouse wheel up | Zoom toward the humanoid |
| Mouse wheel down | Zoom away from the humanoid |
| Space | Preserve the existing animation pause/resume control |
| P | Preserve the existing screenshot control |
| Escape | Preserve the existing exit control |

Movement is input-driven: releasing the arrow keys stops root translation.
The walk clip remains the selected animation throughout this prototype; no idle
clip is introduced. A stopped character can therefore continue animating in
place, matching the original prototype behavior.

## Architecture

Add a `locomotion` module and plugin. It owns only interactive movement and
view control and does not participate in asset loading, glTF validation, or
animation-graph construction.

### Character boundaries

The spawned humanoid root receives a public marker component identifying the
entity whose world-space translation and heading locomotion may change. The
manifest-derived scale and authored facing correction remain intact.

Locomotion state stores:

- current world-space heading;
- normal steering input;
- whether a 180-degree turnaround is active;
- turnaround start and target headings;
- turnaround elapsed time.

The character advances along its current Bevy-forward direction (`-Z` after
rotation). Left and Right change yaw smoothly at a fixed maximum turn rate
while also applying forward movement. Up applies forward movement without
steering.

### Turnaround

Down is edge-triggered: one press starts one 180-degree turn. Repeated presses
while that turn is active do not queue additional turns.

The turn progresses over a fixed duration using smoothstep easing rather than
snapping the transform. Left and Right input does not alter heading during the
turnaround. Holding Down applies forward movement throughout the turn and
continues in the new direction after completion. Releasing Down stops
translation but does not cancel an active turn.

All angle calculations normalize their result so repeated steering and
turnarounds cannot accumulate unbounded yaw values.

### Camera rig

Replace the fixed camera transform with an orbit-camera component containing:

- yaw around the humanoid;
- pitch;
- current orbit distance;
- minimum and maximum distance;
- target height above the humanoid root.

Q and E continuously change orbit yaw while held. Mouse-wheel input changes the
target distance, clamped to safe near and far limits. Distance approaches the
target smoothly so wheel steps do not visibly snap.

Each frame, the camera derives its transform from the humanoid position and the
orbit state, then looks at the humanoid's chest-height target. The camera
follows translation but does not inherit the humanoid's heading, so steering
does not unexpectedly rotate the view.

This camera orbit is the requested "rotate world" behavior: the world appears
to rotate around the centered humanoid, while world-space transforms and travel
direction remain unchanged.

## Movement constants

Initial tuning values:

| Setting | Value |
|---|---|
| Forward speed | 1.5 meters per second |
| Normal steering rate | 90 degrees per second |
| Turnaround duration | 0.75 seconds |
| Camera orbit rate | 90 degrees per second |
| Camera target height | 0.95 meters |
| Minimum camera distance | 1.5 meters |
| Maximum camera distance | 12 meters |
| Zoom sensitivity | 0.75 meters per wheel line |

These values are named constants, not user configuration. They may be adjusted
during visual verification if the first implementation does not look
convincing, but the control semantics must remain unchanged.

## Scene behavior

The inspection ground must be large enough for a useful steering demonstration.
The camera follows the humanoid, while the ground, lighting, scale marker, and
forward marker remain in world space. This makes movement and heading changes
visible rather than rotating the entire scene graph with the character.

The diagnostic overlay adds concise control instructions and reports whether a
turnaround is active. Existing prototype-state, asset, clip, and animation
diagnostics remain unchanged.

## Error and state behavior

Locomotion and camera controls run only in `PrototypeState::Running`.

- Before `Running`, inputs cannot move the partially loaded hierarchy.
- In `Failed`, controls cannot hide or replace the terminal failure.
- If no marked humanoid exists in `Running`, locomotion performs no transform
  mutation; the existing validation contract remains responsible for making
  that state unreachable.
- Non-finite elapsed time or input-derived values must not be written into a
  transform.

The feature introduces no new fatal application state.

## Testing

Pure tests cover:

- straight movement along the current heading;
- left and right steering symmetry;
- frame-rate-independent heading integration;
- angle normalization;
- one 180-degree turnaround reaching the exact opposite heading;
- repeated Down presses not queuing turns;
- forward movement during a turnaround only while Down is held;
- camera orbit direction;
- wheel zoom direction and clamping;
- smooth camera-distance convergence.

App-level tests cover:

- controls only mutating the marked humanoid in `Running`;
- the camera following character translation without inheriting character yaw;
- existing Space, P, and Escape controls remaining registered.

Runtime verification covers:

1. Up walks straight.
2. Left and Right produce smooth, convincing curved travel.
3. Down turns the humanoid around once and permits travel in the opposite
   direction.
4. Q and E orbit in opposite directions while the humanoid stays centered.
5. The mouse wheel zooms in and out without crossing the configured limits.
6. Pause, screenshot, and exit still work.
7. The diagnostic overlay remains readable.

## Acceptance criteria

The feature is complete when:

- all existing validation and runtime tests still pass;
- new locomotion and camera tests pass;
- formatting, Clippy, and all-target checks pass;
- a GPU-backed manual run demonstrates all controls;
- the humanoid turns gradually rather than snapping;
- Down produces exactly one 180-degree reversal per press;
- camera orbit and zoom never alter the humanoid's world-space heading;
- the screenshot control still writes a valid non-empty PNG.
