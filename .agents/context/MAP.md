# MARL Repo Map

## Cargo Workspace
- `Cargo.toml`: virtual workspace manifest with `marl-engine`, `marl-viewer-rs`, and `marl-format` members
- `crates/marl-engine/Cargo.toml`: simulation library plus `marl-engine` binary; optional `gpu` feature
- `crates/marl-viewer-rs/Cargo.toml`: standalone viewer binary and windowing/GPU render dependencies
- `crates/marl-format/Cargo.toml`: small shared format/schema crate

## Core Simulation
- `crates/marl-engine/src/main.rs`: engine binary orchestration, tick loop, output cadence, CLI flag handling for GPU diffusion
- `crates/marl-engine/src/sim/`: extracted simulation helpers for seeding, spatial neighbor logic, starter metabolisms, and stats printing
- `crates/marl-engine/src/config.rs`: compile-time dimensions/species counts plus runtime `SimulationConfig` and `OutputConfig`
- `crates/marl-engine/src/field.rs`: extracellular field storage, CPU diffusion, boundary sources, field tests
- `crates/marl-engine/src/cell.rs`: cell state, rulesets, reactions, transport, fate, mutation
- `crates/marl-engine/src/light.rs`: top-down light attenuation field
- `crates/marl-engine/src/hgt.rs`: horizontal gene transfer helper, not currently wired into the main loop

## Output And Viewer Data
- `crates/marl-engine/src/binary_dump.rs`: raw viewer files (`tick_<T>.field.bin`, `tick_<T>.cells.bin`) and `run_meta.json`
- `crates/marl-format/`: shared binary schema constants, `RunMeta`, field byte-length helper, and packed `ViewerCellRecord`
- `crates/marl-engine/src/data.rs`: optional CSV diagnostics, reaction registry, end-of-run `summary.md`
- `crates/marl-engine/src/snapshot.rs`: optional PPM cross-sections, density maps, and ancestry maps
- `marl.toml`: sample runtime config with binary outputs on and legacy diagnostics off by default

## Standalone Viewer
- `crates/marl-viewer-rs/src/main.rs`: thin viewer binary entrypoint
- `crates/marl-viewer-rs/src/args.rs`: viewer CLI parsing
- `crates/marl-viewer-rs/src/io.rs`: `run_meta.json` and field snapshot loading
- `crates/marl-viewer-rs/src/renderer.rs`: `wgpu` renderer and 3D texture upload
- `crates/marl-viewer-rs/src/app.rs`: `winit` application/event loop handling
- `crates/marl-viewer-rs/src/viewer_raymarch.wgsl`: full-screen raymarch shader for a selected external species in the packed 3D field texture

## GPU Prototype
- `crates/marl-engine/src/gpu/`: optional `gpu` feature implementation for field diffusion
- `crates/marl-engine/src/gpu/shaders/field_diffuse.wgsl`: WGSL compute shader with currently duplicated grid/species constants
- `crates/marl-engine/tests/gpu_diffusion.rs`: CPU/GPU equivalence tests compiled when `gpu` feature is enabled

## Documentation And Agent Context
- `README.md`: user-facing overview, run commands, config and output documentation
- `.agents/plans/`: implementation plans
- `.agents/context/STATUS.md`: current project status and completed work
- `.agents/context/NOTES.md`: durable implementation decisions and gotchas
- `.agents/context/MAP.md`: this concise structure map
- `scripts/check_binary_dump.py`: quick binary output sanity checker
