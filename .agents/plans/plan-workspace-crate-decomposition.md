# Workspace Crate Decomposition Plan

## Goal

Restructure the current single Rust package into a Cargo workspace with separate `marl-engine`, `marl-viewer-rs`, and a small shared `marl-format` crate. Keep the repository root available for project-level files and a possible future Python/uv project. Do not add or scaffold Python files now.

## Current State

- Root `Cargo.toml` is a single package named `marl`.
- The engine binary is `src/main.rs` and imports the library as `marl::...`.
- The engine library modules are in `src/`: `binary_dump.rs`, `cell.rs`, `config.rs`, `data.rs`, `field.rs`, `hgt.rs`, `light.rs`, `snapshot.rs`, and optional `gpu/`.
- The viewer binary is feature-gated at `src/bin/marl-viewer.rs` with shader `src/bin/viewer_raymarch.wgsl`.
- Viewer-only dependencies are currently optional root package dependencies: `serde_json`, `winit`, `wgpu`, `pollster`, `bytemuck`.
- Engine optional GPU diffusion also uses `wgpu`, `pollster`, and `bytemuck` behind feature `gpu`.
- Tests include unit tests under engine modules and `tests/gpu_diffusion.rs` for the GPU feature.
- Existing user-facing commands include `cargo check`, `cargo test`, `cargo check --features gpu`, and `cargo check --features viewer --bin marl-viewer`.

## Target Layout

```text
marl/
  Cargo.toml
  Cargo.lock
  README.md
  marl.toml
  scripts/
  tests/                         # move or keep only if workspace-level tests are needed
  crates/
    marl-format/
      Cargo.toml
      src/lib.rs
    marl-engine/
      Cargo.toml
      src/lib.rs
      src/main.rs
      src/binary_dump.rs
      src/cell.rs
      src/config.rs
      src/data.rs
      src/field.rs
      src/hgt.rs
      src/light.rs
      src/snapshot.rs
      src/gpu/
      tests/gpu_diffusion.rs
    marl-viewer-rs/
      Cargo.toml
      src/main.rs
      src/viewer_raymarch.wgsl
```

Root `Cargo.toml` should become a virtual workspace manifest with `resolver = "3"` and no `[package]` section.

## Non-Goals

- Do not add Python files, `pyproject.toml`, `uv.lock`, `.venv`, or Python package scaffolding.
- Do not redesign the simulation model.
- Do not change the binary field/cell output format except by centralizing schema constants/types in `marl-format`.
- Do not perform broad behavioral refactors during the initial mechanical crate split.
- Do not commit changes unless the user explicitly asks.

## Shared Format Crate

Create `crates/marl-format` as a tiny dependency-free-or-nearly-dependency-free crate for binary viewer schema shared by engine and viewer.

### Responsibilities

- Own durable schema strings and validation helpers:
  - endianness: `"little"`
  - field dtype: `"f32"`
  - field layout: `"z_y_x_species"`
  - cell record stride: `25`
- Own serializable/deserializable metadata type used for `run_meta.json`.
- Own byte-length helpers for field dumps.
- Own the packed viewer cell record type if possible.

### Suggested API

In `crates/marl-format/src/lib.rs`:

```rust
pub const ENDIANNESS: &str = "little";
pub const FIELD_DTYPE: &str = "f32";
pub const FIELD_LAYOUT: &str = "z_y_x_species";
pub const CELL_RECORD_STRIDE: u32 = 25;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RunMeta {
    pub grid_x: u32,
    pub grid_y: u32,
    pub grid_z: u32,
    pub s_ext: u32,
    pub m_int: u32,
    pub field_dtype: String,
    pub field_layout: String,
    pub field_byte_len: u64,
    pub cell_record_stride: u32,
    pub endianness: String,
    pub write_binary_field: bool,
    pub write_binary_cells: bool,
}

impl RunMeta {
    pub fn new(...dimensions and output toggles...) -> Self { ... }
    pub fn validate_field_layout(&self) -> Result<(), FormatError> { ... }
}

pub fn field_byte_len(grid_x: u32, grid_y: u32, grid_z: u32, s_ext: u32) -> Option<u64> { ... }
```

If a packed cell struct is included:

```rust
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct ViewerCellRecord {
    pub pos: [f32; 3],
    pub lineage_id: u64,
    pub starter_type: u8,
    pub energy: f32,
}
```

Do not derive `bytemuck::Pod` for a packed struct unless it is proven valid with current `bytemuck` rules. It is acceptable for `marl-engine` to keep a local `as_bytes<T>()` helper for writing bytes.

## Phase 1: Mechanical Workspace Split

This phase should avoid behavior changes.

