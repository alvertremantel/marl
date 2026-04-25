# 006 -- Species Namespace and Count

**Date:** 2026-03-15
**Status:** Accepted
**Updated:** 2026-03-15 (Iteration 006 -- sparse delta buffer designed, status upgraded from Proposed)

## Context

The Winogradsky column stress-test (mock-winogradsky-scenario.md, Iteration 003) used 8 external and 8 internal species. Of these, 5 external species were functionally distinct (energy-carrier, oxidant, reductant, carbon-source, organic-waste), 2 were reserved for signals, and 1 for structural deposits. Internally, 5 species mapped to internalized versions of external chemicals, and 3 were reserved for enzymes (enzyme-A, enzyme-B, enzyme-C).

This allocation was tight. A fourth metabolic strategy -- say, a methanogen consuming CO2 and H2 to produce methane -- would require at least one new external species (methane) and likely a new internal enzyme slot. With only 2 signal slots and 1 structural slot remaining, adding quorum sensing or biofilm matrix production would exhaust the namespace entirely.

The species count is a fundamental architectural parameter that affects:
1. **VRAM consumption** (linear in species count)
2. **Compute cost** (linear in species count for field update; quadratic for reaction network if all-to-all coupling is considered)
3. **Evolutionary search space** (combinatorial in species count)
4. **Ecological richness** (more species = more possible niches)

This ADR resolves two questions:
- **Q1:** Should internal and external species share a namespace or remain separate?
- **Q2:** What should the species counts be (S external, M internal)?

---

## Q1: Namespace Design -- Separate vs Overlapping

### Option A: Separate Namespaces (Current Design)

External species are indexed 0..S-1. Internal species are indexed 0..M-1. The transport layer explicitly maps between them via `(ext_species: u8, int_species: u8)` pairs per transporter.

**Advantages:**
- Maximum flexibility. Internal species 3 need not correspond to external species 3. A cell could use internal slot 3 for "enzyme-X" while external slot 3 is "carbon-source." This allows cells to use their internal namespace for catalysts, intermediaries, and storage without "wasting" external namespace slots.
- The transport layer IS the membrane. The explicit mapping between namespaces represents selective permeability, which is biologically realistic. Cells evolve which external chemicals they import and where they store them internally.
- Internal namespace can be larger or smaller than external namespace without coupling.

**Disadvantages:**
- More parameters per cell. Each transporter needs 2 species indices (ext + int) plus kinetic parameters = 6 bytes per transporter, times S transporters.
- HGT of reaction rules is slightly more complex: a transferred reaction refers to internal species indices, which have the SAME meaning in all cells (since M and internal semantics are shared). But the transport mapping to external species may differ, meaning a transferred reaction might reference an internal species that the recipient cell transports differently.
- Conceptual overhead for spec readers and implementers.

### Option B: Shared Namespace

A single set of N species shared between external field and intracellular domain. Species 0 is the same substance inside and outside the cell; the transport layer controls flow rates but not identity mapping.

**Advantages:**
- Simpler. Fewer parameters per transporter (just uptake_rate and secrete_rate, no species index mapping).
- HGT transfers are unambiguous: a reaction consuming species 3 and producing species 5 means the same thing in every cell.
- Easier to reason about mass conservation (same units inside and outside).

**Disadvantages:**
- Wastes namespace. If cells need 4 distinct internal enzymes, those enzymes occupy 4 slots in the shared namespace, meaning 4 fewer slots for field chemicals. With N=8, this leaves only 4 field chemicals.
- Enzymes in the field. If the namespace is shared, a cell that dies and releases its contents would release "enzyme-A" into the field. This is physically unrealistic (enzymes are intracellular) and would pollute the field with meaningless chemicals. Either enzymes must not diffuse (special-cased, violating "no predefined types"), or they diffuse and dilute (wasting field bandwidth), or they decay instantly (special-cased).
- No membrane selectivity. All species cross the membrane identically. In reality, cells are selective about what they import and export.

### Option C: Partially Overlapping Namespace (Hybrid)

External species 0..S-1 exist in the field. Internal species 0..S-1 are the "internalized" versions of field species (same identity). Internal species S..M-1 are intracellular-only (enzymes, intermediates, storage). The transport layer only handles species 0..S-1 and each transporter maps ext_i to int_i (identity mapping, no index needed). Species S..M-1 never cross the membrane.

