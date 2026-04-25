# Research: GPU Memory Architecture for the Field Update Pass

**Created:** 2026-03-15 (Iteration 002)
**Purpose:** Analyze whether the field update pass (50M voxels x 8 species, 3D Laplacian stencil, Forward Euler) is feasible on an RTX 4060 8GB at interactive rates. Derive the memory access pattern, evaluate tiling strategies, and identify the binding constraint.

---

## 1. RTX 4060 Hardware Constraints

| Specification | Value | Source |
|---|---|---|
| GPU | AD107 (TSMC 4N, 18.9B transistors) | NVIDIA |
| SMs | 24 | NVIDIA |
| CUDA cores | 3,072 | NVIDIA |
| L1 cache per SM | 128 KB (configurable shared/L1 split) | Ada Lovelace architecture whitepaper |
| L2 cache | 24 MB | NVIDIA |
| VRAM | 8 GB GDDR6 | NVIDIA |
| Memory bus | 128-bit | NVIDIA |
| DRAM bandwidth | 272 GB/s | 17 Gbps effective x 128-bit |
| Effective bandwidth (with L2) | ~400-450 GB/s (NVIDIA claims 453, independent tests show ~380-420 depending on workload) | NVIDIA, TechSpot |
| FP32 throughput | 15 TFLOPS | NVIDIA |
| FP16 throughput | 30 TFLOPS | NVIDIA |
| TDP | 115W | NVIDIA |

Key insight: The 24 MB L2 cache is the RTX 4060's secret weapon. It is 8x larger than the RTX 3060's 3 MB L2. For stencil workloads with good locality, the L2 effectively doubles the available bandwidth. Our analysis must account for this.

---

## 2. Memory Footprint

### 2.1 Field Storage

```
Grid:       500 x 500 x 200 = 50,000,000 voxels
Species:    S = 8
Precision:  float16 (2 bytes per value)

Field buffer:  50M x 8 x 2 = 800 MB
Source buffer:  50M x 8 x 2 = 800 MB (cell secretion/consumption deltas)
Output buffer: 50M x 8 x 2 = 800 MB (field at t+1)

Total field memory: 2.4 GB (double-buffered input + source + output)
```

**Problem:** 2.4 GB is 30% of VRAM. This is tight but feasible. We can reduce to 1.6 GB by writing output in-place (single-buffered with careful ordering -- see Section 5 on Red-Black ordering).

### 2.2 Ghost Cells for Boundary Conditions

Zero-flux Neumann boundaries require a 1-voxel ghost layer mirroring interior neighbors:
```
Padded grid: 502 x 502 x 202 = 51,006,008 voxels
Overhead:    ~2% more voxels. Negligible.
```

### 2.3 Other Buffers

```
Light field:      50M x 2 bytes = 100 MB (scalar per voxel, float16)
Cell registry:    ~100K cells x 346 bytes = 35 MB (sparse, worst case)
Remaining VRAM:   8192 - 1600 - 100 - 35 = ~6.4 GB free
                  (or ~4 GB if using full double-buffering)
```

VRAM is NOT the binding constraint. Even with double-buffering, we use at most 2.5 GB for field data. The cell registry is small. There is ample headroom for additional buffers, staging, and the renderer.

---

## 3. Memory Bandwidth Analysis

### 3.1 Bytes Accessed Per Voxel

The 6-neighbor stencil for one species reads 7 values (center + 6 neighbors):
```
Per species per voxel:
  Reads:    7 values x 2 bytes = 14 bytes (field_in)
          + 1 value  x 2 bytes = 2 bytes  (source delta)
  Writes:   1 value  x 2 bytes = 2 bytes  (field_out)
  Total:    18 bytes per species per voxel

For S=8 species:
  Total per voxel: 8 x 18 = 144 bytes
```

But this overcounts. With caching, each voxel's value is read from DRAM at most once and then reused by its 6 neighbors. The minimum read is:

```
Minimum reads per voxel (perfect caching):
  1 value x 2 bytes x 8 species = 16 bytes (field_in, each value read once)
  + 16 bytes (source delta)
  = 32 bytes read

Writes per voxel:
  16 bytes (field_out)

Minimum traffic per voxel: 48 bytes
Practical traffic per voxel (with cache misses): ~60-80 bytes
```

### 3.2 Total Bandwidth Required Per Tick

```
Minimum:     50M voxels x 48 bytes  = 2.4 GB per tick
Practical:   50M voxels x 70 bytes  = 3.5 GB per tick

At 272 GB/s DRAM bandwidth:
  Minimum time:   2.4 / 272 = 8.8 ms
  Practical time:  3.5 / 272 = 12.9 ms

With L2 cache benefit (~1.5x effective bandwidth, ~400 GB/s):
  Practical time:  3.5 / 400 = 8.8 ms
```

