# MARL Project Status

## Current Branch: tweak/outputs

## Completed: Viewer Crate Decomposition (2026-05-10)

Decomposed the single `crates/marl-viewer-rs` crate (~7 modules, ~3000 lines) into 3 crates: `marl-viewer-core` (pure data + I/O), `marl-viewer-render` (wgpu pipeline + egui GUI), and the thin `marl-viewer-rs` binary.

### New crate topology
- `marl-viewer-core`: pure data and I/O — `args.rs` (CLI parsing, `ViewerArgs`, `ViewMode`, `CellMode`), `io.rs` (snapshot loading, tick discovery, cell record parsing), `camera.rs` (camera basis computation), `types.rs` (shared metadata types: `SnapshotInfo`, `GuiAction`, tick navigation helpers) — zero GPU/windowing deps
- `marl-viewer-render`: wgpu rendering pipeline — `renderer.rs` (GPU surface/device/queue/pipeline, snapshot resources, egui overlay pass, action processing), `gui.rs` (`GuiState`, toolbar/sidebar drawing), `viewer_raymarch.wgsl` — depends on wgpu, bytemuck, pollster, winit, egui*
- `marl-viewer-rs`: thin binary — `main.rs` (~15 lines), `app.rs` (winit `ApplicationHandler`)

### Dependency DAG
```
marl-viewer-core    ← marl-format + serde + serde_json (no GPU/windowing deps)
    ↓
marl-viewer-render  ← marl-viewer-core + wgpu + bytemuck + pollster + winit + egui*
    ↓
marl-viewer-rs      ← marl-viewer-core + marl-viewer-render + winit + pollster
```

### Key design decisions
- `SnapshotInfo` and `GuiAction` live in `marl-viewer-core::types` as shared metadata — both the renderer and GUI need them
- `choose_initial_tick` and `neighbor_tick` are pure functions in `marl-viewer-core::types` (no GUI deps)
- `GuiState` lives in `marl-viewer-render` along with its `show()` method since it requires egui types
- `marl-viewer-core` can be compiled and tested on headless CI (no GPU required)
- The binary crate has only 2 direct deps beyond the viewer crates: `winit` and `pollster`

### Verification
- `cargo fmt --all`: passes
- `cargo build --workspace --release`: zero errors, zero warnings
- `cargo test --workspace`: 72 tests pass (same count as before decomposition)
- `cargo run -p marl-viewer-rs -- --help`: prints usage message and exits cleanly
- `cargo check -p marl-viewer-core`: builds independently (headless CI compatible)
- 11 workspace members present in `cargo metadata`

### Notes
- `marl-viewer-core` has zero GPU/windowing dependencies — only `marl-format`, `serde`, `serde_json`
- `marl-viewer-render` has all egui/wgpu/rfd deps; the binary only directly depends on `winit` and `pollster` beyond the viewer crates
- The original `marl-viewer-rs/src/args.rs`, `io.rs`, `camera.rs`, `renderer.rs`, `gui.rs`, and `viewer_raymarch.wgsl` have been deleted from the binary crate
- `marl-viewer-rs` now contains only `main.rs` and `app.rs`

## Completed: Engine Crate Decomposition (2026-05-10)

Decomposed the monolithic `crates/marl-engine` crate (~12 modules, one binary, one optional GPU feature) into 7 focused crates with a thin binary driving a clear dependency DAG.

### New crate topology
- `marl-config`: compile-time grid constants (`GRID_X`, `S_EXT`, etc.) + runtime `SimulationConfig`/`OutputConfig`/`Config` with `Config::load()` — zero simulation deps (295 lines)
- `marl-cell`: cellular biology — `CellState`, `Ruleset`, `Reaction`, param types, 5-phase `tick()`, `mutate()`, HGT (552 lines)
- `marl-field`: extracellular physics — `Field` (CPU diffusion, boundary sources, tests), `LightField` (Beer-Lambert attenuation) (373 lines)
- `marl-sim`: simulation orchestration — `run()` (full tick loop), seeding, spatial helpers, starter metabolisms, stats printing (depends on `marl-output`) (475 lines + loop body)
- `marl-output`: I/O sidecar — binary dumps, CSV diagnostics (`DataLogger`, `ReactionRegistry`), PPM snapshots, run summaries (1325 lines)
- `marl-gpu`: optional GPU diffusion — `GpuFieldDiffuser`, `GpuContext`, WGSL shader, CPU/GPU equivalence tests (362 lines)
- `marl-engine`: thin binary (~10 lines) — load config, call `marl_sim::run()`

