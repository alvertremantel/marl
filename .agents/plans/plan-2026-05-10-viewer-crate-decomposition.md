# Viewer Crate Decomposition Plan

**Date:** 2026-05-10
**Status:** completed

---

## Goal

Decompose the single `crates/marl-viewer-rs` crate (currently ~7 modules, ~3000 lines total) into **3 crates**: `marl-viewer-core` (data types, I/O, camera — no GPU deps), `marl-viewer-render` (wgpu rendering pipeline, textures, shader), and the thin `marl-viewer-rs` binary (winit app + egui GUI). The goal is to separate the pure-data and file-I/O layer from the GPU rendering layer, enabling independent compilation and testing of each.

## Understanding

The current `crates/marl-viewer-rs` crate has 7 modules in a flat directory:

| Module | Lines | Responsibility | Heavy Dependencies |
|---|---|---|---|
| `main.rs` | 30 | Entry point, arg parsing, event loop creation | winit |
| `app.rs` | 101 | `ApplicationHandler` impl, window creation | winit |
| `args.rs` | 405 | CLI arg parsing, `ViewerArgs`, `ViewMode`, `CellMode` enums | (std only) |
| `io.rs` | 450 | File I/O, snapshot loading, cell record parsing, tick discovery | marl-format, serde_json |
| `camera.rs` | 159 | `CameraBasis` computation (pure math) | (std only) |
| `gui.rs` | 515 | `GuiState`, `GuiAction`, egui toolbar/sidebar | egui + types from args/renderer |
| `renderer.rs` | 1355 | wgpu pipeline, textures, bind groups, snapshot resources, egui overlay | wgpu, bytemuck, egui-wgpu, egui-winit, pollster, rfd |

**Key coupling observations:**

- `renderer.rs` depends on everything: `args.rs` (types + `ViewerArgs`), `camera.rs` (`CameraBasis`, `camera_basis`), `gui.rs` (`GuiState`, `GuiAction`, `choose_initial_tick`, `neighbor_tick`), `io.rs` (`SnapshotPayload`, `LoadedCell`, `load_snapshot`, `discover_field_ticks`, `load_run_meta`).
- `gui.rs` depends on `args.rs` (types) and `renderer.rs` (`SnapshotInfo` struct — defined at renderer.rs line 52).
- `io.rs` depends on `args.rs` (`ViewerArgs`, `CellMode`) and `marl-format` (`RunMeta`).
- `app.rs` depends on `args.rs` and `renderer.rs` (`Renderer`, `RenderResult`).
- `camera.rs` depends only on `args.rs` (`ViewMode` enum).
- `main.rs` depends on `app.rs` and `args.rs`.

**The `SnapshotInfo` coupling problem:** `SnapshotInfo` is a lightweight metadata struct defined in `renderer.rs:52` but used by both `renderer.rs` (for tracking loaded state) and `gui.rs` (for displaying loaded info). It uses only `PathBuf` and primitive types — no wgpu deps. This is a natural candidate for extraction into the core crate.

**Proposed dependency DAG:**

```
marl-viewer-core    ← marl-format + serde + serde_json (no GPU/windowing deps)
    ↓
marl-viewer-render  ← marl-viewer-core + wgpu + bytemuck + pollster
    ↓
marl-viewer-rs      ← marl-viewer-core + marl-viewer-render + winit + egui* + rfd
```

## Approach

**Key design decisions:**

1. **`marl-viewer-core`** contains all pure-data and I/O logic: args parsing, file loading, camera math, and shared metadata types. It has zero GPU or windowing dependencies — only `marl-format`, `serde`, and `serde_json`. This crate can be compiled and tested entirely on a headless CI machine.

2. **`marl-viewer-render`** contains the wgpu rendering pipeline: `Renderer`, `ViewerParams`, texture creation, bind group management, and the WGSL shader. It depends on `marl-viewer-core` for snapshot loading and camera basis, plus `wgpu`, `bytemuck`, and `pollster` for GPU interaction.

3. **`marl-viewer-rs`** (the binary) contains the winit application shell, egui GUI panel, and ties everything together. It depends on both core and render crates plus `winit`, `egui`, `egui-wgpu`, `egui-winit`, and `rfd`.

