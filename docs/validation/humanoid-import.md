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

This archive SHA-256 is the importer's pinned contract: it is the default value of
`-ExpectedArchiveSha256` in `tools/import_quaternius.ps1`, and the archive is
hashed and compared **before** it is extracted. A mismatch aborts the import
before any file is written. itch.io may re-upload the pack, so the parameter can
be overridden — but only as a deliberate, reviewed pack upgrade, after which the
new hash must be written into both the script default and this document.

The archive hash is intentionally *not* stored in `asset.lock.ron`. The lock
pins the extracted GLB that the runtime actually loads, by path, hash, and size.

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
obvious. `character.ron` carries the real path. No unofficial substitute was used.

The v3.0 Standard contract is now exact, and `tools/import_quaternius.ps1`
enforces it with no fallback to the legacy name or to any other pack layout:

| Contract element | Value |
|---|---|
| Archive member | path ending `Unreal-Godot\UAL1_Standard.glb`, **exactly one** match |
| Archive license | a single `License[.txt/.md]` at the **archive root**, **exactly one** match |
| Destination model | `assets\characters\quaternius\UAL1_Standard.glb` |
| Locked asset path | `characters/quaternius/UAL1_Standard.glb` |

Both zero matches and two-or-more matches are hard errors: the importer collects
and sorts every candidate and refuses to guess. Preferring a stale legacy name,
or taking the first of several matches, could silently import the wrong file, so
neither is done. A pack that does not satisfy the contract must be re-qualified
by hand and the contract updated deliberately.

`UAL1_Standard_RM.glb` was deliberately **not** imported. The archive `README.txt`
states that the `_RM` file "has root motion baked into every animation, while the
other has root motion disabled". This milestone requires in-place locomotion.

## Imported files

| Repository path | Bytes | SHA-256 |
|---|---|---|
| `assets/characters/quaternius/UAL1_Standard.glb` | `7618436` | `69591853d817488edaa8fd9bf8fc1d821eaeaf789f8627b3cd23b41c4ed67997` |
| `assets/characters/quaternius/LICENSE.txt` | `332` | copied verbatim from `License.txt` in the archive |
| `assets/characters/quaternius/asset.lock.ron` | generated | UTF-8 without BOM |
| `assets/characters/quaternius/character.ron` | hand-written contract | preserved verbatim by the importer |

`asset.lock.ron` binds the hash and size to the file they describe, so the lock
cannot be silently read against some other GLB:

```ron
(
    gltf_path: "characters/quaternius/UAL1_Standard.glb",
    sha256: "69591853d817488edaa8fd9bf8fc1d821eaeaf789f8627b3cd23b41c4ed67997",
    byte_size: 7618436,
)
```

`gltf_path` is the Bevy asset-server path, relative to `assets/`, and must equal
`gltf_path` in `character.ron`. The importer and `-VerifyOnly` both enforce that.

Import command:

```powershell
.\tools\import_quaternius.ps1 `
  -ArchivePath "$env:USERPROFILE\Downloads\Universal Animation Library[Standard].zip"
```

Verify command (read-only; rewrites nothing):

```powershell
.\tools\import_quaternius.ps1 -VerifyOnly
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
That figure is evidence about *root motion only*; it is deliberately not used as
evidence about which way the character faces (see below).

### Scale

The `Mannequin` mesh `POSITION` accessor bounds are
`min [-0.972, +0.000, -0.164]` and `max [+0.972, +1.829, +0.205]`. A standing
height of `1.829` units means the asset is authored in metres at unit scale, so
`scale: 1.0` needs no correction. The `X` extent is arm span in the bind pose.

### Facing / `yaw_degrees`

`yaw_degrees: 180.0` was derived from bind-pose geometry, not guessed and not
inferred from animation:

- **The asset faces `+Z` in its bind pose.** Joint rest positions were recovered
  from the skin's `inverseBindMatrices` (each joint's mesh-space bind position is
  the translation of the inverted inverse-bind matrix). The foot chain is
  unambiguous — the ball and toe sit *ahead* of the ankle along `+Z`:

  | Joint | x | y | z |
  |---|---|---|---|
  | `foot_l` (ankle) | `+0.0890` | `+0.1037` | `-0.0358` |
  | `ball_l` (ball) | `+0.0890` | `+0.0152` | `+0.1132` |
  | `ball_leaf_l` (toe tip) | `+0.0890` | `+0.0152` | `+0.1921` |

  The toe tip is `+0.2279` ahead of the ankle along `+Z`, and the right-side
  chain mirrors it exactly. The `Mannequin` `POSITION` bounds agree: the mesh
  reaches `+0.205` on `+Z` (toes) versus `-0.164` on `-Z` (heels). Handedness
  agrees too: `hand_l` is at `x = +0.7389`, matching glTF 2.0's convention that
  `-X` is right, `+Y` is up, and the front of an asset faces `+Z`.

