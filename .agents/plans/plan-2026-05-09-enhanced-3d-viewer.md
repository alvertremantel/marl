# Enhanced 3D Viewer with Microbe Voxels

**Date:** 2026-05-09
**Status:** draft

---

## Goal

Enhance `marl-viewer-rs` so a snapshot is viewed as a 3D volume from an isometric-style camera by default, rather than as the current top-down z raymarch. The viewer should also load `tick_<T>.cells.bin` and render occupied cell voxels directly, colored by available microbe identity data, instead of only visualizing extracellular chemical concentration.

The implementation must preserve the existing binary output format and keep current field-only/top-down usage available via flags. The MVP should use the existing cell record's `starter_type` as the microbe category; true genotype-level species IDs are not present in the current output schema and are out of scope unless a later requirement explicitly requests an engine/schema extension.

## Understanding

Current relevant state:

- Workspace packages:
  - `crates/marl-engine`: writes binary snapshots.
  - `crates/marl-format`: owns shared `RunMeta`, format constants, and packed `ViewerCellRecord`.
  - `crates/marl-viewer-rs`: standalone `winit`/`wgpu` viewer.
- Viewer entry flow:
  - `crates/marl-viewer-rs/src/main.rs:25` calls `io::load_field(&args)` and passes the result to `ViewerApp`.
  - `crates/marl-viewer-rs/src/app.rs:54` creates `Renderer::new(window, payload)` once and redraws continuously.
  - `crates/marl-viewer-rs/src/args.rs:7-14` currently supports `output_dir`, `tick`, `species`, `exposure`, `density_scale`, and `steps` only.
  - `crates/marl-viewer-rs/src/io.rs:8-18` defines `FieldPayload` with only field bytes; `load_field()` validates `run_meta.json` and `tick_<T>.field.bin`, but never reads `tick_<T>.cells.bin`.
  - `crates/marl-viewer-rs/src/renderer.rs:91-102` creates uniforms for grid/render/transfer, uploads the field texture, and binds one field texture plus one uniform buffer.
  - `crates/marl-viewer-rs/src/renderer.rs:285-341` uploads the raw field as a 3D `R32Float` texture with dimensions `(grid_x * s_ext, grid_y, grid_z)` because the engine field layout is `[z][y][x][species]`.
  - `crates/marl-viewer-rs/src/viewer_raymarch.wgsl:35-78` maps screen `uv` directly to `(x, y)`, iterates through `z`, and composites one selected external species. This is a top-down volume projection, not an isometric/oblique camera.
- Binary cell data already exists:
  - `crates/marl-engine/src/binary_dump.rs:40-47` writes `tick_<T>.cells.bin` when `OutputConfig.write_binary_cells` is true; this is true by default in `crates/marl-engine/src/config.rs:191-208`.
  - `crates/marl-format/src/lib.rs:192-221` documents `ViewerCellRecord` as packed 25 bytes: `pos:f32[3]`, `lineage_id:u64`, `starter_type:u8`, `energy:f32`.
  - `crates/marl-engine/src/binary_dump.rs:22-28` writes `pos` from integer voxel coordinates, `lineage_id`, `starter_type`, and `energy = internal[0]`.
  - `run_meta.json` includes `cell_record_stride`, `write_binary_cells`, `cell_file_pattern`, and related cell metadata from `crates/marl-engine/src/binary_dump.rs:50-104`.
- The only categorical microbe identity currently available to the viewer is `starter_type` (`0 = phototroph`, `1 = chemolithotroph`, `2 = anaerobe` per `crates/marl-engine/src/cell.rs:176-179`). `lineage_id` is available but is a random lineage tag, not a stable species classification.
- Existing docs describe the viewer as Phase 1 field-only rendering in `README.md:121-129`; this must be updated after implementation.
- Existing context notes call out future cell-buffer ingestion in `.agents/context/STATUS.md:53-56` and describe the current field-only viewer in `.agents/context/NOTES.md:29-36`; these should be updated if the implementation lands.

Constraints and assumptions:

- Do not change the engine binary field/cell file format for this task. Existing snapshots should remain readable.
- Do not add a dependency just for vector math; simple 3-vector helpers are sufficient.
- The default should satisfy the requested behavior: `--view iso` and direct cell rendering by `starter_type` when cell data is present. Preserve current-ish behavior with `--view top --cells off`.
- If `tick_<T>.cells.bin` is absent or `run_meta.json` indicates cells were not written, the viewer should warn and continue with an empty cell overlay. Malformed present cell files should be hard errors.
- This plan does not require interactive orbit controls, snapshot streaming, interpolation, picking, or genotype clustering.

## Approach

Use the existing field raymarch architecture, but replace the screen-to-z traversal with an orthographic ray/AABB traversal through a normalized 3D grid box. The default camera basis will be an isometric-style oblique view that shows the top and two sides of the volume; a `top` mode will retain the old top-down interpretation.

Load cell records into a sparse in-memory list, validate positions, then convert them into a dense `Rgba8Uint` 3D occupancy/identity texture sized `(grid_x, grid_y, grid_z)`. Each occupied voxel stores an encoded color and alpha. The fragment shader will sample both the chemical field texture and the cell texture along the camera ray, compositing translucent chemical density with nearly opaque cell voxels. This avoids instanced cube geometry for the MVP and reuses the existing full-screen triangle pipeline.

Cell coloring modes:

- `--cells starter` (default): color occupied voxels by `starter_type` using the same conceptual ancestry classes already documented by engine snapshots: phototroph/red, chemolithotroph/green, anaerobe/blue, unknown/magenta.
- `--cells energy`: color occupied voxels by `energy` using a simple ramp; normalize against the maximum finite energy in the loaded cell file, with a safe fallback of `1.0`.
- `--cells off`: bind an empty cell texture and render only the chemical field.

Viewer flags:

- Add `--view <iso|top>`; default `iso`.
- Add `--cells <off|starter|energy>`; default `starter`.
- Add `--cell-alpha <f>`; default `0.95`, validate finite and in `(0, 1]`.
- Existing `--species`, `--tick`, `--scale`, `--exposure`, and `--steps` remain available. The shader may use a conservative effective lower bound for isometric sampling so one-voxel cell markers are not skipped on the default grid.

Parallelization boundaries after Phase 1 defines public types:

- Data loading and parser tests are isolated to `crates/marl-viewer-rs/src/io.rs` plus `args.rs` types.
- Renderer texture creation and uniform/camera helpers are isolated to `crates/marl-viewer-rs/src/renderer.rs` and optional `camera.rs`.
- WGSL shader changes are isolated to `crates/marl-viewer-rs/src/viewer_raymarch.wgsl`, once binding layout and uniform fields are settled.
- Docs/context updates should wait until flag names and defaults are final.

## Steps

### Phase 1: CLI and payload model

1. **Add viewer modes and parse new flags**
   - **Location:** `crates/marl-viewer-rs/src/args.rs`
   - **Action:**
     - Add `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub(crate) enum ViewMode { Iso, Top }`.
     - Add `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub(crate) enum CellMode { Off, Starter, Energy }`.
     - Add fields to `ViewerArgs`: `view_mode: ViewMode`, `cell_mode: CellMode`, `cell_alpha: f32`.
     - Refactor parsing so `ViewerArgs::parse()` delegates to a testable `ViewerArgs::parse_from<I>(args: I) where I: IntoIterator<Item = String>`.
     - Parse `--view <iso|top>`, `--cells <off|starter|energy>`, and `--cell-alpha <f>`.
     - Default `view_mode` to `ViewMode::Iso`, `cell_mode` to `CellMode::Starter`, and `cell_alpha` to `0.95`.
     - Update `usage()` to document the new defaults and mention `--view top --cells off` for field-only/top-down rendering.
     - Add unit tests in `args.rs` for defaults, valid new flags, invalid `--view`, invalid `--cells`, and invalid `--cell-alpha`.
   - **Verification:** Run `cargo test -p marl-viewer-rs args` and confirm all new parser tests pass.

