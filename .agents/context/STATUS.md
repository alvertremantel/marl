# MARL Project Status

## Current Branch: feat/configurability

## Completed: Unified Runtime Configuration (2026-04-25)

All non-array-size simulation parameters have been moved from compile-time `const` to a unified runtime config system.

### What changed
- `Cargo.toml`: added `serde` and `toml`; removed unused `half`
- `src/config.rs`: kept compile-time grid constants; replaced all physics/run params with `SimulationConfig` and `OutputConfig` structs
- `src/cell.rs`: `tick()` and `mutate()` now take `&SimulationConfig`
- `src/field.rs`: diffusion and boundary methods now take `&SimulationConfig`
- `src/light.rs`: `update()` now takes `&SimulationConfig`
- `src/snapshot.rs`: `write_all_snapshots()` now takes `&OutputConfig` and `&SimulationConfig`; image toggles respected
- `src/data.rs`: `write_summary()` now takes `&SimulationConfig`
- `src/main.rs`: wires `Config::load()` through initialization, seeding, tick loop, and output
- `marl.toml`: sample config with all defaults and comments
- `README.md`: documented TOML config and CLI flags

### Verification
- `cargo check`: zero errors, zero warnings
- `cargo build --release`: succeeds
- Default run (`--ticks 100 --stats 10`): produces consistent stdout
- TOML run (`--config marl.toml --ticks 100`): loads config correctly
- Partial TOML override (`source_rate_oxidant = 0.8`): produces visibly different chemistry
- Fallback without TOML: works with built-in defaults

### Remaining compile-time constants (array-size determinants)
- `GRID_X`, `GRID_Y`, `GRID_Z`
- `S_EXT`, `M_INT`
- `R_MAX`, `S_RECEPTORS`, `S_TRANSPORTERS`, `S_EFFECTORS`

These cannot be runtime-configurable because they size arrays and `Vec`s throughout the code.

## Completed: GPU Reaction-Diffusion Prototype (2026-04-25)

The first correctness-first GPU diffusion prototype is implemented behind the optional `gpu` Cargo feature.

### What changed
- `Cargo.toml` / `Cargo.lock`: added optional `gpu` feature with `wgpu`, `pollster`, and `bytemuck`
- `src/gpu/`: added GPU context, synchronous `GpuFieldDiffuser`, and naive WGSL field diffusion shader
- `src/main.rs`: added `--gpu-diffusion` behind `#[cfg(feature = "gpu")]`; CPU diffusion remains default
- `src/field.rs`: added `Clone` and focused CPU diffusion tests
- `tests/gpu_diffusion.rs`: added deterministic CPU/GPU equivalence tests for empty, center occupied, boundary-adjacent, dense cluster, one-substep, and default-substep cases

### Verification
- Baseline before edits: `cargo check`, `cargo test`, and `cargo run --release -- --ticks 10 --stats 5 --snapshot 1000 --images 1000`
- `cargo check`: passes
- `cargo test`: passes
- `cargo check --features gpu`: passes
- `cargo test --features gpu`: passes on NVIDIA GeForce RTX 4060 (Vulkan)
- `cargo build --release`: passes
- `cargo build --release --features gpu`: passes
- CPU run: `cargo run --release -- --ticks 10 --stats 5 --snapshot 1000 --images 1000`
- GPU run: `cargo run --release --features gpu -- --ticks 10 --stats 5 --snapshot 1000 --images 1000 --gpu-diffusion`

### Observed short-run timing
- CPU 10-tick release run: about 5.2s, about 1.9 ticks/s by final stats line
- GPU 10-tick release run with full upload/readback each tick: about 1.2s, about 8.9 ticks/s by final stats line
- These runs used normal random seeding, so timing is useful as a coarse sanity check only; deterministic correctness is covered by GPU tests.

### Known limitations
- GPU path only replaces field diffusion; boundary sources, light, cells, mutation, logging, and snapshots remain CPU-side
- Field and occupancy are uploaded every tick and the full field is read back every tick
- Shader constants are duplicated for `128x128x64` and `S_EXT=12`; runtime validation now fails GPU initialization if Rust constants drift
- Params are passed as a read-only storage buffer, not a uniform buffer, to avoid WGSL uniform array stride constraints
