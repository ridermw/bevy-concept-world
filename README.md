# Bevy Concept World

A staged prototype for bringing concept-driven humanoid characters into a 3D Bevy world.

## Current status

Design is complete for the first vertical slice:

- [x] Build gate: Bevy 0.19.1 toolchain builds and starts `animated_mesh_control` without panic.
      See [`docs/validation/engine-smoke-test.md`](docs/validation/engine-smoke-test.md).
- [ ] Visual gate: Confirm animated Fox renders and clips switch in a GPU-accelerated desktop session. *(deferred — requires display)*
- [ ] Qualify a CC0 Quaternius humanoid and in-place walk animation.
- [ ] Load and loop that walk in a standalone Bevy application.

Implementation has started: the engine startup smoke gate has passed. Visual Fox animation
confirmation is deferred pending a GPU-accelerated desktop session. Humanoid items have
not started. See
[`docs/superpowers/specs/2026-08-31-humanoid-walk-prototype-design.md`](docs/superpowers/specs/2026-08-31-humanoid-walk-prototype-design.md).
