# Plan: GPU Reaction-Diffusion Prototype

## Goal

Build a correctness-first, compute-only GPU prototype for MARL's reaction-diffusion field update. The target is not visualization and not a full GPU simulation backend. The first implementation should accelerate and validate the behavior currently implemented in `src/field.rs`, especially `Field::diffuse_tick_with_cells()`, while keeping the CPU cell lifecycle, logging, snapshots, mutation, and visualization-independent output flow intact.

The initial GPU path should be intentionally conservative:

- Use `wgpu` + WGSL for the prototype unless implementation evidence shows a hard blocker.
- Use `f32` field storage initially.
- Preserve the current AoS field layout: `[z][y][x][species]`.
- Implement a naive compute shader first, not the old spec's optimized 2.5D tiled Vulkan shader.
- Include the current occupancy-mask behavior from CPU MARL as a required feature.
- Verify numerical equivalence against the CPU implementation before optimizing.

This plan intentionally diverges from parts of `.agents/spec/005-language-framework.md`, `.agents/spec/field-update.md`, and `.agents/spec/research-gpu-field-update.md`. Those specs target a future large-grid, highly optimized Vulkan architecture. This plan targets a practical first GPU compute foothold for the current codebase.

## Current Understanding

### Current Codebase State

- Project root: `A:\nudev\marl`.
- Rust package: `Cargo.toml`, package name `marl`, edition `2024`.
- Current dependencies: `rand`, `rand_distr`, `rayon`, `serde`, `toml`.
- No GPU dependencies currently exist.
- No shader files currently exist.
- No test suite is currently visible in the repo; verification will likely require adding tests or a small comparison harness.

### Current Compile-Time Dimensions

Defined in `src/config.rs`:

- `GRID_X = 128`
- `GRID_Y = 128`
- `GRID_Z = 64`
- `S_EXT = 12`
- `M_INT = 16`
- `R_MAX = 16`
- `S_RECEPTORS = 8`
- `S_TRANSPORTERS = 8`
- `S_EFFECTORS = 8`

The current field size is `128 * 128 * 64 * 12 = 12,582,912` `f32` values, approximately 48 MiB per field buffer. The CPU `Field` currently allocates two such buffers: `data` and `scratch`.

### Current Field Layout

Implemented in `src/field.rs`:

```rust
fn idx(&self, x: usize, y: usize, z: usize, s: usize) -> usize {
    ((z * GRID_Y + y) * GRID_X + x) * S_EXT + s
}
```

This is AoS-per-voxel layout: `[z][y][x][species]`. All 12 species for one voxel are contiguous.

The old GPU specs recommend SoA layout: `[species][z][y][x]`. Do not refactor to SoA in the first prototype. Preserve the current layout to minimize risk and simplify CPU/GPU comparison.

### Current CPU Diffusion Behavior

Implemented in `src/field.rs`:

- `Field::apply_boundary_sources(&mut self, sim: &SimulationConfig)` adds boundary source terms before diffusion:
  - Top face `z = 0`: species `1` oxidant plus `sim.source_rate_oxidant`, capped at `sim.c_max`.
  - Top face `z = 0`: species `3` carbon plus `sim.source_rate_carbon`, capped at `sim.c_max`.
  - Bottom face `z = GRID_Z - 1`: species `2` reductant plus `sim.source_rate_reductant`, capped at `sim.c_max`.
- `Field::diffuse_tick_with_cells(&mut self, occupancy: &[bool], sim: &SimulationConfig)` runs `sim.diffusion_substeps` sequential substeps.
- `diffusion_substeps` default is `10`.
- Per substep, `dt_sub = sim.dt / sim.diffusion_substeps as f32`.
- Each substep calls `diffusion_step_inner(dt_sub, Some(occupancy), sim)` and swaps `data` and `scratch`.
- The diffusion equation is:

```text
c' = c + dt_sub * (D_local * laplacian(c) - lambda_decay[s] * c)
```

- The 6-neighbor Laplacian uses face-adjacent neighbors only.
- Domain walls use Neumann zero-flux behavior by substituting center value `c` for the missing neighbor.
- Occupied neighbors also use Neumann zero-flux behavior by substituting center value `c`.
- Occupied voxels are excluded from diffusion entirely and copied unchanged.
- New concentrations are clamped with `.max(0.0)`.
- EPS/niche construction uses species `7` as structural concentration:

```rust
let structural = Self::get_from(src, x, y, z, 7);
let niche_factor = 1.0 - sim.alpha_eps * structural / (sim.k_eps + structural);
let d = sim.d_voxel[s] * niche_factor;
```

The occupancy behavior is ecologically important. It prevents dense colonies from becoming unrealistically permeable and creates nutrient starvation in interior cells. It is a v1 GPU requirement.

### Current Tick Ordering

Implemented in `src/main.rs` around the main tick loop:

1. Boundary sources: `field.apply_boundary_sources(sim)`.
2. Occupancy grid construction from `cell_map` into `Vec<bool>`.
3. Diffusion: `field.diffuse_tick_with_cells(&occupancy, sim)`.
4. Light: `light.update(&field, &cell_map, sim)`.
5. CPU cell loop:
   - `read_neighbor_environment(...)`
   - `cell.tick(...)`
   - `apply_deltas_to_neighbors(...)`
6. Fate events: division/death.
7. Logging and periodic output.

The first GPU prototype should replace only step 3 initially. Step 1 may remain CPU-side at first. Step 4 and later remain CPU-side initially.

### Relevant Old Specification Material

- `.agents/spec/research-gpu-field-update.md`: detailed GPU memory and stencil analysis, but optimized for `500x500x200`, float16, SoA, 2.5D tiling, Vulkan/RTX 4060 assumptions.
- `.agents/spec/field-update.md`: equation, Neumann boundary concept, EPS diffusion modification, GPU strategy summary.
- `.agents/spec/005-language-framework.md`: proposes Rust + `ash` Vulkan, with `wgpu` as fallback.
- `.agents/spec/006-species-namespace-count.md`: `S=12`, `M=16`, shared-memory pressure for 2.5D tiling.

Use these specs as background, not as binding implementation instructions for the prototype.

## Key Decisions For This Prototype

### Decision 1: Use `wgpu` First

Use `wgpu` for the initial prototype unless immediate implementation evidence shows it cannot support the current field size and dispatch pattern.

Rationale:

- Current buffers are tens of MiB, not the old spec's 800 MiB to 1.2 GiB large-grid buffers.
- `wgpu` is safer and faster to integrate than raw Vulkan via `ash`.
- WGSL is adequate for a naive `f32` stencil shader.
- The first goal is correctness and architectural learning, not maximum throughput.
- If `wgpu` becomes limiting later, the shader behavior and tests can inform a later `ash` implementation.

This intentionally disagrees with the old spec's recommendation to start with `ash`.

### Decision 2: Keep Current AoS Layout First

Do not refactor `Field` to SoA for the first prototype.

Rationale:

- `src/field.rs`, `src/light.rs`, `src/snapshot.rs`, `src/data.rs`, and `src/main.rs` all assume current access patterns.
- A layout refactor would make GPU correctness harder to isolate.
- AoS is acceptable for a first naive shader and may be good enough at current grid size.

Revisit SoA only after correctness tests exist and profiling shows layout is a meaningful bottleneck.

### Decision 3: Use `f32` First

Do not introduce float16 in the first prototype.

Rationale:

- CPU uses `f32` today.
- `f32` makes CPU/GPU comparisons straightforward.
- Current memory footprint is modest.
- Float16 can be evaluated later as an optimization with separate precision tests.

### Decision 4: Preserve Occupancy Semantics