**Result: The field update pass takes ~9-13 ms per tick at full grid resolution.** This gives us 77-111 ticks per second for the field pass alone. Since 1 tick = 1 day and we want the simulation to run at maybe 10-100 ticks/sec for interactive exploration, this is comfortable.

### 3.3 Compute vs. Bandwidth Bound

```
FLOPs per voxel per species:
  Laplacian: 7 additions + 1 multiply = 8 FLOPs
  Euler step: 3 multiplies + 2 additions = 5 FLOPs
  Total: ~13 FLOPs per species per voxel

Total FLOPs: 50M x 8 x 13 = 5.2 GFLOPs per tick

At 15 TFLOPS (FP32) or 30 TFLOPS (FP16):
  Compute time (FP32): 5.2 / 15,000 = 0.35 ms
  Compute time (FP16): 5.2 / 30,000 = 0.17 ms
```

**The field update is overwhelmingly memory-bandwidth bound, not compute bound.** The arithmetic intensity is ~13 FLOPs / 18 bytes = 0.72 FLOPs/byte, well below the machine balance point of ~55 FLOPs/byte (15 TFLOPS / 272 GB/s). This means optimization efforts should focus entirely on reducing memory traffic, not on reducing FLOPs.

---

## 4. Tiling Strategy: 2.5D Streaming

The standard GPU optimization for 3D stencils is **2.5D blocking** (Micikevicius, 2009; Nguyen et al., 2010):

### 4.1 The Technique

Instead of launching a 3D grid of threads that each independently read their 7-point stencil from global memory:

1. **Tile the XY plane** into 2D blocks (e.g., 32x8 threads per workgroup)
2. **Stream along Z** — each workgroup processes one Z-layer at a time, advancing from z=0 to z=199
3. **Store 3 XY planes in shared memory**: z-1 (below), z (current), z+1 (above)
4. At each Z step:
   - Load the new z+1 plane from global memory into shared memory
   - Compute the stencil for the current z plane using all three planes
   - Write the result for the current z plane to global memory
   - Shift: z-1 <- z, z <- z+1, load new z+1

### 4.2 Shared Memory Requirements

```
Tile size: (Tx + 2) x (Ty + 2) values  (including 1-cell halo for neighbors)
For Tx=32, Ty=8:  34 x 10 = 340 values per plane
3 planes:          340 x 3 = 1,020 values
At float16:        1,020 x 2 = 2,040 bytes per species
For S=8 species:   2,040 x 8 = 16,320 bytes = 16 KB

Ada Lovelace shared memory per SM: up to 100 KB (configurable)
This fits easily with room for additional registers and local variables.
```

### 4.3 Why This Works

Without tiling, each stencil evaluation reads 7 values from global memory. The center value is reused by 6 neighbors, but with a naive launch pattern, the L1/shared memory is too small to hold the entire Z-column, so values get evicted and re-fetched.

With 2.5D streaming:
- Each XY-plane value is loaded from global memory **once** and used for **3 Z-levels** (as below-plane, current-plane, then above-plane)
- The XY halo is loaded by neighboring threads in the same workgroup
- The effective read amplification drops from 7x to ~1.3x (the 0.3x is the halo overlap between adjacent tiles)

### 4.4 Expected Bandwidth Reduction

```
Naive (no tiling):    7 reads per voxel per species = 14 bytes
2.5D tiled:           ~1.3 reads per voxel per species = 2.6 bytes
Reduction:            ~5.4x fewer global memory reads

Total bandwidth with tiling:
  50M x 8 x (2.6 read + 2 source + 2 write) = 50M x 52.8 bytes = 2.64 GB per tick

Time at 272 GB/s: 2.64 / 272 = 9.7 ms
Time at 400 GB/s (with L2): 2.64 / 400 = 6.6 ms
```

The 2.5D approach doesn't dramatically change the total time because we were already close to the theoretical minimum. The real benefit is **robustness** -- without tiling, performance depends heavily on the L2 cache being large enough to hold the working set. With tiling, performance is predictable regardless of cache behavior.

---

## 5. Data Layout: Species-First vs. Voxel-First

### 5.1 Option A: Array of Structures (AoS) — Voxel-First

```
Memory layout: [v0_s0, v0_s1, ..., v0_s7, v1_s0, v1_s1, ..., v1_s7, ...]
Stride between same species at adjacent voxels: S x 2 = 16 bytes
```

