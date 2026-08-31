# Humanoid Asset Import

**Date:** 2026-08-31
**Task:** Phase 1 — qualify, import, and integrity-lock the CC0 Quaternius humanoid

## Source

- Official page: `https://quaternius.itch.io/universal-animation-library`
- Pack: Universal Animation Library, **Standard** (free) tier
- Pack version: **3.0** (`v3.0`); the page's latest devlog,
  "Added Root Motion and other fixes", is dated 16 June 2026 and the free upload
  carries the same date
- License stated on the page and inside the archive: **CC0 1.0 Universal**
- Downloaded on: 2026-08-31

### Download method

The archive was obtained through itch.io's official name-your-own-price flow with
no account and no payment, using the same three requests the site's own
"Download Now" button issues:

1. `GET https://quaternius.itch.io/universal-animation-library` — read the page
   CSRF token.
2. `POST https://quaternius.itch.io/universal-animation-library/download_url` —
   returns the signed, time-limited download-page URL.
3. `POST https://quaternius.itch.io/universal-animation-library/file/17958403?source=game_download&as_props=1&after_download_lightbox=true`
   — returns the signed CDN URL for the free upload, which was then fetched.

No mirror, reupload, or third-party host was used.

### Downloaded archive

| Property | Value |
|---|---|
| File name | `Universal Animation Library[Standard].zip` |
| Byte size | `15904933` |
| SHA-256 | `cc73fc4e495b82958207316596317a3f40b9fa38065bde1027937452da537724` |

The archive SHA-256 is recorded here for provenance only. It is not pinned in
`asset.lock.ron`, because itch.io may re-upload the pack; the lock pins the
extracted GLB that the runtime actually loads.

### Archive contents

```
Universal Animation Library[Standard]/Godot_Setup.png
Universal Animation Library[Standard]/License.txt
Universal Animation Library[Standard]/README.txt
Universal Animation Library[Standard]/Unity/UAL1_Standard.fbx
Universal Animation Library[Standard]/Unity/UAL1_Standard_RM.fbx
Universal Animation Library[Standard]/Unity_Setup.png
Universal Animation Library[Standard]/Unreal-Godot/UAL1_Standard.glb
Universal Animation Library[Standard]/Unreal-Godot/UAL1_Standard_RM.glb
Universal Animation Library[Standard]/Unreal_Setup.png
```

## Deviation from the plan: model file name

The design plan named the target GLB `AnimationLibrary_Godot_Standard.glb`. Pack
v3.0 does not contain a file with that name; the glTF-binary humanoid intended for
ordinary glTF-compatible engines is `Unreal-Godot/UAL1_Standard.glb`.

The official file name was preserved rather than renamed, so provenance stays
obvious. `tools/import_quaternius.ps1` searches for
`AnimationLibrary_Godot_Standard.glb` first and falls back to `UAL1_Standard.glb`,
and `character.ron` carries the real path. No unofficial substitute was used.

`UAL1_Standard_RM.glb` was deliberately **not** imported. The archive `README.txt`
states that the `_RM` file "has root motion baked into every animation, while the
other has root motion disabled". This milestone requires in-place locomotion.

## Imported files

| Repository path | Bytes | SHA-256 |
|---|---|---|
| `assets/characters/quaternius/UAL1_Standard.glb` | `7618436` | `69591853d817488edaa8fd9bf8fc1d821eaeaf789f8627b3cd23b41c4ed67997` |
| `assets/characters/quaternius/LICENSE.txt` | `332` | copied verbatim from `License.txt` in the archive |
| `assets/characters/quaternius/asset.lock.ron` | generated | UTF-8 without BOM |
| `assets/characters/quaternius/character.ron` | hand-written contract | — |

Import command:

```powershell
.\tools\import_quaternius.ps1 `
  -ArchivePath "$env:USERPROFILE\Downloads\Universal Animation Library[Standard].zip"
