# Engine Crate Decomposition Plan

**Date:** 2026-05-10
**Status:** draft

---

## Goal

Decompose the monolithic `crates/marl-engine` crate (currently ~12 modules, one binary, one optional GPU feature) into **7 focused crates**: `marl-config`, `marl-cell`, `marl-field`, `marl-sim`, `marl-output`, `marl-gpu`, and a thin `marl-engine` binary crate. The goal is proper separation of concerns — each crate has exactly one domain responsibility, clear dependency edges, and independently testable units.

## Understanding

The current `crates/marl-engine` crate bundles seven distinct responsibilities:

| Responsibility | Current File(s) | Lines |
|---|---|---|
| Grid constants + runtime config + CLI/TOML loading | `config.rs` | ~295 |
| Cell biology (state, ruleset, 5-phase tick, mutation, HGT) | `cell.rs`, `hgt.rs` | ~552 |
| Extracellular field + CPU diffusion + boundary sources | `field.rs` | ~313 |
| Light attenuation | `light.rs` | ~60 |
| Simulation orchestration (main loop, seeding, spatial, starters, stats) | `main.rs`, `sim/` | ~862 |
| Binary output for viewer | `binary_dump.rs` | ~135 |
| CSV diagnostics + PPM snapshots + run summaries | `data.rs`, `snapshot.rs` | ~1190 |
| Optional GPU diffusion | `gpu/` | ~362 |

Key coupling points:
- `cell.rs` imports `use crate::config::*` — needs compile-time constants (`GRID_X`, `S_EXT`, `M_INT`, etc.) and `SimulationConfig` for the `tick()` method and `mutate()`.
- `field.rs` imports `use crate::config::*` — needs grid constants and `SimulationConfig` for diffusion parameters.
- `main.rs` imports from `cell`, `config`, `field`, `light`, `sim`, `binary_dump`, `snapshot`, and optionally `gpu`.
- `binary_dump.rs` imports from `cell` and `config` and depends on `marl_format`.
- `snapshot.rs` imports from `config`, `field`, `light`, and `cell`.
- `data.rs` imports from `cell`, `config`, `field`, and `light`.
- `sim/` modules import from `cell`, `config`, `field`.
- `gpu/` module imports from `config`, `field` and depends on `wgpu`/`bytemuck`/`pollster`.

The GPU shader (`field_diffuse.wgsl`) has *hardcoded* grid/species constants (`const GRID_X: u32 = 128u`, etc.) that currently duplicate the Rust constants — this already creates a maintenance hazard that would worsen with further crate splitting but is outside this plan's scope (the GPU prototype is always gated behind an optional feature and already requires specific dimensions).

## Approach

**Dependency DAG (top-down):**

```
marl-config        ← zero internal deps (only serde + toml)
    ↓
marl-cell          ← depends on marl-config + rand + rand_distr
marl-field         ← depends on marl-config + rayon
    ↓
marl-sim           ← depends on marl-config + marl-cell + marl-field + rand
marl-output        ← depends on marl-config + marl-cell + marl-field + marl-format
marl-gpu           ← depends on marl-config + marl-field + wgpu + bytemuck + pollster (optional)
    ↓
marl-engine (bin)  ← depends on marl-sim + marl-output [+ marl-gpu]
```

**Key design decisions:**

1. `marl-config` keeps both compile-time constants and runtime config together. Splitting them would create a circular dependency: runtime config types are parameterized by constants, and separating them means two crates with `S_EXT`-sized arrays flowing both ways. The combined crate is ~295 lines — small enough to be a leaf node.

2. `marl-cell` owns the entire cell biology: `CellState`, `Ruleset`, `Reaction`, all param types, the 5-phase `tick()`, `mutate()`, and `hgt.rs`. This is the "domain model" — 552 lines and a natural seam.

3. `marl-field` owns `Field` (chemical concentrations + CPU diffusion) and `LightField` (Beer-Lambert). These are separate in the current codebase but share the same physics layer (`SimulationConfig`, grid constants) — keeping them together avoids an extra tiny crate.