2. **Replace field-only payload with snapshot payload and cell parsing**
   - **Location:** `crates/marl-viewer-rs/src/io.rs`, `crates/marl-viewer-rs/src/main.rs`, `crates/marl-viewer-rs/src/app.rs`
   - **Action:**
     - Rename `FieldPayload` to `SnapshotPayload` and rename `bytes` to `field_bytes` for clarity.
     - Add `LoadedCell { pos: [u32; 3], lineage_id: u64, starter_type: u8, energy: f32 }`.
     - Rename `load_field(&ViewerArgs)` to `load_snapshot(&ViewerArgs)` and update `main.rs` imports/call sites.
     - Update `ViewerApp` in `app.rs` to store and pass `SnapshotPayload` instead of `FieldPayload`.
     - Keep existing `run_meta.json`, species range, and field byte-length validation unchanged.
     - If `args.cell_mode == CellMode::Off`, skip cell loading and return `cells: Vec::new()`.
     - Otherwise load `tick_<T>.cells.bin` from `args.output_dir`; if the file is missing or `meta.write_binary_cells == false`, print a warning to stderr and return an empty cell list.
     - If the cell file exists, require `len % meta.cell_record_stride == 0` and `meta.cell_record_stride == marl_format::CELL_RECORD_STRIDE` after `meta.validate()`.
     - Parse each 25-byte record manually with `from_le_bytes`; do not transmute or take references to packed fields.
     - Validate `pos` values are finite, non-negative, close to integer voxel indices (absolute difference from `round()` <= `0.001`), and within `grid_x`, `grid_y`, `grid_z`; reject malformed records with a path/tick/record-index error.
     - Preserve `lineage_id`, `starter_type`, and finite `energy` values. Treat non-finite energy as a malformed record.
     - Print a load summary in `main.rs` including field bytes and cell count.
     - Add unit tests in `io.rs` for: parsing one valid record; parsing multiple records; rejecting bad byte length; rejecting out-of-bounds position; rejecting non-integral position; skipping load when `CellMode::Off`.
   - **Verification:** Run `cargo test -p marl-viewer-rs io` and `cargo check -p marl-viewer-rs`.

### Phase 2: Camera uniforms and cell occupancy texture

3. **Introduce deterministic camera basis helpers**
   - **Location:** `crates/marl-viewer-rs/src/renderer.rs` or new `crates/marl-viewer-rs/src/camera.rs` plus `crates/marl-viewer-rs/src/main.rs` module declaration if a new file is used.
   - **Action:**
     - Add a small helper type, e.g. `CameraBasis { right: [f32; 3], up: [f32; 3], dir: [f32; 3], zoom: f32 }`.
     - Implement `camera_basis(ViewMode::Iso)` as an orthographic oblique view looking from above/front/right. Use normalized vectors and make `right`, `up`, and `dir` orthonormal enough for stable ray traversal. Suggested basis:
       - Start with `dir = normalize([1.0, -1.0, -0.8])`, where negative world z means looking down into the simulation volume.
       - `right = normalize([dir.y, -dir.x, 0.0])`.
       - `up = normalize(cross(right, dir))`.
       - `zoom = 1.55` initially, adjusted only if manual smoke testing shows clipping.
     - Implement `camera_basis(ViewMode::Top)` as a top-down orthographic view that behaves like the current projection: `dir = [0.0, 0.0, -1.0]`, `right = [1.0, 0.0, 0.0]`, `up = [0.0, 1.0, 0.0]`, `zoom = 1.05`.
     - Add pure unit tests that each basis vector is finite and approximately unit length, and that pairwise dot products are near zero.
   - **Verification:** Run `cargo test -p marl-viewer-rs camera` if a new module is created, or `cargo test -p marl-viewer-rs renderer` if helpers live in `renderer.rs`.