1. Replace root `Cargo.toml` with a workspace manifest:
   - `[workspace] members = ["crates/marl-format", "crates/marl-engine", "crates/marl-viewer-rs"]`
   - `resolver = "3"`
   - `[workspace.package] edition = "2024", version = "0.3.0+6"`
   - `[workspace.dependencies]` for current shared dependencies.
2. Create `crates/marl-engine/Cargo.toml`:
   - package name `marl-engine`
   - lib crate name can remain Rust-normalized as `marl_engine`
   - binary named `marl-engine`
   - feature `gpu = ["dep:bytemuck", "dep:pollster", "dep:wgpu"]`
   - dependencies: `marl-format`, `rand`, `rand_distr`, `rayon`, `serde`, `toml`; optional GPU deps.
3. Create `crates/marl-viewer-rs/Cargo.toml`:
   - package name `marl-viewer-rs`
   - binary named `marl-viewer-rs`
   - dependencies: `marl-format`, `serde`, `serde_json`, `bytemuck`, `pollster`, `wgpu`, `winit`.
   - Do not require a `viewer` feature; the viewer crate itself is opt-in by package selection.
4. Create `crates/marl-format/Cargo.toml`:
   - package name `marl-format`
   - dependency on `serde` with derive.
5. Move files:
   - `src/lib.rs` -> `crates/marl-engine/src/lib.rs`
   - `src/main.rs` -> `crates/marl-engine/src/main.rs`
   - engine modules -> `crates/marl-engine/src/`
   - `src/gpu/` -> `crates/marl-engine/src/gpu/`
   - `src/bin/marl-viewer.rs` -> `crates/marl-viewer-rs/src/main.rs`
   - `src/bin/viewer_raymarch.wgsl` -> `crates/marl-viewer-rs/src/viewer_raymarch.wgsl`
   - `tests/gpu_diffusion.rs` -> `crates/marl-engine/tests/gpu_diffusion.rs`
6. Update imports:
   - In engine `src/main.rs`, replace `use marl::...` with `use marl_engine::...`.
   - In moved tests, replace `marl::...` with `marl_engine::...`.
7. Update shader include path if needed in viewer main after moving shader beside the source file.

Verification for Phase 1:

- `cargo fmt --all`
- `cargo check --workspace`
- `cargo test -p marl-engine`
- `cargo check -p marl-engine --features gpu`
- `cargo check -p marl-viewer-rs`

## Phase 2: Extract `marl-format`

This phase centralizes binary schema shared by engine and viewer.

1. Implement `crates/marl-format/src/lib.rs` with:
   - schema constants
   - `RunMeta`
   - `field_byte_len`
   - validation error type or simple `Result<(), String>` validation helper
   - optional `ViewerCellRecord`
2. Update `crates/marl-engine/src/binary_dump.rs`:
   - import `marl_format::{RunMeta, ViewerCellRecord, field_byte_len, ...}` as appropriate
   - keep conversion from `&CellState` to the shared packed cell record in engine if the record is shared
   - use `RunMeta::new(...)` or equivalent instead of ad hoc JSON construction
3. Update `crates/marl-viewer-rs/src/main.rs`:
   - remove local `RunMeta` definition
   - deserialize `marl_format::RunMeta`
   - replace local `validate_meta` logic with shared validation helper where possible
   - keep viewer-specific species bounds checks in the viewer
4. Preserve on-disk JSON field names and values exactly.

Verification for Phase 2:

- `cargo fmt --all`
- `cargo test -p marl-format`
- `cargo test -p marl-engine`
- `cargo check -p marl-viewer-rs`
- If feasible, run a short engine command and validate generated `run_meta.json` with `scripts/check_binary_dump.py`.

## Phase 3: Internal Engine Source Decomposition

Only start after workspace and format extraction pass verification.

Primary target: shrink `crates/marl-engine/src/main.rs`, currently responsible for simulation orchestration, helpers, starter factories, stats, and spatial utilities.

Suggested modules:

- `src/sim/mod.rs`
- `src/sim/runner.rs`: main tick loop orchestration and setup currently in `main()`.
- `src/sim/seeding.rs`: `seed_cells`, depth band calculations, initial cell population setup.
- `src/sim/spatial.rs`: `empty_neighbors`, `read_neighbor_environment`, `apply_deltas_to_neighbors`, `find_empty_neighbor`, `find_cell_neighbor`.
- `src/sim/starter_metabolisms.rs`: `inactive_receptor`, `inactive_transport`, `inactive_reaction`, `inactive_effector`, `make_phototroph`, `make_chemolithotroph`, `make_anaerobe`.
- `src/sim/stats.rs`: `print_stats`, `print_z_profile`.

Minimal API recommendation:

- Keep `src/main.rs` thin:
  ```rust
  fn main() {
      let cfg = marl_engine::config::Config::load();
      marl_engine::sim::run(cfg);
  }
  ```
