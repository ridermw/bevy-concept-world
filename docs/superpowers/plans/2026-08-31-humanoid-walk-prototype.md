# Humanoid Walk Prototype Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a standalone Bevy 0.19.1 application that validates and loads the CC0 Quaternius humanoid, then loops its named `Walk_Loop` animation in a deterministic inspection scene.

**Architecture:** Keep `Q:\git\bevy` as read-only reference material and build the application in `Q:\git\bevy-concept-world`. Bootstrap code validates checked-in provenance and integrity metadata before Bevy loads the GLB; Bevy state transitions then separate loading, scene validation, animation-player setup, running, and fatal diagnostics.

**Tech Stack:** Rust 1.98.0, Bevy 0.19.1, RON, Serde, SHA-256, PowerShell asset-import tooling, Quaternius Universal Animation Library v3 Standard.

---

## File map

| File | Responsibility |
|---|---|
| `Cargo.toml` | Pin Bevy and supporting libraries |
| `Cargo.lock` | Reproducible dependency resolution |
| `rust-toolchain.toml` | Pin Rust 1.98.0 |
| `.cargo/config.toml` | Disable incremental compilation on the Windows ReFS Dev Drive |
| `.gitignore` | Ignore Rust output and temporary import directories |
| `src/main.rs` | Assemble plugins and start the application |
| `src/lib.rs` | Export prototype modules for integration tests |
| `src/config.rs` | Parse and validate the character manifest and integrity lock |
| `src/state.rs` | Define runtime states and failure reporting |
| `src/character.rs` | Load glTF, validate named assets, spawn scene, and attach animation |
| `src/inspection.rs` | Spawn deterministic camera, lighting, floor, and orientation markers |
| `src/diagnostics.rs` | Render status/error text and handle pause/exit input |
| `assets/characters/quaternius/character.ron` | Stable runtime contract for the humanoid |
| `assets/characters/quaternius/asset.lock.ron` | Generated SHA-256 and byte-size lock |
| `assets/characters/quaternius/AnimationLibrary_Godot_Standard.glb` | Qualified humanoid, rig, and animation library |
| `assets/characters/quaternius/LICENSE.txt` | Preserved CC0 license |
| `tools/import_quaternius.ps1` | Import the official ZIP and generate the lock file |
| `tests/config_contract.rs` | Manifest, path, hash, and validation contract tests |
| `tests/app_contract.rs` | Headless state-transition and metadata-selection tests |
| `docs/validation/engine-smoke-test.md` | Record the exact tagged Bevy example command and outcome |
| `docs/validation/humanoid-smoke-test.md` | Record release run, observed orientation, and baseline measurements |
| `docs/validation/humanoid-walk.png` | Fixed-camera visual acceptance evidence |

## Scope boundary

This plan implements one stationary humanoid looping one in-place walk. It does
not implement custom concept-art meshes, retargeting, movement, collision,
root-motion extraction, animation blending, or multiple characters.

### Task 1: Verify the tagged Bevy animation example

**Files:**
- Create: `docs/validation/engine-smoke-test.md`
- Modify: `README.md`

- [ ] **Step 1: Create an isolated Bevy 0.19.1 worktree**

Run:

```powershell
git -C Q:\git\bevy worktree add --detach Q:\git\bevy-v0.19.1 v0.19.1
New-Item -ItemType Directory -Force Q:\git\bevy-v0.19.1\.cargo | Out-Null
```

Expected: Git reports that `Q:\git\bevy-v0.19.1` is prepared at tag `v0.19.1`.

- [ ] **Step 2: Add the ReFS-safe Cargo configuration to the worktree**

Create `Q:\git\bevy-v0.19.1\.cargo\config.toml`:

```toml
# Rust 1.98 cannot finalize incremental sessions on Windows ReFS Dev Drives.
# CI does not cache target artifacts, and release builds are already nonincremental.
[build]
incremental = false
```

- [ ] **Step 3: Build the relevant example**

Run:

```powershell
cargo build --manifest-path Q:\git\bevy-v0.19.1\Cargo.toml --example animated_mesh_control
```

Expected: `Finished dev profile` with exit code 0.

- [ ] **Step 4: Run the example and inspect the walk clip**

Run:

```powershell
cargo run --manifest-path Q:\git\bevy-v0.19.1\Cargo.toml --example animated_mesh_control
```

Expected: a Fox renders and animates. Press Enter once to switch from `Run` to
the named `Walk` clip, Space to pause/resume, and close the window after the
animation is confirmed.

- [ ] **Step 5: Record the smoke-test evidence**

Create `docs/validation/engine-smoke-test.md`:

```markdown
# Bevy 0.19.1 engine smoke test

- Date: 2026-08-31
- Source: `Q:\git\bevy-v0.19.1`
- Tag: `v0.19.1`
- Rust: `rustc 1.98.0`
- Build: `cargo build --manifest-path Q:\git\bevy-v0.19.1\Cargo.toml --example animated_mesh_control`
- Run: `cargo run --manifest-path Q:\git\bevy-v0.19.1\Cargo.toml --example animated_mesh_control`
- Result: Fox scene loaded; Run and Walk clips played; pause/resume worked.
```

