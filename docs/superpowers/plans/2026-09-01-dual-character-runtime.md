# Dual Character Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver a playable Bevy 0.19.1 vertical slice with one stable locomotion root and two resident, validated, synchronized character visuals that toggle instantly.

**Architecture:** Bootstrap inserts a validated `CharacterCatalog`. `CharacterPlugin` preloads and validates both catalog entries, creates one identity-transform `Humanoid` parent, and spawns one tagged visual scene root per variant beneath it. Each visual owns its manifest transform and animation graph; shared locomotion, turnaround, and camera tracking remain on the parent.

**Tech Stack:** Rust 1.98, Bevy 0.19.1 ECS/state/animation/world serialization, integration tests with headless `App`/`World`.

---

### Task 1: Bootstrap the complete catalog

**Files:**
- Modify: `src/main.rs`
- Modify: `tests/config_contract.rs`

- [x] **Step 1: Write the failing catalog contract test**

Add a behavioral contract that loads the checked-in catalog and requires both advertised variants:

```rust
#[test]
fn the_checked_in_character_catalog_contains_reference_and_technician() {
    let asset_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let catalog =
        load_character_catalog(&asset_root).expect("both checked-in character contracts must load");

    assert_eq!(
        catalog.reference.id,
        "quaternius-universal-animation-library-v3-standard"
    );
    assert_eq!(
        catalog.technician_man.id,
        "midcreek-cel-shift-technician-man"
    );
    assert_ne!(catalog.reference.gltf_path, catalog.technician_man.gltf_path);
}
```

- [x] **Step 2: Run the test to verify RED**

Run: `cargo test --test config_contract the_checked_in_character_catalog_contains_reference_and_technician -- --exact`

Expected: FAIL before catalog loading and the technician contract exist.

- [x] **Step 3: Switch bootstrap to the catalog**

Replace the single-config import/load/match arm with:

```rust
config::load_character_catalog,
```

```rust
let catalog = asset_root
    .as_ref()
    .ok()
    .map(|root| load_character_catalog(&root.path));
```

```rust
(_, Ok(_), Some(Ok(catalog))) => {
    app.insert_resource(catalog);
    None
}
```

Keep the existing first-failure ordering and `FailureReport`/`Failed` behavior.

- [x] **Step 4: Run the test to verify GREEN**

Run: `cargo test --test config_contract the_checked_in_character_catalog_contains_reference_and_technician -- --exact`

Expected: PASS.

- [x] **Step 5: Commit**

Commit the bootstrap/catalog foundation together with the already completed catalog and generated-asset changes.

### Task 2: Model dual load and readiness decisions

**Files:**
- Modify: `src/character.rs`
- Modify: `tests/runtime_contract.rs`

- [x] **Step 1: Write failing pure readiness tests**

Add tests requiring both variants before readiness and preserving variant identity in failures:

```rust
#[test]
fn both_variants_must_be_ready_before_validation_can_begin() {
    assert_eq!(
        evaluate_catalog_load([
            (CharacterVariant::Reference, LoadOutcome::Ready),
            (CharacterVariant::TechnicianMan, LoadOutcome::Waiting),
        ]),
        CatalogLoadOutcome::Waiting,
    );
}

#[test]
fn either_variant_failure_blocks_the_catalog() {
    assert_eq!(
        evaluate_catalog_load([
            (CharacterVariant::Reference, LoadOutcome::Ready),
            (CharacterVariant::TechnicianMan, LoadOutcome::Failed),
        ]),
        CatalogLoadOutcome::Failed(CharacterVariant::TechnicianMan),
    );
}

#[test]
fn both_spawned_variants_must_validate_before_running() {
    let mut readiness = VariantReadiness::default();
    readiness.mark_ready(CharacterVariant::Reference, 1);
    assert!(!readiness.all_ready());
    readiness.mark_ready(CharacterVariant::TechnicianMan, 1);
    assert!(readiness.all_ready());
}
```

- [x] **Step 2: Run the tests to verify RED**

Run: `cargo test --test runtime_contract both_variants_must_be_ready_before_validation_can_begin either_variant_failure_blocks_the_catalog both_spawned_variants_must_validate_before_running`

Expected: compile failure because catalog-level load/readiness APIs do not exist.

- [x] **Step 3: Implement minimal catalog decisions**

Add public testable types:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogLoadOutcome {
    Waiting,
    Ready,
    Failed(CharacterVariant),
    TimedOut(CharacterVariant),
}

