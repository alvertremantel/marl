# Viewer GUI Shell Around WGSL Renderer

**Date:** 2026-05-09
**Status:** draft

---

## Goal

Add a minimal native GUI shell around the current `marl-viewer-rs` WGSL/wgpu renderer so users can launch the viewer, choose an output directory, and move between available snapshot ticks with buttons instead of restarting the binary with different CLI flags. Preserve the existing shader, binary snapshot format, and CLI flags as initial/default values.

If the core GUI integration is straightforward, expose the existing view configuration flags (`species`, `view`, `cells`, `cell-alpha`, `scale`, `exposure`, `steps`) as simple GUI controls too. These controls are secondary; directory loading and tick navigation are the MVP.

## Understanding

Recent commit reviewed: `a31a4bd viewer: add isometric 3D volume rendering and direct microbe voxel overlay`.

Current viewer state after that commit:

- `crates/marl-viewer-rs/src/main.rs:18-39` parses `ViewerArgs`, immediately calls `io::load_snapshot(&args)`, prints a load message, then starts the `winit` app. If the default or provided data directory is missing, the process exits before a window exists. A GUI directory picker therefore requires decoupling window startup from snapshot load success.
- `crates/marl-viewer-rs/src/app.rs:13-100` owns `ViewerApp { payload, args, renderer }`. On `resumed`, it creates one `winit::window::Window` and calls `Renderer::new(window, payload, args)`. Event handling only forwards resize/redraw/close; there is no input/UI layer.
- `crates/marl-viewer-rs/src/args.rs:27-38` holds all current runtime view settings: `output_dir`, `tick`, `species`, `exposure`, `density_scale`, `steps`, `view_mode`, `cell_mode`, `cell_alpha`. Existing CLI parsing and tests should remain valid; GUI controls should initialize from these fields.
- `crates/marl-viewer-rs/src/io.rs:43-96` loads exactly one tick into `SnapshotPayload`, validating `run_meta.json`, `tick_<T>.field.bin`, and optional `tick_<T>.cells.bin`. There is no helper to discover which ticks exist in an output directory.
- `crates/marl-viewer-rs/src/renderer.rs:34-370` owns the wgpu surface/device/queue/pipeline, a single bind group, params buffer, field texture, and cell texture. `Renderer::render()` acquires a surface frame, runs one raymarch render pass using `viewer_raymarch.wgsl`, submits, and presents. Snapshot resources are not reloadable after construction.
- `crates/marl-viewer-rs/src/viewer_raymarch.wgsl` already produces the desired 3D output. The GUI should wrap/overlay this output, not rewrite the shader.
- Shared metadata lives in `crates/marl-format/src/lib.rs`; `RunMeta` includes grid dimensions, `s_ext`, field byte length, and binary cell flags. Snapshot files are named `tick_<T>.field.bin` and `tick_<T>.cells.bin`.
- Existing README/context files in the working tree already describe the enhanced 3D viewer. Preserve those edits and only extend them for GUI behavior if implementation lands.

Dependency research:

- `egui-wgpu 0.34.2` depends on `wgpu 29.0.1`, matching the workspace's `wgpu = "29"` line.
- `egui-winit 0.34.2` depends on `winit 0.30.13`, matching the workspace's `winit = "0.30"` line.
- `egui 0.34.2` / `egui-wgpu 0.34.2` require Rust `1.92`; the local toolchain is `rustc 1.95.0`, so the version is usable here.
- `egui_winit::State::new(...)`, `State::on_window_event(...)`, `State::take_egui_input(...)`, and `State::handle_platform_output(...)` are the correct integration points for winit 0.30.
- `egui_wgpu::Renderer::new(...)`, `update_texture(...)`, `update_buffers(...)`, `render(...)`, `free_texture(...)`, and `ScreenDescriptor` are the correct wgpu integration points. `egui_wgpu::Renderer::render` expects `wgpu::RenderPass<'static>`, so call `render_pass.forget_lifetime()` on the egui pass.