**Advantages:**
- Clean biological analog. External chemicals can be internalized; internal enzymes cannot leak.
- Simpler transport (no species index mapping needed -- ext_i always maps to int_i).
- HGT reactions referencing species 0..S-1 have clear external-world meaning. Reactions referencing species S..M-1 are "intracellular metabolism" that depends only on internal state.
- No field pollution by enzymes.

**Disadvantages:**
- Rigid boundary at index S. Internal-only species cannot evolve to become secretable (unless we add a transport mutation that promotes an internal species to external status -- which has interesting evolutionary implications but adds complexity).
- Less flexible than Option A for unusual metabolisms that might want to internalize external chemicals into non-corresponding slots.

### Decision: Option A (Separate Namespaces) -- Maintained

After analysis, the current design (separate namespaces with explicit transport mapping) remains the best choice. The key arguments:

1. **Enzyme pollution is a showstopper for Option B.** Having enzymes exist in the field creates artifacts that would confound ecological analysis.
2. **Option C is tempting but too rigid.** The hard boundary at index S prevents evolutionary innovation where an internal metabolite becomes a secreted signal (which is how real quorum sensing evolved -- intracellular metabolites that happen to be secretable).
3. **Option A's extra complexity is modest.** The transport layer adds 2 bytes (ext_idx, int_idx) per transporter, for a total of 16 bytes at S=8. This is <5% of the 314-byte ruleset.
4. **HGT compatibility is handled by shared internal namespace.** All cells use the same M internal species indices. A transferred reaction rule references internal indices, which are universal. Whether the recipient cell transports external species into those internal slots differently is part of the evolutionary dynamic, not a bug.

---

## Q2: Species Count -- How Many?

### Current: S=8 external, M=8 internal

Total field VRAM: 50M voxels x 8 species x 2 bytes (float16) = 800 MB per buffer.
Double-buffered: 1,600 MB.
Delta buffer (float32): 50M x 8 x 4 = 1,600 MB.
Total field VRAM: ~3,200 MB.

### Analysis of the Winogradsky Ceiling

The Winogradsky scenario used these 8 external species:

| Index | Role | Status |
|-------|------|--------|
| 0 | energy-carrier | Active |
| 1 | oxidant (O2) | Active |
| 2 | reductant (H2S) | Active |
| 3 | carbon-source (CO2) | Active |
| 4 | organic-waste | Active |
| 5 | signal-A | Reserved |
| 6 | signal-B | Reserved |
| 7 | structural (EPS) | Reserved |

Five functional species consumed all the "chemistry slots," leaving only 3 for signals and structure. A richer ecology needs:
- **Methane** (methanogen product, consumed by methanotrophs) -- 1 slot
- **Nitrogen species** (NH4+, NO3-) for nitrogen cycling -- 1-2 slots
- **Iron species** (Fe2+/Fe3+) for iron cycling -- 1-2 slots
- **Additional signals** for distinct quorum sensing circuits -- 1-2 slots
- **Secondary metabolites / toxins** for chemical warfare -- 1-2 slots

This suggests a minimum of 12-16 external species for a rich microbial ecology.

For internal species, the Winogradsky scenario used:
- 5 internalized external chemicals (energy, oxidant, reductant, carbon, organic)
- 3 enzymes (A, B, C)

A richer metabolism might need:
- More enzyme/catalyst slots for complex pathways (4-6)
- Intermediate metabolites for multi-step pathways (2-3)
- Storage compounds (poly-P, glycogen analogs) (1-2)

This suggests M = 12-16 internal species as well.

### VRAM Cost Analysis

The field VRAM cost scales linearly with species count:

| Config | S (ext) | Field (float16) | Double-buffer | Delta (float32) | Total Field | Remaining (8 GB) |
|--------|---------|------------------|---------------|-----------------|-------------|-------------------|
| Current | 8 | 800 MB | 1,600 MB | 1,600 MB | 3,200 MB | 4,800 MB |
| Moderate | 12 | 1,200 MB | 2,400 MB | 2,400 MB | 4,800 MB | 3,200 MB |
| Rich | 16 | 1,600 MB | 3,200 MB | 3,200 MB | 6,400 MB | 1,600 MB |
| Maximum | 20 | 2,000 MB | 4,000 MB | 4,000 MB | 8,000 MB | 0 MB |

At S=16, the field alone consumes 6.4 GB of 8 GB VRAM, leaving only 1.6 GB for light field (100 MB), cell registry, and the Vulkan driver overhead (~200-400 MB). This is dangerously tight.

