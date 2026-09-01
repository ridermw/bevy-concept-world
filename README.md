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
      `bevy-concept-world` loads both locked GLBs, validates the exact `Scene` and `Walk_Loop`
      names against each real file, discovers exactly one `AnimationPlayer` in each spawned
      hierarchy, attaches both animation graphs, and reaches `Running` only after both clips
      start at phase zero. Both exported clips use the same approximately 1.33333-second cycle,
      so both players run at 1.0x while the duration guard keeps their normalized gait phase
      aligned.
      See [`docs/validation/humanoid-smoke-test.md`](docs/validation/humanoid-smoke-test.md).
- [x] Add the first concept-derived character: the generated Midcreek Cel Shift male
      technician is a deterministic 1.73 m blockout built on the qualified Quaternius
      armature and animation library. Its GLB, manifest, provenance, regeneration command,
      SHA-256, and byte size are checked in under
      `assets/characters/midcreek/technician-man/`.
- [x] Keep both character variants resident under one stable `Humanoid` locomotion root.
      `Tab` switches only the active child visibility; locomotion, camera state, animation
      phase, and the inactive model remain intact.
- [x] Harden the runtime states: real load-state polling with wall-clock timeouts, an
      observable `Validating` state, an unattended capture that verifies its own output file,
      and a nonzero exit for every failure path. Integration tests cover the failure branches,
      including a missing validation start marker; the successful production startup and
      capture path was exercised against the release binary and is recorded in
      [`docs/validation/humanoid-smoke-test.md`](docs/validation/humanoid-smoke-test.md).
- [ ] Visual gate: Confirm the humanoid on screen. The committed screenshot can only settle the
      static criteria — upright, correctly scaled, facing the forward marker, limbs intact.
      Gait advancement, a clean loop seam, and pause/resume must be confirmed by watching the
      live window and writing the observation down.
      *(blocked — this host has no GPU; wgpu falls back to a software rasterizer that
      renders no 3D geometry, and the process does not terminate after capture verification)*