Add to `README.md` under `## Current status`:

```markdown
- [x] Bevy 0.19.1 animated-mesh smoke test
- [ ] Quaternius humanoid imported and validated
- [ ] Standalone humanoid walk prototype
```

- [ ] **Step 6: Commit**

```powershell
git -c core.safecrlf=false add README.md docs/validation/engine-smoke-test.md
git commit -m "docs: record Bevy animation smoke test" `
  -m "Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>" `
  -m "Copilot-Session: 2d7d0fad-8c89-49c2-919a-d16440eaccb8"
```

### Task 2: Scaffold the pinned standalone application

**Files:**
- Create: `Cargo.toml`
- Create: `Cargo.lock`
- Create: `rust-toolchain.toml`
- Create: `.cargo/config.toml`
- Create: `.gitignore`
- Create: `src/main.rs`
- Create: `src/lib.rs`

- [ ] **Step 1: Write the initial package manifest**

Create `Cargo.toml`:

```toml
[package]
name = "bevy-concept-world"
version = "0.1.0"
edition = "2024"
rust-version = "1.98"
publish = false

[dependencies]
bevy = "=0.19.1"
ron = "0.10"
serde = { version = "1", features = ["derive"] }
sha2 = "0.10"
thiserror = "2"

[dev-dependencies]
tempfile = "3"

[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3
```

- [ ] **Step 2: Pin the toolchain and build behavior**

Create `rust-toolchain.toml`:

```toml
[toolchain]
channel = "1.98.0"
components = ["clippy", "rustfmt"]
profile = "minimal"
```

Create `.cargo/config.toml`:

```toml
# Rust 1.98 cannot finalize incremental sessions on Windows ReFS Dev Drives.
# CI does not cache target artifacts, and release builds are already nonincremental.
[build]
incremental = false
```

Create `.gitignore`:

```gitignore
/target/
/.superpowers/
/.asset-import/
```

- [ ] **Step 3: Add a compiling application shell**

Create `src/lib.rs`:

```rust
pub mod character;
pub mod config;
pub mod diagnostics;
pub mod inspection;
pub mod state;
```

Create `src/main.rs`:

```rust
use bevy::prelude::*;

fn main() {
    App::new().add_plugins(DefaultPlugins).run();
}
```

- [ ] **Step 4: Generate and check the lockfile**

Run:

```powershell
cargo check
```

Expected: Cargo creates `Cargo.lock` and prints `Finished dev profile`.

- [ ] **Step 5: Commit**

```powershell
git -c core.safecrlf=false add Cargo.toml Cargo.lock rust-toolchain.toml .cargo/config.toml .gitignore src
git commit -m "build: scaffold pinned Bevy application" `
  -m "Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>" `
  -m "Copilot-Session: 2d7d0fad-8c89-49c2-919a-d16440eaccb8"
```

### Task 3: Import and lock the Quaternius humanoid

**Files:**
- Create: `tools/import_quaternius.ps1`
- Create: `assets/characters/quaternius/character.ron`
- Create: `assets/characters/quaternius/asset.lock.ron`
- Create: `assets/characters/quaternius/AnimationLibrary_Godot_Standard.glb`
- Create: `assets/characters/quaternius/LICENSE.txt`

- [ ] **Step 1: Write the import tool**

Create `tools/import_quaternius.ps1`:

```powershell
param(
    [Parameter(Mandatory = $true)]
    [string]$ArchivePath
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$destination = Join-Path $repo 'assets\characters\quaternius'
$temp = Join-Path $repo '.asset-import\quaternius'

if (-not (Test-Path -LiteralPath $ArchivePath -PathType Leaf)) {
    throw "Archive not found: $ArchivePath"
}

if (Test-Path -LiteralPath $temp) {
    Remove-Item -LiteralPath $temp -Recurse -Force
}

New-Item -ItemType Directory -Force -Path $temp, $destination | Out-Null
Expand-Archive -LiteralPath $ArchivePath -DestinationPath $temp

$model = Get-ChildItem -LiteralPath $temp -Recurse -File |
    Where-Object Name -EQ 'AnimationLibrary_Godot_Standard.glb' |
    Select-Object -First 1
$license = Get-ChildItem -LiteralPath $temp -Recurse -File |
    Where-Object Name -Match '^licen[sc]e(\.txt)?$' |
    Select-Object -First 1

if (-not $model) {
    throw 'AnimationLibrary_Godot_Standard.glb was not found in the archive.'
}
if (-not $license) {
    throw 'A license file was not found in the archive.'
}

$modelTarget = Join-Path $destination $model.Name
$licenseTarget = Join-Path $destination 'LICENSE.txt'
Copy-Item -LiteralPath $model.FullName -Destination $modelTarget -Force
Copy-Item -LiteralPath $license.FullName -Destination $licenseTarget -Force

$hash = (Get-FileHash -LiteralPath $modelTarget -Algorithm SHA256).Hash.ToLowerInvariant()
$size = (Get-Item -LiteralPath $modelTarget).Length

@"
(
    sha256: "$hash",
    byte_size: $size,
)
"@ | Set-Content -LiteralPath (Join-Path $destination 'asset.lock.ron') -Encoding utf8NoBOM

Write-Output "Imported: $modelTarget"
Write-Output "SHA-256: $hash"
Write-Output "Bytes: $size"
```

