# Humanoid Walk Prototype Design

**Date:** August 31, 2026  
**Status:** Approved design; implementation not started

## Purpose

Prove the shortest reliable path from a working Bevy installation to a humanoid
character visibly looping a walk animation. This prototype establishes the
runtime and asset contracts needed before creating custom meshes from concept
art.

The bundled Bevy Fox is only an engine smoke test. The first persistent
application deliverable uses a humanoid asset.

## Decisions

| Decision | Selection |
|---|---|
| Application repository | Standalone sibling repository at `Q:\git\bevy-concept-world` |
| Engine baseline | Bevy `0.19.1`, pinned exactly |
| Rust toolchain | Rust `1.98.0` |
| Windows build setting | Cargo incremental compilation disabled |
| Smoke-test asset | Bundled Bevy `Fox.glb` |
| Humanoid source | Quaternius Universal Animation Library |
| Humanoid license | Creative Commons Zero v1.0 Universal |
| Initial locomotion | In-place looping walk |
| Initial scene | Fixed camera, ground, light, and scale reference |

## Source material

- Bevy engine: `https://github.com/bevyengine/bevy`, tag `v0.19.1`
- Quaternius Universal Animation Library:
  `https://quaternius.com/packs/universalanimationlibrary.html`
- Download page:
  `https://quaternius.itch.io/universal-animation-library`

The Quaternius package was selected because it provides GLB files, a universal
humanoid rig, CC0 licensing, and a broad animation library that includes
locomotion. The implementation must use the free package unless the user
explicitly approves a paid source package.

## Architecture

### Repository boundary

The Bevy checkout at `Q:\git\bevy` is read-only reference material. No project
code or planning documents belong there.

`bevy-concept-world` is an independent Cargo application with:

- `bevy = "=0.19.1"` in `Cargo.toml`;
- a committed `Cargo.lock`;
- a pinned Rust `1.98.0` toolchain;
- `.cargo/config.toml` containing:

  ```toml
  # Rust 1.98 cannot finalize incremental sessions on Windows ReFS Dev Drives.
  # CI does not cache target artifacts, and release builds are already nonincremental.
  [build]
  incremental = false
  ```

The application must not use a path dependency on `Q:\git\bevy`.

### Application boundaries

The implementation should separate these responsibilities:

| Unit | Responsibility |
|---|---|
| Application state | Own `Loading`, `Validating`, `Running`, and `Failed` transitions |
| Asset manifest | Declare source, integrity, scene, animation, scale, and orientation |
| Character loader | Load the root GLB and all dependencies |
| Character validator | Confirm the required scene, clip, player, and finite transforms |
| Animation setup | Build and attach the single-clip animation graph |
| Inspection scene | Provide fixed camera, ground, light, and scale references |
| Diagnostics UI | Display state, selected asset and clip, duration, speed, and errors |

Each unit should have a narrow interface and should not depend on internal
details of the other units.

## Execution flow

### Phase 0: engine smoke test

1. Check out Bevy tag `v0.19.1`.
2. Apply the local nonincremental Cargo configuration.
3. Build the relevant Bevy example.
4. Run the bundled animated Fox example.
5. Confirm that a skinned mesh loads and animates.
6. Record the successful command in this repository.

Phase 0 is deliberately brief. It must not grow into copied Fox code, a custom
Fox application, or engine modifications.

### Phase 1: humanoid qualification

1. Download the free Quaternius Universal Animation Library package.
2. Preserve the package license and source URL.
3. Select the GLB variant intended for ordinary glTF-compatible engines.
4. Calculate and record the SHA-256 and byte size of the selected GLB.
5. Verify that the complete asset and its dependencies load.
6. Enumerate the available scenes and named animation clips.
7. Select an in-place walk clip by its exact exported name.
8. Record the character's unit scale and forward-axis correction.

The design intentionally avoids retargeting. The supplied model, skeleton, skin,
and animation remain together for this vertical slice.

### Phase 2: standalone humanoid prototype

1. Create the minimal Bevy application.
2. Load the root `Gltf` asset rather than selecting numeric glTF indices.
3. Wait until the root asset and all dependencies have loaded.
4. Validate the configured default scene and exact named walk clip.
5. Spawn the humanoid into the inspection scene.
6. Locate the spawned `AnimationPlayer`.
7. Attach an `AnimationGraph` containing the selected walk clip.
8. Loop the selected in-place walk indefinitely; the character root must not
   translate during the clip.
9. Expose pause and resume on Space and application exit on Escape.
10. Display the diagnostic overlay.

## Asset manifest

The checked-in manifest is the contract between runtime code and imported
content. It must contain:

- logical asset identifier;
- runtime GLB path;
- source URL;
- pack name and version or download date;
- asserted license and preserved license-file path;
- SHA-256;
- byte size;
- scene selector;
- exact required walk-clip name;
- expected animation-player count;
- scale correction;
- facing correction;
- whether the clip is in-place or uses root motion.

Runtime code must not silently substitute another scene or clip when the
manifest does not match the asset.

## Runtime states and errors

### Loading

Start loading the root GLB and wait for all dependencies. The application
remains responsive and displays the asset path being loaded.

### Validating

Confirm:

- the root `Gltf` asset exists;
- the configured scene exists;
- exactly one required named walk clip exists;
- the scene produces the expected `AnimationPlayer`;
- the animation graph can be attached;
- character transforms are finite;
- the configured scale is positive.

### Running

The humanoid is visible in the fixed inspection scene and continuously loops
the selected in-place walk. Space toggles pause and resume.

### Failed

Failure is explicit and persistent. The application must show and log:

- application state;
- asset path;
- underlying load error, when available;
- expected scene selector;
- expected and discovered animation names;
- expected and discovered animation-player counts;
- manifest hash and actual hash when integrity validation fails.

The application must not replace a failed humanoid with a static placeholder or
present a blank scene as success.

## Inspection scene

The scene exists to expose asset defects, not to resemble the final world. It
contains:

- a fixed perspective camera;
- a neutral ground plane;
- one directional light with visible shadows;
- ambient light sufficient to inspect the model;
- a one-meter scale marker;
- a forward-direction marker;
- a diagnostic overlay.

Camera placement, light settings, and character transform remain deterministic
between runs. This allows meaningful screenshots and later visual comparisons.

## Verification

### Automated checks

- Manifest parsing accepts the checked-in manifest.
- Manifest parsing rejects missing required fields.
- Hash validation accepts the qualified GLB.
- Hash validation rejects changed bytes.
- Asset validation reports a missing scene.
- Asset validation reports a missing or renamed walk clip.
- Asset validation rejects zero, negative, or non-finite scale.
- Animation setup reaches `Running` only after an `AnimationPlayer` is found.

### Local quality gate

Before any commit intended for sharing:

1. `cargo fmt --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo test`
4. A release-mode smoke run of the humanoid scene

### Visual acceptance

The prototype is complete when:

- the humanoid is upright and correctly scaled relative to the meter marker;
- its intended forward direction matches the scene marker;
- the walk loops continuously without a visible teleport at the seam;
- the model deforms without obvious collapsed or detached limbs;
- feet alternate and the body visibly advances through walk poses;
- the entity remains stationary because this milestone uses in-place
  locomotion;
- pause and resume work without reloading the asset;
- failures produce actionable on-screen diagnostics.

A fixed-camera screenshot and the exact run command are retained as evidence.

## Performance baseline

Optimization is not part of this vertical slice. The implementation records:

- debug and release startup time;
- steady-state frame time for one humanoid;
- entity count;
- mesh and material count;
- the sum of decoded byte sizes for textures loaded by the humanoid scene.

These measurements are a baseline for future custom meshes, not pass/fail
targets.

## Explicitly out of scope

- Creating a mesh from concept art.
- Retargeting animations to another skeleton.
- Runtime retargeting.
- World-space locomotion or root-motion extraction.
- Keyboard character movement, collisions, or navigation.
- Animation blending or a locomotion state machine.
- Multiple animated characters.
- Final-world environment art.
- Asset optimization beyond correcting a blocking defect.

## Follow-on milestone

After this design is implemented and verified, the next design will replace the
Quaternius reference mesh while preserving its known-good humanoid skeleton and
in-place walk contract. That milestone will define Blender authoring,
skin-weight transfer, export settings, concept-art acceptance criteria, and
side-by-side visual review.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Bevy API drift | Pin exactly to `0.19.1` and commit the lockfile |
| Git checkout and app disagree | Use the checkout only for the matching tagged example |
| Numeric glTF index changes | Select scenes and animations through validated manifest data |
| Export names are absent or duplicated | Inspect the downloaded pack before finalizing the manifest |
| Character faces the wrong direction | Record and visibly validate the facing correction |
| Root motion moves the entity unexpectedly | Select the in-place package/clip and validate stationary root behavior |
| Binary asset changes unnoticed | Pin SHA-256 and byte size |
| Licensing becomes ambiguous | Preserve the CC0 license file and source metadata beside the manifest |
| Fox phase consumes implementation time | Treat it as an unchanged one-command smoke gate |

## Definition of done

The design is fulfilled when a fresh checkout of `bevy-concept-world` can,
using documented commands, validate and launch the qualified CC0 humanoid in a
fixed inspection scene, continuously loop its named in-place walk animation,
pause and resume it, and report asset-contract failures clearly.
