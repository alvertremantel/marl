# MARL Repo Map

## Core Simulation
- `src/main.rs`: orchestration, seeding, tick loop, output cadence, CLI flag handling for GPU diffusion
- `src/config.rs`: compile-time dimensions/species counts plus runtime `SimulationConfig` and `OutputConfig`
- `src/field.rs`: extracellular field storage, CPU diffusion, boundary sources, field tests
- `src/cell.rs`: cell state, rulesets, reactions, transport, fate, mutation
- `src/light.rs`: top-down light attenuation field
- `src/hgt.rs`: horizontal gene transfer helper, not currently wired into the main loop

## Output And Viewer Data
- `src/binary_dump.rs`: raw viewer files (`tick_<T>.field.bin`, `tick_<T>.cells.bin`) and `run_meta.json`
- `src/data.rs`: optional CSV diagnostics, reaction registry, end-of-run `summary.md`
- `src/snapshot.rs`: optional PPM cross-sections, density maps, and ancestry maps
- `marl.toml`: sample runtime config with binary outputs on and legacy diagnostics off by default

## Standalone Viewer
- `src/bin/marl-viewer.rs`: optional `viewer` feature binary; parses `run_meta.json`, loads one field snapshot, opens a `winit`/`wgpu` window, and drives rendering
- `src/bin/viewer_raymarch.wgsl`: full-screen raymarch shader for a selected external species in the packed 3D field texture
- `Cargo.toml`: declares `viewer` feature and `marl-viewer` binary with `required-features = ["viewer"]`

## GPU Prototype
- `src/gpu/`: optional `gpu` feature implementation for field diffusion
- `src/gpu/shaders/field_diffuse.wgsl`: WGSL compute shader with currently duplicated grid/species constants
- `tests/gpu_diffusion.rs`: CPU/GPU equivalence tests compiled when `gpu` feature is enabled

## Documentation And Agent Context
- `README.md`: user-facing overview, run commands, config and output documentation
- `.agents/plans/`: implementation plans
- `.agents/context/STATUS.md`: current project status and completed work
- `.agents/context/NOTES.md`: durable implementation decisions and gotchas
- `.agents/context/MAP.md`: this concise structure map
- `scripts/check_binary_dump.py`: quick binary output sanity checker
