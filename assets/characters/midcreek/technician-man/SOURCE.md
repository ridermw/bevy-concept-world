# Midcreek Cel Shift male technician

This vertical-slice character is derived from the original Midcreek Cel Shift
concept artwork in the local `midcreek-concept` repository:

- `themes/cel-shift/masters/character-man/01-sheet.png`
- `themes/cel-shift/masters/character-man/02-facings.png`
- `themes/cel-shift/masters/character-man/03-scale.png`
- `themes/cel-shift/masters/character-man/04-work-poses.png`
- `themes/cel-shift/masters/character-man/05-silhouette.png`
- `themes/cel-shift/masters/character-both/01-pair.png`

The visible mesh is an original low-detail blockout generated for this
repository. It reuses the armature and animations from the CC0 Quaternius
Universal Animation Library v3.0 Standard asset already preserved under
`assets/characters/quaternius`.

The concept contract fixes the male technician at 1.73 m with a clean-shaven
face, short dark hair, long-sleeved slate work shirt, lime high-visibility vest
with orange trim and broad silver bands, blue denim, dark boots, blue hard hat,
ear defenders, corded orange ear plugs, and a tool belt.

## Provenance and license

The Midcreek concept direction and the generated visible modules are original
project work. The source armature, hidden contract mesh, and animation actions
come from the Quaternius Universal Animation Library v3.0 Standard asset in
`assets/characters/quaternius/UAL1_Standard.glb`, preserved under CC0-1.0.
`character.ron` records both parts of that provenance and points back to this
file as the license/source note.

## Deterministic regeneration

The accepted toolchain is **Blender 5.2.1 LTS** (build hash
`9e2066aef7ef`, built 2026-08-25) with the bundled Khronos glTF Blender I/O
exporter **v5.2.40**. Run this exact command in Windows PowerShell from the
repository root:

```powershell
& 'C:\Program Files\Blender Foundation\Blender 5.2\blender.exe' `
  --background --factory-startup `
  --python .\tools\generate_midcreek_technician.py -- `
  --source .\assets\characters\quaternius\UAL1_Standard.glb `
  --output .\assets\characters\midcreek\technician-man\technician-man.glb
```

The generator writes one output:

```text
assets/characters/midcreek/technician-man/technician-man.glb
```

The visible rest-pose design is 1.73 m from the boot soles at ground level to
the hard-hat crown. The generator uses solid-color materials and removes the
generated modules' unused UV layers. Blender's bevel UV interpolation produced
small run-to-run float differences even though the rendered geometry was
unchanged; removing those unused attributes makes the GLB byte-for-byte
deterministic.

Do not overwrite the checked-in GLB while establishing determinism. Generate
two isolated candidates under the gitignored `.asset-import` directory:

```powershell
$blender = 'C:\Program Files\Blender Foundation\Blender 5.2\blender.exe'
$source = '.\assets\characters\quaternius\UAL1_Standard.glb'
$first = '.\.asset-import\technician-man-run1.glb'
$second = '.\.asset-import\technician-man-run2.glb'

& $blender --background --factory-startup `
  --python .\tools\generate_midcreek_technician.py -- `
  --source $source --output $first
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

& $blender --background --factory-startup `
  --python .\tools\generate_midcreek_technician.py -- `
  --source $source --output $second
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Get-FileHash $first, $second -Algorithm SHA256
Get-Item $first, $second | Select-Object FullName, Length
```

Both hashes and both byte sizes must match before either candidate is copied
over the checked-in asset. The accepted Blender 5.2.1 LTS output is:

```text
SHA-256 ee9ba685ee57a26fe08fb1b54d6b8d436268f4979713b58226a078a32449fd51
byte size 3,424,928
```

After copying the stable candidate to `technician-man.glb`, calculate the lock
values:

```powershell
$asset = '.\assets\characters\midcreek\technician-man\technician-man.glb'
$sha256 = (Get-FileHash $asset -Algorithm SHA256).Hash.ToLowerInvariant()
$byteSize = (Get-Item $asset).Length
"sha256=$sha256"
"byte_size=$byteSize"
```

Update the `sha256` and `byte_size` fields in `asset.lock.ron`, then verify that
the loader independently re-hashes the checked-in bytes:

```powershell
cargo test --test config_contract `
  validates_the_real_midcreek_technician_contract -- --exact
cargo test --test app_contract `
  the_midcreek_technician_glb_contains_the_required_visual_modules_only -- --exact
```

## Vertical-slice limitations

- The visible body and equipment are low-detail rigid modules parented to
  bones, not production topology with final skin weights.
- The source Quaternius mesh remains tiny and below the scene as the hidden
  skinned contract mesh that causes Bevy to instantiate the expected
  `AnimationPlayer`.
- The GLB retains the source animation library; the runtime contract selects
  only `Walk_Loop`.
- Materials are flat solid colors with no texture workflow. Unused generated
  UVs are intentionally stripped for deterministic export.
- Silhouette, equipment recognition, clipping, deformation quality, and loop
  appearance still require acceptance in a GPU-accelerated live Bevy run.
