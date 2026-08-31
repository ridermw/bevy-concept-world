# Bevy Concept World

A staged prototype for bringing concept-driven humanoid characters into a 3D Bevy world.

## Current status

Design is complete for the first vertical slice:

- [x] Build gate: Bevy 0.19.1 toolchain builds and starts `animated_mesh_control` without panic.
      See [`docs/validation/engine-smoke-test.md`](docs/validation/engine-smoke-test.md).
- [ ] Visual gate: Confirm animated Fox renders and clips switch in a GPU-accelerated desktop session. *(deferred — requires display)*
- [x] Qualify a CC0 Quaternius humanoid and in-place walk animation: imported from the
      official free pack and integrity-locked by asset path, SHA-256, and byte size.
      See [`docs/validation/humanoid-import.md`](docs/validation/humanoid-import.md).
- [x] Validate the manifest and asset integrity in Rust: `src/config.rs` parses
      `character.ron` and `asset.lock.ron`, rejects unsafe paths and out-of-range fields,
      and re-hashes the GLB before any asset loading. Covered by `tests/config_contract.rs`.
- [ ] Load and loop that walk in a standalone Bevy application.

Implementation has started: the engine startup smoke gate has passed. Visual Fox animation
confirmation is deferred pending a GPU-accelerated desktop session.

The humanoid asset is imported and locked. `assets/characters/quaternius/UAL1_Standard.glb`
comes from the official CC0 Quaternius Universal Animation Library v3.0 Standard download,
parses as a single `Scene` with one skinned `Mannequin` mesh and exactly one `Walk_Loop`
clip whose root translation is zero for the whole clip. Its preserved CC0 license, stable
`character.ron` contract, and generated `asset.lock.ron` are checked in.

## Manifest and integrity validation

`bevy_concept_world::config::load_character_config` is implemented and tested. Given an
asset root it:

- parses `character.ron` and `asset.lock.ron` as strict UTF-8 RON, failing on invalid bytes
  rather than substituting replacement characters;
- rejects blank required fields, a non-positive or non-finite `scale`, a zero
  `expected_animation_players`, and `root_motion: true`;
- rejects `gltf_path` and `license_path` values that are rooted, drive-qualified, or
  traversing, in Unix *and* Windows syntax, on every host OS;
- canonicalizes the resolved license and GLB paths and refuses anything that escapes the
  canonical asset root through a symlink or junction;
- requires the license file to exist and be non-empty;
- requires the lock's `gltf_path` to match the manifest's and its `sha256` to be exactly
  64 ASCII hex characters, compared case-insensitively;
- re-hashes the GLB and compares SHA-256 and byte size against the lock.

Standalone humanoid animation is still **not** complete: no loader, inspection scene, or
animation code exists yet, so the walk has never been played. The recorded `yaw_degrees` of
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