### Dependency DAG
```
marl-config        ← zero internal deps
    ↓
marl-cell          ← marl-config + rand + rand_distr
marl-field         ← marl-config + rayon
    ↓
marl-sim           ← marl-config + marl-cell + marl-field + marl-output (+ optional marl-gpu)
marl-output        ← marl-config + marl-cell + marl-field + marl-format
marl-gpu           ← marl-config + marl-field + wgpu + bytemuck + pollster
    ↓
marl-engine (bin)  ← marl-sim (+ gpu feature forwarding)
```

### Verification
- `cargo fmt --all`: passes
- `cargo build --workspace --release`: zero errors, one pre-existing viewer warning
- `cargo test --workspace`: 72 tests pass (2 marl-field, 11 marl-format, 1 marl-gpu, 2 marl-output, 56 marl-viewer-rs)
- `cargo run -p marl-engine --release -- --ticks 50 --stats 10 --snapshot 100 --output /tmp/marl_smoke_test`: completes, produces run_meta.json, tick_0/49.{field,cells}.bin, summary.md
- `cargo check -p marl-engine --features gpu`: passes
- `cargo check -p marl-sim --features gpu`: passes
- `cargo check -p marl-viewer-rs`: passes
- `cargo check -p marl-gpu`: passes

### Notes
- `marl-engine` is now a pure binary crate; its old `lib.rs` and all moved modules have been removed
- GPU feature is forwarded through: `marl-engine --features gpu → marl-sim/gpu → dep:marl-gpu`
- The WGSL shader constant duplication remains a pre-existing issue outside this plan's scope
- `marl-sim::run()` takes `Config` by value + a `use_gpu_diffusion: bool` flag, matching the old `main()` behavior
- All previously public `pub` fields/types remain accessible through their new crate paths (e.g., `marl_cell::cell::CellState`, `marl_field::field::Field`)

## Completed: Viewer GUI Shell (2026-05-09)

The standalone viewer now includes an `egui` GUI shell with directory loading, tick navigation, and view settings controls overlaid on the existing WGSL raymarch renderer.

### What changed
- `Cargo.toml` / `crates/marl-viewer-rs/Cargo.toml`: added workspace dependencies for `egui`, `egui-wgpu`, `egui-winit`, and `rfd` (native folder picker)
- `crates/marl-viewer-rs/src/args.rs`: added `ViewMode::as_str()`, `ViewMode::all()`, `CellMode::as_str()`, `CellMode::all()` for GUI labels
- `crates/marl-viewer-rs/src/io.rs`: extracted `load_run_meta()`, added `discover_field_ticks()` and `parse_field_tick_file_name()` for tick discovery; added tick discovery tests
- `crates/marl-viewer-rs/src/renderer.rs`: major refactor — extracted `SnapshotGpuResources`, `SnapshotInfo`, `build_viewer_params()`, `create_snapshot_bind_group()`; added placeholder 1×1×1 resources so the window opens even without valid snapshot data; added `apply_args()` for atomic snapshot reload; integrated `egui_ctx`/`egui_state`/`egui_renderer`/`gui`; render loop now includes raymarch pass (LoadOp::Clear) followed by egui overlay pass (LoadOp::Load)
- `crates/marl-viewer-rs/src/gui.rs` (new): `GuiState`, `GuiAction` enum, tick navigation helpers (`choose_initial_tick`, `neighbor_tick`), GUI drawing with toolbar (directory/tick entry, native folder picker, nav buttons, reload), collapsible side panel (species, view mode, cell mode, cell alpha, density scale, exposure, steps, Apply/Reset), and unit tests
- `crates/marl-viewer-rs/src/main.rs`: removed hard startup dependency on `load_snapshot()`; viewer binary always opens a window even with invalid/missing output dir
- `crates/marl-viewer-rs/src/app.rs`: added `handle_window_event()` forwarding for egui input; event loop requests redraw on egui consumption
- `README.md`: updated viewer section with GUI features, directory picker, tick navigation, and view settings