Constraints:

- Do not change engine output or `marl-format` binary schema.
- Keep all current CLI flags and defaults. CLI values become initial GUI state, not removed behavior.
- The viewer must still support legacy/top field-only rendering through current settings.
- Use `/tmp/opencode/...` for verification output data to avoid adding more untracked files under repository `output/` (which is not currently ignored).
- Do not commit or depend on local runtime artifacts such as `output/` or `.worktrees/`.

## Approach

Use `egui` through `egui-winit` + `egui-wgpu`, not `eframe`. `eframe` would own the event loop/device/surface and force a larger rewrite or custom paint callback; direct egui integration lets the existing WGSL raymarch remain the first/background render pass on the same wgpu surface, followed by an egui overlay pass for controls.

Make the renderer capable of existing without a successfully loaded snapshot. On startup, build a 1×1×1 all-zero placeholder texture/bind group so the window and GUI can open even if the current/default output directory is invalid. Loading a directory or changing ticks then replaces the GPU snapshot resources in place.

Add a small GUI state/action layer:

- Top toolbar or left side panel with:
  - output directory text field,
  - `Open…` native directory picker button (via `rfd`) plus text-field fallback,
  - `Load Directory`,
  - current loaded tick/grid/cell summary,
  - `First`, `Prev`, numeric tick entry + `Go`, `Reload`, `Next`, `Last`.
- Tick navigation uses discovered `tick_<T>.field.bin` files sorted numerically, not `tick + 1`, so snapshot intervals larger than one work.
- Failed loads keep the previous good snapshot displayed and show the error in the GUI status area. If there was no previous snapshot, the placeholder stays visible.
- Optional view controls use a simple `Apply View Settings` button. For MVP simplicity, applying these settings may reload the current snapshot from disk instead of trying to micro-optimize uniform-only changes.

Parallelization boundaries after dependency versions are added:

- `io.rs` tick discovery helpers and tests are independent of egui/wgpu UI work.
- `gui.rs` state/action helpers and pure tick-selection tests are independent of renderer resource refactoring.
- Renderer egui integration and reloadable GPU resources both touch `renderer.rs` and should be done sequentially by one agent.
- Docs/context updates should wait until GUI behavior and dependency choices are final.

## Steps

### Phase 1: Dependencies and small reusable helpers

1. **Add GUI dependencies compatible with current wgpu/winit**
   - **Location:** root `Cargo.toml`, `crates/marl-viewer-rs/Cargo.toml`
   - **Action:**
     - Add workspace dependencies:
       - `egui = "0.34.2"`
       - `egui-wgpu = "0.34.2"`
       - `egui-winit = "0.34.2"`
       - `rfd = "0.17.2"` for native folder picking; keep the GUI text field as fallback if `pick_folder()` returns `None`.
     - Add those dependencies to `crates/marl-viewer-rs/Cargo.toml` with `.workspace = true`.
     - Do not add `eframe`.
   - **Verification:** Run `cargo check -p marl-viewer-rs`; run `cargo tree -p marl-viewer-rs -i wgpu` and `cargo tree -p marl-viewer-rs -i winit` and confirm only one `wgpu 29.x` and one `winit 0.30.x` are selected.

2. **Add tick discovery helpers**
   - **Location:** `crates/marl-viewer-rs/src/io.rs`
   - **Action:**
     - Add `pub(crate) fn load_run_meta(output_dir: &std::path::Path) -> Result<RunMeta, Box<dyn Error>>` by extracting the existing `run_meta.json` read/parse/`validate()` logic from `load_snapshot`.
     - Update `load_snapshot(&ViewerArgs)` to call `load_run_meta(&args.output_dir)` before species/file validation.
     - Add `pub(crate) fn discover_field_ticks(output_dir: &std::path::Path) -> Result<Vec<u64>, Box<dyn Error>>` that scans `std::fs::read_dir(output_dir)`, accepts only exact file names matching `tick_<digits>.field.bin`, parses the tick as `u64`, sorts ascending, deduplicates, and returns an error if the directory cannot be read. Returning an empty vector for a readable directory with no field files is acceptable; the GUI will display "no snapshots found".
     - Add pure helper `fn parse_field_tick_file_name(name: &str) -> Option<u64>` for unit tests.
   - **Verification:** Add/update `io.rs` tests for valid names, malformed names, sorted discovery, empty discovery, and unreadable directory errors. Run `cargo test -p marl-viewer-rs io`.