#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct VariantReadiness {
    reference_players: Option<usize>,
    technician_players: Option<usize>,
}
```

Implement `evaluate_catalog_load`, `mark_ready`, `players`, and `all_ready` without hiding a failed variant.

- [x] **Step 4: Run the tests to verify GREEN**

Run the same `runtime_contract` selectors and expect PASS.

- [x] **Step 5: Commit**

Commit the catalog-level load/readiness decision model.

### Task 3: Spawn one stable root with two visual children

**Files:**
- Modify: `src/character.rs`
- Modify: `src/locomotion.rs`
- Modify: `tests/app_contract.rs`
- Modify: `tests/locomotion_contract.rs`

- [x] **Step 1: Write failing headless hierarchy and locomotion tests**

Build a headless app/world fixture and assert:

```rust
assert_eq!(humanoids.iter().count(), 1);
assert_eq!(variants.iter().count(), 2);
assert!(children.iter_descendants(humanoid).any(|entity| entity == reference));
assert!(children.iter_descendants(humanoid).any(|entity| entity == technician));
assert_eq!(*world.entity(humanoid).get::<Transform>().unwrap(), Transform::IDENTITY);
assert_eq!(
    world.entity(reference).get::<Transform>().unwrap().scale,
    Vec3::splat(catalog.reference.scale)
);
assert_eq!(
    world.entity(technician).get::<Transform>().unwrap().scale,
    Vec3::splat(catalog.technician_man.scale)
);
```

Adapt locomotion tests to insert `CharacterCatalog`, give the shared root an identity transform, and assert movement changes only parent translation/heading while child local transforms remain unchanged.

- [x] **Step 2: Run the tests to verify RED**

Run: `cargo test --test app_contract stable_humanoid_root_owns_both_visual_variants -- --exact`

Run: `cargo test --test locomotion_contract locomotion_changes_only_the_shared_root -- --exact`

Expected: FAIL because the current scene root is also the moving/scaled humanoid and only one scene exists.

- [x] **Step 3: Implement stable-root spawning**

Refactor runtime resources around two variant records:

```rust
struct PreparedVariant {
    variant: CharacterVariant,
    scene: Handle<WorldAsset>,
    graph: Handle<AnimationGraph>,
    node: AnimationNodeIndex,
}

#[derive(Component)]
struct PendingVariant {
    variant: CharacterVariant,
    graph: Handle<AnimationGraph>,
    node: AnimationNodeIndex,
}
```

On entering `Validating`, spawn:

```rust
let humanoid = commands
    .spawn((
        Name::new("Humanoid"),
        Humanoid,
        HumanoidController::default(),
        Transform::IDENTITY,
    ))
    .id();