The GPU shader must match current CPU occupancy behavior:

- If `occupancy[voxel]` is true, copy all species unchanged.
- If a neighbor is out of bounds or occupied, substitute center value for that neighbor.

Do not implement the old spec's ghost-cell-only boundary model as the first GPU behavior, because it does not cover cell-body exclusion.

### Decision 5: Keep Cells CPU-Owned Initially

Do not implement GPU cell updates, sparse delta sorting, or GPU-side cell lifecycle in this prototype.

Rationale:

- The user explicitly asked about compute/simulation, but the smallest useful compute step is field diffusion.
- CPU-owned cells already work and have significant dynamic behavior: mutation, division, death, `HashMap` occupancy, per-cell rulesets.
- Moving cells to GPU would multiply scope and obscure field-shader validation.

## Proposed Architecture

### New Module Structure

Add a GPU module gated behind a Cargo feature.

Recommended files:

- `src/gpu/mod.rs`
- `src/gpu/context.rs`
- `src/gpu/field_diffusion.rs`
- `src/gpu/shaders/field_diffuse.wgsl`
- Optional later: `src/gpu/shaders/boundary_sources.wgsl`

Recommended feature in `Cargo.toml`:

```toml
[features]
default = []
gpu = ["dep:wgpu", "dep:pollster", "dep:bytemuck"]

[dependencies]
wgpu = { version = "...", optional = true }
pollster = { version = "...", optional = true }
bytemuck = { version = "...", features = ["derive"], optional = true }
```

The implementing agent must choose current compatible crate versions by checking the Rust ecosystem at implementation time. Do not guess stale versions if Cargo resolution fails.

### Public GPU API

Expose a narrow API that mirrors the CPU diffusion call.

Suggested shape:

```rust
#[cfg(feature = "gpu")]
pub struct GpuFieldDiffuser { ... }

#[cfg(feature = "gpu")]
impl GpuFieldDiffuser {
    pub fn new() -> Result<Self, GpuError>;

    pub fn diffuse_tick_with_cells(
        &mut self,
        field: &mut Field,
        occupancy: &[bool],
        sim: &SimulationConfig,
    ) -> Result<(), GpuError>;
}
```

For v1, this API may upload the full field and occupancy each call and read back the full field after each tick. That is not optimal, but it is correct and easy to verify. Later phases can keep field buffers resident on GPU.

### Shader Inputs

The WGSL shader should read:

- `field_in`: storage buffer, `array<f32>`.
- `field_out`: storage buffer, `array<f32>`.
- `occupancy`: storage buffer or uniform-compatible buffer, `array<u32>` preferred over packed bools.
- `params`: uniform buffer containing scalar parameters and arrays.

Avoid `Vec<bool>` layout on GPU. Convert CPU occupancy to `Vec<u32>` where `0 = empty`, `1 = occupied`.

### Parameter Buffer

WGSL uniform layout can be picky. Use a plain, padded Rust struct with `bytemuck::Pod` and explicit alignment.

Suggested data:

```rust
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct DiffusionParams {
    dt_sub: f32,
    alpha_eps: f32,
    k_eps: f32,
    _pad0: f32,
    d_voxel: [f32; S_EXT],
    lambda_decay: [f32; S_EXT],
}
```

Grid dimensions and species count can be compile-time constants in the WGSL shader for v1, matching `src/config.rs`:

- `GRID_X = 128u`
- `GRID_Y = 128u`
- `GRID_Z = 64u`
- `S_EXT = 12u`

If the implementation wants to avoid duplicating constants, use a build-time generation step later. Do not add build-time generation in v1 unless needed.

### Shader Dispatch

Use a straightforward 1D or 3D dispatch.

Recommended v1: 1D global invocation over voxels, with each invocation processing all species for one voxel.

Pseudo-WGSL:

```wgsl
@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let voxel = gid.x;
    if (voxel >= GRID_SIZE) { return; }

    let x = voxel % GRID_X;
    let y = (voxel / GRID_X) % GRID_Y;
    let z = voxel / (GRID_X * GRID_Y);

    let base = voxel * S_EXT;

    if (occupancy[voxel] != 0u) {
        for (var s = 0u; s < S_EXT; s = s + 1u) {
            field_out[base + s] = field_in[base + s];
        }
        return;
    }

    let structural = field_in[base + 7u];
    let niche_factor = 1.0 - params.alpha_eps * structural / (params.k_eps + structural);

    for (var s = 0u; s < S_EXT; s = s + 1u) {
        let c = field_in[base + s];
        let xm = neighbor_or_center(x, y, z, s, c, /* x - 1 */);
        ...
        let laplacian = xm + xp + ym + yp + zm + zp - 6.0 * c;
        let d = params.d_voxel[s] * niche_factor;
        let decay = params.lambda_decay[s] * c;
        let new_c = c + params.dt_sub * (d * laplacian - decay);
        field_out[base + s] = max(new_c, 0.0);
    }
}
```

Implement neighbor lookup carefully so it exactly matches `src/field.rs`:

- If neighbor coordinate is outside domain, return `c`.
- Else compute neighbor voxel index.
- If `occupancy[neighbor_voxel] != 0`, return `c`.
- Else return `field_in[neighbor_voxel * S_EXT + s]`.

### Substeps

For v1, run one GPU dispatch per diffusion substep and ping-pong GPU buffers between substeps.

Implementation detail:

- Upload `field.data` into GPU buffer A.
- Allocate GPU buffer B of same size.
- Upload `occupancy_u32` once per tick.
- For each substep `0..sim.diffusion_substeps`:
  - Bind A as `field_in`, B as `field_out`.
  - Dispatch `ceil(GRID_SIZE / 256)` workgroups.
  - Insert necessary command ordering by sequencing passes in one command encoder or using explicit barriers as required by `wgpu`.
  - Swap A and B handles for the next substep.
- Read back the final buffer into `field.data`.

Do not attempt to put the 10-substep loop inside one shader invocation for v1. That would require global synchronization between substeps, which compute shaders do not provide across workgroups.

### Boundary Sources

For the first GPU prototype, leave `Field::apply_boundary_sources(sim)` on CPU before calling the GPU diffuser. This preserves current tick ordering and avoids another shader.

Later, if field buffers become GPU-resident across ticks, add a boundary-source compute shader or fold boundary sources into the first diffusion substep. That is not required for v1.

### Light

Do not implement GPU light in this plan's first milestone.

Reason:

- `src/light.rs` reads `Field` and `cell_map` after diffusion.
- If v1 reads field back after GPU diffusion, current CPU light continues to work unchanged.
- GPU light is a separate compute pattern and should wait until field diffusion is validated.

### Output And Visualization

Do not implement visualization, render targets, PPM generation, or GPU-side snapshot rendering.

Existing output stays CPU-side:

- `src/snapshot.rs`
- `src/data.rs`
- stdout stats in `src/main.rs`

The user's stated intent is to leave visualization to external programs consuming outputs.

## Implementation Phases

### Phase 0: Establish Baseline And Safety Net

Purpose: create deterministic CPU outputs for comparing GPU diffusion.

Steps:

1. Inspect existing CLI in `src/main.rs` and config loading in `src/config.rs` to determine whether a GPU flag should be CLI-visible immediately or hidden behind tests first.
2. Run current verification commands before code edits:
   - `cargo check`
   - `cargo test` if tests exist
   - `cargo run --release -- --ticks 10 --stats 5` or the current equivalent CLI invocation if syntax differs
3. Record baseline behavior:
   - Ensure current CPU run completes.
   - Note current ticks/sec for a small run.
   - Do not change output formats.

Verification:

- `cargo check` passes before GPU edits.
- A short CPU run still works before GPU edits.