**Key insight: the delta buffer is the bottleneck.** The delta buffer uses float32 (4 bytes per value) because it accumulates cell secretion/consumption values that may require atomic operations or precise accumulation. At S=16, the delta buffer alone is 3.2 GB.

### Optimization: Sparse Delta Buffer

The delta buffer does not need to be dense. Only voxels containing cells (or adjacent to cells) have nonzero deltas. At 100K cells with 6 face-adjacent neighbors each, at most ~700K voxels (1.4% of 50M) need delta storage. A sparse delta buffer using a hash map or sorted coordinate list would reduce delta VRAM from 3.2 GB to ~45 MB at S=16:

```
700K voxels x 16 species x 4 bytes = 44.8 MB
```

This changes the VRAM picture dramatically:

| Config | S (ext) | Field (2x float16) | Delta (sparse, float32) | Light | Total | Remaining |
|--------|---------|---------------------|-------------------------|-------|-------|-----------|
| Current | 8 | 1,600 MB | ~22 MB | 100 MB | 1,722 MB | 6,278 MB |
| Moderate | 12 | 2,400 MB | ~34 MB | 100 MB | 2,534 MB | 5,466 MB |
| Rich | 16 | 3,200 MB | ~45 MB | 100 MB | 3,345 MB | 4,655 MB |
| Maximum | 24 | 4,800 MB | ~67 MB | 100 MB | 4,967 MB | 3,033 MB |

With a sparse delta buffer, even S=24 fits comfortably in 8 GB VRAM.

**Implementation cost of sparse deltas:** The sparse buffer uses a sort-based scatter-gather pattern adapted from GPU Particle-In-Cell codes (Green, 2010). Cells write deltas to a per-cell buffer (no race conditions), then a GPU radix sort groups cells by voxel index, a segment-boundary kernel identifies per-voxel ranges, and a gather kernel sums and applies deltas directly to the field. No atomics needed. Total overhead: ~0.5 ms per tick for the sort, ~7 MB VRAM for all temporary buffers at 100K cells. Full design in [[research-gpu-field-update]] Section 12.

### Internal Species VRAM Cost

Internal species are per-cell, not per-voxel. The cost is:

```
100K cells x M internal x 4 bytes (float32) = M x 400 KB
```

At M=16: 6.4 MB. At M=32: 12.8 MB. This is negligible. Internal species count is NOT VRAM-constrained.

The ruleset size scales with M and R_max:
- Receptors: S x 6 bytes
- Transport: S x 6 bytes
- Reactions: R_MAX x 10 bytes (substrate, product, catalyst indices scale with M but are u8, so no size change up to M=256)
- Effectors: S x 6 bytes
- Fate: 6 bytes
- Meta: 4 bytes

At S=16, M=16, R_MAX=24:
- Receptors: 96 bytes
- Transport: 96 bytes
- Reactions: 240 bytes
- Effectors: 96 bytes
- Fate + meta: 10 bytes
- **Total: 538 bytes/cell**

At 100K cells: 54 MB. At 1M cells: 538 MB. Still fits.

### Compute Cost Scaling

The field update pass reads 6 neighbors + 1 center value per species per voxel:
- Memory bandwidth: proportional to S (species count)
- At S=8, ~10 ms/tick. At S=16, ~20 ms/tick. At S=24, ~30 ms/tick.
- 30 ms/tick = 33 ticks/sec, still excellent for the use case.

The cell update pass evaluates R_MAX reactions per cell:
- Each reaction references 3-4 internal species (substrate, product, catalyst, cofactor)
- Compute scales as O(R_MAX) per cell, independent of M (species are accessed by index)
- Increasing R_MAX from 16 to 24 increases cell update cost by ~50%, but this is on the sparse cell population, not the dense field, so the absolute cost is small.

### Recommended Configuration

| Parameter | Current | Proposed | Rationale |
|-----------|---------|----------|-----------|
| S (external) | 8 | **12** | Sufficient for 5 core metabolites + 3 signals + 2 structural + 2 reserve. Methane and nitrogen cycling possible. |
| M (internal) | 8 | **16** | 12 internalized external + 4 dedicated enzyme/intermediate slots. Or: any allocation the cell evolves via transport mapping. |
| R_MAX | 16 | **16** (unchanged) | Already above Kauffman's RAF threshold at M=16 (ratio = 1.0). Increase to 24 only if empirical runs show metabolic bottleneck. |