- [ ] **Step 2: Download the official free Standard ZIP**

Open:

```text
https://quaternius.itch.io/universal-animation-library
```

Select **Download Now**, choose the free/name-your-own-price path, and download:

```text
Universal Animation Library[Standard].zip
```

The page must identify the pack as CC0 and the downloaded archive must contain
`AnimationLibrary_Godot_Standard.glb`.

- [ ] **Step 3: Run the importer**

Run:

```powershell
.\tools\import_quaternius.ps1 `
  -ArchivePath "$env:USERPROFILE\Downloads\Universal Animation Library[Standard].zip"
```

Expected: the script prints the imported GLB path, a 64-character lowercase
SHA-256, and a positive byte count.

- [ ] **Step 4: Add the stable character contract**

Create `assets/characters/quaternius/character.ron`:

```ron
(
    id: "quaternius-universal-animation-library-v3-standard",
    gltf_path: "characters/quaternius/AnimationLibrary_Godot_Standard.glb",
    source_url: "https://quaternius.itch.io/universal-animation-library",
    pack_version: "3.0",
    downloaded_on: "2026-08-31",
    license: "CC0-1.0",
    license_path: "characters/quaternius/LICENSE.txt",
    scene_name: "Scene",
    animation_name: "Walk_Loop",
    expected_animation_players: 1,
    scale: 1.0,
    yaw_degrees: 0.0,
    root_motion: false,
)
```

- [ ] **Step 5: Verify the imported names**

Run the Bevy scene viewer:

```powershell
cargo run --manifest-path Q:\git\bevy-v0.19.1\Cargo.toml --release `
  --example scene_viewer -- `
  Q:\git\bevy-concept-world\assets\characters\quaternius\AnimationLibrary_Godot_Standard.glb
```

Expected: one upright humanoid is visible. Confirm the asset contains scene
`Scene` and animation `Walk_Loop`. If the model faces away from the forward
marker used by the scene viewer, update only `yaw_degrees` in `character.ron`
to `180.0`.

- [ ] **Step 6: Commit**

```powershell
git -c core.safecrlf=false add tools assets/characters/quaternius
git commit -m "assets: import CC0 Quaternius humanoid" `
  -m "Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>" `
  -m "Copilot-Session: 2d7d0fad-8c89-49c2-919a-d16440eaccb8"
```

### Task 4: Implement manifest and integrity validation with TDD

**Files:**
- Create: `src/config.rs`
- Create: `tests/config_contract.rs`

- [ ] **Step 1: Write failing manifest tests**

Create `tests/config_contract.rs`:

```rust
use bevy_concept_world::config::{CharacterConfig, ConfigError, load_character_config};
use std::fs;
use tempfile::tempdir;

#[test]
fn loads_a_valid_character_contract() {
    let root = tempdir().unwrap();
    let asset_dir = root.path().join("characters/quaternius");
    fs::create_dir_all(&asset_dir).unwrap();
    fs::write(asset_dir.join("model.glb"), b"known model").unwrap();
    fs::write(asset_dir.join("LICENSE.txt"), "CC0-1.0").unwrap();
    fs::write(
        asset_dir.join("character.ron"),
        r#"(
            id: "fixture",
            gltf_path: "characters/quaternius/model.glb",
            source_url: "https://example.invalid/model",
            pack_version: "1",
            downloaded_on: "2026-08-31",
            license: "CC0-1.0",
            license_path: "characters/quaternius/LICENSE.txt",
            scene_name: "Scene",
            animation_name: "Walk_Loop",
            expected_animation_players: 1,
            scale: 1.0,
            yaw_degrees: 0.0,
            root_motion: false,
        )"#,
    )
    .unwrap();
    fs::write(
        asset_dir.join("asset.lock.ron"),
        r#"(sha256: "f5c932e75140f77efb96fb611594e19cca0719a267df98edafe8948a4a6acb63", byte_size: 11)"#,
    )
    .unwrap();

    let config = load_character_config(root.path()).unwrap();
    assert_eq!(config.animation_name, "Walk_Loop");
    assert_eq!(config.expected_animation_players, 1);
}