### Phase 1: Add Cargo Feature And Module Skeleton

Purpose: introduce GPU code without affecting default CPU builds.

Steps:

1. Edit `Cargo.toml`:
   - Add `[features] default = []` if absent.
   - Add optional `gpu` feature.
   - Add optional dependencies: `wgpu`, `pollster`, `bytemuck`.
2. Add `src/gpu/mod.rs` behind `#[cfg(feature = "gpu")]` from the crate root.
3. Update `src/main.rs` or `src/lib.rs` module declarations as appropriate for current project structure.
4. Add a minimal `GpuError` type or use `anyhow` only if the project already accepts broad error dependencies. Prefer a small local error enum for minimal dependency surface.

Verification:

- `cargo check` passes without `--features gpu`.
- `cargo check --features gpu` resolves dependencies and compiles the empty module.

### Phase 2: Add CPU Reference Test Utilities

Purpose: make CPU/GPU comparison possible without relying on full simulation randomness.

Steps:

1. Add deterministic test helpers in a suitable location:
   - If this remains a binary-only crate, use unit tests inside `src/field.rs` or create integration tests that can access public APIs.
   - If needed, refactor minimally to expose `Field`, `SimulationConfig`, and constants to tests without changing behavior.
2. Create a deterministic field initializer for tests:
   - Fill every voxel/species with a reproducible nonuniform pattern, e.g. arithmetic function of `(x, y, z, s)`.
   - Include nonzero EPS species `7` in some regions.
   - Include zero and near-zero concentrations to test clamping.
3. Create deterministic occupancy patterns:
   - Empty occupancy.
   - Single occupied voxel near center.
   - Occupied voxel adjacent to a domain boundary.
   - Small dense cluster.
4. Create helper to compare two `Field` buffers:
   - Absolute tolerance target for f32 naive GPU: start with `1e-5` to `1e-4`.
   - Report maximum absolute error and index of worst mismatch.

Verification:

- CPU-only tests compile and pass without `--features gpu`.
- Tests do not require a GPU unless marked/gated with `#[cfg(feature = "gpu")]`.

### Phase 3: Write Naive WGSL Diffusion Shader

Purpose: implement one diffusion substep exactly matching CPU behavior.

Steps:

1. Create `src/gpu/shaders/field_diffuse.wgsl`.
2. Implement constants for current grid and species count:
   - `GRID_X = 128u`
   - `GRID_Y = 128u`
   - `GRID_Z = 64u`
   - `S_EXT = 12u`
   - `GRID_SIZE = GRID_X * GRID_Y * GRID_Z`
3. Define WGSL storage buffers:
   - `@group(0) @binding(0) var<storage, read> field_in: array<f32>;`
   - `@group(0) @binding(1) var<storage, read_write> field_out: array<f32>;`
   - `@group(0) @binding(2) var<storage, read> occupancy: array<u32>;`
   - `@group(0) @binding(3) var<uniform> params: DiffusionParams;`
4. Implement `idx(voxel, s)` and neighbor helpers.
5. Implement exact CPU diffusion semantics:
   - Occupied current voxel copies all species unchanged.
   - Empty voxel computes EPS structural from species `7`.
   - Per species, fetch neighbors with wall/occupied substitution.
   - Apply decay and clamp negative to zero.
6. Use `@workgroup_size(256)` unless `wgpu` or adapter limits suggest otherwise.

Verification:

- `cargo check --features gpu` compiles shader module loading code once added in Phase 4.
- Shader source should be kept simple and reviewed line-by-line against `src/field.rs::diffusion_step_inner`.

### Phase 4: Implement `GpuFieldDiffuser`

Purpose: run the WGSL shader from Rust for one full diffusion tick.

Steps:

1. In `src/gpu/context.rs`, create a minimal `wgpu` context:
   - Instance.
   - Adapter request.
   - Device and queue request.
   - Prefer default limits unless buffer size requires explicit limits.