### Verification
- `cargo fmt --all`: passes
- `cargo check -p marl-viewer-rs`: passes (one pre-existing warning about `lineage_id` field)
- `cargo test -p marl-viewer-rs`: 56 tests pass (args, camera, gui, io, renderer)
- `cargo test -p marl-format`: 11 tests pass
- `cargo test -p marl-engine`: 4 tests pass
- `cargo run -p marl-viewer-rs -- --help`: prints CLI help with all flags preserved
- `cargo tree -p marl-viewer-rs -i wgpu`: single `wgpu 29.x`
- `cargo tree -p marl-viewer-rs -i winit`: single `winit 0.30.x`
- Manual visual verification pending on a display/GPU-capable machine

### Notes
- `egui-winit`/`egui-wgpu` were chosen over `eframe`; the WGSL raymarch remains the background pass
- Snapshot GPU resources are built atomically and replaced only on success; placeholder 1×1×1 resources are used when no valid data is loaded
- The native directory picker (`rfd`) is available via the "Open…" button; a text field fallback is always available
- View settings apply triggers a full snapshot reload (acceptable for MVP); CLI flags still work and set initial GUI draft values

## Completed: Enhanced 3D Viewer with Microbe Voxels (2026-05-09)

The standalone viewer now renders an isometric 3D volume by default, with direct microbe-occupied voxel markers overlaid on the translucent chemical field. Cell records are loaded from `tick_<T>.cells.bin` and uploaded as an `Rgba8Uint` 3D occupancy/identity texture.

### What changed
- `crates/marl-viewer-rs/src/args.rs`: added `--view <iso|top>`, `--cells <off|starter|energy>`, `--cell-alpha <f>` flags; refactored parser for testability; new defaults are `iso`/`starter`/`0.95`
- `crates/marl-viewer-rs/src/io.rs`: renamed `FieldPayload` to `SnapshotPayload`; added `LoadedCell` type and cell `.bin` parsing with position validation
- `crates/marl-viewer-rs/src/camera.rs` (new): `CameraBasis`, `camera_basis(ViewMode)` helpers for isometric and top-down orthographic cameras
- `crates/marl-viewer-rs/src/renderer.rs`: expanded `ViewerParams` with camera and cell uniforms; added `Rgba8Uint` 3D cell texture creation with starter/energy coloring; updated bind group to include cell texture (binding 2)
- `crates/marl-viewer-rs/src/viewer_raymarch.wgsl`: replaced top-down z-stepping with orthographic ray/AABB traversal; cell voxel compositing with per-voxel deduplication; effective step count to avoid skipping one-voxel markers
- `crates/marl-viewer-rs/src/main.rs`, `app.rs`: wired `SnapshotPayload` and `ViewerArgs` through load/launch flow; enhanced load messaging and window title
- `README.md`: updated viewer section with isometric defaults, cell rendering flags, and microbe coloring notes

### Verification
- `cargo fmt --all`: passes
- `cargo check -p marl-viewer-rs`: passes (one expected warning about `lineage_id` field)
- `cargo test -p marl-viewer-rs`: 35 tests pass (args, camera, io, renderer)
- `cargo test -p marl-format`: passes
- `cargo test -p marl-engine`: passes
- `cargo run -p marl-viewer-rs -- --help`: prints updated help with new flags
- Manual visual verification pending on a display/GPU-capable machine

### Notes
- Microbe coloring uses `starter_type` (ancestry category from seeding), not inferred genotype-level species
- Cell occupancy is uploaded as `Rgba8Uint` 3D texture; unoccupied voxels remain transparent
- Isometric camera is a fixed orthographic oblique view; no interactive orbit controls yet
- Legacy `--view top --cells off` is preserved for field-only rendering

## Completed: Workspace Crate Decomposition (2026-04-25)

The repository is now a Cargo workspace with separate engine, viewer, and shared binary format crates. The root directory is no longer a Rust package, leaving room for project-level files and a future Python/uv component without scaffolding one now.