3. **Add labels/conversions for view modes**
   - **Location:** `crates/marl-viewer-rs/src/args.rs`
   - **Action:**
     - Add methods usable by GUI labels, e.g. `ViewMode::as_str() -> &'static str`, `ViewMode::all() -> [ViewMode; 2]`, `CellMode::as_str() -> &'static str`, `CellMode::all() -> [CellMode; 3]`.
     - Keep the existing parser behavior and tests unchanged except for adding tests for these helpers.
   - **Verification:** Run `cargo test -p marl-viewer-rs args`.

### Phase 2: Make snapshot GPU resources reloadable

4. **Remove hard startup dependency on a valid snapshot**
   - **Location:** `crates/marl-viewer-rs/src/main.rs`, `crates/marl-viewer-rs/src/app.rs`
   - **Action:**
     - In `main.rs`, stop calling `load_snapshot(&args)` before creating the event loop.
     - Change `ViewerApp::new(...)` to accept only `ViewerArgs` (plus optional initial status if desired), not `SnapshotPayload`.
     - In `ViewerApp::resumed`, call `Renderer::new(window, args)` instead of `Renderer::new(window, payload, args)`.
     - Preserve `--help` behavior: parsing `--help` still prints usage and exits with status `0` before opening a window.
     - Initial snapshot load should be attempted inside `Renderer::new`; load failure should become a GUI-visible status message and stderr warning, not process exit.
   - **Verification:** Run `cargo run -p marl-viewer-rs -- --help` and confirm usage still prints and exits. Run `cargo check -p marl-viewer-rs` after adapting signatures.

5. **Introduce reloadable snapshot resource structs**
   - **Location:** `crates/marl-viewer-rs/src/renderer.rs`
   - **Action:**
     - Add a private `SnapshotGpuResources` struct holding the currently bound snapshot GPU state:
       - `bind_group: wgpu::BindGroup`
       - `params_buffer: wgpu::Buffer`
       - `params: ViewerParams`
       - `field_texture: wgpu::Texture`
       - `field_view: wgpu::TextureView`
       - `cell_texture: wgpu::Texture`
       - `cell_view: wgpu::TextureView`
     - Add a private/public-for-GUI `SnapshotInfo` struct with only lightweight display data:
       - `output_dir: PathBuf`
       - `tick: u64`
       - `species: u32`
       - `view_mode: ViewMode`
       - `cell_mode: CellMode`
       - `cell_count: usize`
       - `field_bytes: usize`
       - `grid: [u32; 3]`
       - `s_ext: u32`
     - Store `bind_group_layout: wgpu::BindGroupLayout`, `snapshot: SnapshotGpuResources`, `loaded_info: Option<SnapshotInfo>`, and mutable `args: ViewerArgs` in `Renderer`.
     - Extract current params construction into `fn build_viewer_params(payload: &SnapshotPayload, args: &ViewerArgs, width: u32, height: u32) -> Result<ViewerParams, Box<dyn Error>>`.
     - Extract bind-group creation into `fn create_snapshot_bind_group(device, bind_group_layout, field_view, params_buffer, cell_view) -> wgpu::BindGroup`.
     - Keep `create_field_texture`, `create_cell_texture`, `create_empty_cell_texture`, and `build_cell_texture_data` behavior unchanged except for adapting ownership/field names.
   - **Verification:** Run `cargo test -p marl-viewer-rs renderer` and `cargo check -p marl-viewer-rs`.