2. In `src/gpu/field_diffusion.rs`, implement `GpuFieldDiffuser`:
   - Own `wgpu::Device`, `wgpu::Queue`, shader module, bind group layout, pipeline.
   - Allocate field buffers sized to `GRID_X * GRID_Y * GRID_Z * S_EXT * size_of::<f32>()`.
   - Allocate occupancy buffer sized to `GRID_X * GRID_Y * GRID_Z * size_of::<u32>()`.
   - Allocate params uniform buffer.
   - Allocate staging/readback buffer.
3. Implement CPU-to-GPU upload:
   - Write `field.data` to GPU buffer A.
   - Convert `&[bool]` occupancy to `Vec<u32>` and write to occupancy buffer.
   - Populate `DiffusionParams` from `SimulationConfig`.
4. Implement substep dispatch loop:
   - For each substep, update params only if needed. `dt_sub` is constant across substeps.
   - Create bind group for current A/B ordering, or pre-create two bind groups.
   - Dispatch `ceil(GRID_SIZE / 256)` workgroups.
   - Swap A/B roles.
5. Implement GPU-to-CPU readback:
   - Copy final GPU field buffer to staging buffer.
   - Map staging buffer for read.
   - Copy bytes back into `field.data`.
6. Keep this API synchronous for v1 using `pollster::block_on` or equivalent. Do not attempt async integration into the simulation loop yet.

Verification:

- `cargo check --features gpu` passes.
- A small standalone GPU unit/integration test can initialize a field, run GPU diffusion, and read back without panicking.

### Phase 5: CPU/GPU Equivalence Tests

Purpose: prove the shader matches CPU behavior before using it in normal runs.

Steps:

1. Add GPU-gated tests, e.g. `#[cfg(all(test, feature = "gpu"))]`.
2. Test one substep if possible:
   - CPU path currently exposes only full tick methods publicly. If needed, add a test-only helper or expose a carefully named method behind `#[cfg(test)]` to run one substep. Avoid making private internals public just for tests unless necessary.
3. Test full tick with default `diffusion_substeps = 10`:
   - Clone initial field into `cpu_field` and `gpu_field`.
   - Run `cpu_field.diffuse_tick_with_cells(&occupancy, &sim)`.
   - Run `gpu_diffuser.diffuse_tick_with_cells(&mut gpu_field, &occupancy, &sim)`.
   - Compare all values with a tolerance.
4. Include test cases:
   - Empty occupancy, nonuniform field.
   - Occupied center voxel.
   - Occupied cluster.
   - Boundary-adjacent occupancy.
   - EPS-rich region affecting diffusion.
5. Print or assert useful diagnostics:
   - Max absolute error.
   - Max relative error when denominator is safe.
   - Worst index decoded to `(x, y, z, s)`.

Verification:

- `cargo test` passes without GPU feature.
- `cargo test --features gpu` passes on a machine with compatible GPU.
- If no compatible GPU exists in CI/local environment, tests should skip gracefully rather than failing unrelated development. Do not hide real shader errors when a GPU is available.

### Phase 6: Optional CLI Integration

Purpose: allow real MARL runs to use GPU diffusion without changing default behavior.

Steps:

1. Add a CLI flag only behind `#[cfg(feature = "gpu")]`, e.g. `--gpu-diffusion`.
2. Keep CPU diffusion as default even when compiled with GPU feature.
3. Initialize `GpuFieldDiffuser` once before the main tick loop if `--gpu-diffusion` is set.
4. In the tick loop, replace only:

```rust
field.diffuse_tick_with_cells(&occupancy, sim);
```

with:

```rust
if let Some(diffuser) = gpu_diffuser.as_mut() {
    diffuser.diffuse_tick_with_cells(&mut field, &occupancy, sim)?;
} else {
    field.diffuse_tick_with_cells(&occupancy, sim);
}
```