- The root-motion variant's `+Z` translation is **not** used here. A clip's local
  root translation is expressed in the root joint's own space; without resolving
  that joint's bind and parent transforms it says nothing about world-space
  direction, so it cannot establish facing. The static bind pose above can, and
  does.

- Bevy's `Transform::forward()` is `-Z`
  (`crates/bevy_transform/src/components/transform.rs`).
- Bevy 0.19.1's glTF loader can rotate an imported scene between the two
  conventions, but `GltfConvertCoordinates` derives `Default` with
  `rotate_scene_entity: false` and `rotate_meshes: false`, so **no conversion is
  applied by default**. Its own correction constant is
  `Quat::from_xyzw(0.0, 1.0, 0.0, 0.0)` — a 180° yaw about `Y`, the same
  correction recorded here.

A 180° yaw about `Y` maps the asset's `+Z` bind-pose facing onto Bevy's `-Z`
forward, which is why `yaw_degrees: 180.0` is recorded.

**Visual orientation confirmation is still PENDING.** The 180° value is derived
from the asset data and the engine's documented conventions; it has not been
compared against an on-screen forward marker, because that requires the
inspection scene (not yet implemented) and a GPU-accelerated desktop session.
Re-check this value during the visual acceptance gate.

## Importer behaviour

`tools/import_quaternius.ps1` runs on Windows PowerShell 5.1 and PowerShell 7+.
It writes generated text through `[System.IO.File]::WriteAllText` with
`UTF8Encoding($false)` rather than `Set-Content -Encoding utf8NoBOM`, which does
not exist in 5.1.

Import is transactional. The archive is hashed before extraction; a complete
replacement directory — GLB, `LICENSE.txt`, the preserved `character.ron`, and a
freshly generated `asset.lock.ron` — is built in the gitignored
`.asset-import\staged-destination`; every staged file is validated against the
contract and against the lock; only then is the live directory moved aside and
the staged directory moved into place, and the result is validated again. Any
failure restores the previous directory. A new model or license therefore cannot
be left paired with a stale lock.

`-VerifyOnly` re-runs exactly that validation against the live directory and
writes nothing: it checks that `gltf_path` in `asset.lock.ron` equals `gltf_path`
in `character.ron` and equals the contract path, that the referenced GLB exists
and is a well-formed GLB container, that its size and lowercase SHA-256 match the
lock, and that the declared license file exists and is non-empty.

## Importer error paths

`tools/import_quaternius.ps1` was exercised against the real archive and against
controlled temporary fixtures, under PowerShell 7.6.5 and Windows PowerShell
5.1.26100.8875. The real `assets/characters/quaternius` contents were verified
unchanged after every failure case.

| Case | Result |
|---|---|
| `-ArchivePath` omitted | `Cannot process command because of one or more missing mandatory parameters: ArchivePath.` |
| Archive path does not exist | `Archive not found: ...\does-not-exist.zip` |
| Archive is not a `.zip` | `Archive must be a .zip file, got '.rar': ...` |
| Archive hash differs from the pinned contract | `Archive SHA-256 mismatch, refusing to extract.` — raised **before** extraction |
| Archive has no contract model | `The contract model 'Unreal-Godot\UAL1_Standard.glb' was not found in the archive. Archive contains: ...` |
| Archive has two contract models | `The contract model 'Unreal-Godot\UAL1_Standard.glb' is ambiguous: 2 matches found (...). Refusing to guess which one is official.` |
| Archive root has no license file | `A license file was not found at the archive root '...'` |
| Archive root has two license files | `The archive root license is ambiguous: 2 candidates found (...). Refusing to guess which one is official.` |
| Staged model is not a valid GLB | `Not a glTF-binary file (magic 'this', expected 'glTF'): ...\staged-destination\UAL1_Standard.glb` — rolled back, destination untouched |
| Official archive, pinned default hash | `Result: OK` — same SHA-256 and byte size as the committed GLB |
| `-VerifyOnly` after import | `Result: OK` |

Re-running the importer on the same archive reproduced the identical SHA-256 and
byte size, under both PowerShell hosts.

The script expands only into `<repo>\.asset-import\quaternius`, stages into
`<repo>\.asset-import\staged-destination`, and parks the previous directory in
`<repo>\.asset-import\previous-destination` (all gitignored). Before removing any
of them it recomputes the full path and refuses to continue if it does not match
the expected location, or if the path is not a directory, so no broad or
caller-controlled deletion is possible.

## Conclusion

Asset qualification and integrity lock **PASSED**. Runtime loading, animation
playback, and visual acceptance are **NOT** covered by this document and remain
open.