#[test]
fn rejects_a_changed_model() {
    let root = tempdir().unwrap();
    let asset_dir = root.path().join("characters/quaternius");
    fs::create_dir_all(&asset_dir).unwrap();
    fs::write(asset_dir.join("model.glb"), b"changed").unwrap();
    fs::write(asset_dir.join("LICENSE.txt"), "CC0-1.0").unwrap();
    fs::write(asset_dir.join("character.ron"), valid_manifest()).unwrap();
    fs::write(
        asset_dir.join("asset.lock.ron"),
        r#"(sha256: "f5c932e75140f77efb96fb611594e19cca0719a267df98edafe8948a4a6acb63", byte_size: 11)"#,
    )
    .unwrap();

    assert!(matches!(
        load_character_config(root.path()),
        Err(ConfigError::Integrity { .. })
    ));
}

#[test]
fn rejects_non_positive_scale() {
    let mut config = CharacterConfig::fixture();
    config.scale = 0.0;
    assert!(matches!(config.validate(), Err(ConfigError::InvalidScale(0.0))));
}

fn valid_manifest() -> &'static str {
    r#"(
        id: "fixture",
        gltf_path: "characters/quaternius/model.glb",
        source_url: "https://example.invalid/model",
        pack_version: "1",
        downloaded_on: "2026-08-31",
        license: "CC0-1.0",
        license_path: "characters/quaternius/LICENSE.txt",
        scene_name: "Scene",
        animation_name: "Walk_Loop",
        expected_animation_players: 1,
        scale: 1.0,
        yaw_degrees: 0.0,
        root_motion: false,
    )"#
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```powershell
cargo test --test config_contract
```

Expected: compilation fails because `config` types and functions do not exist.

- [ ] **Step 3: Implement configuration validation**

Create `src/config.rs`:

```rust
use bevy::prelude::Resource;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;

const CONFIG_PATH: &str = "characters/quaternius/character.ron";
const LOCK_PATH: &str = "characters/quaternius/asset.lock.ron";

#[derive(Resource, Debug, Clone, Deserialize)]
pub struct CharacterConfig {
    pub id: String,
    pub gltf_path: String,
    pub source_url: String,
    pub pack_version: String,
    pub downloaded_on: String,
    pub license: String,
    pub license_path: String,
    pub scene_name: String,
    pub animation_name: String,
    pub expected_animation_players: usize,
    pub scale: f32,
    pub yaw_degrees: f32,
    pub root_motion: bool,
}

#[derive(Debug, Deserialize)]
struct AssetLock {
    sha256: String,
    byte_size: u64,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse {path}: {source}")]
    Parse {
        path: PathBuf,
        source: ron::error::SpannedError,
    },
    #[error("character scale must be positive and finite, got {0}")]
    InvalidScale(f32),
    #[error("expected at least one animation player")]
    InvalidPlayerCount,
    #[error("root motion must be disabled for this prototype")]
    RootMotionEnabled,
    #[error("asset integrity mismatch: expected {expected_hash}/{expected_size}, got {actual_hash}/{actual_size}")]
    Integrity {
        expected_hash: String,
        expected_size: u64,
        actual_hash: String,
        actual_size: u64,
    },
}

impl CharacterConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if !self.scale.is_finite() || self.scale <= 0.0 {
            return Err(ConfigError::InvalidScale(self.scale));
        }
        if self.expected_animation_players == 0 {
            return Err(ConfigError::InvalidPlayerCount);
        }
        if self.root_motion {
            return Err(ConfigError::RootMotionEnabled);
        }
        Ok(())
    }

    pub fn fixture() -> Self {
        Self {
            id: "fixture".into(),
            gltf_path: "characters/quaternius/model.glb".into(),
            source_url: "https://example.invalid/model".into(),
            pack_version: "1".into(),
            downloaded_on: "2026-08-31".into(),
            license: "CC0-1.0".into(),
            license_path: "characters/quaternius/LICENSE.txt".into(),
            scene_name: "Scene".into(),
            animation_name: "Walk_Loop".into(),
            expected_animation_players: 1,
            scale: 1.0,
            yaw_degrees: 0.0,
            root_motion: false,
        }
    }
}

pub fn load_character_config(asset_root: &Path) -> Result<CharacterConfig, ConfigError> {
    let config_path = asset_root.join(CONFIG_PATH);
    let lock_path = asset_root.join(LOCK_PATH);
    let config: CharacterConfig = parse_ron(&config_path)?;
    let lock: AssetLock = parse_ron(&lock_path)?;
    config.validate()?;

    let model_path = asset_root.join(&config.gltf_path);
    let bytes = read(&model_path)?;
    let actual_hash = format!("{:x}", Sha256::digest(&bytes));
    let actual_size = bytes.len() as u64;
    if actual_hash != lock.sha256 || actual_size != lock.byte_size {
        return Err(ConfigError::Integrity {
            expected_hash: lock.sha256,
            expected_size: lock.byte_size,
            actual_hash,
            actual_size,
        });
    }

    read(&asset_root.join(&config.license_path))?;
    Ok(config)
}

fn parse_ron<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, ConfigError> {
    let text = String::from_utf8_lossy(&read(path)?).into_owned();
    ron::from_str(&text).map_err(|source| ConfigError::Parse {
        path: path.to_owned(),
        source,
    })
}

fn read(path: &Path) -> Result<Vec<u8>, ConfigError> {
    fs::read(path).map_err(|source| ConfigError::Read {
        path: path.to_owned(),
        source,
    })
}
```