4. **Expand renderer uniforms and bind group layout**
   - **Location:** `crates/marl-viewer-rs/src/renderer.rs`, `crates/marl-viewer-rs/src/viewer_raymarch.wgsl`
   - **Action:**
     - Update `ViewerParams` to remain `#[repr(C)]` and `bytemuck::Pod`, with 16-byte-aligned fields only. Use this shape or equivalent:
       - `grid: [u32; 4]` = `grid_x`, `grid_y`, `grid_z`, `s_ext`.
       - `render: [u32; 4]` = `width`, `height`, `species`, `steps`.
       - `transfer: [f32; 4]` = `exposure`, `density_scale`, `cell_alpha`, unused.
       - `axis_scale: [f32; 4]` = normalized box dimensions, e.g. `[grid_x/max_dim, grid_y/max_dim, grid_z/max_dim, 0.0]`.
       - `cam_right: [f32; 4]`, `cam_up: [f32; 4]`, `cam_dir: [f32; 4]`, each with vector xyz and `cam_right.w = zoom`.
       - `options: [u32; 4]` reserved for future toggles; set `options.x = 1` when cells are enabled, `0` when off.
     - Update WGSL `Params` struct to exactly match the Rust layout.
     - In `Renderer::new`, compute `axis_scale` using `max(grid_x, grid_y, grid_z)` as `f32`; reject zero grid values before division even though valid metadata should prevent them.
     - Add a unit test asserting `std::mem::size_of::<ViewerParams>() % 16 == 0`.
   - **Verification:** Run `cargo test -p marl-viewer-rs renderer` and `cargo check -p marl-viewer-rs`.

5. **Create and bind a 3D cell texture**
   - **Location:** `crates/marl-viewer-rs/src/renderer.rs`
   - **Action:**
     - Add renderer fields `_cell_texture: wgpu::Texture` and `_cell_view: wgpu::TextureView`.
     - Add `create_cell_texture(device, queue, payload) -> Result<(wgpu::Texture, wgpu::TextureView), Box<dyn Error>>`.
     - Use `wgpu::TextureFormat::Rgba8Uint`, `wgpu::TextureDimension::D3`, `wgpu::TextureUsages::COPY_DST | TEXTURE_BINDING`, and dimensions `(grid_x, grid_y, grid_z)`.
     - Add bind group layout entry for the cell texture, e.g. binding `2` with `wgpu::TextureSampleType::Uint` and `TextureViewDimension::D3`. Keep binding `0` as field texture and binding `1` as params unless a different layout is consistently reflected in WGSL.
     - Build a dense `Vec<u8>` of length `grid_x * grid_y * grid_z * 4`. Index as `((z * grid_y + y) * grid_x + x) * 4`.
     - For `CellMode::Starter`, encode `starter_type`: `0 => [230, 60, 55, alpha]`, `1 => [70, 220, 90, alpha]`, `2 => [75, 130, 255, alpha]`, otherwise `[230, 75, 230, alpha]`.
     - For `CellMode::Energy`, compute max finite energy across cells; encode a dark-purple-to-yellow ramp by normalized energy; use the same alpha.
     - For duplicate cell positions, keep the higher-energy record for `Energy`, or keep the first and count a warning for `Starter`. The engine should not produce duplicates, so any duplicate warning is diagnostic only.
     - Add pure tests for the byte-building helper: empty cells all zero; starter colors at expected voxel index; out-of-order positions index correctly; duplicate policy is deterministic.
   - **Verification:** Run `cargo test -p marl-viewer-rs renderer` and `cargo check -p marl-viewer-rs`.

### Phase 3: Isometric raymarch shader with cell compositing