6. **Add placeholder snapshot resources**
   - **Location:** `crates/marl-viewer-rs/src/renderer.rs`
   - **Action:**
     - Add `fn placeholder_payload(args: &ViewerArgs) -> SnapshotPayload` using `RunMeta::new(1, 1, 1, 1, 0, true, false)`, `field_bytes = vec![0, 0, 0, 0]`, no cells, `tick = args.tick`, `species = 0`, and cell mode forced off for shader options.
     - Use that placeholder to create valid 1×1×1 field/cell textures and a valid bind group when initial `load_snapshot(&args)` fails.
     - Set `loaded_info = None` for placeholders and use a window title like `MARL Viewer - no snapshot loaded`.
     - Add `fn set_window_title(&self)` or equivalent to centralize title updates for loaded vs placeholder states.
   - **Verification:** Add a unit test for `placeholder_payload` dimensions/byte length if helper visibility allows. Run `cargo test -p marl-viewer-rs renderer`.

7. **Implement atomic reload/apply behavior**
   - **Location:** `crates/marl-viewer-rs/src/renderer.rs`
   - **Action:**
     - Add `fn try_load_snapshot_resources(&self, args: &ViewerArgs) -> Result<(SnapshotGpuResources, SnapshotInfo), Box<dyn Error>>` or equivalent. It must:
       - call `io::load_snapshot(args)`,
       - create new field/cell textures,
       - build new `ViewerParams` using current surface width/height,
       - create a new params buffer and bind group,
       - return resources without mutating existing renderer state until all steps succeed.
     - Add `fn apply_args(&mut self, new_args: ViewerArgs) -> Result<(), Box<dyn Error>>` that calls the above helper and replaces `self.snapshot`, `self.loaded_info`, and `self.args` only on success.
     - On failure, keep the old snapshot/placeholder resources intact and return the error for GUI status.
     - Update `resize()` to mutate `self.snapshot.params.render[0..2]` and write to `self.snapshot.params_buffer` instead of old top-level fields.
     - Update `render()` to use the existing `self.pipeline` plus `self.snapshot` resources (`bind_group`, `params`, etc.) instead of old top-level snapshot fields.
   - **Verification:** Run `cargo test -p marl-viewer-rs renderer` and `cargo check -p marl-viewer-rs`. Manually run with a deliberately missing path on a display-capable machine later to confirm the window opens instead of exiting.

### Phase 3: Add egui integration over the WGSL output

8. **Create GUI state and action module**
   - **Location:** new `crates/marl-viewer-rs/src/gui.rs`, `crates/marl-viewer-rs/src/main.rs`
   - **Action:**
     - Add `mod gui;` in `main.rs`.
     - Define `pub(crate) struct GuiState` initialized from `ViewerArgs` with fields like:
       - `directory_text: String`
       - `tick_text: String`
       - `available_ticks: Vec<u64>`
       - draft view fields copied from `ViewerArgs` (`species`, `view_mode`, `cell_mode`, `cell_alpha`, `density_scale`, `exposure`, `steps`)
       - `status: Option<GuiStatus>` where status records info/error text.
     - Define `pub(crate) enum GuiAction` with at least:
       - `OpenDirectoryDialog`
       - `LoadDirectory(PathBuf)`
       - `LoadTick(u64)`
       - `ReloadCurrent`
       - `StepTick(i32)` or explicit `First/Prev/Next/Last`
       - optional `ApplyViewSettings(ViewerArgs)` for Phase 5.
     - Add pure helpers for tick selection:
       - `choose_initial_tick(requested: u64, available: &[u64]) -> Option<u64>`: return requested if present, otherwise return `0` if present, otherwise first/min tick.
       - `neighbor_tick(current: u64, available: &[u64], delta: i32) -> Option<u64>`: navigate sorted discovered ticks, clamping at ends.
     - Add `GuiState::sync_loaded(&mut self, info: &SnapshotInfo, args: &ViewerArgs, available_ticks: Vec<u64>)` to update text/drafts after a successful load.
     - Add `GuiState::set_error(...)` / `set_info(...)` helpers.
   - **Verification:** Add `gui.rs` tests for `choose_initial_tick`, `neighbor_tick`, tick text parsing, and sync behavior. Run `cargo test -p marl-viewer-rs gui`.

