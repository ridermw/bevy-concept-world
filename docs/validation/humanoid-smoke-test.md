# Humanoid walk smoke test

- **Recorded session dates:** the entries below are grouped by the date each was
  written down. Entries labelled 2026-08-31 are the initial runs; entries
  labelled 2026-09-01 are the final quality-gate re-run. These are recorded
  session dates only — they are not a claim about how much time passed between
  them, and nothing here depends on an overnight interval. See
  [Final verification](#final-verification--recorded-2026-09-01).
- **Bevy:** 0.19.1 (pinned, `Cargo.lock` committed)
- **Rust:** 1.98.0
- **Assets:** Quaternius Universal Animation Library v3.0 Standard,
  `assets/characters/quaternius/UAL1_Standard.glb`, plus the generated
  `assets/characters/midcreek/technician-man/technician-man.glb` (both
  SHA-256/size locked)
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
| Visual acceptance — static criteria (upright, scale, facing, deformation) | **NOT VERIFIED** |
| Visual acceptance — live criteria (gait advancement, loop seam, pause/resume) | **NOT VERIFIED** |
| Performance baseline: the binary emits every required number | **PASSED** (instrumented) |
| Performance baseline: the numbers themselves | **NOT MEASURED** (needs a GPU host) |

## Dual-character review smoke — recorded 2026-09-01

The final review build exercised the production binary after adding the
resident Midcreek technician, stable parent visibility, and validation
watchdog correction:

```powershell
$env:HUMANOID_WALK_CAPTURE_SECONDS = '3'
.\target\release\bevy-concept-world.exe
```

The production bootstrap and runtime path loaded both locked contracts and
both real GLBs, then emitted:

```text
loading character glTFs: characters/quaternius/UAL1_Standard.glb,
  characters/midcreek/technician-man/technician-man.glb
both glTFs match their manifests; entering Validating
spawned both character scenes; validating their hierarchies
prototype state: Some(Loading) -> Some(Validating)
Quaternius reference hierarchy validated; waiting for the other variant
both character walk loops started in phase
prototype state: Some(Validating) -> Some(Running)
unattended capture verified: docs/validation/humanoid-walk.png (232688 bytes);
  exiting
```

The phase-synchronization correction changes that startup line in the current
build to report the common reference cycle and both applied speeds:

```text
both character walk loops started at phase zero with a shared 1.3333s cycle
  (reference 1.0000x, technician 1.0313x)
```

The technician clip is approximately 1.375 seconds, so running it at
approximately 1.03125x gives it the same effective cycle duration as the
approximately 1.33333-second reference clip. Both players still start at seek
time zero; `Tab` changes only visibility, and `Space` pauses or resumes both
players without changing their relative normalized phase.

No B0004 hierarchy warning appeared. The captured overlay showed:

```text
State: Running
Active model: Quaternius reference
Quaternius reference: ready, players 1/1
Midcreek technician - man: ready, players 1/1
Animation players: 2 (2 with an animation graph)
```

The new PNG had SHA-256
`d4c165381bab7fb75ce633c9500e77251efa387cd8f6b422ff0209e5ea389235`
and was non-empty, but the Microsoft Basic Render Driver drew only the overlay
against the clear color: no ground, markers, reference model, or technician
were visible. It is valid runtime-state evidence but not a visual acceptance
artifact, so it was rejected and the previously committed reference visual
screenshot was restored unchanged
(`12f557a6a86842e5a7439da5d31e1d7f485e4128a6e5a88cead2ca8bc234bd78`,
2,354,139 bytes).

After logging capture verification and sending `AppExit::Success`, the process
still had not returned more than four minutes later and was externally
terminated.
Therefore this run confirms production bootstrap, both-variant readiness,
active Reference selection, non-empty capture, and absence of B0004, but it
does **not** claim a clean process exit or GPU-rendered visual acceptance. The
exact blocker is the host's CPU-only DX12 Microsoft Basic Render Driver; a
GPU-accelerated desktop session remains required.

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

## Observed run — success (recorded 2026-08-31)

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

## Observed runs — failures (recorded 2026-08-31)

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

The locks are enforced before Bevy starts, by `load_character_catalog`. The
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

## Final verification — recorded 2026-09-01

The full local quality gate was re-run from a clean working tree on the same
host, in this order. Every command was run from the repository root. "2026-09-01"
is the date this entry was written down, not a claim that a night passed since
the entries above.

| Command | Result |
|---|---|
| `cargo fmt --check` | exit **0**, no diff |
| `cargo clippy --all-targets -- -D warnings` | exit **0**, no warnings |
| `cargo test` | exit **0** — **122 tests passed**, 0 failed, 0 ignored |
| `cargo check --all-targets` | exit **0** |
| `cargo build --release` then an unattended capture run | exit **0** |

The test total is `tests/app_contract.rs` 20 + `tests/config_contract.rs` 48 +
`tests/perf_contract.rs` 19 + `tests/runtime_contract.rs` 35; the two unit-test
binaries and the doc-test target contribute 0 each. The 19 `perf_contract`
tests, and the `bevy_concept_world::perf` log line they cover, were added after
the release capture run transcribed below; that run's transcript is reproduced
unchanged, so it does not contain the baseline line.

The unattended run used a bounded three-second delay, which is enough on this
host because `Running` is reached in well under a second and the capture itself
is separately bounded by the four-minute grace period:

```powershell
$env:HUMANOID_WALK_CAPTURE_SECONDS = '3'
.\target\release\bevy-concept-world.exe
```

It reproduced the original run's state sequence. The transcript below is that
run's output filtered to lines matching these three patterns:

```
bevy_concept_world::diagnostics
bevy_concept_world::character
bevy_render::renderer
```

An earlier transcription of this transcript omitted the
`spawned scene 'Scene'` line and described it as "not in this filter". That was
wrong: the line is logged under `bevy_concept_world::character` and therefore
does match. It is restored to its correct position below — between the
manifest-match line and the first `Validating` transition — which is where the
2026-08-31 transcript above and a later debug run of the same binary both put
it.

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

(The `bevy_concept_world::perf` baseline line described under
[Performance baseline](#performance-baseline--instrumented-not-measured-here)
did not exist when this run was made, and is quoted from its own run there.)

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

This run is also the one that **succeeded**: it wrote a verified, non-empty PNG
and exited `0`. It should not be confused with the earlier, longer runs on this
host that aborted inside wgpu or reported
`We timed out while waiting on the last successful submission to complete!` from
`wgpu-core`'s queue wait — those produced no verified capture at all. The
distinction matters because the teardown cost below was measured on the
*successful* run, after its screenshot was already written and verified.

The process exited **0**, but only after 653 seconds of wall clock: 44 seconds
from launch to the verified screenshot, and the remainder inside wgpu's
teardown, with the application's own work already finished. Teardown on this
adapter has been measured at roughly **8 to 10 minutes** across runs. That is
a further reason no timing baseline is taken from this adapter.

The file was therefore deleted from `docs/validation/humanoid-walk.png` rather
than committed, and the evidence is kept only under the honest
overlay-software-renderer name. **The visual gate is not claimed.**

## Performance baseline — instrumented, not measured here

The design asks for debug and release startup time, steady-state frame time for
one humanoid, entity count, mesh and material count, and decoded texture bytes.
All counts (entities, meshes, materials, images, and decoded bytes) are
app-wide totals — they include the inspection scene, UI and font assets, and
any fallback or placeholder assets Bevy loads alongside the humanoid, not the
humanoid in isolation. They are useful comparative baselines across builds and
mesh swaps, not isolated humanoid costs.

**The binary now measures and prints every one of them itself.** `src/perf.rs`
adds Bevy's `FrameTimeDiagnosticsPlugin` and a `LogDiagnosticsPlugin` filtered
to `frame_time` and `fps` at one line every five seconds, plus a single
`performance baseline:` line emitted on entering `Running`. Nothing has to be
instrumented, timed by hand, or estimated on the GPU host; the exact commands
and the line-to-number mapping are in the README under
[*Performance baseline*](../../README.md#performance-baseline).

`startup_to_running` is `Time<Real>::elapsed()` sampled when entering `Running`.
`Time<Real>` begins at the first app update, so it measures asset loading and
state progression from that point. It explicitly excludes pre-App bootstrap
(manifest parsing, integrity re-hash) and the work `App::run()` does before the
first frame — plugin finish/cleanup, window creation, and wgpu adapter/device
initialization. External wall-clock timing is needed for the full process
startup.

**No baseline values are claimed from this host.** wgpu selects the "Microsoft
Basic Render Driver" software rasterizer here and the PBR pass draws nothing, so:

- **Startup and frame time** would measure a software rasterizer. Frame time on
  this adapter is tens to hundreds of milliseconds and drifts steadily upward
  over a single run. Those numbers say nothing about a GPU host.
- **Entity, mesh, and material counts** come from a run in which the render path
  never drew the skinned mesh, and **decoded texture bytes** describe what that
  path happened to keep on the CPU side. They are not a trustworthy record of
  how much of the scene a real adapter would upload.

Publishing plausible-looking numbers from this host would be worse than
publishing none, because they would be inherited as a comparison point for the
concept-art meshes in the next milestone.

### Instrumentation evidence — not a baseline

A bounded **debug** startup run on this software-rasterizer host was used only
to confirm the lines exist and have the documented shape. It was launched from
an empty temporary working directory so it could not write into the checkout:

```powershell
$env:BEVY_CONCEPT_WORLD_ASSET_ROOT = '<repo>\assets'
$env:HUMANOID_WALK_CAPTURE_SECONDS = '12'
<repo>\target\debug\bevy-concept-world.exe
```

It reached `Running`, emitted the baseline line, logged frame time every five
seconds, and verified its own capture before requesting exit. **The values below
are software-rasterizer readings and are explicitly not adopted as the
baseline.**

```
INFO bevy_concept_world::perf: performance baseline: startup_to_running=0.125s entities=468 meshes=10 standard_materials=6 images=11 decoded_image_bytes=3850248 (3.67 MiB) images_without_cpu_data=5
INFO bevy_diagnostic: fps       :   15.618996   (avg 36.942580)
INFO bevy_diagnostic: frame_time:   64.024600ms (avg 34.483408ms)
...
INFO bevy_diagnostic: fps       :    7.231599   (avg 18.465167)
INFO bevy_diagnostic: frame_time:  138.282000ms (avg 67.718719ms)
```

The drift from a 34 ms average to a 68 ms average inside ninety seconds is
itself why this adapter cannot supply a steady-state figure.

The real measurements are taken in the same GPU-host session that closes the
visual gate, and recorded here. They are a baseline for future comparison, not
pass/fail targets, so nothing in this milestone is blocked on their values —
only on their being real.

## Captured evidence

[`humanoid-overlay-software-renderer.png`](humanoid-overlay-software-renderer.png)
is a real `Screenshot::primary_window()` capture from the release binary. It
shows the diagnostic overlay reporting, in this order and with this content
(transcribed by eye from the image — the line breaks are real, but the exact
run of spaces between `Scene: Scene` and `Clip: Walk_Loop` and the leading
indent of the clip line are reproduced approximately, not byte for byte):

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
`humanoid-walk.png`. At the time of this software-renderer run, its output was
kept only under the honest overlay name. A later hardware-GPU run added
`docs/validation/humanoid-walk.png`; the two files represent different runs.

## Why the visual gate is still open

The 3D inspection scene cannot be rendered on this host:

- The UI pass renders, so the window, swapchain, screenshot path, and render
  graph all work.
- The PBR pass produces nothing. No pipeline or shader error is logged.
- Shutdown after a **successful** capture — one that wrote and verified a
  non-empty PNG and then requested exit — took roughly **8 to 10 minutes** of
  wall clock across release runs on the software adapter, all of it inside
  wgpu's teardown after the application's own work was finished. That is
  separate from the earlier, longer runs on this host that never produced a
  verified capture at all: those aborted inside wgpu or reported
  `We timed out while waiting on the last successful submission to complete!`
  from `wgpu-core`'s queue wait. Both behaviours originate in the software
  adapter, not in application code, but only the first is a teardown cost
  after success. A debug-profile run made while adding the performance
  diagnostics also wrote and verified its screenshot normally and then sat in
  the same teardown for longer than that range, and was terminated rather than
  waited out; the cost tracks the adapter, not the build profile's application
  code.

## Hardware-GPU visual review

The base walk was subsequently run on an Apple M4 Pro Metal adapter and a
hardware-GPU screenshot was committed. The interactive steering feature still
requires the human feel check described at the end of this document. Use the
exact commands below from the repository root, then confirm each acceptance
criterion by eye.

```powershell
cd <path-to>\bevy-concept-world
cargo run --release
```

Do not set `HUMANOID_WALK_CAPTURE_SECONDS` for this run: the point is to watch
the animation, and an unattended run exits as soon as it has captured a frame.

Controls:

| Key | Effect |
|---|---|
| `Up` | Walk straight ahead |
| `Left` / `Right` | Walk forward while steering |
| `Down` | Turn around once and continue in the opposite direction while held |
| `Q` / `E` | Orbit the camera left or right |
| Mouse wheel | Zoom the orbit camera within its near/far limits |
| `Space` | Pause or resume the walk without reloading the asset |
| `P` | Write `docs/validation/humanoid-walk.png` from the current orbit-camera view |
| `Esc` | Exit — `0` from `Running`, nonzero from `Failed` |

Wait until the overlay in the top-left reads as follows. Compare the content of
each line, not its exact spacing:

```
State: Running
Asset: characters/quaternius/UAL1_Standard.glb
Scene: Scene   Clip: Walk_Loop
Animation players: 1 (1 with an animation graph)
  clip 1.33s  speed 1.00x  playing
Space: pause/resume   P: screenshot   Esc: exit
```

`1 (1 with an animation graph)` and `playing` are both required; `1 (0 with an
animation graph)` means a player was found but never wired up. This is the same
state the software-renderer host already reaches.

Then press `P`. The screenshot path is relative to the working directory, so a
run started from the repository root writes
**`docs/validation/humanoid-walk.png`** inside the checkout. Commit that image
and record the outcome in this file.

### What the PNG can prove, and what it cannot

A still frame is evidence for the **static** criteria only. Record these from
the committed image:

1. the humanoid upright and roughly 1.83 m against the one-meter yellow marker;
2. the humanoid facing along the cyan `-Z` forward marker, confirming the
   recorded `yaw_degrees: 180.0`;
3. no collapsed, detached, or exploded limbs;
4. a plausible mid-walk pose with the feet in different positions.

The remaining criteria are statements about **change over time**, and no single
PNG can establish any of them. They must be confirmed by **watching the live
window**, and the observation written down here in words — the image is not
evidence for them:

5. **Gait advancement:** the body really moves through successive walk poses and
   the feet alternate, rather than holding one pose. A frozen character also
   produces a plausible-looking still.
6. **Loop seam:** watching several consecutive loops shows no visible teleport
   or hitch at the wrap point. A single frame cannot be at the seam and away
   from it at once.
7. **Root stationary:** the character root stays at the origin across whole
   loops, because this clip is in-place. One frame cannot distinguish "at the
   origin" from "passing through the origin".
8. **Pause and resume:** pressing `Space` freezes the pose and flips the overlay
   to `paused`, and pressing it again resumes from that pose without reloading
   the asset. This is a control behaviour, not an appearance.

Record criteria 5–8 as an explicit observation ("watched N loops; feet
alternate; no seam hitch; `Space` froze and resumed"), not as an inference from
the screenshot.

### Also record the performance baseline

While that session is open, take the performance baseline. Nothing has to be
instrumented: run the binary in each profile and read its own log, exactly as
described in the README under
[*Performance baseline*](../../README.md#performance-baseline). Copy the
`performance baseline:` line for the debug run and for the release run, and a
`frame_time` line from well after startup in the release run, into this file.

## Steering and camera controls — automated verification recorded 2026-09-01

The interactive locomotion milestone adds:

- Up Arrow for straight forward travel;
- Left and Right Arrow for forward travel along smooth, frame-rate-independent
  steering arcs;
- Down Arrow for one eased 180-degree turnaround per press, with continued
  travel while the key remains held;
- Q and E for camera orbit around the humanoid;
- mouse-wheel zoom with smooth convergence and 1.5–12 m limits.

The automated quality gate passed from the isolated
`feature/steering-controls` worktree:

| Command | Result |
|---|---|
| `cargo fmt --check` | exit **0**, no diff |
| `cargo clippy --all-targets -- -D warnings` | exit **0**, no warnings |
| `cargo test` | exit **0** — **166 tests passed**, 0 failed, 0 ignored |
| `cargo check --all-targets` | exit **0** |
| `cargo build --release` | exit **0** |
| Bounded release capture from session storage | exit **0**, 74,939-byte PNG |

The test total is `tests/app_contract.rs` 26 +
`tests/config_contract.rs` 48 + `tests/locomotion_contract.rs` 38 +
`tests/perf_contract.rs` 19 + `tests/runtime_contract.rs` 35. The unit-test
binaries and doc-test target contribute 0 each.

The release run used the checked-in asset root but a separate working directory
under session storage, so it could not overwrite the committed hardware-GPU
evidence image. It reached `Loading` → `Validating` → `Running`, looped
`Walk_Loop`, wrote and verified a 74,939-byte PNG, and exited successfully.

The locomotion contracts cover straight travel, left/right symmetry, exact
arc integration across different frame sizes, shallow-steering numerical
stability, angle wrapping, turnaround continuation and leftover-frame time,
Running-only transform mutation, camera follow ordering, orbit direction,
line/pixel wheel conversion, zoom clamping, smooth zoom convergence, and safe
handling of absent or duplicate camera/character entities.

### Remaining human gate

Automated tests establish deterministic controls and transforms, but not whether
the motion looks convincing. On a hardware-GPU host, confirm:

1. Left and Right create visually smooth turns with the model facing its travel
   direction.
2. Down creates one natural-looking 180-degree reversal and then continues in
   the opposite direction while held.
3. Q and E orbit in opposite directions while keeping the humanoid centered.
4. Mouse-wheel zoom is responsive and respects useful near/far framing.
5. Space, P, and Escape retain their previous behavior.

This visual steering-feel check is intentionally assigned to the human operator;
it is not inferred from the software-renderer capture.
