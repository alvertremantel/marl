# MARL Implementation Notes

## Workspace Crate Decomposition (2026-04-25)

### Durable decisions

1. **The root manifest is a virtual Cargo workspace.**
   The Rust packages now live under `crates/`, leaving the repository root for shared docs/config/output and a possible future Python/uv project. No Python/uv scaffolding exists yet.

2. **The engine and viewer are separate packages.**
   `marl-engine` owns simulation code and the optional `gpu` diffusion feature. `marl-viewer-rs` owns `winit`/viewer `wgpu` rendering dependencies, so engine builds no longer need a viewer feature gate.

3. **`marl-format` owns binary interop schema.**
   Shared constants, `RunMeta`, `field_byte_len`, and `ViewerCellRecord` live in `crates/marl-format`. The engine still manually writes full `run_meta.json` to preserve all historical fields, but uses shared constants and the shared packed cell record.

4. **Old metadata remains readable.**
   `RunMeta::m_int` uses `#[serde(default)]` so older `run_meta.json` files without `m_int` still deserialize. New engine metadata includes `m_int`.

5. **Main files are thinner but the simulation loop remains centralized.**
   Engine `main.rs` still owns the tick loop, while helper code moved to `crates/marl-engine/src/sim/`. Viewer `main.rs` is now orchestration only, with CLI/loading/rendering/app code in focused modules.

## Standalone WGPU Viewer Phase 1 (2026-04-25)

### Durable decisions

1. **Viewer dependencies are feature-gated.**
   Superseded by the workspace split: viewer dependencies now live in the separate `marl-viewer-rs` package, so default engine builds do not pull in `winit`/windowing dependencies.

2. **Phase 1 renders one snapshot and one species.**
   The viewer reads `run_meta.json`, loads `tick_<T>.field.bin`, validates the binary layout, and uploads the selected snapshot once. Two-snapshot interpolation, async streaming, cell rendering, and voxel picking remain future work.

3. **3D texture layout packs species into texture X.**
   Because the engine writes `[z][y][x][species]` floats, the viewer uploads the field as an `R32Float` 3D texture with dimensions `(grid_x * s_ext, grid_y, grid_z)`. Shader lookup computes `tex_x = voxel_x * s_ext + species`.

4. **The raymarch shader uses `textureLoad`, not filtering.**
   `R32Float` is bound as a non-filterable 3D texture. Current rendering samples exact voxel/species values and uses `--scale`, `--exposure`, and `--steps` as transfer-function controls.

5. **`wgpu` 29 surface acquisition returns `CurrentSurfaceTexture`.**
   The viewer handles `Success`, `Suboptimal`, `Timeout`, `Occluded`, `Outdated`, `Lost`, and `Validation` directly instead of using the older `Result<_, SurfaceError>` pattern.

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

## Enhanced 3D Viewer with Microbe Voxels (2026-05-09)

### Durable decisions

1. **`starter_type` is the MVP microbe identity indicator.**
   The viewer colors occupied voxels by `starter_type` (0 = phototroph, 1 = chemolithotroph, 2 = anaerobe, other = magenta). This is an ancestry marker from the initial seeding, not an inferred genotype-level species. True species classification from reaction topologies would require an engine schema extension.

2. **Cell occupancy is uploaded as `Rgba8Uint` 3D texture.**
   The viewer converts sparse `LoadedCell` records into a dense texture sized `(grid_x, grid_y, grid_z)`. Each occupied voxel stores RGBA bytes encoding the marker color and alpha. Unoccupied voxels are zeroed. For the default `128x128x64` grid this texture is approximately 4 MiB.

3. **The isometric camera uses orthographic ray/AABB traversal.**
   The camera basis (`right`, `up`, `dir`, `zoom`) is computed in pure Rust (`camera.rs`) and passed to the shader as uniforms. The fragment shader constructs an orthographic ray per pixel, intersects it with the normalized simulation box, and marches through the volume. This replaces the old screen-`uv`→`z` mapping.

4. **Cell voxels are composited once per voxel crossing during raymarch.**
   The shader tracks the previous voxel coordinate and only applies cell color/alpha when the current step enters a new voxel. This prevents the same occupied voxel from contributing multiple times.