4. **`SnapshotInfo`** (currently in renderer.rs) moves to `marl-viewer-core` since both the render and GUI crates need it. It has no GPU dependencies.

5. **`GuiAction`** (the action enum from gui.rs) moves to `marl-viewer-core` because `renderer.rs` processes `GuiAction` values in a large match block. The GUI crate defines actions, the renderer processes them — the type must live in the shared core crate.

6. **`CellMode`** enum functions used by the renderer (`build_cell_texture_data`, `starter_color`, `energy_color`) move with the renderer into `marl-viewer-render`. These are pure data transformation functions, not I/O.

## Steps

### Phase 1: Create `marl-viewer-core` crate

Extract all non-rendering, non-GUI logic into a standalone crate.

1. **Create the crate scaffold**
   - **Location:** `crates/marl-viewer-core/Cargo.toml`, `crates/marl-viewer-core/src/lib.rs`
   - **Action:** Create `Cargo.toml` with package name `marl-viewer-core`, workspace version/edition, deps: `marl-format` (path), `serde` (workspace), `serde_json` (workspace). Create `src/lib.rs` with `pub mod args; pub mod io; pub mod camera; pub mod types;`.
   - **Verification:** `cargo check -p marl-viewer-core`

2. **Move `args.rs`**
   - **Location:** `crates/marl-viewer-core/src/args.rs`
   - **Action:** Move the entire contents of `crates/marl-viewer-rs/src/args.rs` (405 lines). This includes:
     - `DEFAULT_OUTPUT_DIR` constant
     - `ViewMode` enum with `as_str()` and `all()`
     - `CellMode` enum with `as_str()` and `all()`
     - `ViewerArgs` struct with `parse()`, `parse_from()`, `new()` (if any)
     - `next_value()`, `parse_value()` helpers
     - `usage()` function
     - All `#[cfg(test)]` unit tests (they test pure parsing — no rendering needed)
   - **Verification:** `cargo test -p marl-viewer-core -- args`

3. **Move `io.rs`**
   - **Location:** `crates/marl-viewer-core/src/io.rs`
   - **Action:** Move the entire contents of `crates/marl-viewer-rs/src/io.rs` (450 lines). Update imports:
     - `use marl_format::RunMeta` → unchanged (marl-format is a dep)
     - `use crate::args::{CellMode, ViewerArgs}` → `use crate::args::{CellMode, ViewerArgs}`
     - All `fs`, `PathBuf`, `Error` imports stay the same
   - **Verification:** `cargo test -p marl-viewer-core -- io`
     - Note: `discover_field_ticks` tests create temp directories — they will continue to work since they only need `std::fs`

4. **Move `camera.rs`**
   - **Location:** `crates/marl-viewer-core/src/camera.rs`
   - **Action:** Move the entire contents of `crates/marl-viewer-rs/src/camera.rs` (159 lines). Update imports:
     - `use crate::args::ViewMode` → `use crate::args::ViewMode`
   - **Verification:** `cargo test -p marl-viewer-core -- camera`

5. **Create `types.rs` with shared metadata types**
   - **Location:** `crates/marl-viewer-core/src/types.rs`
   - **Action:** Create this file with the following types extracted from their current locations:
     - `SnapshotInfo` struct (from `renderer.rs` lines 51-62): fields `output_dir: PathBuf`, `tick: u64`, `species: u32`, `view_mode: ViewMode`, `cell_mode: CellMode`, `cell_count: usize`, `field_bytes: usize`, `grid: [u32; 3]`, `s_ext: u32`. Derive `Debug, Clone`.
     - `GuiAction` enum (from `gui.rs` lines 179-191): `OpenDirectoryDialog`, `LoadDirectory(PathBuf)`, `LoadTick(u64)`, `ReloadCurrent`, `FirstTick`, `LastTick`, `PrevTick`, `NextTick`, `ApplyViewSettings`, `ResetDraftFromLoaded`. Derive `Debug, Clone`.
     - The `LoadedCell` struct is already in `io.rs` and stays there — it's not shared outside I/O.
   - **Verification:** `cargo check -p marl-viewer-core`

