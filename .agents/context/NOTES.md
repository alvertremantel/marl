# MARL Implementation Notes

## Unified Runtime Configuration (2026-04-25)

### Gotchas

1. **Serde `#[serde(default)]` on structs is required for partial TOML tables.**
   Without `#[serde(default)]` on `SimulationConfig` and `OutputConfig`, a TOML file that contains `[output]` with only a subset of fields will fail to deserialize entirely, causing a silent fallback to built-in defaults. The `#[serde(default)]` attribute tells serde to use `Default::default()` for any missing fields within the table.

2. **`f32::EPSILON` vs `EPSILON` in cell.rs.**
   The old code had both `f32::EPSILON` (machine epsilon, ~1.19e-7) and the config constant `EPSILON` (0.001). Only the config constant was moved to `sim.epsilon`. The machine epsilon in the substrate term division guard was left unchanged.

3. **`Normal::new(0.0f32, sim.mutation_stddev)` requires `sim.mutation_stddev > 0.0`.**
   The default is 0.1. If a user sets this to 0.0 or negative in TOML, `Normal::new()` will panic at runtime. This is acceptable for now since it's an advanced parameter; future validation could clamp it.

4. **Division neighbor distance parameterization.**
   The plan did not explicitly mention parameterizing `find_empty_neighbor`, but `division_neighbor_distance` was in the config struct. It has been wired through `main.rs` to replace the hardcoded `* 2` multiplier.

5. **`data.rs` summary references physics constants.**
   The plan suggested leaving `data.rs` mostly untouched, but `write_summary()` directly referenced `LAMBDA_MAINTENANCE`, `SOURCE_RATE_OXIDANT`, `SOURCE_RATE_CARBON`, and `SOURCE_RATE_REDUCTANT`. These had to be wired through `sim: &SimulationConfig` to avoid compile errors after the constants were removed.

6. **Phototroph seeding bounds fix.**
   The original code hardcoded `seed_cells(..., 0, 3, ...)`. After parameterization, `photo_lo` can equal `photo_hi` if `z_scale` is very small (e.g., small GRID_Z). Added `.max(photo_lo as f32 + 1.0)` to ensure at least a 1-layer range for `rng.random_range()`.

### Design decisions retained from the plan

- Starter metabolism factory functions (`make_phototroph`, etc.) remain hardcoded. Their detailed biochemical parameters are not runtime-configurable. Only the high-level seeding geometry (depth bands, margins, counts) is configurable.
- No per-physics-parameter CLI flags beyond run-control (`--ticks`, `--stats`, etc.). The TOML file is the primary physics interface.
- `OutputConfig` does not control CSV column layout. Snapshot formats remain stable to avoid breaking downstream analysis scripts.