- If GPU feature handling currently depends on reading CLI args in `main.rs`, either:
  - keep that tiny flag scan in `main.rs` and pass `RunOptions { use_gpu_diffusion }`, or
  - move it into `sim::RunOptions::from_env()`.

Verification for Phase 3:

- `cargo fmt --all`
- `cargo check -p marl-engine`
- `cargo test -p marl-engine`
- `cargo check -p marl-engine --features gpu`
- Short smoke run: `cargo run -p marl-engine -- --ticks 2 --stats 1 --snapshot 1000 --images 1000`

## Phase 4: Internal Viewer Source Decomposition

Only start after workspace and format extraction pass verification. This phase is independent of engine source decomposition after Phase 2.

Primary target: shrink `crates/marl-viewer-rs/src/main.rs`, currently responsible for CLI parsing, metadata loading, rendering setup, texture upload, and winit application handling.

Suggested modules:

- `src/args.rs`: `ViewerArgs`, `usage`, `parse_value`, `next_value`.
- `src/io.rs`: `FieldPayload`, `load_field`.
- `src/renderer.rs`: `ViewerParams`, `Renderer`, `RenderResult`, `create_field_texture`.
- `src/app.rs`: `ViewerApp` and `ApplicationHandler` implementation.
- `src/main.rs`: only parse args, load field, create event loop/app, run.

Verification for Phase 4:

- `cargo fmt --all`
- `cargo check -p marl-viewer-rs`
- `cargo run -p marl-viewer-rs -- --help` should print usage and exit with the existing help behavior.

## Builder Wave Strategy

Use parallel builders only on non-overlapping files/directories.

### Wave 1: Mechanical split and format crate scaffold

- Builder A owns root workspace manifest and engine crate movement:
  - `Cargo.toml`
  - `crates/marl-engine/**`
  - move `tests/gpu_diffusion.rs`
  - no viewer edits except paths needed for workspace validity
- Builder B owns viewer crate movement and format crate scaffold:
  - `crates/marl-viewer-rs/**`
  - `crates/marl-format/**`
  - no engine module edits except reporting required imports

After Wave 1, main agent must inspect and resolve integration conflicts manually.

### Wave 2: Shared format integration

- Builder A owns engine binary dump integration:
  - `crates/marl-engine/src/binary_dump.rs`
  - tests in that file
- Builder B owns viewer metadata integration:
  - `crates/marl-viewer-rs/src/main.rs`
  - use `marl-format::RunMeta`

After Wave 2, main agent verifies JSON compatibility and workspace checks.

### Wave 3: Source decomposition

- Builder A owns engine `main.rs` decomposition into `src/sim/**`.
- Builder B owns viewer `main.rs` decomposition into `src/{args,io,renderer,app}.rs`.

Do not run Wave 3 until workspace and format tests are passing.

## Documentation Updates

Update after verification:

- `README.md` command examples:
  - `cargo run -p marl-engine -- ...`
  - `cargo run -p marl-engine --features gpu -- ...`
  - `cargo run -p marl-viewer-rs -- ...`
  - `cargo check --workspace`
- `.agents/context/MAP.md` to reflect workspace crates.
- `.agents/context/STATUS.md` with completed workspace split and verification results.
- `.agents/context/NOTES.md` with durable decisions:
  - root is a virtual Cargo workspace
  - viewer is a separate crate, not a feature-gated bin
  - `marl-format` owns binary schema shared by engine/viewer
  - Python/uv project intentionally not scaffolded yet

## Risks And Mitigations

- Risk: workspace move breaks relative output paths. Mitigation: run from repo root; preserve `output/run_128x128x64` defaults and document commands.
- Risk: viewer shader include path breaks after move. Mitigation: keep shader beside viewer `main.rs` or update `include_str!` path explicitly.
- Risk: old package name `marl` appears in tests/imports/docs. Mitigation: search for `marl::`, `--bin marl-viewer`, `--features viewer`, and update intentionally.
- Risk: format extraction accidentally changes JSON field names. Mitigation: preserve serde field names and compare generated `run_meta.json` shape before/after where possible.
- Risk: parallel builders collide. Mitigation: assign exact path ownership per wave and integrate after each wave before launching the next.

## Completion Criteria

- Root is a virtual Cargo workspace.
- `marl-engine`, `marl-viewer-rs`, and `marl-format` crates exist under `crates/`.
- Engine builds and tests pass without viewer dependencies.
- Viewer builds as its own package without requiring a Cargo feature.
- `marl-format` is used by both engine and viewer for run metadata/schema validation.
- Existing binary output layout remains compatible.
- README and agent context files reflect the new layout and commands.
- No Python/uv scaffolding has been added.