- [ ] **Step 4: Run the contract tests**

Run:

```powershell
cargo test --test config_contract
```

Expected: all configuration contract tests pass.

- [ ] **Step 5: Commit**

```powershell
git -c core.safecrlf=false add src/config.rs tests/config_contract.rs
git commit -m "feat: validate humanoid asset contract" `
  -m "Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>" `
  -m "Copilot-Session: 2d7d0fad-8c89-49c2-919a-d16440eaccb8"
```

### Task 5: Add runtime states and deterministic inspection scene

**Files:**
- Create: `src/state.rs`
- Create: `src/inspection.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Define runtime state and fatal report**

Create `src/state.rs`:

```rust
use bevy::prelude::*;

#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum PrototypeState {
    #[default]
    Loading,
    Validating,
    Running,
    Failed,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct FailureReport {
    pub summary: String,
    pub details: Vec<String>,
}

pub fn fail(
    next_state: &mut NextState<PrototypeState>,
    report: &mut FailureReport,
    summary: impl Into<String>,
    details: Vec<String>,
) {
    report.summary = summary.into();
    report.details = details;
    next_state.set(PrototypeState::Failed);
}
```

- [ ] **Step 2: Implement the inspection scene**

Create `src/inspection.rs`:

```rust
use bevy::prelude::*;

pub struct InspectionPlugin;

impl Plugin for InspectionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 250.0,
        ..default()
    });

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(3.5, 2.4, 5.5).looking_at(Vec3::new(0.0, 0.9, 0.0), Vec3::Y),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 12_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -0.8,
            -0.6,
            0.0,
        )),
    ));

    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(12.0, 12.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.18, 0.20, 0.23))),
    ));

    let marker_material = materials.add(Color::srgb(0.2, 0.8, 0.3));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.04, 0.04, 1.0))),
        MeshMaterial3d(marker_material),
        Transform::from_xyz(0.0, 0.02, -0.5),
    ));

    let meter_material = materials.add(Color::srgb(0.9, 0.8, 0.2));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.04, 1.0, 0.04))),
        MeshMaterial3d(meter_material),
        Transform::from_xyz(-1.2, 0.5, 0.0),
    ));
}
```

- [ ] **Step 3: Assemble the initial application**

Replace `src/main.rs` with:

```rust
use bevy::prelude::*;
use bevy_concept_world::{
    config::load_character_config,
    inspection::InspectionPlugin,
    state::{FailureReport, PrototypeState},
};
use std::path::Path;

fn main() {
    let config = load_character_config(Path::new("assets"));
    let mut app = App::new();
    app.add_plugins((DefaultPlugins, InspectionPlugin))
        .init_resource::<FailureReport>();

    match config {
        Ok(config) => {
            app.insert_resource(config);
            app.insert_state(PrototypeState::Loading);
        }
        Err(error) => {
            app.insert_resource(FailureReport {
                summary: "Character bootstrap failed".into(),
                details: vec![error.to_string()],
            });
            app.insert_state(PrototypeState::Failed);
        }
    }

    app.run();
}
```

- [ ] **Step 4: Check compilation**

Run:

```powershell
cargo check
```

Expected: `Finished dev profile` with exit code 0.

- [ ] **Step 5: Commit**

```powershell
git -c core.safecrlf=false add src/main.rs src/state.rs src/inspection.rs
git commit -m "feat: add prototype states and inspection scene" `
  -m "Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>" `
  -m "Copilot-Session: 2d7d0fad-8c89-49c2-919a-d16440eaccb8"
```

### Task 6: Load, validate, and animate the humanoid

**Files:**
- Create: `src/character.rs`
- Create: `tests/app_contract.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write metadata-selection tests**

Create `tests/app_contract.rs`:

```rust
use bevy_concept_world::character::{DiscoveredGltf, SelectionError, validate_discovered};

#[test]
fn accepts_the_required_scene_clip_and_player_count() {
    let discovered = DiscoveredGltf {
        scenes: vec!["Scene".into()],
        animations: vec!["Idle_Loop".into(), "Walk_Loop".into()],
        animation_players: 1,
    };

    assert!(validate_discovered(&discovered, "Scene", "Walk_Loop", 1).is_ok());
}

#[test]
fn rejects_a_renamed_walk_clip() {
    let discovered = DiscoveredGltf {
        scenes: vec!["Scene".into()],
        animations: vec!["Walk".into()],
        animation_players: 1,
    };

    assert!(matches!(
        validate_discovered(&discovered, "Scene", "Walk_Loop", 1),
        Err(SelectionError::MissingAnimation { .. })
    ));
}

#[test]
fn rejects_an_unexpected_player_count() {
    let discovered = DiscoveredGltf {
        scenes: vec!["Scene".into()],
        animations: vec!["Walk_Loop".into()],
        animation_players: 2,
    };

    assert!(matches!(
        validate_discovered(&discovered, "Scene", "Walk_Loop", 1),
        Err(SelectionError::AnimationPlayerCount { .. })
    ));
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```powershell
cargo test --test app_contract
```

Expected: compilation fails because the character-selection API is absent.

- [ ] **Step 3: Implement pure metadata validation**

Start `src/character.rs` with:

```rust
use bevy::{prelude::*, world_serialization::WorldInstanceReady};
use std::time::Duration;
use thiserror::Error;

