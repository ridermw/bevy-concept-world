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
- [ ] Visual gate: Confirm on screen that the humanoid is upright, correctly scaled,
      facing the forward marker, and deforming cleanly through the walk loop.
      *(blocked — this host has no GPU; wgpu falls back to a software rasterizer that
      renders no 3D geometry and then times out)*
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