4. `marl-sim` is the orchestration layer: the tick loop, seeding, spatial utilities, starter metabolism factories, and stats. The binary's `main()` logic moves into `marl_sim::run(config)`. The actual `main.rs` becomes ~20 lines of glue.

5. `marl-output` is the I/O sidecar: binary dumps, CSV logging (`DataLogger`, `ReactionRegistry`), PPM snapshots, and the run summary writer. It depends on `marl-format` for the shared binary schema.

6. `marl-gpu` becomes its own crate (not a feature-gated module) since it has unique heavy dependencies (`wgpu`). The engine binary and `marl-sim` use it via an optional dependency.

7. The existing `marl-format` crate (already separate) remains unchanged — it's the shared binary schema crate used by both engine and viewer.

## Steps

### Phase 1: Create `marl-config` crate

Extract compile-time constants and runtime configuration into a standalone crate with zero simulation dependencies.

1. **Create the crate scaffold**
   - **Location:** `crates/marl-config/Cargo.toml`, `crates/marl-config/src/lib.rs`
   - **Action:** Create `crates/marl-config/Cargo.toml` with `[package] name = "marl-config"`, workspace version/edition, deps on `serde` (derive) and `toml` from workspace. Create `src/lib.rs` as a re-export module.
   - **Verification:** `cargo check -p marl-config`

2. **Move compile-time constants**
   - **Location:** `crates/marl-config/src/lib.rs`
   - **Action:** Move `pub const GRID_X, GRID_Y, GRID_Z, S_EXT, M_INT, R_MAX, S_RECEPTORS, S_TRANSPORTERS, S_EFFECTORS` from `crates/marl-engine/src/config.rs:12-24` into `marl-config/src/lib.rs`. These are the only compile-time constants — everything else is runtime-configurable.
   - **Verification:** `cargo check -p marl-config`

3. **Move runtime config types**
   - **Location:** `crates/marl-config/src/lib.rs`
   - **Action:** Move `SimulationConfig`, `OutputConfig`, `Config` structs with their `Default` impls and `Config::load()` method from `crates/marl-engine/src/config.rs:33-294` into `marl-config/src/lib.rs`. Keep all serde attributes and CLI parsing logic intact. These types use `S_EXT` for array sizes, so they must live after the constants.
   - **Verification:** `cargo check -p marl-config`

4. **Add workspace member**
   - **Location:** `Cargo.toml` (workspace root)
   - **Action:** Add `"crates/marl-config"` to `[workspace] members`.
   - **Verification:** `cargo check --workspace`

5. **Update `marl-engine` to depend on `marl-config`**
   - **Location:** `crates/marl-engine/Cargo.toml`
   - **Action:** Add `marl-config = { path = "../marl-config" }` to `[dependencies]`.
   - **Verification:** `cargo check -p marl-engine` (will fail until imports are updated — see Phase 4)

### Phase 2: Create `marl-cell` crate

Extract cell biology into a standalone crate that depends only on `marl-config` + `rand`/`rand_distr`.

6. **Create the crate scaffold**
   - **Location:** `crates/marl-cell/Cargo.toml`, `crates/marl-cell/src/lib.rs`
   - **Action:** Create `Cargo.toml` with deps: `marl-config`, `rand`, `rand_distr` (all workspace). Create `src/lib.rs` with `pub mod cell; pub mod hgt;`.
   - **Verification:** `cargo check -p marl-cell`

7. **Move cell state, param types, and `CellEvent` enum**
   - **Location:** `crates/marl-cell/src/cell.rs`
   - **Action:** Move from `crates/marl-engine/src/cell.rs` the following items (preserve all doc comments and derives):
     - `ReceptorParams` (lines 50-55)
     - `TransportParams` (lines 63-69)
     - `Reaction` (lines 83-92)
     - `EffectorParams` (lines 100-106)
     - `FateParams` (lines 119-127)
     - `Ruleset` (lines 138-151)
     - `CellEvent` (lines 154-160)
     - `CellState` (lines 168-183)
     - All `impl CellState` methods including `tick()` (lines 185-369) — change `use crate::config::*` to `use marl_config::*`
     - All `impl Ruleset` methods including `mutate()` (lines 395-524) — change `use rand::Rng`/`use rand_distr` imports and `use marl_config::*`
   - **Verification:** `cargo check -p marl-cell`