use crate::{
    config::CharacterConfig,
    state::{FailureReport, PrototypeState, fail},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredGltf {
    pub scenes: Vec<String>,
    pub animations: Vec<String>,
    pub animation_players: usize,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SelectionError {
    #[error("missing scene {expected}; discovered {discovered:?}")]
    MissingScene {
        expected: String,
        discovered: Vec<String>,
    },
    #[error("missing animation {expected}; discovered {discovered:?}")]
    MissingAnimation {
        expected: String,
        discovered: Vec<String>,
    },
    #[error("expected {expected} animation players, discovered {actual}")]
    AnimationPlayerCount { expected: usize, actual: usize },
}

pub fn validate_discovered(
    discovered: &DiscoveredGltf,
    scene: &str,
    animation: &str,
    expected_players: usize,
) -> Result<(), SelectionError> {
    if !discovered.scenes.iter().any(|name| name == scene) {
        return Err(SelectionError::MissingScene {
            expected: scene.into(),
            discovered: discovered.scenes.clone(),
        });
    }
    if !discovered.animations.iter().any(|name| name == animation) {
        return Err(SelectionError::MissingAnimation {
            expected: animation.into(),
            discovered: discovered.animations.clone(),
        });
    }
    if discovered.animation_players != expected_players {
        return Err(SelectionError::AnimationPlayerCount {
            expected: expected_players,
            actual: discovered.animation_players,
        });
    }
    Ok(())
}
```

- [ ] **Step 4: Add the Bevy character plugin**

Append to `src/character.rs`:

```rust
pub struct CharacterPlugin;

impl Plugin for CharacterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(PrototypeState::Loading), begin_loading)
            .add_systems(
                Update,
                poll_loading.run_if(in_state(PrototypeState::Loading)),
            );
    }
}

#[derive(Resource)]
struct CharacterAsset(Handle<Gltf>);

#[derive(Component)]
struct PendingCharacter {
    graph: Handle<AnimationGraph>,
    node: AnimationNodeIndex,
}

fn begin_loading(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    config: Res<CharacterConfig>,
) {
    commands.insert_resource(CharacterAsset(asset_server.load(config.gltf_path.clone())));
}

fn poll_loading(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    asset: Res<CharacterAsset>,
    config: Res<CharacterConfig>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut next_state: ResMut<NextState<PrototypeState>>,
    mut report: ResMut<FailureReport>,
) {
    if let Some(bevy::asset::LoadState::Failed(error)) = asset_server.get_load_state(&asset.0) {
        fail(
            &mut next_state,
            &mut report,
            "Character asset failed to load",
            vec![config.gltf_path.clone(), error.to_string()],
        );
        return;
    }

    if !asset_server.is_loaded_with_dependencies(&asset.0) {
        return;
    }

    let Some(gltf) = gltfs.get(&asset.0) else {
        fail(
            &mut next_state,
            &mut report,
            "Loaded glTF is unavailable",
            vec![config.gltf_path.clone()],
        );
        return;
    };

    let scenes = gltf.named_scenes.keys().cloned().collect::<Vec<_>>();
    let animations = gltf.named_animations.keys().cloned().collect::<Vec<_>>();
    let discovered = DiscoveredGltf {
        scenes,
        animations,
        animation_players: config.expected_animation_players,
    };

    if let Err(error) = validate_discovered(
        &discovered,
        &config.scene_name,
        &config.animation_name,
        config.expected_animation_players,
    ) {
        fail(
            &mut next_state,
            &mut report,
            "Character contract validation failed",
            vec![error.to_string()],
        );
        return;
    }

    let scene = gltf.named_scenes[&config.scene_name].clone();
    let clip = gltf.named_animations[&config.animation_name].clone();
    let (graph, node) = AnimationGraph::from_clip(clip);
    let graph = graphs.add(graph);
    let transform = Transform::from_scale(Vec3::splat(config.scale))
        .with_rotation(Quat::from_rotation_y(config.yaw_degrees.to_radians()));

    commands
        .spawn((WorldAssetRoot(scene), transform, PendingCharacter { graph, node }))
        .observe(start_animation);
    next_state.set(PrototypeState::Validating);
}