9. **Initialize egui/winit/wgpu objects in the renderer**
   - **Location:** `crates/marl-viewer-rs/src/renderer.rs`, `crates/marl-viewer-rs/src/app.rs`
   - **Action:**
     - Add renderer fields:
       - `egui_ctx: egui::Context`
       - `egui_state: egui_winit::State`
       - `egui_renderer: egui_wgpu::Renderer` (import as `EguiRenderer` to avoid naming conflict)
       - `gui: GuiState`
     - Initialize after wgpu `device` exists:
       - `let egui_ctx = egui::Context::default();`
       - `let egui_state = egui_winit::State::new(egui_ctx.clone(), egui::ViewportId::ROOT, window.as_ref(), Some(window.scale_factor() as f32), window.theme(), Some(device.limits().max_texture_dimension_2d as usize));`
       - `let egui_renderer = egui_wgpu::Renderer::new(&device, surface_format, egui_wgpu::RendererOptions::default());`
       - `let gui = GuiState::new(&args);`
     - Add `Renderer::handle_window_event(&mut self, event: &WindowEvent) -> egui_winit::EventResponse` that calls `self.egui_state.on_window_event(&self.window, event)`, requests redraw if `response.repaint`, and returns the response.
     - In `app.rs::window_event`, call `renderer.handle_window_event(&event)` before the `match`. Still always handle `CloseRequested`, `Resized`, `ScaleFactorChanged`, and `RedrawRequested` even if egui consumed the event.
   - **Verification:** Run `cargo check -p marl-viewer-rs`. Run `cargo test -p marl-viewer-rs` to catch import/module issues.

10. **Render egui as an overlay pass**
    - **Location:** `crates/marl-viewer-rs/src/renderer.rs`, `crates/marl-viewer-rs/src/gui.rs`
    - **Action:**
      - Add `GuiState::show(&mut self, ctx: &egui::Context, loaded: Option<&SnapshotInfo>, args: &ViewerArgs) -> Vec<GuiAction>` that draws a simple top toolbar or left side panel. Initial MVP contents:
        - directory label + text edit,
        - `Open…` and `Load Directory` buttons,
        - current tick/grid/cell summary or "No snapshot loaded",
        - `First`, `Prev`, tick text input, `Go`, `Reload`, `Next`, `Last`,
        - status/error label.
      - In `Renderer::render()` before acquiring or drawing the surface frame:
        - collect egui input with `self.egui_state.take_egui_input(&self.window)`,
        - call `self.egui_ctx.begin_pass(raw_input)`,
        - call `self.gui.show(...)` and collect actions,
        - call `let full_output = self.egui_ctx.end_pass()`,
        - destructure or move `full_output` fields carefully, then call `self.egui_state.handle_platform_output(&self.window, platform_output)`,
        - process `GuiAction`s (actual action behavior is filled in Phase 4), and request redraw when actions change state.
      - Tessellate with `let paint_jobs = self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);`.
      - For each `full_output.textures_delta.set`, call `self.egui_renderer.update_texture(&self.device, &self.queue, id, image_delta)`.
      - Create `egui_wgpu::ScreenDescriptor { size_in_pixels: [self.config.width, self.config.height], pixels_per_point: full_output.pixels_per_point }`.
      - Call `self.egui_renderer.update_buffers(&self.device, &self.queue, &mut encoder, &paint_jobs, &screen_descriptor)` before render passes and submit returned command buffers together with the frame encoder.
      - Keep the existing raymarch pass as the first pass with `LoadOp::Clear`.
      - Add a second render pass on the same frame view with `LoadOp::Load`; call `let mut pass = pass.forget_lifetime(); self.egui_renderer.render(&mut pass, &paint_jobs, &screen_descriptor);`.
      - After rendering, call `self.egui_renderer.free_texture(&id)` for each `full_output.textures_delta.free`.
      - Preserve `RenderResult::{Drawn, Skip, Reconfigure}` behavior for surface acquisition.
    - **Verification:** Run `cargo check -p marl-viewer-rs`. On a display/GPU-capable machine, run `cargo run -p marl-viewer-rs -- /tmp/opencode/definitely-missing-marl-dir` and verify a window opens with a GUI status error instead of exiting.

