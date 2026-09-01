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
| Bootstrap: asset root resolved, manifest parsed, integrity re-hashed | **PASSED** |
| glTF loads with all dependencies (root, direct, recursive states polled) | **PASSED** |
| Runtime contract: exact named scene and clip validated against the real file | **PASSED** |
| Runtime contract: spawned hierarchy produced the expected `AnimationPlayer` count | **PASSED** |
| Animation graph attached and clip looping | **PASSED** |
| `Loading` → `Validating` → `Running` are three observed transitions | **PASSED** |
| Unattended capture verified a non-empty file before exiting `0` | **PASSED** |
| Every failure path observed exits nonzero | **PASSED** |
| Asset root found from an unrelated working directory | **PASSED** |
| Visual acceptance (upright, scale, facing, deformation, loop seam) | **NOT VERIFIED** |

## Commands

Build:

```powershell
cargo build --release
```

Unattended capture used on this host, because there is no desktop session in
which to press `P`:

```powershell
$env:HUMANOID_WALK_CAPTURE_SECONDS = '3'
.\target\release\bevy-concept-world.exe
```

## Observed run — success

```
WARN bevy_render::renderer: The selected adapter is using a driver that only
     supports software rendering. This is likely to be very slow.
INFO bevy_concept_world::diagnostics: unattended capture enabled: 3s after reaching Running
INFO bevy_concept_world::character: loading character glTF: characters/quaternius/UAL1_Standard.glb
INFO bevy_concept_world::diagnostics: prototype state: None -> Some(Loading)
INFO bevy_concept_world::character: glTF matches the manifest (scene 'Scene', clip 'Walk_Loop'); entering Validating
INFO bevy_concept_world::character: spawned scene 'Scene'; validating the spawned hierarchy
INFO bevy_concept_world::diagnostics: prototype state: Some(Loading) -> Some(Validating)
INFO bevy_concept_world::character: looping 'Walk_Loop' on the humanoid
INFO bevy_concept_world::diagnostics: prototype state: Some(Validating) -> Some(Running)
INFO bevy_concept_world::diagnostics: unattended capture: writing docs/validation/humanoid-walk.png
INFO bevy_concept_world::diagnostics: unattended capture verified: docs/validation/humanoid-walk.png (63013 bytes); exiting
```

Process exit code: **0**.

This is authoritative for the asset contract. The application only logs
`glTF matches the manifest` after matching those exact names against the names
the loaded `Gltf` really declares, and it only logs `looping 'Walk_Loop'` and
enters `Running` after counting the `AnimationPlayer` entities the spawned
hierarchy really contains and finding exactly the
`expected_animation_players: 1` the manifest declares. A mismatch enters the
terminal `Failed` state instead — see the failure runs below, which were
produced by actually breaking each contract.

`Validating` is a state the run really occupies: `Loading` only *prepares* the
scene and animation-graph handles and requests the transition, and the scene is
spawned from `OnEnter(Validating)` in a later frame. Both transitions therefore
appear in the log, in order.

## Observed runs — failures

Each was produced by breaking one thing and running the same binary. Exit codes
were read from the process, not from a shell pipeline.

| Broken input | Reported summary | Exit |
|---|---|---|
| `HUMANOID_WALK_CAPTURE_SECONDS=abc` | `Unattended capture request is invalid` | **1** |
| `BEVY_CONCEPT_WORLD_ASSET_ROOT=Q:\nowhere-at-all` | `Asset root could not be resolved` | **1** |
| `animation_name: "Stroll_Loop"` in a copied asset root | `Character glTF does not match the manifest` | **1** |
| First two bytes of the GLB zeroed, lock left alone | `Character contract failed` (integrity mismatch) | **1** |
| First two bytes of the GLB zeroed, lock regenerated to match | `Character glTF failed to load` | **1** |

### Runtime contract failure: a clip name the file does not declare

```
ERROR bevy_concept_world::diagnostics: Character glTF does not match the manifest
  glTF declares no animation named 'Stroll_Loop'; discovered animations: A_TPose,
  Crouch_Fwd_Loop, ..., Walk_Formal_Loop, Walk_Loop
INFO bevy_concept_world::diagnostics: prototype state: Some(Loading) -> Some(Failed)
```

The discovered list is read from the loaded `Gltf`, so this run is also direct
evidence that the real file declares `Walk_Loop` and 42 other clips.

### Asset load failure: a corrupt GLB whose lock was regenerated to match

This is the case that used to hang in `Loading` forever. The lock was rebuilt
from the corrupted bytes so bootstrap *passes* and the failure happens inside
Bevy's loader, where only the load states can see it:

```
ERROR bevy_asset::server: Failed to load asset 'characters/quaternius/UAL1_Standard.glb'
      with asset loader 'bevy_gltf::loader::GltfLoader': invalid glTF file: expected value at line 1 column 1
ERROR bevy_concept_world::diagnostics: Character glTF failed to load
  root asset loader error: ... invalid glTF file: expected value at line 1 column 1
  direct dependency loader error: ... invalid glTF file: expected value at line 1 column 1
  recursive dependency loader error: ... invalid glTF file: expected value at line 1 column 1
  load states: root=Failed(...), dependencies=Failed(...), recursive dependencies=Failed(...)
INFO bevy_concept_world::diagnostics: prototype state: Some(Loading) -> Some(Failed)
```

All three of Bevy's load states are polled with `AssetServer::get_load_states`,
so a failure in a *dependency* of the root asset is reported too, rather than
leaving the root `Loaded` and the run stuck.

### Integrity failure: the checked-in lock really is enforced

The lock is enforced before Bevy starts, by `load_character_config`. The
mutation above (two bytes zeroed) was first run *without* regenerating
`asset.lock.ron`, and bootstrap rejected it:

```
ERROR bevy_concept_world::diagnostics: Character contract failed
  asset root: ...\bcw-integrity-309098886 (from Override)
  asset integrity mismatch for ...\characters/quaternius/UAL1_Standard.glb:
  expected sha256=69591853d817488edaa8fd9bf8fc1d821eaeaf789f8627b3cd23b41c4ed67997 byte_size=7618436,
  got      sha256=7f819eebf7ad91870fdf3bc40e984e9be7c168667bd3ffa3e75ea9b32ea10246 byte_size=7618436
INFO bevy_concept_world::diagnostics: prototype state: None -> Some(Failed)
```

Exit code **1**. The size is unchanged, so only the re-hash could have caught
it. The same behaviour is covered by
`tests/config_contract.rs::rejects_a_model_whose_bytes_changed_at_the_same_size`
and `::rejects_a_model_whose_size_changed_behind_the_same_prefix`, which mutate
a real fixture GLB on disk.

## Asset root resolution

The binary no longer depends on `CARGO_MANIFEST_DIR` at runtime. Resolution
order is `BEVY_CONCEPT_WORLD_ASSET_ROOT` → `<cwd>/assets` → `assets` beside the
executable or in up to four of its ancestor directories. Both live paths were
exercised:

- launched from the repository root (`cargo`-style): `assets` found via
  `WorkingDirectory`;
- launched with the working directory set to an empty temporary folder:
  `assets` found via `ExecutableDirectory`, three levels above
  `target\release\`, and the run reached `Running` and captured normally. The
  screenshot is written relative to the working directory, so that run wrote
  into the temporary folder, whose `docs\validation\` directory the application
  created itself.

## Captured evidence

[`humanoid-overlay-software-renderer.png`](humanoid-overlay-software-renderer.png)
is a real `Screenshot::primary_window()` capture from the release binary. It
shows the diagnostic overlay reporting:

```
State: Running
Asset: characters/quaternius/UAL1_Standard.glb
Scene: Scene   Clip: Walk_Loop
Animation players: 1 (1 with an animation graph)
  clip 1.33s  speed 1.00x  playing
Space: pause/resume   P: screenshot   Esc: exit
```

The player count is taken from *every* `AnimationPlayer` in the world, and the
parenthesised count from those that also carry an `AnimationGraphHandle`. A
discovered player that was never wired up would show as `1 (0 with an animation
graph)` rather than being invisible.

The rest of that frame is Bevy's default clear colour. **No 3D geometry is
present in it.** It is overlay evidence only and is deliberately *not* named
`humanoid-walk.png`; the fixed-camera visual acceptance image required by the
design has not been produced. `docs/validation/humanoid-walk.png` is
intentionally absent from the repository for that reason — the successful
capture run above wrote it, it was inspected, and it was kept only under the
honest name.

## Why the visual gate is still open

The 3D inspection scene cannot be rendered on this host:

- The UI pass renders, so the window, swapchain, screenshot path, and render
  graph all work.
- The PBR pass produces nothing. No pipeline or shader error is logged.
- Shutdown after a successful capture took roughly eight minutes of wall clock
  on the software adapter, all of it inside wgpu's teardown after the
  application had already written and verified its screenshot and requested
  exit. Earlier, longer runs on this host also reproduced
  `We timed out while waiting on the last successful submission to complete!`
  from `wgpu-core`'s queue wait. Both originate in the software adapter, not in
  application code.

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