6. **Add workspace member**
   - **Location:** `Cargo.toml` (workspace root)
   - **Action:** Add `"crates/marl-viewer-core"` to `[workspace] members`.
   - **Verification:** `cargo check --workspace`

### Phase 2: Create `marl-viewer-render` crate

Extract the wgpu rendering pipeline, textures, and GPU resource management.

7. **Create the crate scaffold**
   - **Location:** `crates/marl-viewer-render/Cargo.toml`, `crates/marl-viewer-render/src/lib.rs`
   - **Action:** Create `Cargo.toml` with package name `marl-viewer-render`, deps: `marl-format` (path), `marl-viewer-core` (path), `bytemuck` (workspace), `pollster` (workspace), `wgpu` (workspace). Create `src/lib.rs` with `pub mod renderer;`.
   - **Verification:** `cargo check -p marl-viewer-render`

8. **Move and adapt `renderer.rs`**
   - **Location:** `crates/marl-viewer-render/src/renderer.rs`
   - **Action:** Move the contents of `crates/marl-viewer-rs/src/renderer.rs` (1355 lines). Update all imports:
     - `use crate::args::{CellMode, ViewMode, ViewerArgs}` → `use marl_viewer_core::args::{CellMode, ViewMode, ViewerArgs}`
     - `use crate::camera::{CameraBasis, camera_basis}` → `use marl_viewer_core::camera::{CameraBasis, camera_basis}`
     - `use crate::gui::{GuiAction, GuiState, choose_initial_tick, neighbor_tick}` → these types/functions now live in `marl-viewer-core`. `GuiState` stays in the GUI (Phase 3) — the renderer's match block on `GuiAction` arms only needs the enum (now in `marl_viewer_core::types::GuiAction`). The `choose_initial_tick` and `neighbor_tick` functions are currently in `gui.rs` — they should be extracted or referenced from the core crate. **Action:** move `choose_initial_tick` and `neighbor_tick` to `marl-viewer-core/src/types.rs` (they are pure functions with no UI dependencies — they take `&[u64]` and return `Option<u64>`).
     - `use crate::io::{LoadedCell, SnapshotPayload, discover_field_ticks, load_run_meta, load_snapshot}` → `use marl_viewer_core::io::{...}`
     - `SnapshotInfo` is now in `marl_viewer_core::types::SnapshotInfo`
     - `use std::borrow::Cow` stays
     - `use wgpu::util::DeviceExt` stays
     - `use winit::dpi::PhysicalSize` and `use winit::event::WindowEvent` — the renderer uses winit types for resize/window events. These are winit types but used functionally (method signatures). Keep them — `marl-viewer-render` depends on `winit` for these types.
     - `use winit::window::Window` — `Arc<Window>` is needed for the renderer. Add `winit` to `marl-viewer-render/Cargo.toml` deps (workspace).
   - **Verification:** `cargo check -p marl-viewer-render`

9. **Move the WGSL shader**
   - **Location:** `crates/marl-viewer-render/src/viewer_raymarch.wgsl`
   - **Action:** Copy `crates/marl-viewer-rs/src/viewer_raymarch.wgsl` to `crates/marl-viewer-render/src/viewer_raymarch.wgsl`. The `include_str!("viewer_raymarch.wgsl")` in renderer.rs will resolve correctly from the new location (same directory).
   - **Verification:** Confirm the `include_str!` path works: `cargo check -p marl-viewer-render`

10. **Move renderer tests**
    - **Location:** `crates/marl-viewer-render/src/renderer.rs` (inline `#[cfg(test)]` at lines 1210-1355)
    - **Action:** The tests are already embedded in renderer.rs via `#[cfg(test)] mod tests`. They move with the file. Update test imports if they reference `crate::args` etc.
    - **Verification:** `cargo test -p marl-viewer-render`

### Phase 3: Create `marl-viewer-rs` binary crate

Rebuild the binary as a thin crate that wires core + render + gui.