### Phase 4: Directory loading and tick navigation behavior

11. **Wire directory loading actions**
    - **Location:** `crates/marl-viewer-rs/src/renderer.rs`, `crates/marl-viewer-rs/src/gui.rs`, `crates/marl-viewer-rs/src/io.rs`
    - **Action:**
      - Add renderer method `fn load_directory_from_gui(&mut self, dir: PathBuf) -> Result<(), Box<dyn Error>>`:
        - call `discover_field_ticks(&dir)`,
        - if no ticks are found, return an error like `no tick_*.field.bin snapshots found in <dir>`,
        - choose a tick with `choose_initial_tick(self.args.tick, &ticks)`,
        - create `new_args = self.args.clone()` with `output_dir = dir` and chosen `tick`,
        - optionally call `load_run_meta(&new_args.output_dir)` first to clamp `new_args.species` to `meta.s_ext - 1` if the current species is out of range,
        - call `self.apply_args(new_args)`,
        - on success call `self.gui.sync_loaded(...)` with the discovered ticks.
      - For `GuiAction::OpenDirectoryDialog`, call `rfd::FileDialog::new().set_directory(current_directory_if_valid).pick_folder()`. If it returns a path, call the same load-directory method. If it returns `None`, leave state unchanged and optionally set an info status `directory selection cancelled`.
      - For `GuiAction::LoadDirectory(PathBuf)`, load the path typed in the text field.
      - Always keep typed path fallback functional even if the native picker fails on a platform.
    - **Verification:** Run `cargo test -p marl-viewer-rs gui`, `cargo test -p marl-viewer-rs io`, and `cargo check -p marl-viewer-rs`. Manual: generate smoke data under `/tmp/opencode/marl-viewer-gui-smoke`, launch with a missing directory, use the GUI path field or `Open…` button to load the smoke directory, and verify the volume appears.

12. **Wire tick navigation actions**
    - **Location:** `crates/marl-viewer-rs/src/renderer.rs`, `crates/marl-viewer-rs/src/gui.rs`
    - **Action:**
      - For `First`/`Last`, load the first/last tick from `GuiState.available_ticks`.
      - For `Prev`/`Next`, call `neighbor_tick(current_tick, &available_ticks, -1 or +1)` and load the returned tick if any.
      - For `Go`, parse `tick_text` as `u64`; require that `tick_<T>.field.bin` exists or that `T` is in `available_ticks`; on failure show an error and keep the old snapshot.
      - For `Reload`, reload `self.args` exactly as currently loaded.
      - Disable or visually gray navigation buttons when no tick list is available or the current tick is at the end. If full disabling is awkward in egui, no-op with a status message is acceptable.
      - After each successful tick load, refresh `available_ticks` by rescanning the directory so newly written simulation ticks can be picked up by pressing `Reload` or another directory/tick action.
      - Update the window title and GUI summary after successful load.
    - **Verification:** Add GUI helper tests for navigation edge cases (`current below first`, `current above last`, empty ticks, non-contiguous ticks like `[0, 500, 1000]`). Run `cargo test -p marl-viewer-rs gui`. Manual: with smoke data containing multiple ticks, click `Next`, `Prev`, `Last`, `First`, and `Go` and verify the title/summary tick changes and bad tick input shows an error without blanking the old render.