```

## Asset validation

The GLB was parsed directly from its binary container and inspected before the
manifest was written.

| Check | Result |
|---|---|
| GLB magic / container version | `0x46546C67`, version `2` |
| Declared GLB length vs. file size | `7618436` == `7618436` |
| Generator | `Khronos glTF Blender I/O v4.5.48` |
| Scene count | `1` |
| Scene name | `Scene` (default scene index `0`) |
| Mesh count / name | `1` — `Mannequin` |
| Skin count / name | `1` — `Armature` |
| Mesh is skinned | `JOINTS_0` vertex attribute present |
| Node count | `67` |
| Animation count | `43` |
| Clips named `Walk_Loop` | exactly `1` |

`Walk_Formal_Loop` also exists and is a distinct clip; the exact name `Walk_Loop`
resolves unambiguously.

### In-place locomotion

`Walk_Loop` drives the skin root joint `root` (node `64`) over `41` keys and
`1.333 s`. Its translation output is exactly zero on every axis for the whole
clip:

```
x range +0.00000..+0.00000  delta +0.00000
y range +0.00000..+0.00000  delta +0.00000
z range +0.00000..+0.00000  delta +0.00000
```

For contrast, the same clip in the rejected `UAL1_Standard_RM.glb` translates
`+1.30000` along `Z`. This confirms `root_motion: false` for the imported file.

### Scale

The `Mannequin` mesh `POSITION` accessor bounds are
`min [-0.972, +0.000, -0.164]` and `max [+0.972, +1.829, +0.205]`. A standing
height of `1.829` units means the asset is authored in metres at unit scale, so
`scale: 1.0` needs no correction. The `X` extent is arm span in the bind pose.

### Facing / `yaw_degrees`

`yaw_degrees: 180.0` was derived from measurement, not guessed:

- glTF 2.0 defines `+Z` as forward. The root-motion variant of `Walk_Loop`
  advances along `+Z`, so this asset follows the spec and faces `+Z`.
- Bevy's `Transform::forward()` is `-Z`
  (`crates/bevy_transform/src/components/transform.rs`).
- Bevy 0.19.1's glTF loader can rotate an imported scene between the two
  conventions, but `GltfConvertCoordinates` derives `Default` with
  `rotate_scene_entity: false` and `rotate_meshes: false`, so **no conversion is
  applied by default**. Its own correction constant is
  `Quat::from_xyzw(0.0, 1.0, 0.0, 0.0)` — a 180° yaw about `Y`, the same
  correction recorded here.

**Visual orientation confirmation is still PENDING.** The 180° value is derived
from the asset data and the engine's documented conventions; it has not been
compared against an on-screen forward marker, because that requires the
inspection scene (not yet implemented) and a GPU-accelerated desktop session.
Re-check this value during the visual acceptance gate.

## Importer error paths

`tools/import_quaternius.ps1` was exercised against controlled temporary
fixtures. The real `assets/characters/quaternius` contents were verified
unchanged afterwards.

| Case | Result |
|---|---|
| `-ArchivePath` omitted | `Cannot process command because of one or more missing mandatory parameters: ArchivePath.` |
| Archive path does not exist | `Archive not found: ...\does-not-exist.zip` |
| Archive is not a `.zip` | `Archive must be a .zip file, got '.rar': ...` |
| Archive has no known model file | `None of the expected model files (AnimationLibrary_Godot_Standard.glb, UAL1_Standard.glb) were found in the archive. Archive contains: License.txt, SomethingElse.glb` |
| Archive has no license file | `A license file was not found in the archive: ...\no-license.zip` |

Re-running the importer on the same archive reproduced the identical SHA-256 and
byte size.

The script expands only into `<repo>\.asset-import\quaternius` (gitignored). It
recomputes that path, refuses to continue if it does not match the expected
location, and refuses to delete it if it is not a directory, so no broad or
caller-controlled deletion is possible.

## Conclusion

Asset qualification and integrity lock **PASSED**. Runtime loading, animation
playback, and visual acceptance are **NOT** covered by this document and remain
open.