Adjust error handling to fit current `main.rs`, which currently prints warnings for output errors and otherwise likely uses direct control flow. Do not introduce broad architecture changes to main.

5. Ensure all downstream CPU code still sees `field.data` populated after diffusion:
   - `light.update(...)`
   - `read_neighbor_environment(...)`
   - `print_stats(...)`
   - `logger.snapshot_chemistry(...)`
   - `snapshot::write_all_snapshots(...)`

Verification:

- `cargo run --release -- --ticks 10 --stats 5` still uses CPU and works.
- `cargo run --release --features gpu -- --ticks 10 --stats 5 --gpu-diffusion` uses GPU path and works.
- CPU and GPU short runs from the same seed/config should produce close chemistry profiles after a small number of ticks, subject to floating-point tolerance.

### Phase 7: Benchmark And Decide Next Architecture Step

Purpose: avoid optimizing blindly.

Steps:

1. Add coarse timing around CPU diffusion and GPU diffusion paths.
2. Measure at current grid size `128x128x64`:
   - CPU diffusion time per tick.
   - GPU upload time.
   - GPU dispatch time if measurable.
   - GPU readback time.
   - Total GPU diffusion call time.
3. If practical, test a larger compile-time grid such as `256x256x128` on a separate branch or local change without committing dimension changes unless explicitly requested.
4. Record findings in `.agents/context/NOTES.md` and `.agents/context/STATUS.md` after verification.

Decision points after benchmarking:

- If GPU total time is slower due to upload/readback, consider persistent GPU field buffers before shader optimization.
- If shader dispatch dominates, consider SoA layout or 2.5D tiling.
- If readback dominates but CPU light/cells require field every tick, decide whether to move light or cell environment sampling to GPU next.
- If CPU cell work dominates, do not spend time optimizing field shader yet.

Verification:

- Benchmark output is reproducible enough to compare CPU vs GPU paths.
- Notes clearly separate upload/readback overhead from shader compute time.

## Future Phases Not In V1

These are intentionally deferred.

### Persistent GPU Field Buffers

Keep field data resident on GPU across ticks. This requires changing the sync boundary with CPU cell updates, light, and output. It is likely necessary for large-grid speedups but should wait until v1 correctness is proven.

### GPU Boundary Sources

Implement boundary source injection on GPU to avoid CPU mutation of `field.data` before upload. This matters only after persistent GPU field buffers exist.

### GPU Light

Port `src/light.rs` to a compute pass. Pattern is one thread per `(x, y)` column with sequential `z` loop. Requires occupancy on GPU and field access to species `4`.

### GPU Cell Environment Sampling

Instead of reading the full field back each tick, GPU could sample neighbor environments for each CPU-owned cell and return a compact `cells.len() * S_EXT` buffer to CPU. This may be a better next step than full GPU cell updates.

### GPU Cell Updates And Sparse Deltas

Only consider after field diffusion and possibly light are GPU-resident. The old spec's sparse delta sorting design becomes relevant here, but it is not needed for v1.

### SoA Layout

Refactor `Field` to species-first storage only if profiling justifies it. This affects many files and should be handled as a separate plan.

### Float16 Storage

Evaluate only after f32 correctness and profiling. Requires precision tests, concentration distribution checks, and likely shader/storage feature review.

### 2.5D Tiled Shader

Implement only after the naive shader is correct and demonstrably too slow. Current grid size may not justify the complexity.

### Raw Vulkan / `ash`

Move from `wgpu` to `ash` only if actual limits or profiling justify it:

- Buffer size limits block target grid size.
- Storage binding limits block desired layout.
- Shared memory / subgroup control is required for performance.
- Dispatch overhead is measurable and significant.

Do not assume these are current blockers.

## Risks And Mitigations

### Risk: GPU Tests Are Environment-Dependent

Mitigation:

- Gate GPU tests behind `--features gpu`.
- Detect adapter availability and skip only when no compatible adapter exists.
- Keep CPU tests independent.

### Risk: Floating-Point Differences Cause Noisy Comparisons

Mitigation:

- Start with `f32`.
- Use tolerances and report worst mismatch.
- Compare one substep first if possible.
- Avoid enabling fast-math-like behavior manually.

### Risk: WGSL Uniform Layout Mismatch

Mitigation:

- Use `#[repr(C)]` and `bytemuck`.
- Add padding explicitly.
- Prefer storage buffer for params if uniform alignment becomes annoying, since params are tiny and read-only.

### Risk: Upload/Readback Makes GPU Path Slower

Mitigation:

- Accept this for v1 if correctness is proven.
- Measure upload, dispatch, readback separately.
- Use results to decide whether persistent buffers are worth implementing.

### Risk: Duplicated Constants Drift

Mitigation:

- For v1, document duplicated constants in shader.
- Add assertions in Rust that GPU shader assumptions match `GRID_X`, `GRID_Y`, `GRID_Z`, and `S_EXT`.
- Later, generate shader constants from Rust/build script if the GPU path matures.

### Risk: `Vec<bool>` Is Not GPU-Friendly

Mitigation:

- Convert to `Vec<u32>` before upload.
- Keep CPU occupancy construction unchanged.

### Risk: Adding GPU Code Bloats Main Simulation Path

Mitigation:

- Keep GPU behind `#[cfg(feature = "gpu")]`.
- Keep CPU default path unchanged.
- Keep public GPU API narrow.

## Acceptance Criteria

The first successful implementation should satisfy all of these:

1. Default build remains CPU-only:
   - `cargo check`
   - `cargo build --release`
2. GPU build compiles:
   - `cargo check --features gpu`
   - `cargo build --release --features gpu`
3. CPU tests pass without GPU feature:
   - `cargo test`
4. GPU comparison tests pass when run on a compatible machine:
   - `cargo test --features gpu`
5. GPU diffusion matches CPU diffusion within documented tolerance for:
   - Empty occupancy.
   - Center occupied voxel.
   - Boundary-adjacent occupancy.
   - Dense occupied cluster.
   - EPS-rich field region.
6. Existing CPU CLI behavior remains unchanged when GPU flag is absent.
7. If CLI integration is implemented, `--gpu-diffusion` affects only the diffusion step and leaves light, cells, logging, snapshots, and output formats unchanged.
8. `.agents/context/STATUS.md` and `.agents/context/NOTES.md` are updated after implementation and verification, summarizing:
   - What GPU path exists.
   - Which commands passed.
   - Known limitations.
   - Benchmark observations.

## Suggested Subagent Strategy

If using subagents, split work as follows:

- Builder 1: Cargo feature/module skeleton and `wgpu` context.
- Builder 2: WGSL shader and Rust parameter/buffer layout.
- Builder 3: CPU/GPU comparison tests and deterministic fixtures.
- Reviewer: Check exact semantic match against `src/field.rs::diffusion_step_inner`, especially occupancy and boundary behavior.
- Fixer: Apply reviewer-recommended corrections literally.

Avoid parallel edits to the same files unless explicitly coordinated. The highest-collision files are `Cargo.toml`, `src/main.rs`, and `src/field.rs`.

## Implementation Notes For Future Agents

- Do not remove or weaken CPU diffusion.
- Do not change output file formats.
- Do not move visualization into this plan.
- Do not refactor field layout in the same change as first GPU shader.
- Do not introduce float16 until f32 correctness is established.
- Do not implement GPU cell updates in the first milestone.
- Preserve `SimulationConfig` as the source of runtime diffusion parameters.
- Treat species `7` as the current structural/EPS species because CPU code hardcodes it today.
- If making the structural species configurable, do that as a separate small change before or after the GPU prototype, not hidden inside shader work.
