# MARL Implementation Notes

## Engine Viewer Data Pipeline (2026-04-25)

### Durable decisions

1. **Binary viewer output is the default data product.**
   `write_binary_field` and `write_binary_cells` default to `true`; `write_tick_log`, `write_csv_snapshots`, `write_ancestry_map`, and `write_density_map` default to `false`.

2. **Field dump layout is exact Rust field memory order.**
   `tick_<T>.field.bin` is raw little-endian `f32` values from `Field::data`, laid out as `[z][y][x][species]`. At `128x128x64` and `S_EXT=12`, each field file is `50,331,648` bytes.

3. **Cell dump records are packed and headerless.**
   `tick_<T>.cells.bin` is a contiguous array of 25-byte `ViewerCell` records: `pos:f32[3]`, `lineage_id:u64`, `starter_type:u8`, `energy:f32`. The viewer should derive cell count from `file_size / cell_record_stride`.

4. **`ViewerCell` is `#[repr(C, packed)]`.**
   Do not take references to multi-byte fields of a packed record. Copy values by value or cast the whole slice to bytes.

5. **Metadata is conditional on binary output.**
   `run_meta.json` is written only when at least one binary output type is enabled, and records binary toggles plus layout details.

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

## GPU Reaction-Diffusion Prototype (2026-04-25)

### Gotchas

1. **WGSL uniform arrays of `f32` failed validation under `wgpu` 29.**
   The original planned `var<uniform>` params struct with `[f32; 12]` arrays produced a shader validation error because uniform array stride must satisfy stricter alignment. The implementation uses `var<storage, read>` for the small params struct instead. This is acceptable for v1 and keeps the Rust struct compact.

2. **Shader constants are intentionally duplicated for v1.**
   `src/gpu/shaders/field_diffuse.wgsl` hardcodes `GRID_X=128`, `GRID_Y=128`, `GRID_Z=64`, and `S_EXT=12`. `GpuFieldDiffuser::new()` performs release-mode runtime validation and returns `GpuError::InvalidInput` if `config.rs` drifts.

3. **The GPU path is synchronous and copy-heavy.**
   `GpuFieldDiffuser::diffuse_tick_with_cells()` uploads `field.data`, uploads occupancy as `u32`, dispatches one compute pass per diffusion substep, copies the final GPU buffer to a staging buffer, blocks on `device.poll`, then copies back into `field.data`. This preserves CPU-owned light/cells/output but is not the final architecture.

4. **Occupancy semantics are covered by deterministic GPU tests.**
   The WGSL path copies occupied voxels unchanged and treats occupied neighbors like walls by substituting the center concentration. Tests cover empty occupancy, center occupancy, boundary-adjacent occupancy, dense cluster occupancy, one substep, and default 10 substeps.

5. **`--gpu-diffusion` falls back to CPU if initialization or dispatch fails.**
   CPU diffusion remains the default even when compiled with `--features gpu`. If GPU initialization fails, the run logs a warning and continues on CPU.

### Benchmark observation

- On NVIDIA GeForce RTX 4060 via Vulkan, a 10-tick release run showed about 5.2s CPU vs about 1.2s GPU with upload/readback every tick. Treat this as coarse only; the simulation seed is random and no fine-grained upload/dispatch/readback timings are recorded yet.
