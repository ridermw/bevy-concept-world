# Bevy Concept World

A staged prototype for bringing concept-driven humanoid characters into a 3D Bevy world.

## Current status

Design is complete for the first vertical slice:

- [x] Build gate: Bevy 0.19.1 toolchain builds and starts `animated_mesh_control` without panic.
      See [`docs/validation/engine-smoke-test.md`](docs/validation/engine-smoke-test.md).
- [ ] Visual gate: Confirm animated Fox renders and clips switch in a GPU-accelerated desktop session. *(deferred — requires display)*
- [x] Qualify a CC0 Quaternius humanoid and in-place walk animation: imported from the
      official free pack and integrity-locked by SHA-256 and byte size.
      See [`docs/validation/humanoid-import.md`](docs/validation/humanoid-import.md).
- [ ] Load and loop that walk in a standalone Bevy application.

Implementation has started: the engine startup smoke gate has passed. Visual Fox animation
confirmation is deferred pending a GPU-accelerated desktop session.

The humanoid asset is imported and locked. `assets/characters/quaternius/UAL1_Standard.glb`
comes from the official CC0 Quaternius Universal Animation Library v3.0 Standard download,
parses as a single `Scene` with one skinned `Mannequin` mesh and exactly one `Walk_Loop`
clip whose root translation is zero for the whole clip. Its preserved CC0 license, stable
`character.ron` contract, and generated `asset.lock.ron` are checked in.

Standalone humanoid animation is **not** complete: no Rust manifest, loader, validator,
inspection scene, or animation code exists yet, so the walk has never been played. The
recorded `yaw_degrees` of `180.0` is derived from the asset data and Bevy's coordinate
conventions and still awaits on-screen visual confirmation. See
[`docs/superpowers/specs/2026-08-31-humanoid-walk-prototype-design.md`](docs/superpowers/specs/2026-08-31-humanoid-walk-prototype-design.md).

## Re-importing the humanoid asset

The humanoid is checked in, so this is only needed to refresh the pack. Download
`Universal Animation Library[Standard].zip` (free tier) from the official page
<https://quaternius.itch.io/universal-animation-library>, then run:

```powershell
.\tools\import_quaternius.ps1 `
  -ArchivePath "$env:USERPROFILE\Downloads\Universal Animation Library[Standard].zip"
```

The script expands the archive only into the gitignored `.asset-import\quaternius`
staging directory, copies the GLB and the archive's own license file into
`assets\characters\quaternius`, and regenerates `asset.lock.ron`. It never edits
`character.ron`; update that by hand if the pack's scene or clip names change.
