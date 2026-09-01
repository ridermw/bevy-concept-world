# Bevy Concept World

A staged prototype for bringing concept-driven humanoid characters into a 3D Bevy world.

## Current status

- [x] Build gate: Bevy 0.19.1 toolchain builds and starts `animated_mesh_control` without panic.
      See [`docs/validation/engine-smoke-test.md`](docs/validation/engine-smoke-test.md).
- [ ] Visual gate: Confirm animated Fox renders and clips switch in a GPU-accelerated desktop session. *(deferred — requires display)*
- [x] Qualify a CC0 Quaternius humanoid and in-place walk animation: imported from the
      official free pack and integrity-locked by asset path, SHA-256, and byte size.
      See [`docs/validation/humanoid-import.md`](docs/validation/humanoid-import.md).
- [x] Validate the manifest and asset integrity in Rust: `src/config.rs` parses
      `character.ron` and `asset.lock.ron`, rejects unsafe paths and out-of-range fields,
      and re-hashes the GLB before any asset loading. Covered by `tests/config_contract.rs`.
- [x] Load and loop that walk in a standalone Bevy application: a release run of
      `bevy-concept-world` loads the locked GLB, validates the exact `Scene` and `Walk_Loop`
      names against the real file, discovers exactly one `AnimationPlayer` in the spawned
      hierarchy, attaches the animation graph, and reaches `Running` with the clip looping.
      See [`docs/validation/humanoid-smoke-test.md`](docs/validation/humanoid-smoke-test.md).
- [x] Harden the runtime states: real load-state polling with wall-clock timeouts, an
      observable `Validating` state, an unattended capture that verifies its own output file,
      and a nonzero exit for every failure path. Each failure path was exercised against the
      release binary; the results are in
      [`docs/validation/humanoid-smoke-test.md`](docs/validation/humanoid-smoke-test.md).
- [ ] Visual gate: Confirm the humanoid on screen. The committed screenshot can only settle the
      static criteria — upright, correctly scaled, facing the forward marker, limbs intact.
      Gait advancement, a clean loop seam, and pause/resume must be confirmed by watching the
      live window and writing the observation down.
      *(blocked — this host has no GPU; wgpu falls back to a software rasterizer that
      renders no 3D geometry and then times out)*