11. **Create `marl-viewer-rs/gui.rs`**
    - **Location:** `crates/marl-viewer-rs/src/gui.rs`
    - **Action:** Move the contents of `crates/marl-viewer-rs/src/gui.rs` (515 lines). Update imports:
      - `use crate::args::{CellMode, ViewMode, ViewerArgs}` → `use marl_viewer_core::args::{CellMode, ViewMode, ViewerArgs}`
      - `use crate::renderer::SnapshotInfo` → `use marl_viewer_core::types::SnapshotInfo`
      - `GuiAction` is now defined in `marl_viewer_core::types`, but `GuiState::show()` returns `Vec<GuiAction>` and the enum needs to be in scope. Either re-export through the gui module or import `marl_viewer_core::types::GuiAction`.
      - `choose_initial_tick` and `neighbor_tick` move to `marl-viewer-core` (see step 8). Remove their definitions from gui.rs. Import them from `marl_viewer_core::types` (or wherever they land).
      - Keep the `egui` imports as-is.
    - **Verification:** `cargo check -p marl-viewer-rs` (step 13 after all moves)

12. **Update `marl-viewer-rs/src/main.rs` and `app.rs`**
    - **Location:** `crates/marl-viewer-rs/src/main.rs`, `crates/marl-viewer-rs/src/app.rs`
    - **Action:** These files stay in `marl-viewer-rs`. Update their imports:
      - `main.rs`: `use app::ViewerApp;` stays. `use args::ViewerArgs;` → `use marl_viewer_core::args::ViewerArgs`. The module declarations become `mod app; mod gui;` (remove `mod args; mod camera; mod io; mod renderer;` since those are now external crates).
      - `app.rs`: `use crate::args::ViewerArgs` → `use marl_viewer_core::args::ViewerArgs`. `use crate::renderer::{RenderResult, Renderer}` → `use marl_viewer_render::renderer::{RenderResult, Renderer}`.
    - **Verification:** `cargo check -p marl-viewer-rs`