### Phase 5: Optional existing view configuration controls

13. **Expose existing CLI view settings in the GUI if integration remains simple**
    - **Location:** `crates/marl-viewer-rs/src/gui.rs`, `crates/marl-viewer-rs/src/renderer.rs`
    - **Action:**
      - Add a collapsible `View Settings` section or side panel with:
        - `species` drag value clamped to `0..=s_ext-1` when metadata is loaded,
        - `view_mode` radio buttons or combo (`iso`, `top`),
        - `cell_mode` radio buttons or combo (`off`, `starter`, `energy`),
        - `cell_alpha` slider `(0.01..=1.0)`,
        - `density_scale` slider/drag `(0.01..=20.0)` with current default `2.0`,
        - `exposure` slider/drag `(0.1..=100.0)` with current default `18.0`,
        - `steps` drag value `1..=4096` with current default `160`,
        - `Apply View Settings` and `Reset Draft From Loaded` buttons.
      - On `Apply View Settings`, construct a `ViewerArgs` from current renderer args plus drafts and call `self.apply_args(new_args)`. This full reload is acceptable for the first GUI version; optimize uniform-only settings later if needed.
      - Validate the same constraints as `ViewerArgs::parse_from`: positive finite exposure/scale, steps > 0, cell alpha in `(0, 1]`, species in range when metadata is loaded.
      - If this phase becomes non-trivial or destabilizes core directory/tick behavior, skip it and leave all existing CLI flags documented as still supported initial settings.
    - **Verification:** Run `cargo test -p marl-viewer-rs gui`, `cargo test -p marl-viewer-rs args`, `cargo test -p marl-viewer-rs renderer`, and `cargo check -p marl-viewer-rs`. Manual: change `species`, `view`, `cells`, and `cell-alpha` in the GUI, click Apply, and verify the render/title/status updates. Also verify CLI flags still set initial GUI draft values, e.g. `cargo run -p marl-viewer-rs -- /tmp/opencode/marl-viewer-gui-smoke --tick 1 --view top --cells off --species 1`.

### Phase 6: Documentation and context

14. **Update user-facing viewer docs**
    - **Location:** `README.md` viewer section
    - **Action:**
      - Update the standalone viewer section to mention the GUI shell, directory picker/text field, and tick buttons.
      - Keep CLI examples, but clarify CLI flags now initialize the GUI/view rather than being the only way to change views.
      - If Phase 5 is implemented, document that view settings can be adjusted in the GUI; otherwise state that view settings remain CLI-only for now.
    - **Verification:** Read the README section and ensure command syntax matches `args.rs::usage()` and actual GUI behavior.

15. **Update durable agent context after implementation lands**
    - **Location:** `.agents/context/STATUS.md`, `.agents/context/NOTES.md`, `.agents/context/MAP.md`
    - **Action:**
      - In `STATUS.md`, add a completed entry for the viewer GUI shell with changed files and verification commands actually run.
      - In `NOTES.md`, record durable decisions: `egui-winit`/`egui-wgpu` were chosen over `eframe`; the WGSL raymarch remains the background pass; snapshot resources are reloadable; startup uses placeholder resources when no data is loaded.
      - In `MAP.md`, update `marl-viewer-rs` file descriptions to include `gui.rs` and egui overlay responsibilities.
      - Do not claim manual GPU/display checks were run unless they actually were.
    - **Verification:** Read the updated context files and ensure they do not overwrite unrelated existing status/notes edits.

### Phase 7: Final verification

16. **Run automated verification**
    - **Location:** repository root
    - **Action:** Run:
      - `cargo fmt --all`
      - `cargo test -p marl-viewer-rs`
      - `cargo check -p marl-viewer-rs`
      - `cargo test -p marl-format`
      - `cargo test -p marl-engine`
      - `cargo run -p marl-viewer-rs -- --help`
      - `cargo tree -p marl-viewer-rs -i wgpu`
      - `cargo tree -p marl-viewer-rs -i winit`
    - **Verification:** All commands must pass. If pre-existing warnings remain (for example an unused `lineage_id` field), record them but do not hide new warnings from GUI code.