- [ ] Performance baseline: startup time, steady-state frame time, entity count, mesh and
      material count, and decoded texture bytes. *(instrumented and ready to read; **not yet
      measured** — see [Performance baseline](#performance-baseline). The binary now logs every
      one of these numbers itself, so the GPU-host run only has to be started and its log
      read)*
- [ ] Replace the reference mesh with the first concept-art-derived mesh.

The humanoid runtime is implemented and its asset contract is verified end to end against
the real GLB. What is **not** yet verified is how it looks: the recorded `yaw_degrees` of
`180.0`, the unit scale, and the quality of the skinned deformation and loop seam have never
been confirmed on screen, because the validation host has no GPU-accelerated display. The
exact remaining check and its acceptance criteria are written down in
[`docs/validation/humanoid-smoke-test.md`](docs/validation/humanoid-smoke-test.md).

The humanoid asset is imported and locked. `assets/characters/quaternius/UAL1_Standard.glb`
comes from the official CC0 Quaternius Universal Animation Library v3.0 Standard download,
parses as a single `Scene` with one skinned `Mannequin` mesh and exactly one `Walk_Loop`
clip whose root translation is zero for the whole clip. Its preserved CC0 license, stable
`character.ron` contract, and generated `asset.lock.ron` are checked in.

## Run

```powershell
cargo run --release
```

Controls:

| Key | Effect |
|---|---|
| `Space` | Pause or resume the walk animation without reloading the asset |
| `P` | Write `docs/validation/humanoid-walk.png` from the fixed inspection camera |
| `Esc` | Exit — with a **nonzero** exit code if the run is in `Failed`, zero otherwise |

The on-screen overlay reports the runtime state, the asset, the selected scene and clip, the
number of animation players discovered in the spawned scene *and* how many of them were wired
to an animation graph, the clip duration and playback speed, and the full detail of any fatal
failure.

The log carries the performance numbers: one `performance baseline:` line on reaching
`Running`, and a filtered `frame_time` / `fps` line every five seconds. See
[Performance baseline](#performance-baseline).

### Where the assets come from

The binary does not depend on its compile-time crate directory. The asset root is resolved at
startup, in this order:

1. `BEVY_CONCEPT_WORLD_ASSET_ROOT`, if set. It must name an existing directory; a set-but-missing
   override is a fatal error, not a fallback, because silently ignoring it would load a
   *different* character than the operator asked for.
2. `<current working directory>/assets`. This is the `cargo run` case, since cargo sets the
   working directory to the package root.
3. `assets` beside the executable, then in up to four of its ancestor directories. This covers
   both a copied binary shipped next to its own `assets/` folder and `target/release/…`
   launched from an unrelated working directory.

If none of these resolves, the run fails with the full list of paths it tried.

The screenshot path stays relative to the working directory, so a scripted run writes into the
checkout it was launched from. Its parent directory is created before the capture is requested.

### Unattended capture

On a host with no interactive desktop session, set `HUMANOID_WALK_CAPTURE_SECONDS` to a number
of seconds to take the same screenshot unattended and then exit.

The value must be finite, non-negative, and no larger than a day. Anything else — an empty
string, a word, a negative number, `inf`, `NaN` — is a fatal error with a nonzero exit, never a
silent fall back to attended mode: a scripted run that quietly became interactive would hang
until its harness killed it and be misreported as an infrastructure timeout instead of a typo.

An unattended run only exits `0` after `docs/validation/humanoid-walk.png` is confirmed to exist
on disk and to be non-empty. Any stale file at that path is deleted before the capture is
requested, so a previous run's image can never be mistaken for this one's. A missing file, an
empty file, a run that never reached `Running`, and any fatal failure all exit nonzero with an
actionable report.

## Closing the visual gate on a GPU host

Everything except the on-screen check is verified, and the performance numbers are
instrumented but unmeasured. To close both, clone or copy this checkout onto a machine with a
GPU-accelerated desktop session and run, **from the repository root**:

```powershell
cd <path-to>\bevy-concept-world
cargo run --release
```

Wait for the overlay to read `State: Running`. Compare the content of each line, not its exact
spacing:

```
State: Running
Asset: characters/quaternius/UAL1_Standard.glb
Scene: Scene   Clip: Walk_Loop
Animation players: 1 (1 with an animation graph)
  clip 1.33s  speed 1.00x  playing
Space: pause/resume   P: screenshot   Esc: exit
```

Then press `P`. That writes `docs/validation/humanoid-walk.png` — relative to the working
directory, so run it from the repository root — from the fixed inspection camera. Press
`Space` once to confirm the overlay flips to `paused` and the pose freezes, press it again to
confirm it resumes, then press `Esc` (exit code `0` from `Running`).

**The PNG and the live window prove different things, and the difference matters.** A single
still frame can only show the static criteria — upright posture, scale against the one-meter
marker, facing along the `-Z` marker, no collapsed or detached limbs, a stationary root. It
cannot show gait advancement, a clean loop seam, or that `Space` pauses and resumes, because
each of those is a statement about *change over time*. Those three must be confirmed by
watching the running window, and the observation written down; the committed image is not
evidence for them. The full split is in
[*Remaining work to close the visual gate*](docs/validation/humanoid-smoke-test.md#remaining-work-to-close-the-visual-gate).
Commit the image and record both the still-frame result and the live observation there.

## Performance baseline

**Instrumented, not yet measured.** The design asks for debug and release startup time,
steady-state frame time for one humanoid, entity count, mesh and material count, and the sum of
decoded texture bytes for the humanoid scene. The binary now reports all of them itself, so the
GPU-host session does not have to instrument anything: it only has to run the binary and read
the log.

**No values are claimed on this validation host.** wgpu selects the "Microsoft Basic Render
Driver" software rasterizer here, where the PBR pass produces no geometry. Frame time measured
on it ran from about 34 ms to about 68 ms average within ninety seconds of a single run and kept
climbing, and a single screenshot readback took tens of seconds. Timings measured against that
adapter would describe the software rasterizer, not the prototype, and the asset-derived counts
cannot be trusted from a render path that never draws the mesh. Publishing them would be worse
than publishing nothing, because they would look like a baseline for future custom meshes.

### Exact commands

Startup timing is *not* separately timed by a stopwatch: the application measures it on
`Time<Real>` from application startup to the frame that enters `Running`, and prints it. Run
each profile once, from the repository root.

Debug:

```powershell
cargo run
```

Release:

```powershell
cargo build --release
.\target\release\bevy-concept-world.exe
```

Both are attended runs. Do **not** set `HUMANOID_WALK_CAPTURE_SECONDS` for a frame-time reading:
an unattended run exits as soon as it has captured a frame, which is long before frame time has
settled. Leave the window open for at least a minute, then press `Esc`.

To keep only the two lines that matter:

```powershell
cargo run --release 2>&1 | Select-String -Pattern 'performance baseline|frame_time'
```

### Which log line carries which number

| Number the design asks for | Log line |
|---|---|
| Startup time to `Running` (per profile) | `performance baseline: … startup_to_running=…s` |
| Entity count | `performance baseline: … entities=…` |
| Mesh count | `performance baseline: … meshes=…` |
| Material count | `performance baseline: … standard_materials=…` |
| Decoded texture bytes | `performance baseline: … decoded_image_bytes=<exact bytes> (<same value, binary-prefixed>)` |
| Steady-state frame time | `frame_time:   64.024600ms (avg 34.483408ms)` — shape only, from the software host |
| Frames per second | `fps       :   15.618996   (avg 36.942580)` — shape only, from the software host |

The parenthesised size beside `decoded_image_bytes` scales its unit (`B`, `KiB`, `MiB`, `GiB`);
the exact byte count is always printed beside it, so the unit is a reading aid, never the only
record.

The `performance baseline:` line is emitted **exactly once**, on entering `Running`, by
`src/perf.rs`. It looks like this (the numbers below are placeholders, not measurements):

```
INFO bevy_concept_world::perf: performance baseline: startup_to_running=<s>s entities=<n> meshes=<n> standard_materials=<n> images=<n> decoded_image_bytes=<n> (<n> <unit>) images_without_cpu_data=<n>
```

`images_without_cpu_data` is the number of `Image` assets with no CPU-side data. Those
contribute nothing to `decoded_image_bytes`, so the count is printed beside it rather than
folded in as zero — otherwise the texture total could be silently understated.

The `frame_time` and `fps` lines come from Bevy's own `FrameTimeDiagnosticsPlugin` and
`LogDiagnosticsPlugin`, filtered to just those two diagnostics and throttled to one line every
five seconds so they cannot bury the state and capture lines. They are logged under the
`bevy_diagnostic` target and start from the first frame, so read the ones printed *after* the
`performance baseline:` line — those are the steady-state numbers, and the `avg` column is the
one to record.

Record the resulting numbers in
[`docs/validation/humanoid-smoke-test.md`](docs/validation/humanoid-smoke-test.md). They are a
baseline for comparison against future concept-art meshes, not pass/fail targets, so nothing in
this milestone is blocked on their values — only on their being real.

## Failure model

Two different things can go wrong, and the README used to conflate them.

**Bootstrap failures** happen before Bevy loads anything, in `main`: the asset root cannot be
resolved, `character.ron` or `asset.lock.ron` is missing or malformed, a field is out of range,
a path escapes the asset root, the GLB's SHA-256 or byte size does not match the lock, or the
unattended-capture request cannot be parsed.

**Runtime contract failures** happen after Bevy starts, against the real asset: the glTF or one
of its dependencies fails to load, the file declares no scene or no animation with the name the
manifest requires, the spawned hierarchy does not contain the expected number of
`AnimationPlayer` entities, the load never completes, or the spawned scene never becomes ready.

Either way the window still opens, the application enters the terminal `Failed` state, and the
reason is shown on screen and logged. It is never replaced with a placeholder or a blank scene
presented as success. In a scripted run — one where `HUMANOID_WALK_CAPTURE_SECONDS` is set —
`Failed` also exits nonzero.

## Runtime structure

| Module | Responsibility |
|---|---|
| `src/lib.rs` | Resolve the asset root: env override, working directory, then executable-relative |
| `src/config.rs` | Parse and validate `character.ron` and `asset.lock.ron` before Bevy starts |
| `src/state.rs` | `Loading` / `Validating` / `Running` / `Failed`, the first-failure-wins report, and the `Escape` exit code |
| `src/inspection.rs` | Fixed camera, ground, key light with shadows, ambient light, one-meter marker, `-Z` forward marker |
| `src/character.rs` | Poll the real load states, validate the glTF's real names, spawn from `OnEnter(Validating)`, discover its real `AnimationPlayer`s, loop the clip |
| `src/diagnostics.rs` | Status and failure overlay, pause/resume, screenshot, exit, and the verified unattended capture |
| `src/perf.rs` | Filtered frame-time logging and the one-time `performance baseline:` line taken on entering `Running` |

The four states are genuinely sequential. `Loading` polls
`AssetServer::get_load_states` — the root asset, its direct dependencies, *and* its recursive
dependencies, so a failed buffer or image is reported instead of leaving the root `Loaded` and
the run stuck. When everything is loaded and the manifest's names are confirmed, `Loading`
*prepares* handles and requests `Validating` without spawning anything, so `Validating` is a
state the run really occupies and really logs. The scene is spawned from
`OnEnter(Validating)`, and only the `WorldInstanceReady` observer — after it has walked the
spawned hierarchy and found the expected `AnimationPlayer` entities — may request `Running`.

Both `Loading` and `Validating` have a wall-clock timeout. Every timing decision in the
application is made on `Time<Real>`, not on Bevy's virtual clock, so a stalled or throttled
render loop cannot stretch a budget that is meant to be wall-clock.

`src/character.rs` never substitutes an expected value for an observed one: the discovered
scene and clip names come from the loaded `Gltf`, and the animation-player count comes from
walking the spawned hierarchy. Both are checked by pure functions covered in
`tests/app_contract.rs` and `tests/runtime_contract.rs`, which also assert the manifest's
`Scene` and `Walk_Loop` really exist in the checked-in GLB.

## Tests

| Suite | Covers |
|---|---|
| `tests/config_contract.rs` | Manifest and lock parsing, path safety, range checks, integrity re-hashing, against real temporary-directory fixtures |
| `tests/app_contract.rs` | Exact named-asset matching, player-count validation, spawn transform, and the real checked-in GLB |
| `tests/runtime_contract.rs` | Asset-root resolution, load-state evaluation and timeouts, capture-environment parsing, capture verification, exit codes, and two App/`World`-level tests over the real state machine and the real spawned-hierarchy walk |
| `tests/perf_contract.rs` | Baseline byte and duration formatting, decoded-image accounting, and a real `App` asserting that frame-time diagnostics are registered and that the baseline is taken once, only on reaching `Running` |

## Manifest and integrity validation

`bevy_concept_world::config::load_character_config` is implemented and tested. Given an
asset root it:

- parses `character.ron` and `asset.lock.ron` as strict UTF-8 RON, failing on invalid bytes
  rather than substituting replacement characters;
- rejects blank required fields, a non-positive or non-finite `scale`, a non-finite
  `yaw_degrees`, a zero `expected_animation_players`, and `root_motion: true`;
- rejects `gltf_path` and `license_path` values that are rooted, drive-qualified, or
  traversing, in Unix *and* Windows syntax, on every host OS;
- canonicalizes the resolved license and GLB paths and refuses anything that escapes the
  canonical asset root through a symlink or junction;
- requires the license file to exist and be non-empty;
- requires the lock's `gltf_path` to match the manifest's and its `sha256` to be exactly
  64 ASCII hex characters, compared case-insensitively;
- re-hashes the GLB and compares SHA-256 and byte size against the lock.

Standalone humanoid animation is now implemented and its asset contract is verified end to
end against the real GLB, but it has still never been **seen**: the recorded `yaw_degrees` of
`180.0` is derived from the asset's bind-pose geometry (the toe and ball joints sit ahead of
the ankle along `+Z`) and Bevy's `-Z` forward convention, and still awaits on-screen visual
confirmation. See
[`docs/superpowers/specs/2026-08-31-humanoid-walk-prototype-design.md`](docs/superpowers/specs/2026-08-31-humanoid-walk-prototype-design.md).

## Re-importing the humanoid asset

The humanoid is checked in, so this is only needed to refresh the pack. Download
`Universal Animation Library[Standard].zip` (free tier) from the official page
<https://quaternius.itch.io/universal-animation-library>, then run:

```powershell
.\tools\import_quaternius.ps1 `
  -ArchivePath "$env:USERPROFILE\Downloads\Universal Animation Library[Standard].zip"
```

To re-check what is already imported without writing anything:

```powershell
.\tools\import_quaternius.ps1 -VerifyOnly
```

The script runs on Windows PowerShell 5.1 and PowerShell 7+. It verifies the
archive's SHA-256 against the pinned v3.0 Standard hash before extracting,
expands only into the gitignored `.asset-import` staging area, and requires
exactly one archive member ending `Unreal-Godot\UAL1_Standard.glb` and exactly
one license file at the archive root — zero or several of either is an error
rather than a guess. It then stages a complete replacement for
`assets\characters\quaternius`, validates it, and swaps it in atomically with
rollback, so a refreshed model can never be paired with a stale
`asset.lock.ron`. `character.ron` is preserved verbatim, never generated; update
it by hand if the pack's scene or clip names change. Override
`-ExpectedArchiveSha256` only for a deliberate pack upgrade, and record the new
hash in the script and in
[`docs/validation/humanoid-import.md`](docs/validation/humanoid-import.md).