**VRAM at S=12, M=16:**
- Field (2x float16): 2,400 MB
- Delta (sparse): ~34 MB
- Light: 100 MB
- Cells (100K): ~54 MB ruleset + 6.4 MB internal
- Driver overhead: ~300 MB
- **Total: ~2,895 MB** (36% of 8 GB)

This leaves 5.1 GB of headroom -- enough for 1M+ cells, additional debug buffers, and the renderer.

### Why Not S=16 or Higher Now?

S=12 is a deliberate choice for several reasons:

1. **Shared memory for 2.5D tiling.** The tiling strategy loads 3 XY planes into shared memory per workgroup. At S=12 with a 32x8 tile:
   - Per-species plane in shared memory: 34 x 10 x 4 bytes (float32 compute) = 1,360 bytes
   - 12 species x 3 planes x 1,360 = 48,960 bytes
   - RTX 4060 shared memory limit: 49,152 bytes (48 KB)
   - This barely fits. S=16 would require 65,280 bytes, exceeding the limit.
   - **Mitigation:** Process species in two batches of 6, or reduce tile size, or use register tiling instead of shared memory. All add complexity.

2. **Vulkan descriptor count.** At S=12 with SoA layout: 12 (field_in) + 12 (field_out) + 1 (light) + 1 (cells) = 26 descriptors. Comfortably within Vulkan limits but approaching practical descriptor set management complexity.

3. **Evolutionary search space.** At M=16 and R_MAX=16, the number of possible reaction topologies is C(16^3, 16) -- vastly larger than at M=8. This may slow evolutionary convergence. Empirical testing will determine if this is a problem.

4. **S is a compile-time constant.** Increasing S later requires recompiling shaders and reallocating buffers but NOT redesigning the architecture. Starting at 12 and increasing to 16 if needed is safer than starting at 16 and discovering shared memory problems.

---

## Consequences

1. **Species count becomes a two-parameter configuration:** S (external) and M (internal), specified at initialization time (or compile time for shader specialization constants).
2. **Sparse delta buffer designed and costed.** The dense delta buffer is infeasible at S > 8. The sort-based sparse delta system (Section 12 of research-gpu-field-update.md) reduces delta VRAM from 2,400 MB to ~7 MB at 100K cells, adding only ~0.5 ms to the per-tick budget. This prerequisite is now resolved.
3. **Shared memory tiling needs redesign.** The current 2.5D tiling strategy (research-gpu-field-update.md) must be adapted for S=12. Options: batch species in groups of 6, reduce tile size to 16x8, or use register-based tiling. This is an implementation concern, not an architectural one.
4. **Ruleset sizing increases.** From ~314 bytes/cell (S=8, R_MAX=16) to ~538 bytes/cell (S=12, M=16, R_MAX=16). Still negligible vs. field VRAM.
5. **R_MAX remains 16 for now.** The Kauffman RAF threshold at M=16 is R_MAX/M = 1.0, right at the phase transition. This is intentional: it means autocatalytic loops are possible but not guaranteed, creating strong evolutionary pressure. If empirical runs show that M=16 makes autocatalysis too hard, R_MAX can be increased to 20-24.

---

## Open Questions

1. **Should S be a runtime parameter or compile-time constant?** Vulkan specialization constants allow compile-time parameterization without shader recompilation. This is the recommended approach.
2. **Should R_MAX scale with M?** A fixed ratio R_MAX/M ~ 2 would maintain the Kauffman threshold as M increases. At M=16, R_MAX=32 would be well above threshold but double the reaction evaluation cost.
3. **How to handle the shared memory constraint at S > 12?** The most promising approach is species batching: process 6 species per dispatch, two dispatches per tick. Each dispatch reads/writes only its species subset. This doubles dispatch count but keeps shared memory within limits.

---

## References

- Kauffman, S.A. (1986). Autocatalytic sets of proteins. J. Theor. Biol., 119(1), 1-24.
- Stewart, P.S. (2003). Diffusion in biofilms. J. Bacteriol., 185(5), 1485-1491.
- Hordijk, W. & Steel, M. (2017). Chasing the tail. BioSystems, 152, 1-10.
- PhysiCell (Ghaffarizadeh et al., 2018): BioFVM handles "dozens of diffusing substrates" on desktop hardware with CPU OpenMP parallelism. MARL targets similar substrate counts but with GPU acceleration.