17. **Run interop/manual smoke verification**
    - **Location:** repository root, output under `/tmp/opencode`
    - **Action:** Run:
      - `cargo run -p marl-engine -- --ticks 3 --stats 1 --snapshot 1 --images 1000 --output /tmp/opencode/marl-viewer-gui-smoke`
      - `python scripts/check_binary_dump.py /tmp/opencode/marl-viewer-gui-smoke 1`
      - On a display/GPU-capable machine: `cargo run -p marl-viewer-rs -- /tmp/opencode/marl-viewer-gui-smoke --tick 0`
      - On a display/GPU-capable machine: launch with a missing directory, then load `/tmp/opencode/marl-viewer-gui-smoke` through the GUI.
    - **Verification:** Confirm the GUI opens, the shader output is visible behind/under the controls, directory loading works, tick buttons change snapshots, bad paths/ticks show errors without crashing, and optional view settings (if implemented) apply correctly. If no display/GPU is available, record manual visual verification as pending.

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `egui-wgpu`/workspace `wgpu` version drift creates duplicate wgpu versions | Low | Type mismatch or larger dependency graph | Pin `egui*` to `0.34.2`; verify `cargo tree -i wgpu` and `cargo tree -i winit`. |
| Startup without a valid snapshot complicates renderer assumptions | Medium | Window still exits or render panics before data load | Always create a valid 1×1×1 placeholder `SnapshotGpuResources`; keep `loaded_info: Option<SnapshotInfo>` only for UI semantics. |
| Reload failures leave renderer in partially replaced GPU state | Medium | Blank/invalid render after bad path/tick | Build new textures/params/bind group into locals first; replace current state only after all steps succeed. |
| Native directory picker fails on some Linux environments | Medium | `Open…` button appears broken | Keep text field + `Load Directory` as a mandatory fallback; treat `pick_folder() == None` as non-fatal. |
| Egui pass clears over shader output | Medium | GUI appears but volume disappears | Use first raymarch pass with `LoadOp::Clear`, second egui pass with `LoadOp::Load`. |
| Egui render pass lifetime mismatch under wgpu 29 | Medium | Compile error around `RenderPass<'static>` | Use `render_pass.forget_lifetime()` exactly for the egui pass after `update_buffers`. |
| Full reload for every view-setting apply is slow for large snapshots | Medium | UI stutters on Apply | Accept for MVP; keep Apply explicit instead of live-updating sliders; optimize uniform-only updates later if needed. |
| GUI overlays instead of reserving layout space around render | Medium | Controls obscure a small part of the volume | Accept for MVP; use compact/collapsible panels. Render-to-texture or viewport carving can be a later refinement. |
| Manual GUI behavior is hard to test headlessly | High | Automated tests miss visual/event-loop issues | Put parsing/tick-selection logic under unit tests; require manual display/GPU smoke and record if unavailable. |

## Verification

Verification is layered:

- Unit tests: `cargo test -p marl-viewer-rs` for CLI helper labels, tick discovery, GUI tick-selection helpers, and existing renderer/IO tests.
- Build/dependency checks: `cargo check -p marl-viewer-rs`, `cargo tree -p marl-viewer-rs -i wgpu`, and `cargo tree -p marl-viewer-rs -i winit`.
- Regression tests: `cargo test -p marl-format` and `cargo test -p marl-engine` to ensure shared format/engine output was not disturbed.
- Interop: generate fresh smoke data under `/tmp/opencode/marl-viewer-gui-smoke` and validate with `scripts/check_binary_dump.py`.
- Manual visual checks on a display/GPU-capable machine: launch with valid data, launch with missing data then load via GUI, navigate ticks, and apply optional view settings if implemented.
