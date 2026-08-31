# Engine Smoke Test

**Date:** 2026-08-31  
**Task:** Phase 0 — verify local Bevy 0.19.1 toolchain with the bundled animated Fox

## Source

- Repository: `https://github.com/bevyengine/bevy`
- Tag: `v0.19.1`
- Commit: `b56fc29d3` (HEAD at tag)
- Worktree: `Q:\git\bevy-v0.19.1` (detached, not a tracked checkout of bevy-concept-world)

## Rust toolchain

```
rustc 1.98.0 (88d9e12ae 2026-08-18)
```

## Local Cargo configuration applied

`Q:\git\bevy-v0.19.1\.cargo\config.toml` (untracked / gitignored):

```toml
# Rust 1.98 cannot finalize incremental sessions on Windows ReFS Dev Drives.
# CI does not cache target artifacts, and release builds are already nonincremental.
[build]
incremental = false
```

## Commands

### Build

```powershell
Set-Location "Q:\git\bevy-v0.19.1"
cargo build --example animated_mesh_control
```

Result: `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 5m 52s`

### Run (bounded 20-second process)

```powershell
cargo run --example animated_mesh_control
# Process observed for 20 s then stopped with Stop-Process -Id <PID>
```

## Observed result

The example binary started and produced the following log lines (no panic):

```
INFO  bevy_diagnostic: SystemInfo { os: "Windows 11 Enterprise", kernel: "26200",
      cpu: "AMD EPYC 7763 64-Core Processor", core_count: "8", memory: "64.0 GiB" }
ERROR wgpu_hal::vulkan: Registry lookup failed to get ICD manifest files.
      Possibly missing Vulkan driver?
ERROR wgpu_hal::vulkan: vkCreateInstance: Found no drivers!
INFO  bevy_render::renderer: AdapterInfo { name: "Microsoft Basic Render Driver",
      vendor: 5140, device_type: Cpu, backend: Dx12, driver: "10.0.26100.8875" }
WARN  bevy_render::renderer: The selected adapter is using a driver that only
      supports software rendering. This is likely to be very slow.
WARN  bevy_audio: No audio device found.
INFO  bevy_pbr::cluster: GPU clustering is supported on this device.
INFO  bevy_render::batching: GPU preprocessing is fully supported on this device.
INFO  bevy_winit::system: Creating new window animated_mesh_control (66v0)
```

**No startup panic was observed.** The window was created and the process remained
alive with active CPU consumption (~190 s CPU time accumulated in ~20 s wall time)
until it was stopped externally.

## Scope and limitations

This run was conducted in a server environment with no physical GPU and no
display server. Bevy fell back to the Microsoft Basic Render Driver (DX12
software renderer). The window object was created and the render loop started
normally.

**Visual clip switching is NOT verified here.** This smoke test only confirms
that the engine initialises, loads the Fox asset, and enters the render loop
without panicking. Observation of the animation clips and interactive keyboard
controls remains for the final UI smoke gate, which requires a desktop session
with a GPU-accelerated display.

## Conclusion

Phase 0 engine smoke test **PASSED** (no-panic startup confirmed). The Bevy
0.19.1 toolchain is functional on this machine and the `animated_mesh_control`
example builds and runs cleanly.
