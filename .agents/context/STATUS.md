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