### What changed
- `Cargo.toml`: converted to a virtual workspace with `crates/marl-engine`, `crates/marl-viewer-rs`, and `crates/marl-format`
- `crates/marl-engine/`: owns the simulation library and `marl-engine` binary; optional GPU diffusion remains behind the `gpu` feature
- `crates/marl-format/`: owns shared binary schema constants, `RunMeta`, field byte-length validation, and the packed 25-byte `ViewerCellRecord`
- `crates/marl-viewer-rs/`: owns the standalone `wgpu` viewer binary without a `viewer` feature gate
- `crates/marl-engine/src/sim/`: extracted seeding, spatial, starter metabolism, and stats helpers from the engine binary
- `crates/marl-viewer-rs/src/{args,io,renderer,app}.rs`: split viewer CLI, loading, rendering, and app/event-loop code
- `README.md` / `.agents/context/MAP.md` / `.agents/context/NOTES.md`: updated for workspace paths and commands

### Verification
- `cargo fmt --all`: passes
- `cargo check --workspace`: passes
- `cargo build --workspace`: passes
- `cargo test -p marl-format`: passes
- `cargo test -p marl-engine`: passes
- `cargo check -p marl-engine --features gpu`: passes
- `cargo run -p marl-viewer-rs -- --help`: prints CLI help
- `cargo run -p marl-engine -- --ticks 2 --stats 1 --snapshot 1 --images 1000 --output output/workspace_smoke`: succeeds
- `python scripts/check_binary_dump.py output/workspace_smoke 1`: passes

### Notes
- Engine commands now use `cargo run -p marl-engine -- ...`.
- Viewer commands now use `cargo run -p marl-viewer-rs -- ...`.
- `marl-format::RunMeta` defaults `m_int` during deserialization so older metadata without that field remains readable; new engine metadata writes `m_int`.
- No Python/uv files were created.

## Completed: Standalone WGPU Viewer Phase 1 (2026-04-25)

The repository now has a separate, feature-gated `marl-viewer` binary that ingests engine binary field snapshots and renders one species with a basic GPU raymarch pass.

### What changed
- `Cargo.toml` / `Cargo.lock`: added optional `viewer` feature with `winit`, `serde_json`, `wgpu`, `pollster`, and `bytemuck`
- `src/bin/marl-viewer.rs`: parses CLI flags, validates `run_meta.json`, loads `tick_<T>.field.bin`, creates a `winit`/`wgpu` window, uploads the field as a 3D `R32Float` texture, and renders continuously
- `src/bin/viewer_raymarch.wgsl`: full-screen triangle shader that raymarches the selected external species through the z volume
- `README.md`: documented viewer usage and tuning flags

### Verification
- `cargo fmt`: passes
- `cargo check`: passes
- `cargo check --features viewer --bin marl-viewer`: passes
- `cargo check --features gpu`: passes
- `cargo test`: passes
- `cargo run --features viewer --bin marl-viewer -- --help`: prints CLI help

### Immediate next steps (superseded by Enhanced 3D Viewer, 2026-05-09)
- ~~Add cell buffer ingestion/instanced rendering.~~ → Done: cell occupancy/identity texture with starter/energy coloring.
- Add two-snapshot streaming/interpolation pipeline.
- Add camera/ray/AABB picking for exact voxel queries.
- Add interactive orbit controls.

## Completed: Engine Viewer Data Pipeline (2026-04-25)

The simulation now emits raw binary data for high-fidelity viewer ingestion by default, with legacy CSV/PPM diagnostics opt-in.

### What changed
- `src/binary_dump.rs`: writes raw `tick_<T>.field.bin`, packed `tick_<T>.cells.bin`, and `run_meta.json`
- `src/config.rs`: added binary output toggles and disabled legacy CSV/PPM defaults
- `src/data.rs`: made `ticks.csv` optional via `write_tick_log`
- `src/main.rs`: writes run metadata at startup when binary output is enabled and dumps binary snapshots on `snapshot_interval`
- `marl.toml` / `README.md`: documented binary defaults and legacy opt-in outputs

### Verification
- `cargo check`: passes
- `cargo test`: passes, including binary layout tests
- `cargo check --features gpu`: passes
- Short release run produced only `run_meta.json`, `summary.md`, `tick_0/1.field.bin`, and `tick_0/1.cells.bin`
- Verified no `.csv` or `.ppm` files are generated by default and `scripts/check_binary_dump.py` parses the field/cell dumps

### Notes
- Field layout is raw little-endian `f32` in `[z][y][x][species]` order.
- Cell files contain packed 25-byte `ViewerCell` records; cell count is `file_size / cell_record_stride`.
- `tick_0` snapshots are written after tick 0 has executed, matching existing snapshot timing.

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