```

Then spawn both `WorldAssetRoot` entities as children of `humanoid`, tag each with `CharacterVariant`, apply `character_transform(config.scale, config.yaw_degrees)`, and attach the ready observer.

Remove all `CharacterConfig` resource gates from locomotion. Set parent rotation only from controller heading:

```rust
transform.rotation = Quat::from_rotation_y(update.heading);
transform.translation += update.translation;
```

- [x] **Step 4: Run the tests to verify GREEN**

Run both targeted integration tests and expect PASS.

- [x] **Step 5: Commit**

Commit the stable-root/dual-visual hierarchy and locomotion adaptation.

### Task 4: Validate and start both players in phase

**Files:**
- Modify: `src/character.rs`
- Modify: `tests/app_contract.rs`
- Modify: `tests/runtime_contract.rs`

- [x] **Step 1: Write failing player wiring tests**

Add a headless world test with two tagged scene roots and one real `AnimationPlayer` descendant each. Require each player to receive its own `AnimationGraphHandle` and `AnimationTransitions`, require both to report a playing animation, and require `Running` only after the second successful validation.

```rust
assert_eq!(readiness.players(CharacterVariant::Reference), Some(1));
assert_eq!(readiness.players(CharacterVariant::TechnicianMan), Some(1));
assert!(readiness.all_ready());
assert!(reference_player.playing_animations().next().is_some());
assert!(technician_player.playing_animations().next().is_some());
```

Also add a failure test where one variant has zero or two players and assert the failure details name that variant and prevent `Running`.

- [x] **Step 2: Run the tests to verify RED**

Run the new exact selectors. Expected: FAIL because the first ready hierarchy currently enters `Running`.

- [x] **Step 3: Wire each variant and synchronize start**

When each `WorldInstanceReady` arrives, discover and validate only that variant's descendants. Store its actual player entities in a resource. Do not play either graph yet. Once both variants have validated, iterate both stored player sets in one observer execution, insert their own graph handles/transitions, call `play(..., Duration::ZERO).repeat()` for every player, mark both ready, remove pending markers, and set `Running`.

If either count mismatches, call `fail` with variant label, scene, path, expected count, and actual count. Never continue to `Running`.

- [x] **Step 4: Run the tests to verify GREEN**

Run the targeted tests and expect PASS.

- [x] **Step 5: Commit**

Commit dual validation and synchronized animation startup.

### Task 5: Toggle visibility without disturbing runtime state

**Files:**
- Modify: `src/character.rs`
- Modify: `src/diagnostics.rs`
- Modify: `tests/app_contract.rs`

- [x] **Step 1: Write failing visibility/state preservation tests**

Create a headless running app with one parent and two variant children. Record parent `Transform`, `HumanoidController`, orbit `Transform`/`OrbitCamera`, and both players' animation seek times. Press Tab and update once. Assert:

```rust
assert_eq!(selection.active(), CharacterVariant::TechnicianMan);
assert_eq!(*reference_visibility, Visibility::Hidden);
assert_eq!(*technician_visibility, Visibility::Inherited);
assert_eq!(parent_transform_before, parent_transform_after);
assert_eq!(controller_before, controller_after);
assert_eq!(orbit_before, orbit_after);
assert_eq!(reference_phase_before, reference_phase_after);
assert_eq!(technician_phase_before, technician_phase_after);
```

Also assert the default state has Reference visible and Technician hidden, with exactly one visible variant.

- [x] **Step 2: Run the tests to verify RED**

Run the new exact selectors. Expected: FAIL because `handle_controls` does not mutate selection or visibility.

- [x] **Step 3: Implement selection visibility**

Initialize `CharacterSelection` in `CharacterPlugin`. Spawn reference with `Visibility::Inherited` and technician with `Visibility::Hidden`. Add a testable helper:

```rust
pub fn variant_visibility(active: CharacterVariant, variant: CharacterVariant) -> Visibility {
    if active == variant {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}
```

Handle `toggle_model` by mutating only `CharacterSelection` and the two tagged entities' `Visibility`. Do not respawn, seek, pause, rewrite transforms, or replace controllers.

- [x] **Step 4: Run the tests to verify GREEN**

Run the targeted visibility/preservation tests and expect PASS.

- [x] **Step 5: Commit**

Commit instantaneous model selection.

### Task 6: Report per-variant diagnostics and preserve global controls

**Files:**
- Modify: `src/diagnostics.rs`
- Modify: `tests/app_contract.rs`

- [x] **Step 1: Write failing overlay tests**

Extract a pure overlay summary helper or run `update_overlay` in a headless app. Require lines for:

```text
Active model: Quaternius reference
Quaternius reference: ready, players 1/1
Midcreek technician - man: ready, players 1/1
```

Keep the existing control assertions for Space, P, Tab, and Escape. Add a test proving pause/resume changes both players.

- [x] **Step 2: Run the tests to verify RED**

Run the new exact selectors. Expected: FAIL because diagnostics still read one `CharacterConfig` and do not report selection/readiness per variant.

- [x] **Step 3: Implement catalog-aware diagnostics**

Read optional `CharacterCatalog`, `CharacterSelection`, and `VariantReadiness`. Render active model and each variant's asset/readiness/player count. Keep querying all `AnimationPlayer` entities so pause/resume affects both resident players. Keep screenshot handling on `P`.

- [x] **Step 4: Run the tests to verify GREEN**

Run targeted diagnostics/control tests and expect PASS.

- [x] **Step 5: Commit**

Commit dual-model diagnostics.

### Task 7: Full verification and final review

**Files:**
- Modify only files needed to correct failures found by the gates.

- [x] **Step 1: Run formatting**

Run: `cargo fmt --all -- --check`

If it fails, run `cargo fmt --all`, then repeat the check.

- [x] **Step 2: Run Clippy with warnings denied**

Run:

```text
cargo clippy --all-targets -- -D warnings
```

Expected: PASS with no warnings. Bevy systems that legitimately exceed seven
parameters use the repository's established targeted
`#[allow(clippy::too_many_arguments)]` convention.

- [x] **Step 3: Run the full test suite**

Run:

```text
cargo test
```

Expected: every suite passes. Final result: 187 passed, 0 failed.

- [x] **Step 4: Compile all targets and release**

Run:

```text
cargo check --all-targets
cargo build --release
```

Expected: PASS with no compile errors.

- [x] **Step 5: Prove generated-asset determinism**

Run the Blender 5.2.1 LTS generator twice into isolated outputs and compare
SHA-256 plus byte size before replacing the checked-in asset.

Expected and observed for both outputs:

```text
sha256 2870e6293b8d3af3c4dfa45c8e476f07cf64ec9d6b3569017abc498ef746c79d
byte_size 3425968
```

- [x] **Step 6: Run the bounded release screenshot smoke**

Run:

```powershell
$env:HUMANOID_WALK_CAPTURE_SECONDS = '3'
.\target\release\bevy-concept-world.exe
```

Observed: reached `Running`, both variants reported ready with one player each,
both loops started in phase, active model was Reference, screenshot verification
reported 232,688 bytes, and the log contained no B0004 warning. The Microsoft
Basic Render Driver produced no 3D scene and process shutdown hung after capture
verification, so the blank new image was rejected and the existing valid visual
acceptance screenshot was restored.

- [x] **Step 7: Self-review the final diff**

Confirm:

```text
- bootstrap inserts CharacterCatalog
- either advertised contract/load/hierarchy failure prevents Running
- exactly one Humanoid root exists
- both variant scene roots remain resident
- both players start together and keep animating
- only Visibility and CharacterSelection change on Tab
- locomotion/camera target the stable parent only
- diagnostics identify active model and both readiness/player states
- P remains screenshot and Space affects both players
- generated asset and lock are included
```

- [x] **Step 8: Commit final corrections**

Commit any gate-driven corrections with:

```text
Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>
```