6. **Rewrite the shader traversal from top-down z stepping to ray/AABB stepping**
   - **Location:** `crates/marl-viewer-rs/src/viewer_raymarch.wgsl`
   - **Action:**
     - Update bindings to include `field_tex`, `params`, and `cell_tex` with the same binding numbers used in `renderer.rs`.
     - Keep the full-screen triangle vertex shader.
     - In `fs_main`, convert fragment coordinates to aspect-correct screen coordinates in `[-1, 1]`.
     - Define the normalized simulation box centered at the origin:
       - `half_box = 0.5 * params.axis_scale.xyz`.
       - World x/y map directly to texture x/y.
       - World z is inverted relative to simulation depth so `texture z = 0` is visually the top surface: `tex_z = 0.5 - world_z / params.axis_scale.z`.
     - Construct an orthographic ray:
       - `screen_scale = params.cam_right.w`.
       - `origin = params.cam_right.xyz * screen.x * screen_scale + params.cam_up.xyz * screen.y * screen_scale - params.cam_dir.xyz * 2.0`.
       - `dir = params.cam_dir.xyz`.
     - Add a robust slab `intersect_box(origin, dir, -half_box, half_box)` helper that returns no hit for pixels outside the volume projection.
     - March from near to far with an effective step count high enough not to skip one-voxel cell markers on the default grid. Use at least `max(params.render.w, 2u * max(grid_x, grid_y, grid_z))`, while still allowing larger user-provided `--steps` values. Sample at voxel centers along the ray. Track the previous voxel coordinate and only apply cell compositing when the voxel changes so a single occupied voxel is not alpha-applied multiple times.
     - Field sample: compute `voxel_x`, `voxel_y`, `voxel_z`, then `field_tex_x = voxel_x * species_count + species`, preserving the packed field texture layout.
     - Cell sample: `textureLoad(cell_tex, vec3<i32>(voxel_x, voxel_y, voxel_z), 0)` returns `vec4<u32>`; if alpha is nonzero and cells are enabled, convert RGB/alpha to floats and composite as a direct occupied voxel marker.
     - Composite order should be front-to-back: field density contributes translucent color first for the sample, then cell voxel color can override/occlude with alpha from the texture. Stop early when accumulated alpha exceeds `0.985`.
     - Preserve the existing chemical palette or a visually similar one for the field so old `--species` inspection remains usable.
     - Keep a dark background for miss pixels and unoccupied empty space.
   - **Verification:**
     - Run `cargo check -p marl-viewer-rs` to catch Rust/WGSL include and binding name drift.
     - Generate a smoke snapshot with `cargo run -p marl-engine -- --ticks 2 --stats 1 --snapshot 1 --images 1000 --output output/viewer_iso_smoke` and validate it with `python scripts/check_binary_dump.py output/viewer_iso_smoke 1`.
     - On a machine with a display/GPU, run `cargo run -p marl-viewer-rs -- output/viewer_iso_smoke --tick 1 --view iso --species 1 --cells starter` and verify the volume is oblique/isometric and occupied voxels are visible in starter colors.
     - Also run `cargo run -p marl-viewer-rs -- output/viewer_iso_smoke --tick 1 --view top --species 1 --cells off` and verify the legacy field-only top view still renders.

7. **Update renderer lifecycle messaging and resize behavior**
   - **Location:** `crates/marl-viewer-rs/src/main.rs`, `crates/marl-viewer-rs/src/renderer.rs`
   - **Action:**
     - Update the initial stderr load message to include `view`, `cell_mode`, and loaded cell count.
     - Update `window.set_title()` to include tick, external species, cell count, and view mode.
     - Ensure `resize()` still updates only `params.render[0]` and `params.render[1]`; camera basis and axis scale should remain unchanged across window resizes.
   - **Verification:** Run `cargo run -p marl-viewer-rs -- --help` and confirm help prints. Then run `cargo check -p marl-viewer-rs`.

### Phase 4: Documentation and durable context

8. **Document the enhanced viewer**
   - **Location:** `README.md:121-129`, optionally `INFO.md` if the viewer section there is extended later.
   - **Action:**
     - Replace the Phase 1 field-only viewer description with a short description of isometric volume rendering plus direct cell voxel overlay.
     - Update the example command to show default isometric/cell behavior.
     - Add the new flags to the useful viewer flags list: `--view`, `--cells`, `--cell-alpha`.
     - Document that microbe coloring currently uses `starter_type` from cell records, not inferred genotype-level species.
   - **Verification:** Review the rendered Markdown or read the section to confirm command syntax matches `args.rs::usage()`.