13. **Update `marl-viewer-rs/Cargo.toml`**
    - **Location:** `crates/marl-viewer-rs/Cargo.toml`
    - **Action:** Update dependencies:
      - Add: `marl-viewer-core = { path = "../marl-viewer-core" }`, `marl-viewer-render = { path = "../marl-viewer-render" }`
      - Remove: `marl-format`, `serde`, `serde_json` (they're now transitive via marl-viewer-core; keep only if directly used — they are not, since io.rs and args.rs moved out)
      - Keep: `bytemuck`, `egui`, `egui-wgpu`, `egui-winit`, `pollster`, `rfd`, `wgpu`, `winit` (all still used by app.rs, gui.rs, or the renderer — though wgpu/bytemuck/pollster become transitive via marl-viewer-render; explicit for clarity)
      - Optimization: `wgpu`, `bytemuck`, `pollster` are only needed transitively through `marl-viewer-render`. They can be removed from the binary's direct deps but keeping them is harmless and clearer.
    - **Verification:** `cargo check -p marl-viewer-rs`

14. **Delete moved files from `marl-viewer-rs`**
    - **Location:** `crates/marl-viewer-rs/src/`
    - **Action:** Remove `args.rs`, `io.rs`, `camera.rs`, `renderer.rs`, `viewer_raymarch.wgsl` (moved to marl-viewer-core and marl-viewer-render). Keep only `main.rs`, `app.rs`, `gui.rs`.
    - **Verification:** `cargo check -p marl-viewer-rs` passes cleanly

### Phase 4: Integration and verification

15. **Run full workspace build**
    - **Location:** Workspace root
    - **Action:** `cargo build --workspace --release`
    - **Verification:** Zero errors, zero warnings across all crates

16. **Run full test suite**
    - **Location:** Workspace root
    - **Action:** `cargo test --workspace`
    - **Verification:** All tests pass — especially `marl-viewer-core` tests (args parsing, io, camera)

17. **Verify viewer launches**
    - **Location:** Workspace root
    - **Action:** `cargo run -p marl-viewer-rs --release -- --help`
    - **Verification:** Prints usage message and exits cleanly. (Cannot verify GUI rendering in a headless env, but the `--help` path exercises arg parsing + module linking.)

18. **Run `cargo fmt`**
    - **Location:** Workspace root
    - **Action:** `cargo fmt --all`
    - **Verification:** No formatting changes (or only cosmetic whitespace)

19. **Update workspace `Cargo.toml` members**
    - **Location:** `Cargo.toml` (workspace root)
    - **Action:** Ensure `[workspace] members` includes `"crates/marl-viewer-core"` and `"crates/marl-viewer-render"` alongside existing entries.
    - **Verification:** `cargo metadata --format-version=1 --no-deps | jq '.workspace_members'` shows all 5 viewer-related crates

20. **Update agent context files**
    - **Location:** `.agents/context/MAP.md`, `.agents/context/STATUS.md`, `.agents/context/NOTES.md`
    - **Action:** Update MAP.md to reflect the new viewer crate topology. Add STATUS.md entry. Add NOTES.md entries documenting:
      - `marl-viewer-core` contains all pure-data and I/O logic (args, io, camera, shared types) — zero GPU/windowing deps
      - `marl-viewer-render` contains the wgpu rendering pipeline and shader
      - `marl-viewer-rs` is the thin binary + winit app + egui GUI
      - `SnapshotInfo` and `GuiAction` live in `marl-viewer-core::types` as shared metadata
      - `choose_initial_tick` and `neighbor_tick` live in `marl-viewer-core::types` (pure functions)
    - **Verification:** Visual review

## Final Crate Layout

```
crates/
  marl-viewer-core/      Pure data + I/O (no GPU/windowing deps)
    Cargo.toml
    src/lib.rs            pub mod args; pub mod io; pub mod camera; pub mod types;
    src/args.rs            ViewerArgs, ViewMode, CellMode, CLI parsing, usage()
    src/io.rs              SnapshotPayload, LoadedCell, load_snapshot, discover_field_ticks, cell record parsing
    src/camera.rs          CameraBasis, camera_basis()
    src/types.rs           SnapshotInfo, GuiAction, choose_initial_tick(), neighbor_tick()

  marl-viewer-render/     wgpu rendering pipeline
    Cargo.toml
    src/lib.rs             pub mod renderer;
    src/renderer.rs         Renderer, ViewerParams, texture creation, bind groups, egui overlay pass, action processing
    src/viewer_raymarch.wgsl  Full-screen raymarch compute shader

  marl-viewer-rs/          Thin binary + winit app + egui GUI
    Cargo.toml
    src/main.rs             Entry point (<30 lines)
    src/app.rs              ViewerApp ApplicationHandler
    src/gui.rs              GuiState, GUI drawing (toolbar, sidebar, view settings)
```

## Dependency Summary

```
marl-viewer-core
  deps: marl-format, serde, serde_json

marl-viewer-render
  deps: marl-format, marl-viewer-core, wgpu, bytemuck, pollster, winit

marl-viewer-rs
  deps: marl-viewer-core, marl-viewer-render, winit, egui, egui-wgpu, egui-winit, rfd
```

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Circular dep between core and render via `SnapshotInfo` | Low | High | Resolved by placing `SnapshotInfo` in `marl-viewer-core::types` — a leaf module with zero deps beyond `std` |
| `GuiAction` must be shared but gui.rs needs egui | Low | Medium | The `GuiAction` enum has no egui types — just `PathBuf` and `u64`. It lives cleanly in core. |
| `choose_initial_tick` / `neighbor_tick` are in gui.rs but used by renderer | Low | Medium | These are pure functions on `&[u64]` with no GUI deps. Move them to `marl-viewer-core::types`. |
| `renderer.rs` uses winit types (`PhysicalSize`, `WindowEvent`, `Window`) | Medium | Medium | `marl-viewer-render` already depends on `winit` for these types — they are windowing primitives, not GUI logic. No extra dependency cost. |
| Viewer shader `include_str!` path breaks | Low | Low | The shader moves with renderer.rs to the same directory. `include_str!("viewer_raymarch.wgsl")` resolves identically. |
| Tests that create temp dirs (`discover_field_ticks`) need filesystem access | Low | Low | These tests already work; they only need `std::fs` which is available everywhere. |

## Verification Strategy

1. `cargo build --workspace --release` — zero errors
2. `cargo test --workspace` — all tests pass
3. `cargo run -p marl-viewer-rs --release -- --help` — usage printed, exit 0
4. `cargo check -p marl-viewer-core` — builds independently (headless CI compatible)
5. `cargo fmt --all` — consistent formatting
6. Agent context files updated with new crate topology