8. **Move `hgt.rs`**
   - **Location:** `crates/marl-cell/src/hgt.rs`
   - **Action:** Move the entire contents of `crates/marl-engine/src/hgt.rs` — change `use crate::cell::Ruleset` to `use crate::cell::Ruleset` (it will now be a sibling module).
   - **Verification:** `cargo check -p marl-cell`

### Phase 3: Create `marl-field` crate

Extract physical simulation layer: extracellular field + diffusion + light.

9. **Create the crate scaffold**
   - **Location:** `crates/marl-field/Cargo.toml`, `crates/marl-field/src/lib.rs`
   - **Action:** Create `Cargo.toml` with deps: `marl-config`, `rayon` (workspace). Create `src/lib.rs` with `pub mod field; pub mod light;`.
   - **Verification:** `cargo check -p marl-field`

10. **Move `field.rs`**
    - **Location:** `crates/marl-field/src/field.rs`
    - **Action:** Move the entire contents of `crates/marl-engine/src/field.rs`. Change `use crate::config::*` to `use marl_config::*`. Keep all tests in `#[cfg(test)]`. The `Field` struct, `new()`, `get`/`set`, `read_voxel`, `apply_deltas`, `diffusion_step_inner`, `diffuse_tick_with_cells`, `diffuse_tick`, `apply_boundary_sources` all move intact.
    - **Verification:** `cargo test -p marl-field`

11. **Move `light.rs`**
    - **Location:** `crates/marl-field/src/light.rs`
    - **Action:** Move the entire contents of `crates/marl-engine/src/light.rs`. Change `use crate::config::*` to `use marl_config::*`. Change `use crate::field::Field` to `use crate::field::Field`. Change `use std::collections::HashMap` — keep it (no change). The `LightField` struct and `update()` method move intact.
    - **Verification:** `cargo check -p marl-field`

### Phase 4: Update `marl-engine` to use `marl-config`, `marl-cell`, `marl-field`

Wire the new crate boundaries into the existing `marl-engine` crate before extracting further.

12. **Update `marl-engine/Cargo.toml`**
    - **Location:** `crates/marl-engine/Cargo.toml`
    - **Action:** Add `marl-cell = { path = "../marl-cell" }` and `marl-field = { path = "../marl-field" }` to `[dependencies]`. Ensure `marl-config` is already present (from Phase 1 step 5).
    - **Verification:** `cargo check -p marl-engine` (will fail; step 13 fixes it)

13. **Update imports in `marl-engine` source files**
    - **Location:** All `.rs` files under `crates/marl-engine/src/` that reference cell, field, or config symbols
    - **Action:** Perform these import rewrites:
      - `marl-engine/src/lib.rs`: replace `pub mod cell; pub mod field; pub mod config;` with `pub mod binary_dump; pub mod data; pub mod sim; pub mod snapshot;` (remove cell, field, config, hgt, light — they now live in other crates). Keep `#[cfg(feature = "gpu")] pub mod gpu;` (Phase 7 handles gpu).
      - All remaining files that use `use crate::config::*`: change to `use marl_config::*`
      - Files that use `use crate::cell::*`: change to `use marl_cell::*` or `use marl_cell::cell::*` (depending on re-export strategy — prefer `use marl_cell::cell::{CellState, CellEvent, ...}` for clarity)
      - Files that use `use crate::field::Field`: change to `use marl_field::field::Field`
      - Files that use `use crate::light::LightField`: change to `use marl_field::light::LightField`
      - Files that use `use crate::hgt::*`: change to `use marl_cell::hgt::*` (if still needed — note HGT is currently disabled)
      - `main.rs` specifically: changes imports as above plus any remaining `crate::` references.
    - **Verification:** `cargo check -p marl-engine`