- [ ] Performance baseline: startup time, steady-state frame time, entity count, mesh and
      material count, and decoded texture bytes. *(instrumented and ready to read; **not yet
      measured** — see [Performance baseline](#performance-baseline). The binary now logs every
      one of these numbers itself, so the GPU-host run only has to be started and its log
      read)*
- [ ] Visual gate for the generated technician: confirm silhouette, equipment, rigid-module
      motion, and clipping in a GPU-accelerated desktop session. The local software adapter
      reaches `Running` and captures the dual-model overlay, but does not draw the 3D scene.

The dual-character runtime is implemented and both asset contracts are verified end to end
against the real GLBs. Bootstrap validates the complete `CharacterCatalog` before Bevy starts
loading. Runtime loading and hierarchy validation are also catalog-wide: either variant can
block `Running`, and a missing validation-watchdog start marker fails actionably instead of
silently disarming the timeout.

The runtime owns one identity-transform `Humanoid` parent with the locomotion controller and
visibility hierarchy, plus one resident visual child per variant. Each child keeps its own
manifest transform, world scene, animation graph, and readiness record. Reference starts
visible, the technician starts hidden, and `Tab` changes only `CharacterSelection` and the two
child `Visibility` values.

What is **not** yet verified is how the generated technician looks in motion: the software
DX12 adapter used by this host can capture the overlay but not the 3D render. The exact
remaining visual check and its acceptance criteria are written down in
[`docs/validation/humanoid-smoke-test.md`](docs/validation/humanoid-smoke-test.md).

The reference humanoid asset is imported and locked.
`assets/characters/quaternius/UAL1_Standard.glb`
comes from the official CC0 Quaternius Universal Animation Library v3.0 Standard download,
parses as a single `Scene` with one skinned `Mannequin` mesh and exactly one `Walk_Loop`
clip whose root translation is zero for the whole clip. Its preserved CC0 license, stable
`character.ron` contract, and generated `asset.lock.ron` are checked in.

The technician asset is generated locally from original Midcreek concept direction and the
same CC0 rig/animation source. It is a low-detail vertical-slice blockout, not final topology
or skinning. See
[`assets/characters/midcreek/technician-man/SOURCE.md`](assets/characters/midcreek/technician-man/SOURCE.md)
for provenance, limitations, and the deterministic Blender 5.2.1 LTS workflow.

## Run

```powershell
cargo run --release
```

Controls:

| Key | Effect |
|---|---|
| `Up` | Walk straight ahead |
| `Left` / `Right` | Walk forward while steering left or right |
| `Down` | Start one eased 180° turnaround; while held, keep translating through the turn; releasing stops translation, but the active turn still finishes |
| `Q` / `E` | Orbit the camera left or right |
| Mouse wheel | Zoom the orbit camera, clamped to the configured minimum and maximum distance |
| `Tab` | Switch between the resident Quaternius reference and Midcreek technician without respawning or resetting either |
| `Space` | Pause or resume the walk animation without reloading the asset |
| `P` | Write `docs/validation/humanoid-walk.png` from the current orbit-camera view |
| `Esc` | Exit — with a **nonzero** exit code if the run is in `Failed`, zero otherwise |

The in-place walk animation stays in place when no movement key is held, and the human steering-feel gate remains manual: watch the live motion, orbit, zoom, and turnaround behavior on a GPU desktop session and record the result yourself.

The on-screen overlay reports the runtime state, active model, readiness and discovered player
count for each variant, the total animation players and graph wiring, each clip's duration and
playback speed, and the full detail of any fatal failure. While a turnaround is active it also
shows `Movement: turning around`.

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
Active model: Quaternius reference
Quaternius reference: ready, players 1/1
Midcreek technician - man: ready, players 1/1
Animation players: 2 (2 with an animation graph)
  clip 1.33s  speed 1.00x  playing
  clip 1.33s  speed 1.00x  playing
Arrows: walk/steer/turn around   Q/E: orbit   Wheel: zoom
Tab: switch model   Space: pause/resume
P: screenshot   Esc: exit
```

Then press `P`. That writes `docs/validation/humanoid-walk.png` — relative to the working
directory, so run it from the repository root — from the current interactive camera view. Press
`Tab` twice to confirm each model appears instantly without resetting position or normalized
animation phase. Both `Walk_Loop` clips span the same effective cycle and run at 1.0x. Press
`Space` once to confirm both clips flip to `paused` and both poses freeze, press it again to
confirm they resume, then press `Esc` (exit code `0` from `Running`).

If `ArrowDown` is holding an active turnaround, the overlay also shows `Movement: turning around`
above the help lines.

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
steady-state frame time, entity count, mesh and material count, and decoded
texture bytes. All counts (entities, meshes, materials, images, and decoded bytes) are
app-wide totals — they include the inspection scene, UI and font assets, and any fallback or
placeholder assets Bevy loads alongside the humanoid, not the humanoid in isolation. They are
useful comparative baselines across builds and mesh swaps, not isolated humanoid costs. The
binary now reports all of them itself, so the GPU-host session does not have to instrument
anything: it only has to run the binary and read the log.

Both variants are intentionally resident, spawned, and animated for the whole run. Hiding a
variant with `Tab` does not unload its GLB, despawn its entities, release its meshes/materials,
or stop its animation player. This spends memory and background animation work to make model
switching immediate and phase-preserving. Performance numbers are therefore the dual-resident
baseline, not the cost of only the currently visible model.

**No values are claimed on this validation host.** wgpu selects the "Microsoft Basic Render
Driver" software rasterizer here, where the PBR pass produces no geometry. Frame time measured
on it ran from about 34 ms to about 68 ms average within ninety seconds of a single run and kept
climbing, and a single screenshot readback took tens of seconds. Timings measured against that
adapter would describe the software rasterizer, not the prototype, and the asset-derived counts
cannot be trusted from a render path that never draws the mesh. Publishing them would be worse
than publishing nothing, because they would look like a baseline for future custom meshes.

### Exact commands

Startup timing is *not* separately timed by a stopwatch: `startup_to_running` is
`Time<Real>::elapsed()` sampled when entering `Running`. `Time<Real>` begins at the first app
update, so this measures asset loading and state progression from that point. It excludes
pre-App bootstrap (manifest parsing, the 7.6 MB integrity re-hash) and the work `App::run()`
does before the first frame — plugin finish/cleanup, window creation, and wgpu adapter/device
initialization — and is not a replacement for external wall-clock timing of the full process
startup. Run each profile once, from the repository root.

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
| `src/config.rs` | Parse and validate both `character.ron` / `asset.lock.ron` contracts into `CharacterCatalog` before Bevy starts |
| `src/state.rs` | `Loading` / `Validating` / `Running` / `Failed`, the first-failure-wins report, and the `Escape` exit code |
| `src/inspection.rs` | Initial orbit-camera framing, ground, key light with shadows, ambient light, one-meter marker, `-Z` forward marker |
| `src/character.rs` | Load and validate both glTFs, own selection/readiness, spawn one stable parent plus two resident visual children, wire both real `AnimationPlayer`s, and start them in phase |
| `src/locomotion.rs` | Move the stable parent and keep the orbit camera following that shared root |
| `src/diagnostics.rs` | Active/per-variant overlay, `Tab` selection, global pause/resume, screenshot, exit, and verified unattended capture |
| `src/perf.rs` | Filtered frame-time logging and the one-time `performance baseline:` line taken on entering `Running` |
| `tools/generate_midcreek_technician.py` | Deterministically generate the concept-derived technician GLB with Blender 5.2.1 LTS |

The four states are genuinely sequential. `Loading` polls
`AssetServer::get_load_states` for both root assets, their direct dependencies, and their
recursive dependencies, so a failed buffer or image is reported instead of leaving one root
`Loaded` and the run stuck. Only after both manifests' named assets are confirmed does
`Loading` prepare both scenes/graphs and request `Validating`. `OnEnter(Validating)` creates
the stable parent and both visual children. Each `WorldInstanceReady` observer walks only its
variant hierarchy; only the second successful validation starts both players and requests
`Running`.

Both `Loading` and `Validating` have a wall-clock timeout. Every timing decision in the
application is made on `Time<Real>`, not on Bevy's virtual clock, so a stalled or throttled
render loop cannot stretch a budget that is meant to be wall-clock.

`src/character.rs` never substitutes an expected value for an observed one: discovered scene
and clip names come from each loaded `Gltf`, and each animation-player count comes from walking
that spawned hierarchy. Behavioral integration tests cover catalog loading, distinct asset
handles, stable hierarchy/visibility, synchronized startup, selection preservation, and the
missing-watchdog-marker failure path; no test reads Rust source text and searches for
implementation substrings.

## Tests

| Suite | Covers |
|---|---|
| `tests/config_contract.rs` | Manifest and lock parsing, path safety, range checks, integrity re-hashing, against real temporary-directory fixtures |
| `tests/app_contract.rs` | Exact named-asset matching, stable dual-character hierarchy/visibility, synchronized players, selection controls, overlay behavior, and the real checked-in GLBs |
| `tests/runtime_contract.rs` | Asset-root resolution, load-state evaluation and timeouts, capture-environment parsing, capture verification, exit codes, and two App/`World`-level tests over the real state machine and the real spawned-hierarchy walk |
| `tests/perf_contract.rs` | Baseline byte and duration formatting, decoded-image accounting, and a real `App` asserting that frame-time diagnostics are registered and that the baseline is taken once, only on reaching `Running` |

## Manifest and integrity validation

`bevy_concept_world::config::load_character_catalog` is implemented and tested. Given an
asset root it loads both advertised character directories through
`load_character_config_from` and:

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
end against both real GLBs. The reference has been seen in the committed acceptance image; the
generated technician still awaits GPU-rendered visual confirmation. See
[`docs/superpowers/specs/2026-08-31-humanoid-walk-prototype-design.md`](docs/superpowers/specs/2026-08-31-humanoid-walk-prototype-design.md).

## Regenerating the Midcreek technician

The technician is generated, not hand-edited. Use Blender **5.2.1 LTS** and the exact
repository-root command documented in
[`assets/characters/midcreek/technician-man/SOURCE.md`](assets/characters/midcreek/technician-man/SOURCE.md).
That document also records the concept provenance, CC0 rig/animation dependency, designed
1.73 m rest-pose height, deterministic two-run hash/size check, lock-update procedure, and
vertical-slice limitations.

The accepted deterministic output is:

```text
assets/characters/midcreek/technician-man/technician-man.glb
SHA-256 2870e6293b8d3af3c4dfa45c8e476f07cf64ec9d6b3569017abc498ef746c79d
3,425,968 bytes
```

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