9. **Update agent context after implementation lands**
   - **Location:** `.agents/context/STATUS.md`, `.agents/context/NOTES.md`, `.agents/context/MAP.md`
   - **Action:**
     - In `STATUS.md`, add a completed entry for the enhanced viewer with changed files and verification commands actually run.
     - In `NOTES.md`, add durable decisions: existing cell record is used; `starter_type` is the MVP microbe identity; cell occupancy is uploaded as `Rgba8Uint`; isometric camera is orthographic ray/AABB traversal.
     - In `MAP.md`, update the viewer shader/renderer descriptions to mention cell texture upload and isometric camera.
   - **Verification:** Read the updated context files and ensure they do not claim manual GPU/display checks were run unless they actually were.

### Phase 5: Final verification

10. **Run full local verification**
    - **Location:** repository root
    - **Action:** Run the full relevant command set:
      - `cargo fmt --all`
      - `cargo test -p marl-viewer-rs`
      - `cargo check -p marl-viewer-rs`
      - `cargo test -p marl-format`
      - `cargo test -p marl-engine`
      - `cargo run -p marl-viewer-rs -- --help`
      - `cargo run -p marl-engine -- --ticks 2 --stats 1 --snapshot 1 --images 1000 --output output/viewer_iso_smoke`
      - `python scripts/check_binary_dump.py output/viewer_iso_smoke 1`
      - Manual/display-dependent: `cargo run -p marl-viewer-rs -- output/viewer_iso_smoke --tick 1 --view iso --species 1 --cells starter`
      - Manual/display-dependent legacy check: `cargo run -p marl-viewer-rs -- output/viewer_iso_smoke --tick 1 --view top --species 1 --cells off`
    - **Verification:** All non-manual commands must pass. If manual viewer runs cannot be performed due to no display/GPU in the environment, record that limitation in the completion notes and do not mark visual verification as completed.

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| WGSL/Rust uniform layout mismatch | Medium | Viewer fails at pipeline creation or renders garbage | Use only vec4-shaped fields in `ViewerParams`; mirror names/order in WGSL; add size/alignment tests; run manual viewer smoke. |
| Isometric camera clips the volume or appears unintuitive | Medium | User sees incomplete/poor 3D view | Keep `zoom` in a single camera helper; smoke-test default grid; preserve `--view top`; adjust only `zoom`/basis if needed. |
| Cell voxel alpha is applied multiple times during raymarch | Medium | Cells look over-opaque or bloated | Track previous voxel coordinate in WGSL and composite cell color only once per voxel crossing. |
| Missing cell files break older field-only outputs | Medium | Backward compatibility regression | Treat absent cell file as warning + empty overlay unless malformed file exists; provide `--cells off`. |
| `starter_type` is mistaken for true evolved species | High | Misleading biological interpretation | Document clearly in README and context notes that this is ancestry/starter category only. |
| Dense cell texture allocation grows with grid dimensions | Low for default, medium for larger builds | Increased viewer memory | Default grid uses ~4 MiB for `Rgba8Uint` cells; validate texture dimensions against adapter limits; revisit sparse/instanced rendering for large grids later. |
| Shader syntax is not covered by `cargo check` alone | Medium | CI passes while runtime viewer fails | Include manual/display-dependent viewer smoke in verification; consider adding a shader parser dev-dependency only if runtime validation becomes repeatedly painful. |

## Verification

Verification is layered:

- Parser/unit coverage: `cargo test -p marl-viewer-rs` for CLI modes, cell-record parsing, camera basis, uniform size, and cell texture byte generation. During development, narrower filters such as `cargo test -p marl-viewer-rs args`, `cargo test -p marl-viewer-rs io`, and `cargo test -p marl-viewer-rs renderer` may be used.
- Build/type coverage: `cargo check -p marl-viewer-rs` plus `cargo test -p marl-viewer-rs`.
- Interop coverage: generate a fresh engine snapshot and validate it with `scripts/check_binary_dump.py` before loading it in the viewer.
- Regression coverage: `cargo test -p marl-format` and `cargo test -p marl-engine` ensure shared format and engine output did not regress.
- Manual visual coverage: run the viewer in default isometric cell-overlay mode and in `--view top --cells off` legacy mode on a display/GPU-capable machine. Record if this cannot be performed in the current environment.