14. **Delete moved source files from `marl-engine`**
    - **Location:** `crates/marl-engine/src/`
    - **Action:** Remove `cell.rs`, `hgt.rs`, `config.rs`, `field.rs`, `light.rs` (they've been moved to their respective crates).
    - **Verification:** `cargo check -p marl-engine` passes cleanly

### Phase 5: Create `marl-sim` crate

Extract simulation orchestration: the tick loop, seeding, spatial utilities, starter metabolisms, and stats printing.

15. **Create the crate scaffold**
    - **Location:** `crates/marl-sim/Cargo.toml`, `crates/marl-sim/src/lib.rs`
    - **Action:** Create `Cargo.toml` with deps: `marl-config`, `marl-cell`, `marl-field`, `rand` (workspace). Create `src/lib.rs` with `pub mod seeding; pub mod spatial; pub mod starter_metabolisms; pub mod stats;` and a public `run(config: marl_config::Config)` function (the tick loop, to be implemented in step 17).
    - **Verification:** `cargo check -p marl-sim`

16. **Move the `sim/` modules**
    - **Location:** `crates/marl-sim/src/`
    - **Action:** Move entire directories/modules from `crates/marl-engine/src/sim/` to `crates/marl-sim/src/`:
      - `seeding.rs` → change `use crate::cell::CellState`, `use crate::config::*`, `use crate::field::Field` to `use marl_cell::cell::CellState`, `use marl_config::*`, `use marl_field::field::Field`
      - `spatial.rs` → change `use crate::config::*`, `use crate::field::Field` to `use marl_config::*`, `use marl_field::field::Field`
      - `starter_metabolisms.rs` → change `use crate::cell::{...}` to `use marl_cell::cell::{...}`, change `use crate::config::*` to `use marl_config::*`
      - `stats.rs` → change `use crate::cell::CellState`, `use crate::config::*`, `use crate::field::Field`, `use crate::light::LightField` to their new crate paths
      - `mod.rs` → keep the four `pub mod` lines
    - **Verification:** `cargo check -p marl-sim`

17. **Extract the tick loop into `marl_sim::run()`**
    - **Location:** `crates/marl-sim/src/lib.rs`
    - **Action:** Add a public function `pub fn run(cfg: marl_config::Config)` that contains the entire main loop from `crates/marl-engine/src/main.rs` (lines 21-405). The function:
      - Takes `Config` by value (already owned by caller)
      - Handles field initialization, seeding, the tick loop (boundary sources → diffusion → light → cell updates → fate processing → logging/snapshots)
      - Accepts an optional `#[cfg(feature = "gpu")] use_gpu_diffusion: bool` parameter or detects it from env (current behavior uses `std::env::args().any(|arg| arg == "--gpu-diffusion")` — preserve this or add a `RunOptions` struct)
      - Returns nothing (the current `main()` has no return value other than printing)
      - The GPU feature gating stays: `#[cfg(feature = "gpu")]` blocks for `GpuFieldDiffuser` usage. Until Phase 7, these can be conditionally compiled via a feature on `marl-sim` that depends on `marl-gpu`.
      - For now (before Phase 7), keep GPU code commented out or behind a `marl-sim` feature that will be wired in Phase 7.
    - **Verification:** `cargo check -p marl-sim` (without gpu feature first; then with if feature is added)

18. **Move binary_dump, data, and snapshot imports out of `main.rs` (deferred to Phase 6)**
    - **Action:** For now, the `run()` function may need to call into `marl-output` functions. Since `marl-output` isn't created yet, temporarily hard-code the minimum or accept that Phase 5 completes after Phase 6. The safest order: create `marl-output` next, then finish `marl-sim::run()` by having it depend on `marl-output`.

### Phase 6: Create `marl-output` crate

Extract all output/IO logic: binary dumps, CSV diagnostics, PPM snapshots, run summaries.

19. **Create the crate scaffold**
    - **Location:** `crates/marl-output/Cargo.toml`, `crates/marl-output/src/lib.rs`
    - **Action:** Create `Cargo.toml` with deps: `marl-config`, `marl-cell`, `marl-field`, `marl-format` (all path deps, workspace). No additional deps needed — data.rs uses only `std::fs`, `std::io`, `std::collections`. Create `src/lib.rs` with `pub mod binary_dump; pub mod data; pub mod snapshot;`.
    - **Verification:** `cargo check -p marl-output`

20. **Move `binary_dump.rs`**
    - **Location:** `crates/marl-output/src/binary_dump.rs`
    - **Action:** Move the entire contents of `crates/marl-engine/src/binary_dump.rs`. Update imports:
      - `use crate::cell::CellState` → `use marl_cell::cell::CellState`
      - `use crate::config::*` → `use marl_config::*`
      - `use crate::field::Field` → `use marl_field::field::Field`
      - `marl_format` imports stay the same (it's already a dependency)
      - Keep `#[cfg(test)]` tests intact
    - **Verification:** `cargo test -p marl-output`

21. **Move `data.rs`**
    - **Location:** `crates/marl-output/src/data.rs`
    - **Action:** Move the entire contents of `crates/marl-engine/src/data.rs` (~816 lines). Update imports:
      - `use crate::cell::*` → `use marl_cell::cell::*`
      - `use crate::config::*` → `use marl_config::*`
      - `use crate::field::Field` → `use marl_field::field::Field`
      - `use crate::light::LightField` → `use marl_field::light::LightField`
    - **Verification:** `cargo check -p marl-output`

22. **Move `snapshot.rs`**
    - **Location:** `crates/marl-output/src/snapshot.rs`
    - **Action:** Move the entire contents of `crates/marl-engine/src/snapshot.rs` (~374 lines). Update imports:
      - `use crate::config::*` → `use marl_config::*`
      - `use crate::field::Field` → `use marl_field::field::Field`
      - `use crate::light::LightField` → `use marl_field::light::LightField`
      - Cell references in `write_all_snapshots` and `write_ancestry_xz` → use `marl_cell::cell::CellState`
    - **Verification:** `cargo check -p marl-output`

### Phase 7: Convert `marl-gpu` from feature-gated module to optional crate

The GPU diffusion prototype currently lives at `crates/marl-engine/src/gpu/` behind the `gpu` feature.

23. **Create `marl-gpu` crate scaffold**
    - **Location:** `crates/marl-gpu/Cargo.toml`, `crates/marl-gpu/src/lib.rs`
    - **Action:** Create `Cargo.toml` with package name `marl-gpu`, deps: `marl-config`, `marl-field`, `bytemuck`, `pollster`, `wgpu` (from workspace). Create `src/lib.rs` with `pub mod context; pub mod field_diffusion; pub use context::{GpuContext, GpuError}; pub use field_diffusion::GpuFieldDiffuser;`.
    - **Verification:** `cargo check -p marl-gpu`

24. **Move GPU source files**
    - **Location:** `crates/marl-gpu/src/`
    - **Action:** Move `crates/marl-engine/src/gpu/context.rs` → `crates/marl-gpu/src/context.rs`. Move `crates/marl-engine/src/gpu/field_diffusion.rs` → `crates/marl-gpu/src/field_diffusion.rs`. Move `crates/marl-engine/src/gpu/shaders/field_diffuse.wgsl` → `crates/marl-gpu/src/shaders/field_diffuse.wgsl`.
    - Update imports in moved files:
      - `use super::context::*` → `use crate::context::*`
      - `use crate::config::*` → `use marl_config::*`
      - `use crate::field::Field` → `use marl_field::field::Field`
    - **Verification:** `cargo check -p marl-gpu`

25. **Move GPU tests**
    - **Location:** `crates/marl-engine/tests/gpu_diffusion.rs` → `crates/marl-gpu/tests/gpu_diffusion.rs` (create `crates/marl-gpu/tests/` directory)
    - **Action:** Move the test file, update imports.
    - **Verification:** `cargo test -p marl-gpu`

### Phase 8: Wire `marl-sim` and `marl-engine` binary

Complete the dependency chain and create the thin engine binary.

26. **Finalize `marl-sim` with all dependencies**
    - **Location:** `crates/marl-sim/Cargo.toml`, `crates/marl-sim/src/lib.rs`
    - **Action:** Add `marl-output = { path = "../marl-output" }` to deps. Make GPU an optional dependency: `marl-gpu = { path = "../marl-gpu", optional = true }`. Add feature `gpu = ["dep:marl-gpu"]`.
    - In `lib.rs`: implement the `run()` function that calls into `marl_output::binary_dump`, `marl_output::data::DataLogger`, `marl_output::snapshot`, etc. The function body is the current `main()` loop minus CLI arg parsing (which stays in the binary). The `run()` function accepts `Config` plus an optional `use_gpu_diffusion: bool` flag.
    - **Verification:** `cargo check -p marl-sim` and `cargo check -p marl-sim --features gpu`

27. **Create thin `marl-engine` binary**
    - **Location:** `crates/marl-engine/src/main.rs`
    - **Action:** Replace the current ~405-line `main.rs` with a thin wrapper:
      ```rust
      fn main() {
          let cfg = marl_config::Config::load();
          #[cfg(feature = "gpu")]
          let use_gpu = std::env::args().any(|arg| arg == "--gpu-diffusion");
          #[cfg(not(feature = "gpu"))]
          let use_gpu = false;
          marl_sim::run(cfg, use_gpu);
      }
      ```
    - **Verification:** `cargo run -p marl-engine -- --ticks 2 --stats 1 --snapshot 1000`

28. **Update `marl-engine/Cargo.toml`**
    - **Location:** `crates/marl-engine/Cargo.toml`
    - **Action:** Replace dependencies:
      - Remove: `marl-format`, `rand`, `rand_distr`, `rayon`, `serde`, `toml` (they're now deps of child crates, not needed directly)
      - Keep/add: `marl-config`, `marl-sim`
      - Optional: `gpu = ["dep:marl-gpu", "marl-sim/gpu"]` to re-export the GPU feature
      - The `[features]` section should forward: `gpu = ["marl-sim/gpu"]`
    - **Verification:** `cargo check -p marl-engine` and `cargo check -p marl-engine --features gpu`

29. **Update `marl-engine/src/lib.rs`** (if it still exists)
    - **Location:** `crates/marl-engine/src/lib.rs`
    - **Action:** Remove all `pub mod` declarations. The engine no longer needs to be a library — it's a pure binary crate. If any tests reference `marl_engine::`, convert them to integration tests that use the new crate APIs or remove them. The existing unit tests have already been moved to their respective crates.
    - **Verification:** `cargo test --workspace` (all tests pass from their new locations)

### Phase 9: Integration verification and cleanup

30. **Run full workspace build**
    - **Location:** Workspace root
    - **Action:** `cargo build --workspace --release`
    - **Verification:** Zero errors, zero warnings

31. **Run full test suite**
    - **Location:** Workspace root
    - **Action:** `cargo test --workspace`
    - **Verification:** All tests pass

32. **Run a short smoke test**
    - **Location:** Workspace root
    - **Action:** `cargo run -p marl-engine --release -- --ticks 50 --stats 10 --snapshot 100 --output /tmp/marl_smoke_test`
    - **Verification:** Engine runs to completion, produces `run_meta.json`, `tick_0.field.bin`, `tick_0.cells.bin`, no panics

33. **Verify GPU feature**
    - **Location:** Workspace root
    - **Action:** `cargo check -p marl-engine --features gpu` and if GPU available: `cargo run -p marl-engine --features gpu --release -- --ticks 10 --stats 10 --snapshot 100 --output /tmp/marl_gpu_smoke --gpu-diffusion`
    - **Verification:** Builds and runs without errors

34. **Verify viewer compatibility**
    - **Location:** Workspace root
    - **Action:** `cargo check -p marl-viewer-rs` (the viewer depends on `marl-format` which is unchanged)
    - **Verification:** Viewer builds cleanly

35. **Update agent context files**
    - **Location:** `.agents/context/MAP.md`, `.agents/context/STATUS.md`, `.agents/context/NOTES.md`
    - **Action:** Update MAP.md to reflect the new crate topology. Add a STATUS.md entry noting the decomposition is complete. Add NOTES.md entries documenting:
      - `marl-config` owns compile-time constants and runtime configuration
      - `marl-cell` is the cellular biology crate (state, ruleset, tick, mutation)
      - `marl-field` is the physical simulation crate (diffusion, light)
      - `marl-sim` is the orchestration crate (tick loop, seeding, spatial, starters, stats)
      - `marl-output` is the I/O crate (binary dumps, CSV, PPM, summaries)
      - `marl-gpu` is the optional GPU diffusion crate
      - `marl-engine` is now a thin binary crate
      - Dependency graph between crates
    - **Verification:** Visual review of the context files

## Final Crate Layout

```
crates/
  marl-config/          Compile-time constants + runtime config (serde + toml)
    Cargo.toml
    src/lib.rs
  marl-cell/            Cell biology: CellState, Ruleset, tick, mutation, HGT
    Cargo.toml
    src/lib.rs
    src/cell.rs
    src/hgt.rs
  marl-field/           Extracellular field, diffusion, light attenuation
    Cargo.toml
    src/lib.rs
    src/field.rs
    src/light.rs
  marl-sim/             Simulation orchestration: tick loop, seeding, spatial, starters, stats
    Cargo.toml
    src/lib.rs
    src/seeding.rs
    src/spatial.rs
    src/starter_metabolisms.rs
    src/stats.rs
  marl-output/          Output/I/O: binary dumps, CSV logging, PPM snapshots, summaries
    Cargo.toml
    src/lib.rs
    src/binary_dump.rs
    src/data.rs
    src/snapshot.rs
  marl-gpu/             Optional GPU diffusion (wgpu + bytemuck + pollster)
    Cargo.toml
    src/lib.rs
    src/context.rs
    src/field_diffusion.rs
    src/shaders/
      field_diffuse.wgsl
    tests/
      gpu_diffusion.rs
  marl-format/          Shared binary schema (UNCHANGED — engine ↔ viewer bridge)
    Cargo.toml
    src/lib.rs
  marl-engine/          Thin binary crate — ~30 lines
    Cargo.toml
    src/main.rs
```

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Import churn breaks gated GPU code | Medium | Medium | Make GPU the last phase; keep `#[cfg(feature = "gpu")]` guards from day one |
| Circular dependency between marl-output and marl-sim | Low | High | The current plan ensures marl-sim depends on marl-output (not vice versa); verify before extracting |
| `data.rs` calls `cell.rs` internals (`.internal[0]`, `.ruleset.reactions.iter()`) — these are public | Low | Low | All accessed fields are already `pub`; no refactoring needed |
| GPU shader hardcodes grid constants | Medium | Low | Accept as-is; shader constant duplication is a pre-existing issue outside this plan's scope |
| Tests in `marl-engine/tests/` reference old crate paths | Low | Medium | Only `tests/gpu_diffusion.rs` exists; it moves entirely to `marl-gpu` |
| SnapshotInfo / viewer types | None | — | Viewer is separate; no impact |

## Verification

1. `cargo build --workspace --release` — zero errors
2. `cargo test --workspace` — all tests pass from their new crate locations
3. `cargo run -p marl-engine --release -- --ticks 50 --stats 10 --snapshot 100` — produces valid output
4. `cargo check -p marl-engine --features gpu` — GPU feature compiles
5. `cargo check -p marl-viewer-rs` — viewer still builds (depends on marl-format only, unchanged)
6. `cargo fmt --all` — consistent formatting
7. Agent context files (MAP.md, STATUS.md, NOTES.md) updated with new crate topology