**Problem:** When computing the Laplacian for one species, we read values 16 bytes apart. On a 128-bit memory bus, each cache line (128 bytes on NVIDIA) contains 8 voxels' worth of data for all species. If we only need one species, we waste 7/8 of each cache line.

BUT: If we process all 8 species per voxel in the same thread (which we should, since it's the same stencil), AoS means all 8 species for one voxel are contiguous — a single 16-byte load gets all species for one voxel.

### 5.2 Option B: Structure of Arrays (SoA) — Species-First

```
Memory layout: [v0_s0, v1_s0, v2_s0, ..., vN_s0, v0_s1, v1_s1, ..., vN_s1, ...]
Stride between same species at adjacent voxels: 2 bytes (contiguous)
```

**Advantage:** Adjacent threads processing adjacent voxels read contiguous memory for the same species — perfect coalescing. Each cache line contains 64 consecutive voxels for one species.

**Disadvantage:** To process all species for one voxel, we need 8 reads from 8 different memory regions (each species plane is 100 MB apart). But with 2.5D tiling, all species for a tile are loaded into shared memory together, so this is not a problem at the shared memory level.

### 5.3 Recommendation: SoA (Species-First)

SoA is the standard choice for GPU stencil codes and is recommended here:
- Global memory reads are coalesced (adjacent threads read adjacent addresses)
- The 2.5D tiling handles the cross-species access pattern in shared memory
- All GPU stencil literature assumes SoA layout

The field buffer layout should be: `[species_0: 100MB][species_1: 100MB]...[species_7: 100MB]`

Within each species plane, the voxel ordering should be row-major in X (the innermost loop), then Y, then Z:
```
index(x, y, z, s) = s * (Nx * Ny * Nz) + z * (Nx * Ny) + y * Nx + x
```

---

## 6. Thread-Group ID Swizzling for L2 Locality

NVIDIA's thread-group ID swizzling technique (documented for Battlefield V DXR) remaps the launch order of workgroups to improve L2 cache hit rates:

- Standard dispatch: workgroups launch in row-major order (0,0), (1,0), (2,0)...
- Swizzled dispatch: workgroups launch in tiles of NxN, processing nearby workgroups consecutively

For a 2D dispatch of 16x63 workgroups (500/32 x 500/8), swizzling with N=16 keeps consecutively launched workgroups spatially adjacent, improving L2 hit rates for the halo reads.

**Expected benefit:** 20-40% reduction in effective bandwidth consumption based on NVIDIA's published results (47% improvement on a similar stencil-like workload on RTX 2080). On the RTX 4060 with its already-large 24 MB L2, the benefit may be smaller but still worthwhile.

This is a simple index remapping in the shader — zero additional memory cost.

---

## 7. Float16 Stability Analysis

### 7.1 Float16 Properties

| Property | float16 | float32 |
|---|---|---|
| Mantissa bits | 10 | 23 |
| Decimal precision | ~3.3 digits | ~7.2 digits |
| Min normal | 6.1e-5 | 1.2e-38 |
| Max | 65,504 | 3.4e+38 |
| Epsilon | 9.77e-4 | 1.19e-7 |

### 7.2 Concern: Small Concentration Values

The Forward Euler update is: `c(t+1) = c(t) + dt * [D * laplacian - lambda * c + source]`

If c is small (e.g., 0.001) and the update delta is very small (e.g., 0.0001), the addition `0.001 + 0.0001 = 0.0011` loses precision in float16 because the mantissa only has 10 bits. The relative error is ~0.1%.

**Worse case:** If c is near the float16 minimum normal (6.1e-5), values below this flush to zero. This means chemical concentrations below ~0.0001 effectively become zero in float16.

### 7.3 Impact Assessment

For MARL, this is probably acceptable:
- **Chemical concentrations are typically in the range [0, 10]** after normalization. The interesting dynamics happen at concentrations > 0.01, well within float16's precision.
- **The "flush to zero" behavior acts as a natural noise floor** — concentrations below ~0.0001 are treated as absent. This is actually physically reasonable (below detection threshold).
- **The Laplacian involves subtracting similar values** (center minus average of neighbors). In smooth regions, this difference is small, and float16 quantization introduces noise proportional to ~0.1% of the concentration. In regions with steep gradients, the differences are large and float16 is fine.
- **Forward Euler stability is NOT affected by float16** — the CFL condition (dt <= dx^2 / 6D) is a constraint on the ratio of timestep to diffusivity, independent of floating-point precision. As long as the condition is satisfied (which it is, by construction), the scheme is stable in any precision.

### 7.4 Recommendation

Use float16 for the **field buffer** (external concentrations). Use float32 for **intracellular concentrations** (where the ODE integration involves products of small numbers near catalyst thresholds). This is a pragmatic split:
- Field: 800 MB at float16, 1.6 GB at float32. Float16 saves 800 MB of VRAM — critical.
- Intracellular: 100K cells x 8 species x 4 bytes = 3.2 MB at float32. Negligible cost.

If float16 field precision proves inadequate in practice, the fallback is:
1. **Mixed precision:** Store field in float16, compute Laplacian in float32, write back to float16. This is free on Ada Lovelace (native float16 load/store with float32 ALU).
2. **Bfloat16:** 8-bit exponent (same range as float32) with 7-bit mantissa. Better dynamic range, slightly worse precision. Not natively supported by Vulkan compute, but could be emulated.
3. **Full float32:** Doubles field memory to 1.6 GB. Still fits in 8 GB VRAM but reduces headroom.

---

## 8. Alternative: Multi-Resolution / Adaptive Mesh

Can we reduce the voxel count in regions without cells or chemical activity?

### 8.1 Octree / AMR

Adaptive mesh refinement (AMR) uses coarse resolution where concentrations are smooth and fine resolution near cells or chemical gradients. This could reduce the effective voxel count from 50M to perhaps 5-10M.

**Problems:**
- AMR is complex to implement on GPU (irregular data structures, load balancing)
- The diffusion solver must handle non-uniform grid spacing (variable Laplacian stencils)
- Refinement/coarsening decisions add overhead
- The field update pass is already fast enough (~10 ms) — AMR would reduce this to ~2 ms but add significant implementation complexity

**Verdict:** Not worth it for v1. The full grid at 50M voxels is feasible. Revisit only if we need to scale to larger grids (e.g., 1000^3 = 1B voxels).

### 8.2 Temporal LOD (Skip Quiescent Regions)

Skip field updates in voxels where all concentrations are at steady state (delta < epsilon for N consecutive ticks).

**Problems:**
- Requires tracking per-voxel "activity" state — an additional 50M bytes of metadata
- Creates irregular compute patterns (some voxels updated, others not) — hostile to GPU
- Diffusion inherently propagates activity: a change in one voxel eventually affects all neighbors. Skipping voxels could introduce artifacts at the active/quiescent boundary.

**Verdict:** Not worth it. The field update is already bandwidth-bound, not compute-bound. Skipping voxels saves compute but not bandwidth (we still need to read voxels to check if they're quiescent).

---

## 9. The Cell Update Pass: Memory Considerations

The cell update pass is separate from the field update but shares the field buffer:

```
Cell update reads:
  - 8 float16 values from field (local concentrations): 16 bytes per cell
  - 1 float16 from light field: 2 bytes per cell
  - Cell state (~346 bytes per cell, from cell registry)

Cell update writes:
  - 8 float16 values to delta buffer (secretion/consumption): 16 bytes per cell

At 100K cells: 100K x 380 bytes = 38 MB total memory traffic
Time at 272 GB/s: 38 MB / 272 GB/s = 0.14 ms
```

The cell update pass is negligible compared to the field update pass. Even at 1M cells, it takes ~1.4 ms. The binding constraint is always the dense field update.

**Scatter problem:** Cell deltas must be written to the dense field buffer at the cell's voxel coordinates. This is a scattered write pattern — different cells write to non-contiguous memory locations. On GPU, this requires atomic operations (atomicAdd) to avoid race conditions if multiple cells are adjacent.

At sparse cell densities (100K cells in 50M voxels = 0.2% occupancy), collisions are rare. AtomicAdd on float16 is not natively supported on all hardware; the workaround is:
1. Write to a float32 delta buffer (100 MB for 50M x 8 x float16, promoted to float32 = 200 MB during accumulation)
2. Use a separate pass to add the delta buffer to the field buffer

This is cleaner than scattered atomics and adds only one extra buffer pass (~2 ms).

---

## 10. Putting It All Together: Per-Tick Budget

```
Pass                    | Time (est.) | VRAM traffic | Notes
-----------------------|-------------|-------------|------
Field update (2.5D)    | 7-10 ms     | 2.5-3.5 GB  | Bandwidth-bound
Light attenuation      | 0.5-1 ms    | ~200 MB     | Column sweep, Z-sequential
Cell update            | 0.2-1.5 ms  | 40-400 MB   | Depends on cell count
Delta accumulation     | 1-2 ms      | 800 MB      | Add cell deltas to field
HGT + bookkeeping      | <0.5 ms     | Negligible  | CPU-side, async
-----------------------|-------------|-------------|------
TOTAL                  | 9-15 ms     | 3.5-5 GB    | ~65-110 ticks/sec
```

**At 65-110 ticks/sec, the simulation runs in real-time for interactive exploration.** For overnight batch runs (the primary use case), this means ~5.6M - 9.5M ticks per day = 15,000-26,000 simulated years per day of wall-clock time.

This is more than sufficient for the OEE metrics framework (which requires at least 10,000 ticks for publication-quality data). A single overnight run produces decades of simulated evolution.

---

## 11. Summary and Recommendations

1. **The field update pass is feasible on RTX 4060** at ~10 ms per tick for 50M voxels x 8 species in float16.

2. **Use SoA data layout** (species-first) for coalesced global memory access.

3. **Use 2.5D streaming** (tile XY, stream Z) with shared memory to maximize data reuse. Tile size 32x8 threads, 3 planes in shared memory, ~16 KB per workgroup.

4. **Apply thread-group ID swizzling** for L2 cache locality (free optimization, ~20-40% bandwidth reduction).

5. **Float16 is acceptable** for field concentrations. Use float32 for intracellular ODE integration. Mixed-precision compute (load float16, compute in float32, store float16) is the safest approach.

6. **Skip AMR and temporal LOD** for v1. The full grid is fast enough.

7. **Cell deltas** should accumulate in a separate float32 buffer, then be added to the field in a single pass. This avoids scattered atomics.

8. **The system is overwhelmingly bandwidth-bound.** Optimization effort should focus on memory access patterns, not arithmetic optimization.

---

## 12. Sparse Delta Buffer: GPU Scatter-Gather Design

### 12.1 The Problem

ADR-006 proposes S=12 external species. At S=12, a dense delta buffer (one float32 per species per voxel) costs:

```
50M voxels x 12 species x 4 bytes = 2,400 MB
```

This is infeasible on 8 GB VRAM alongside the field buffers. But only voxels containing cells (or immediately adjacent to cells) have nonzero deltas. At 100K cells with 6 face-adjacent neighbors each, at most ~700K voxels (1.4% of 50M) need delta storage. A sparse representation reduces this to:

```
700K voxels x 12 species x 4 bytes = 33.6 MB
```

The challenge is designing a GPU-friendly scatter-gather pattern that:
1. Lets cells write deltas without race conditions
2. Enables a fast gather pass to apply sparse deltas to the dense field
3. Stays within shared memory and register limits
4. Does not require CPU-GPU synchronization mid-tick

### 12.2 Design: Sort-Based Scatter-Gather (No Atomics)

The recommended approach is adapted from the NVIDIA CUDA Particles pattern (Green, 2010) and Particle-In-Cell (PIC) plasma simulation codes. The key insight is: **sort cells by voxel coordinate hash, then accumulate deltas per-voxel in a sequential scan over the sorted list.** This eliminates atomic operations entirely.

#### Phase 1: Cell Delta Computation (Per-Cell Kernel)

Each cell evaluates its ruleset and writes its deltas to a **per-cell buffer** (not per-voxel). No race conditions because each cell writes to its own slot.

```
Buffer: cell_deltas[N_cells][S]  — float32
Size:   100K cells x 12 species x 4 bytes = 4.8 MB
```

Each cell also writes its target voxel coordinate (3 x u16 = 6 bytes) and any neighbor voxel coordinates where it has transport effects (for secretion into adjacent voxels, if applicable).

```glsl
// Kernel 1: cell_evaluate
// One thread per cell. No inter-thread communication.
layout(local_size_x = 256) in;

void main() {
    uint cell_id = gl_GlobalInvocationID.x;
    if (cell_id >= num_cells) return;

    // Read cell state, field concentrations, light
    CellState cell = cells[cell_id];
    float ext[S];
    for (int s = 0; s < S; s++)
        ext[s] = field_in[cell.voxel_idx * S + s];  // or SoA indexing
    float light = light_field[cell.voxel_idx];

    // Run B+E hybrid ruleset (receptor, transport, reactions, effector, fate)
    float delta[S];
    run_ruleset(cell, ext, light, delta);

    // Write per-cell output — no race condition, each cell has its own slot
    for (int s = 0; s < S; s++)
        cell_deltas[cell_id * S + s] = delta[s];
    cell_voxel_keys[cell_id] = cell.voxel_idx;  // uint32 linear voxel index
}
```

#### Phase 2: Sort Cells by Voxel Index (GPU Radix Sort)

Sort the `(voxel_key, cell_id)` pairs by `voxel_key` using a GPU radix sort. This brings all cells at the same voxel (if any -- at 0.2% occupancy, collisions are very rare) into contiguous memory.

```
Input:  cell_voxel_keys[N_cells] — unsorted uint32 voxel indices
        cell_ids[N_cells]        — identity permutation [0, 1, 2, ..., N-1]
Output: cell_voxel_keys[N_cells] — sorted
        cell_ids[N_cells]        — permuted to match sorted order

Sort algorithm: GPU radix sort (Onesweep, Merrill & Grimshaw 2011; Adinets & Merrill 2022)
Time: ~0.3-0.5 ms for 100K keys on RTX 4060 (radix sort is O(n*k) where k=32 bits)
```

Radix sort is the standard choice for GPU particle simulations. Vulkan implementations exist in the VkRadixSort library (Hoetzlein, 2023) and in the Fuchsia Vulkan radix sort.

#### Phase 3: Find Segment Boundaries (Per-Cell Kernel)

After sorting, identify where each voxel's cells begin and end in the sorted array. This is the "findCellStart" pattern from NVIDIA CUDA Particles.

```glsl
// Kernel 3: find_segment_boundaries
layout(local_size_x = 256) in;

void main() {
    uint idx = gl_GlobalInvocationID.x;
    if (idx >= num_cells) return;

    uint voxel = sorted_voxel_keys[idx];

    // Compare with previous element
    if (idx == 0 || sorted_voxel_keys[idx - 1] != voxel) {
        segment_start[voxel] = idx;  // First cell at this voxel
    }
    if (idx == num_cells - 1 || sorted_voxel_keys[idx + 1] != voxel) {
        segment_end[voxel] = idx + 1;  // One past last cell at this voxel
    }
}
```

**Optimization note:** At 0.2% occupancy, >99.8% of voxels have zero cells and >99.99% of occupied voxels have exactly one cell. The segment boundary detection is almost always trivial (every cell is its own segment). This means the sort-based approach has negligible overhead for the common case.

#### Phase 4: Gather and Apply Deltas (Per-Voxel Kernel, Sparse)

A compact buffer of unique occupied voxel indices is built via stream compaction (prefix sum on the segment_start array). Then a kernel iterates over only occupied voxels, sums the deltas from all cells at that voxel, and applies them to the dense field.

```glsl
// Kernel 4: apply_deltas
// One thread per OCCUPIED voxel (not per grid voxel — sparse dispatch)
layout(local_size_x = 256) in;

void main() {
    uint sparse_idx = gl_GlobalInvocationID.x;
    if (sparse_idx >= num_occupied_voxels) return;

    uint voxel = occupied_voxel_list[sparse_idx];
    uint start = segment_start[voxel];
    uint end   = segment_end[voxel];

    // Sum deltas from all cells at this voxel
    float total_delta[S];
    for (int s = 0; s < S; s++) total_delta[s] = 0.0;

    for (uint c = start; c < end; c++) {
        uint cell_id = sorted_cell_ids[c];
        for (int s = 0; s < S; s++)
            total_delta[s] += cell_deltas[cell_id * S + s];
    }

    // Apply to dense field (write to field_out or to a separate delta layer)
    for (int s = 0; s < S; s++)
        field_sources[voxel * S + s] = total_delta[s];
}
```

**Key property: no atomics, no race conditions.** Each occupied voxel is processed by exactly one thread. The gather loop over cells at that voxel is sequential within the thread, which is safe. At 0.2% occupancy with almost all segments of length 1, the inner loop almost never executes more than once.

### 12.3 Alternative: Direct Atomic Scatter (Simpler, Slower)

For comparison, the direct atomic approach skips phases 2-3 and has each cell atomically add its deltas to the dense field:

```glsl
// Alternative: atomic scatter (one thread per cell)
for (int s = 0; s < S; s++)
    atomicAdd(field_sources[voxel * S + s], delta[s]);
```

**Advantages:**
- Simpler (one kernel, no sort, no segment detection)
- No temporary buffers beyond per-cell deltas

**Disadvantages:**
- Requires `VK_EXT_shader_atomic_float` for float32 atomicAdd. This extension is supported on NVIDIA Ada Lovelace (RTX 40-series) and AMD RDNA 2+ but is NOT universally available. Using it limits portability.
- Atomics are inherently serializing. Even at 0.2% occupancy, the 12 atomic writes per cell (one per species) create memory system overhead due to cache line invalidation.
- The delta buffer must be dense (50M voxels x S species) because any voxel could be targeted. This defeats the sparse storage goal. To make it work with a dense buffer, we'd need the full 2,400 MB allocation.

**Hybrid option:** Use atomics on a **sparse** delta buffer indexed by cell voxel hash. This requires a hash table on the GPU, which is feasible (see SlabHash, Ashkiani et al. 2018) but adds significant implementation complexity.

**Verdict:** The sort-based approach is preferred. It avoids atomics, avoids extension dependencies, works with a compact per-cell buffer (4.8 MB vs 2,400 MB), and the sort cost (~0.5 ms) is negligible relative to the field update pass (~10 ms).

### 12.4 VRAM Budget for Sparse Delta Buffer

| Buffer | Size at 100K cells, S=12 | Notes |
|--------|--------------------------|-------|
| cell_deltas[N][S] | 4.8 MB | Per-cell delta output (float32) |
| cell_voxel_keys[N] | 0.4 MB | uint32 voxel index per cell |
| sorted_cell_ids[N] | 0.4 MB | uint32 permutation array |
| segment_start/end | 0.8 MB | uint32, indexed by voxel (sparse hash or dense) |
| occupied_voxel_list | 0.4 MB | uint32, compacted list of occupied voxels |
| **Total** | **~7 MB** | vs. 2,400 MB for dense delta buffer |

At 1M cells: ~70 MB total. Still vastly smaller than the 2,400 MB dense alternative.

**Note on segment_start/end:** These could be stored densely (50M entries x 4 bytes = 200 MB each) or sparsely. For the sparse approach, a simple open-addressing hash table with 2x the number of occupied voxels as capacity works: 200K entries x 8 bytes (key + value) = 1.6 MB. The dense approach is simpler but costs 400 MB. At 100K cells, the dense approach is acceptable (400 MB < available headroom), but at higher cell counts the hash table becomes preferable.

**Recommended approach for v1:** Use dense segment_start/end arrays (400 MB). This avoids hash table complexity. Total sparse delta system: ~407 MB. Still saves 2,000 MB compared to the dense delta buffer.

**Recommended approach if VRAM is tight:** Switch to a hash-based segment lookup. Total drops to ~9 MB. Implement this as a fallback if the cell population grows large or additional VRAM is needed for the renderer.

### 12.5 Timing Budget

| Phase | Estimated Time | Notes |
|-------|---------------|-------|
| Cell evaluation (Kernel 1) | 0.2-1.5 ms | Same as current, depends on cell count |
| Radix sort (Phase 2) | 0.3-0.5 ms | 100K keys, 32-bit, Onesweep |
| Segment boundaries (Kernel 3) | <0.1 ms | One pass over 100K elements |
| Apply deltas (Kernel 4) | <0.1 ms | ~100K occupied voxels, trivial work per voxel |
| **Total sparse delta pipeline** | **0.6-2.2 ms** | vs. ~2 ms for dense delta accumulation |

The sort-based approach is time-competitive with the dense approach (which requires a full dense-buffer addition pass at ~2 ms) while saving ~2 GB of VRAM. This is the enabling technology for S=12.

### 12.6 Integration with Field Update Pass

The sparse delta system produces a `field_sources` buffer that feeds into the field update pass. Two integration strategies:

**Strategy A: Sparse source buffer.** The field update shader checks whether the current voxel has a nonzero source (via the occupied_voxel_list or a flag buffer). If not, the source term is zero and the branch is skipped. This adds branching to the field shader, which is undesirable.

**Strategy B: Dense source buffer, sparse write.** Kernel 4 writes its output to a dense float16 source buffer (50M x S x 2 bytes = 1,200 MB at S=12). This is smaller than the float32 delta buffer (2,400 MB) because the accumulated values have already been summed and can be downcast to float16. The field update shader reads this buffer unconditionally -- no branching.

**Strategy C (recommended): Zero-initialize + sparse write.** Clear the dense source buffer to zero at the start of each tick (fast memset, ~0.3 ms for 1.2 GB). Kernel 4 writes only to occupied voxels. The field update shader reads the source buffer unconditionally, getting zero for unoccupied voxels. This is the simplest approach and avoids branching.

At float16 precision for the source buffer:
```
50M x 12 x 2 bytes = 1,200 MB
```

This is substantial. An alternative is to use a float32 source buffer at only the occupied voxels, but this requires the field shader to do an indirect lookup. The better approach is:

**Strategy D (recommended for S=12): Fold deltas directly into field during the gather pass.** Instead of writing to a separate source buffer, Kernel 4 directly adds the accumulated deltas to `field_out`:

```glsl
// In Kernel 4, instead of writing to field_sources:
for (int s = 0; s < S; s++) {
    uint field_idx = voxel + s * grid_size;  // SoA addressing
    float16 current = field_out[field_idx];
    field_out[field_idx] = current + float16(total_delta[s]);
}
```

This eliminates the source buffer entirely. The field update pass computes diffusion + decay, writing to `field_out`. Then Kernel 4 adds cell deltas directly to `field_out`. No additional buffer needed.

**Race condition analysis:** Kernel 4 must execute AFTER the field update pass completes (ensured by a Vulkan pipeline barrier). Since Kernel 4 processes each voxel with exactly one thread, there are no write conflicts within Kernel 4 itself. The field update pass has already finished writing, so there are no read-write conflicts.

**Final VRAM budget with Strategy D, S=12:**

| Buffer | Size | Notes |
|--------|------|-------|
| field_in (float16) | 1,200 MB | S=12 species, read-only |
| field_out (float16) | 1,200 MB | S=12 species, written by field pass then delta pass |
| cell_deltas (float32) | 4.8 MB | Per-cell, temporary |
| sort/segment buffers | 2 MB | Keys, IDs, boundaries |
| light field | 100 MB | Scalar |
| cell registry | 54 MB | At 100K cells |
| **Total** | **~2,561 MB** | 32% of 8 GB |

This is a dramatic improvement over the 4,800 MB required by the dense delta buffer approach at S=12. It leaves 5.4 GB of headroom for the renderer, debug visualization, and potential cell population growth to 1M+.

### 12.7 Edge Case: Secretion into Adjacent Voxels

The design above assumes each cell writes deltas only to its own voxel. But the effector pass may secrete chemicals into face-adjacent voxels (e.g., directional secretion, diffusion-like spreading of secreted products). If a cell writes to its 6 neighbors:

- Each cell produces up to 7 delta records (self + 6 neighbors) instead of 1
- The sort-based approach still works: emit 7 `(voxel_key, delta)` records per cell, sort by voxel_key, segment and sum as before
- Buffer sizes increase 7x: cell_deltas becomes ~34 MB at 100K cells. Still small.

However, **the current spec does not require adjacent-voxel secretion.** Cells secrete into their own voxel; diffusion handles spatial spreading on the next tick. This is physically correct (secretion is local, transport is the field's job) and keeps the delta system simple. Adjacent-voxel secretion can be added later if needed.

### 12.8 Consequences for ADR-006

With the sparse delta buffer fully designed, the VRAM constraint that motivated keeping S <= 8 is resolved. The sort-based scatter-gather pattern:

1. Reduces delta VRAM from 2,400 MB (dense float32) to ~7 MB (sparse per-cell)
2. Eliminates the need for `VK_EXT_shader_atomic_float`
3. Adds ~0.5 ms to the per-tick budget (dominated by the radix sort)
4. Integrates cleanly with the existing 2.5D tiling field update

**ADR-006 can now move from Proposed to Accepted.** The sparse delta buffer is no longer a blocking prerequisite -- it is a designed, costed, and timed component.

---

## 13. References

- Micikevicius, P. (2009). 3D Finite Difference Computation on GPUs using CUDA. NVIDIA Technical Report.
- Nguyen, A., Satish, N., Chhugani, J., Kim, C., & Dubey, P. (2010). 3.5-D Blocking Optimization for Stencil Computations on Modern CPUs and GPUs. Proc. IEEE/ACM SC10.
- NVIDIA. (2022). Optimizing Compute Shaders for L2 Locality using Thread-Group ID Swizzling. NVIDIA Developer Blog.
- Sai, R. et al. (2020). Accelerating High-Order Stencils on GPUs. arXiv:2009.04619.
- NVIDIA. (2022). Ada Lovelace Architecture Whitepaper. NVIDIA Corporation.
- Green, S. (2010). Particle Simulation using CUDA. NVIDIA SDK Whitepaper. [Sort-based grid construction: calcHash, radix sort, findCellStart pattern]
- Adinets, A. & Merrill, D. (2022). Onesweep: A Faster Least Significant Digit Radix Sort for GPUs. arXiv:2206.01784.
- Ashkiani, S., Farach-Colton, M., & Owens, J.D. (2018). A Dynamic Hash Table for the GPU. Proc. IEEE IPDPS.
- Tran, S. & Tran, B. (n.d.). CUDA Parallelization of a 2D Particle-in-Cell Code. UC San Diego. [Charge deposition via atomicAdd scatter pattern]
- Khronos Group. (2022). VK_EXT_shader_atomic_float. Vulkan Extension Specification. [Float32 atomicAdd in compute shaders]
