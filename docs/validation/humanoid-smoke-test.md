# Humanoid walk smoke test

- **Original run:** 2026-08-31
- **Final verification:** 2026-09-01 (quality gate re-run; see
  [Final verification](#final-verification--2026-09-01))
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
| Performance baseline (startup, frame time, entity/mesh/material/texture counts) | **DEFERRED** |

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

## Observed run — success (2026-08-31)

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

## Observed runs — failures (2026-08-31)

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

## Final verification — 2026-09-01

The full local quality gate was re-run from a clean working tree on the same
host, in this order. Every command was run from the repository root.

| Command | Result |
|---|---|
| `cargo fmt --check` | exit **0**, no diff |
| `cargo clippy --all-targets -- -D warnings` | exit **0**, no warnings |
| `cargo test` | exit **0** — **103 tests passed**, 0 failed, 0 ignored |
| `cargo check --all-targets` | exit **0** |
| `cargo build --release` then an unattended capture run | exit **0** |

The test total is `tests/app_contract.rs` 20 + `tests/config_contract.rs` 48 +
`tests/runtime_contract.rs` 35; the two unit-test binaries and the doc-test
target contribute 0 each.

The unattended run used a bounded three-second delay, which is enough on this
host because `Running` is reached in well under a second and the capture itself
is separately bounded by the four-minute grace period:

```powershell
$env:HUMANOID_WALK_CAPTURE_SECONDS = '3'
.\target\release\bevy-concept-world.exe
```

It reproduced the original run's state sequence. The output below is filtered
to the state, capture, and character lines (the `spawned scene 'Scene'` line
from the original transcript is not in this filter, not absent from the run):

```
WARN bevy_render::renderer: The selected adapter is using a driver that only
     supports software rendering. This is likely to be very slow.
INFO bevy_concept_world::diagnostics: unattended capture enabled: 3s after reaching Running
INFO bevy_concept_world::character: loading character glTF: characters/quaternius/UAL1_Standard.glb
INFO bevy_concept_world::diagnostics: prototype state: None -> Some(Loading)
INFO bevy_concept_world::character: glTF matches the manifest (scene 'Scene', clip 'Walk_Loop'); entering Validating
INFO bevy_concept_world::diagnostics: prototype state: Some(Loading) -> Some(Validating)
INFO bevy_concept_world::character: looping 'Walk_Loop' on the humanoid
INFO bevy_concept_world::diagnostics: prototype state: Some(Validating) -> Some(Running)
INFO bevy_concept_world::diagnostics: unattended capture: writing docs/validation/humanoid-walk.png
INFO bevy_concept_world::diagnostics: unattended capture verified: docs/validation/humanoid-walk.png (63013 bytes); exiting
```

The written file was checked independently of the application: 63,013 bytes and
the PNG signature `89 50 4e 47 0d 0a 1a 0a`, so it is a real, non-empty PNG.
Its SHA-256 is
`c89a5d8b68a9564cfda915a323299ca8901df8676ff17f1797c38bab2f6118f9` — **byte for
byte identical** to the already-recorded
[`humanoid-overlay-software-renderer.png`](humanoid-overlay-software-renderer.png).

That identity is itself the finding: two independent runs on the software
adapter produced the same image down to the byte, which is only possible because
the frame contains nothing but the static overlay and the clear colour. A frame
containing a walking humanoid could not repeat exactly. So the capture proves
the screenshot pipeline works and the run reached `Running`; it proves nothing
about the render of the character.

The process exited **0**, but only after 653 seconds of wall clock: 44 seconds
from launch to the verified screenshot, and roughly ten minutes of wgpu teardown
afterwards, with the application's own work already finished. That confirms the
shutdown cost recorded below is reproducible, and is a further reason no timing
baseline is taken from this adapter.

The file was therefore deleted from `docs/validation/humanoid-walk.png` rather
than committed, and the evidence is kept only under the honest
overlay-software-renderer name. **The visual gate is not claimed.**

## Performance baseline — deferred

The design asks for debug and release startup time, steady-state frame time for
one humanoid, entity count, mesh and material count, and the sum of decoded
texture bytes for the humanoid scene. **None of these were measured, and none
are estimated here.**

They are deferred for one reason: on this host wgpu selects the "Microsoft Basic
Render Driver" software rasterizer, and the PBR pass draws nothing. A baseline
taken against that adapter would not describe the prototype:

- **Startup and frame time** would measure a software rasterizer. The
  capture-to-verification span above was roughly 41 seconds of wall clock for a
  single readback, and process teardown afterwards runs for minutes inside
  wgpu's queue wait. Those numbers say nothing about a GPU host.
- **Entity count** could be read today, but on its own it is not the baseline
  the design asked for, and quoting it beside four missing numbers would read
  like partial success.
- **Mesh, material, and decoded texture totals** come from what the render path
  actually prepared. A path that never draws the skinned mesh is not a
  trustworthy source for how much of the scene was really uploaded.

Publishing plausible-looking numbers from this host would be worse than
publishing none, because they would be inherited as a comparison point for the
concept-art meshes in the next milestone. These measurements are taken in the
same GPU-host session that closes the visual gate, and recorded here. They are a
baseline for future comparison, not pass/fail targets, so nothing in this
milestone is blocked on their values — only on their being real.

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

This is the **only** gate left. Run the exact commands below on a machine with a
GPU-accelerated desktop session, from the repository root, then confirm each
acceptance criterion by eye.

```powershell
cd <path-to>\bevy-concept-world
cargo run --release
```

Do not set `HUMANOID_WALK_CAPTURE_SECONDS` for this run: the point is to watch
the animation, and an unattended run exits as soon as it has captured a frame.

Controls:

| Key | Effect |
|---|---|
| `Space` | Pause or resume the walk without reloading the asset |
| `P` | Write `docs/validation/humanoid-walk.png` from the fixed inspection camera |
| `Esc` | Exit — `0` from `Running`, nonzero from `Failed` |

Wait until the overlay in the top-left reads exactly this, which is the state
the software-renderer host already reaches:

```
State: Running
Asset: characters/quaternius/UAL1_Standard.glb
Scene: Scene   Clip: Walk_Loop
Animation players: 1 (1 with an animation graph)
  clip 1.33s  speed 1.00x  playing
Space: pause/resume   P: screenshot   Esc: exit
```

`1 (1 with an animation graph)` and `playing` are both required; `1 (0 with an
animation graph)` means a player was found but never wired up.

Then press `P`. The screenshot path is relative to the working directory, so a
run started from the repository root writes
**`docs/validation/humanoid-walk.png`** inside the checkout. Commit that image
and record the outcome in this file.

The image, and the live window it was taken from, must show:

1. the humanoid upright and roughly 1.83 m against the one-meter yellow marker;
2. the humanoid facing along the cyan `-Z` forward marker, confirming the
   recorded `yaw_degrees: 180.0`;
3. alternating feet and a body that advances through walk poses;
4. no collapsed, detached, or exploded limbs;
5. a loop with no visible teleport at the seam;
6. the character root stationary, because this clip is in-place;
7. `Space` pausing and resuming without reloading the asset — the overlay
   switches between `playing` and `paused` and the pose freezes in place.

While that session is open, also record the deferred performance baseline
described above: release startup time to `Running`, steady-state frame time,
entity count, mesh and material count, and decoded texture bytes.
