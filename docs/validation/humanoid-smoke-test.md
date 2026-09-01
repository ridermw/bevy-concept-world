# Humanoid walk smoke test

- **Date:** 2026-09-01
- **Bevy:** 0.19.1 (pinned, `Cargo.lock` committed)
- **Rust:** 1.98.0
- **Asset:** Quaternius Universal Animation Library v3.0 Standard,
  `assets/characters/quaternius/UAL1_Standard.glb` (SHA-256 locked)
- **Scene selector:** `Scene`
- **Clip selector:** `Walk_Loop`
- **Locomotion:** in-place
- **Host:** Windows 11 Enterprise, AMD EPYC 7763, **no GPU and no interactive
  desktop session** — wgpu falls back to the "Microsoft Basic Render Driver"
  (DX12 software rasterizer)

## Result summary

| Gate | Status |
|---|---|
| Manifest and integrity bootstrap | **PASSED** |
| glTF loads with all dependencies | **PASSED** |
| Exact named scene and clip validated against the real file | **PASSED** |
| Spawned hierarchy produced the expected `AnimationPlayer` count | **PASSED** |
| Animation graph attached and clip looping | **PASSED** |
| Reaches `Running` without panic | **PASSED** |
| Visual acceptance (upright, scale, facing, deformation, loop seam) | **NOT VERIFIED** |

## Commands

Build and run:

```powershell
cargo build --release
cargo run --release
```

Unattended capture used on this host, because there is no desktop session in
which to press `P`:

```powershell
$env:HUMANOID_WALK_CAPTURE_SECONDS = '5'
.\target\release\bevy-concept-world.exe
```

## Observed run

```
INFO bevy_render::renderer: AdapterInfo { name: "Microsoft Basic Render Driver",
     device_type: Cpu, backend: Dx12, driver: "10.0.26100.8875" }
WARN bevy_render::renderer: The selected adapter is using a driver that only
     supports software rendering. This is likely to be very slow.
INFO bevy_winit::system: Creating new window Bevy Concept World — humanoid walk
INFO bevy_concept_world::character: loading character glTF: characters/quaternius/UAL1_Standard.glb
INFO bevy_concept_world::diagnostics: prototype state: None -> Some(Loading)
INFO bevy_concept_world::character: spawned scene 'Scene' with clip 'Walk_Loop'; validating spawned hierarchy
INFO bevy_concept_world::character: looping 'Walk_Loop' on the humanoid
INFO bevy_concept_world::diagnostics: prototype state: Some(Loading) -> Some(Running)
```

This is authoritative for the asset contract: the application only logs
`spawned scene 'Scene' with clip 'Walk_Loop'` after matching those exact names
against the names the loaded `Gltf` really declares, and it only logs
`looping 'Walk_Loop'` and enters `Running` after counting the `AnimationPlayer`
entities the spawned hierarchy really contains and finding exactly the
`expected_animation_players: 1` the manifest declares. A mismatch would have
entered the terminal `Failed` state instead.

`Validating` does not appear in the transition log because the world instance
became ready in the same frame the scene was spawned, so the queued
`Validating` transition was superseded by `Running` before the next
`StateTransition` run. This is ordinary Bevy `NextState` coalescing, not a
skipped validation: the spawned-hierarchy check still ran, in the
`WorldInstanceReady` observer, before `Running` was requested.

## Captured evidence

[`humanoid-overlay-software-renderer.png`](humanoid-overlay-software-renderer.png)
is a real `Screenshot::primary_window()` capture from the release binary. It
shows the diagnostic overlay reporting:

```
State: Running
Asset: characters/quaternius/UAL1_Standard.glb
Scene: Scene   Clip: Walk_Loop
Animation players: 1
  clip 1.33s  speed 1.00x  playing
Space: pause/resume   P: screenshot   Esc: exit
```

The rest of that frame is Bevy's default clear colour, sampled as exactly
`(43, 44, 47)` at every point tested. **No 3D geometry is present in it.** It
is overlay evidence only and is deliberately *not* named
`humanoid-walk.png`; the fixed-camera visual acceptance image required by the
design has not been produced.

## Why the visual gate is still open

The 3D inspection scene cannot be rendered on this host:

- The UI pass renders, so the window, swapchain, screenshot path, and render
  graph all work.
- The PBR pass produces nothing. No pipeline or shader error is logged.
- Once the run continues, the software rasterizer stops keeping up entirely and
  wgpu aborts the process:

  ```
  thread 'main' panicked at wgpu-core-29.0.4\src\device\queue.rs:208:29:
  We timed out while waiting on the last successful submission to complete!
  ```

  This was reproduced twice, at roughly 300 s of runtime and again immediately
  after a screenshot was requested 120 s into a run. It originates inside
  `wgpu-core`'s queue wait against the software adapter, not in application
  code; the application itself reached `Running` and stayed there without
  panicking for the whole preceding period.

## Remaining work to close the visual gate

Run the exact command below on a machine with a GPU-accelerated desktop
session, then confirm each acceptance criterion by eye:

```powershell
cargo run --release
# press P while the overlay reads "State: Running"
```

That writes `docs/validation/humanoid-walk.png`. It must show:

1. the humanoid upright and roughly 1.83 m against the one-meter yellow marker;
2. the humanoid facing along the cyan `-Z` forward marker;
3. alternating feet and a body that advances through walk poses;
4. no collapsed, detached, or exploded limbs;
5. a loop with no visible teleport at the seam;
6. the character root stationary, because this clip is in-place;
7. `Space` pausing and resuming without reloading the asset.