5. **A conservative effective step count ensures one-voxel markers are not skipped.**
   The shader uses `max(user_steps, 2 * max(grid_x, grid_y, grid_z))` for the isometric view so a single occupied voxel in the `128x128x64` grid cannot be missed by insufficient ray sampling. User-provided `--steps` can still increase this.

6. **Legacy top-down/field-only rendering is preserved.**
   `--view top --cells off` restores the Phase 1 behavior of top-down z-stepping through one chemical species without cell overlay. This keeps backward compatibility for existing field-only analysis workflows.

### Gotchas

1. **`Rgba8Uint` texture `bytes_per_row` alignment.**
   For grids where `grid_x * 4` is not a multiple of the adapter's `COPY_BYTES_PER_ROW_ALIGNMENT` (typically 256), `queue.write_texture` requires padding or will fail at runtime. At the default `128x128`, `128 * 4 = 512` which is a multiple of 256, so this is not an issue for the current default but may matter for larger or non-power-of-2 grids.

2. **Duplicate cell positions are silently resolved.**
   The engine should never produce duplicate positions, but if it does, the viewer keeps the first cell in `starter` mode and the first cell in `energy` mode (with a warning). This avoids silently showing wrong data.

3. **Shader compilation is runtime-only.**
    `cargo check` passes regardless of WGSL syntax validity. CPU-side tests cannot catch shader errors. A display/GPU-capable smoke test is required to confirm the shader pipeline creates successfully.

## Viewer GUI Shell (2026-05-09)

### Durable decisions

1. **`egui-winit`/`egui-wgpu` were chosen over `eframe`.**
   `eframe` would own the event loop, device, and surface, forcing a larger rewrite or custom paint callback. Direct egui integration lets the existing WGSL raymarch remain the first/background render pass on the same wgpu surface, followed by an egui overlay pass for controls. The raymarch pass uses `LoadOp::Clear`; the egui pass uses `LoadOp::Load`.

2. **Snapshot GPU resources are reloadable.**
   The renderer owns a `SnapshotGpuResources` struct (textures, params buffer, bind group) that can be atomically replaced. When a new directory or tick is loaded, new GPU resources are built into locals first; only after all steps succeed do they replace the active resources. On failure, the old snapshot or placeholder remains visible.

3. **Placeholder resources enable window-open-without-valid-data.**
   When the initial or default output directory is missing or invalid, the renderer creates 1×1×1 all-zero field/cell textures and a valid bind group. This lets the GUI open and display controls so the user can navigate to a valid directory. The window title reports "no snapshot loaded" in this state.

4. **Tick discovery scans the output directory for `tick_<T>.field.bin` files.**
   `discover_field_ticks()` reads the directory, parses filenames matching `tick_<digits>.field.bin`, sorts ascending, and deduplicates. Navigation uses `neighbor_tick()` on the sorted list so snapshot intervals larger than one work. Tick lists are rescanned on each successful load so new ticks written by a running simulation appear after `Reload`.

5. **View settings apply triggers a full snapshot reload.**
   For MVP simplicity, changing species, view mode, cell mode, or rendering parameters reloads the snapshot from disk rather than updating GPU uniforms only. The `Apply` button triggers an explicit reload; sliders/drags do not live-update.

6. **Native directory picker (`rfd`) with text field fallback.**
   The `Open…` button calls `rfd::FileDialog::pick_folder()`. On platforms where this returns `None`, the text field + `Load Dir` button remain functional.

### Gotchas

1. **`forget_lifetime()` is required for the egui render pass.**
   `egui_wgpu::Renderer::render()` expects `RenderPass<'static>`. The raymarch pass captures `&view` and the encoder borrows `&self.device` — this prevents moving the renderer. Using `pass.forget_lifetime()` in a block that ends before `encoder.finish()` works because `encoder` and the underlying command buffer keep the textures alive.

2. **Deprecated egui panel API in 0.34.2.**
   `egui::TopBottomPanel`, `egui::SidePanel`, and `.show()` are deprecated in 0.34.2. The code uses `#[allow(deprecated)]` to suppress warnings since the root-level panel API requires `show_inside()` which needs a `&mut Ui`, nesting everything inside `CentralPanel::show()` which is itself deprecated. A future egui update will migrate to the new API.