fn start_animation(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    pending: Query<&PendingCharacter>,
    mut players: Query<&mut AnimationPlayer>,
    config: Res<CharacterConfig>,
    mut next_state: ResMut<NextState<PrototypeState>>,
    mut report: ResMut<FailureReport>,
) {
    let Ok(pending) = pending.get(ready.entity) else {
        return;
    };

    let player_entities = children
        .iter_descendants(ready.entity)
        .filter(|entity| players.contains(*entity))
        .collect::<Vec<_>>();

    let discovered = DiscoveredGltf {
        scenes: vec![config.scene_name.clone()],
        animations: vec![config.animation_name.clone()],
        animation_players: player_entities.len(),
    };
    if let Err(error) = validate_discovered(
        &discovered,
        &config.scene_name,
        &config.animation_name,
        config.expected_animation_players,
    ) {
        fail(
            &mut next_state,
            &mut report,
            "Spawned scene validation failed",
            vec![error.to_string()],
        );
        return;
    }

    for entity in player_entities {
        let mut player = players.get_mut(entity).unwrap();
        let mut transitions = AnimationTransitions::new();
        transitions
            .play(&mut player, pending.node, Duration::ZERO)
            .repeat();
        commands.entity(entity).insert((
            AnimationGraphHandle(pending.graph.clone()),
            transitions,
        ));
    }

    next_state.set(PrototypeState::Running);
}
```

- [ ] **Step 5: Register the plugin**

In `src/main.rs`, import `character::CharacterPlugin` and change the plugin
tuple to:

```rust
app.add_plugins((DefaultPlugins, InspectionPlugin, CharacterPlugin))
```

- [ ] **Step 6: Run tests and compile**

Run:

```powershell
cargo test --test app_contract
cargo check
```

Expected: all app-contract tests pass and the binary compiles.

- [ ] **Step 7: Commit**

```powershell
git -c core.safecrlf=false add src/main.rs src/character.rs tests/app_contract.rs
git commit -m "feat: load and animate validated humanoid" `
  -m "Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>" `
  -m "Copilot-Session: 2d7d0fad-8c89-49c2-919a-d16440eaccb8"
```

### Task 7: Add diagnostics and controls

**Files:**
- Create: `src/diagnostics.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Implement status overlay and controls**

Create `src/diagnostics.rs`:

```rust
use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
};

use crate::{
    config::CharacterConfig,
    state::{FailureReport, PrototypeState},
};

pub struct DiagnosticsPlugin;

impl Plugin for DiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_overlay)
            .add_systems(Update, (update_overlay, handle_controls));
    }
}

#[derive(Component)]
struct StatusText;

fn spawn_overlay(mut commands: Commands) {
    commands.spawn((
        Text::new("Starting humanoid prototype..."),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            left: px(12),
            ..default()
        },
        StatusText,
    ));
}

fn update_overlay(
    state: Res<State<PrototypeState>>,
    config: Option<Res<CharacterConfig>>,
    report: Res<FailureReport>,
    mut text: Single<&mut Text, With<StatusText>>,
) {
    let details = match state.get() {
        PrototypeState::Failed => {
            format!("{}\n{}", report.summary, report.details.join("\n"))
        }
        _ => config
            .as_ref()
            .map(|config| {
                format!(
                    "State: {:?}\nAsset: {}\nClip: {}\nSpace: pause/resume\nEsc: exit",
                    state.get(),
                    config.id,
                    config.animation_name
                )
            })
            .unwrap_or_else(|| format!("State: {:?}", state.get())),
    };
    text.0 = details;
}

fn handle_controls(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut players: Query<&mut AnimationPlayer>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
    if keys.just_pressed(KeyCode::Space) {
        for mut player in &mut players {
            if player.all_paused() {
                player.resume_all();
            } else {
                player.pause_all();
            }
        }
    }
    if keys.just_pressed(KeyCode::KeyP) {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk("docs/validation/humanoid-walk.png"));
    }
}
```

- [ ] **Step 2: Register diagnostics**

In `src/main.rs`, import `diagnostics::DiagnosticsPlugin` and use:

```rust
app.add_plugins((
    DefaultPlugins,
    InspectionPlugin,
    CharacterPlugin,
    DiagnosticsPlugin,
))
```

- [ ] **Step 3: Check and run**

Run:

```powershell
cargo check
cargo run --release
```

Expected: the window shows the humanoid looping `Walk_Loop`, the overlay reaches
`Running`, Space pauses/resumes, P saves
`docs/validation/humanoid-walk.png`, and Escape exits.

- [ ] **Step 4: Commit**

```powershell
git -c core.safecrlf=false add src/main.rs src/diagnostics.rs
git commit -m "feat: add humanoid diagnostics and controls" `
  -m "Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>" `
  -m "Copilot-Session: 2d7d0fad-8c89-49c2-919a-d16440eaccb8"
```

### Task 8: Verify failure paths and complete documentation

**Files:**
- Modify: `tests/config_contract.rs`
- Modify: `tests/app_contract.rs`
- Modify: `README.md`
- Create: `docs/validation/humanoid-smoke-test.md`

- [ ] **Step 1: Add remaining contract tests**

Append to `tests/config_contract.rs`:

```rust
#[test]
fn rejects_missing_license_file() {
    let root = tempdir().unwrap();
    let asset_dir = root.path().join("characters/quaternius");
    fs::create_dir_all(&asset_dir).unwrap();
    fs::write(asset_dir.join("model.glb"), b"known model").unwrap();
    fs::write(asset_dir.join("character.ron"), valid_manifest()).unwrap();
    fs::write(
        asset_dir.join("asset.lock.ron"),
        r#"(sha256: "f5c932e75140f77efb96fb611594e19cca0719a267df98edafe8948a4a6acb63", byte_size: 11)"#,
    )
    .unwrap();

    let error = load_character_config(root.path()).unwrap_err();
    match error {
        ConfigError::Read { path, .. } => {
            assert!(path.ends_with("characters/quaternius/LICENSE.txt"));
        }
        other => panic!("expected missing-license read error, got {other}"),
    }
}

#[test]
fn rejects_root_motion_for_the_first_milestone() {
    let mut config = CharacterConfig::fixture();
    config.root_motion = true;
    assert!(matches!(
        config.validate(),
        Err(ConfigError::RootMotionEnabled)
    ));
}
```

Append to `tests/app_contract.rs`:

```rust
#[test]
fn reports_discovered_animation_names() {
    let discovered = DiscoveredGltf {
        scenes: vec!["Scene".into()],
        animations: vec!["Idle_Loop".into(), "Jog_Fwd_Loop".into()],
        animation_players: 1,
    };
    let error = validate_discovered(&discovered, "Scene", "Walk_Loop", 1).unwrap_err();
    assert!(error.to_string().contains("Jog_Fwd_Loop"));
}
```

- [ ] **Step 2: Run targeted tests**

Run:

```powershell
cargo test --test config_contract --test app_contract
```

Expected: all contract tests pass.

- [ ] **Step 3: Exercise the integrity failure**

Run:

```powershell
$asset = 'assets\characters\quaternius\AnimationLibrary_Godot_Standard.glb'
$backup = "$asset.bak"
Copy-Item -LiteralPath $asset -Destination $backup
Add-Content -LiteralPath $asset -Value 'integrity-test'
cargo run
Move-Item -LiteralPath $backup -Destination $asset -Force
```

Expected: the application enters `Failed` and displays the expected and actual
hash/size. The original GLB is restored before continuing.

- [ ] **Step 4: Run the complete local quality gate**

Run each command:

```powershell
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo run --release
```

Expected: formatting, Clippy, and tests pass. The release application reaches
`Running`, displays an upright approximately 1.83-meter humanoid, loops
`Walk_Loop` without translating its root, pauses/resumes, and exits cleanly.

- [ ] **Step 5: Record the verified result**

While the release application is in `Running`, press P once.

Expected: `docs/validation/humanoid-walk.png` contains the fixed-camera
humanoid walk scene.

Create `docs/validation/humanoid-smoke-test.md`:

```markdown
# Humanoid walk smoke test

- Date: 2026-08-31
- Bevy: 0.19.1
- Rust: 1.98.0
- Asset: Quaternius Universal Animation Library v3 Standard
- Scene: `Scene`
- Clip: `Walk_Loop`
- Locomotion: in-place
- Result: application reached `Running`; humanoid was upright, forward-facing,
  correctly scaled, continuously animated, stationary at the root, and
  responsive to pause/resume.
- Quality gate: `cargo fmt --check`, Clippy with warnings denied, `cargo test`,
  and release smoke run passed.
```

Update `README.md`:

````markdown
## Run

```powershell
cargo run --release
```

Space pauses or resumes the walk animation. Escape exits.

## Status

- [x] Bevy 0.19.1 animated-mesh smoke test
- [x] Quaternius humanoid imported and integrity-locked
- [x] Standalone humanoid loops `Walk_Loop`
- [ ] Replace the reference mesh with the first concept-art-derived mesh
````

- [ ] **Step 6: Commit**

```powershell
git -c core.safecrlf=false add README.md docs/validation tests
git commit -m "test: verify humanoid walk prototype" `
  -m "Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>" `
  -m "Copilot-Session: 2d7d0fad-8c89-49c2-919a-d16440eaccb8"
```

### Task 9: Final diff and repository check

**Files:**
- Verify all files in the repository

- [ ] **Step 1: Inspect repository state**

Run:

```powershell
git status --short
git --no-pager log --oneline --decorate -10
```

Expected: the worktree is clean and the history contains small commits for the
smoke test, scaffold, asset import, manifest validation, runtime loading,
diagnostics, and final verification.

- [ ] **Step 2: Confirm no upstream Bevy files changed**

Run:

```powershell
git -C Q:\git\bevy status --short
git -C Q:\git\bevy-v0.19.1 status --short
```

Expected: `Q:\git\bevy` is clean, and the tagged worktree contains only its
local `.cargo/config.toml`. No tracked upstream files are modified.

- [ ] **Step 3: Preserve the next milestone boundary**

Do not begin custom modeling or retargeting in this implementation. The next
specification begins only after this repository has a verified humanoid walk
baseline.
