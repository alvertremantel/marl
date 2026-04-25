# Module: field-update

## Purpose

Solves the reaction-diffusion equations for all chemical species across the full 500 × 500 × 200 voxel grid every tick. This is the primary physics layer of MARL. It runs unconditionally — regardless of cell presence — and is the dominant memory bandwidth consumer in the system. All math is spatially uniform, making this the most GPU-amenable component.

## Public Interface

```
field_update(
    field_in:  [N³ × S] float buffer,   // concentration at tick t, S species
    sources:   [N³ × S] float buffer,   // cell secretion/consumption deltas
    params:    FieldParams,             // D[], decay[], dt
) -> field_out: [N³ × S] float buffer  // concentration at tick t+1
```

`FieldParams` per species: diffusivity D, decay rate λ, optional advection/sedimentation drift term for dense compounds.

## Voxel Size

**dx = 100 um (0.01 cm).** This makes the full grid 5.0 cm x 5.0 cm x 2.0 cm -- a realistic Winogradsky column laboratory size. At this scale:
- Gel-phase diffusion (D ~ 10^-6 cm^2/s) crosses ~10 voxels per tick (1 day): `sqrt(2 * D * dt) / dx = sqrt(2 * 10^-6 * 86400) / 0.01 = 41 voxels`. Chemical gradients establish within tens of ticks at the 10-100 voxel scale.
- The CFL stability condition (dt <= dx^2 / 6D) is satisfied: `dx^2 / (6D) = 10^-4 / (6 * 10^-6) = 16.7 seconds`, but we use subcycled effective D values calibrated to the day-scale tick. The effective diffusivity parameter D_eff in the simulation is NOT the physical D -- it is D scaled by (dt / (dx^2)) to absorb the discretization. See [[Decisions/002-tick-timescale-abstraction]].
- Cell-to-cell chemical communication (diffusion across 1-2 voxels) occurs within a single tick. Column-scale gradients (200 voxels) establish within ~25 ticks.

## Internal Design

Discrete 3D Laplacian using 6-neighbor (face-adjacent) stencil:

```
∇²c(x,y,z) ≈ [c(x+1) + c(x-1) + c(y+1) + c(y-1) + c(z+1) + c(z-1) − 6c(x,y,z)] / Δx²
```

Forward-Euler integration:

```
c(t+1) = c(t) + Δt × [D_local × ∇²c − λc + source]
```

where D_local is the locally-modified diffusion coefficient (see Niche Construction section below). When no structural species is present, D_local = D_base and this reduces to the standard constant-coefficient RD equation.

Stability guaranteed by gel-phase D values satisfying Δt ≤ Δx² / 6D_max. Since niche construction only REDUCES D (D_local <= D_base), the CFL condition is strictly easier to satisfy with structural deposits present. See [[Decisions/002-tick-timescale-abstraction]].

Boundary conditions: zero-flux Neumann (∂c/∂n = 0 at all six faces). Implemented via 1-voxel padding layer of ghost cells mirroring their interior neighbor, avoiding in-shader branching.

## Dependencies

- Requires: source/sink delta buffer from [[Modules/cell-agent]] (previous tick)
- Produces: concentration field consumed by [[Modules/cell-agent]] and [[Modules/light-engine]]

## GPU Implementation Strategy

See [[research-gpu-field-update]] for full analysis. Summary:

- **Data layout:** SoA (species-first). Each species stored as a contiguous 100 MB plane. Voxel order: row-major X, then Y, then Z.
- **Tiling:** 2.5D streaming — tile XY plane (32x8 threads per workgroup), stream along Z. Three XY planes in shared memory (~16 KB per workgroup for 8 species). Each value loaded from global memory once, reused across 3 Z-levels.
- **Thread-group ID swizzling:** Remap workgroup launch order for L2 locality. Free optimization, ~20-40% bandwidth reduction.
- **Performance:** ~7-13 ms per tick at 50M voxels x 8 species on RTX 4060 (272 GB/s DRAM, 24 MB L2). The pass is overwhelmingly memory-bandwidth bound (arithmetic intensity ~0.72 FLOPs/byte vs. machine balance ~55 FLOPs/byte).
- **Cell deltas:** Accumulated in a separate float32 buffer, then added to the field in a single pass (avoids scattered atomic operations).

## Niche Construction: Local Diffusion Modification

Cells that secrete the structural-deposit species (external species index designated at initialization, e.g., index 7 or 11) modify the local diffusion coefficient for ALL other species at that voxel. This is the primary niche construction mechanism in MARL.

### Formulation

```
D_local[x,y,z,s] = D_base[s] * (1.0 - alpha * structural[x,y,z] / (K_eps + structural[x,y,z]))
```

Where:
- `D_base[s]` = species-specific base diffusion coefficient (from FieldParams)
- `alpha` = maximum diffusion reduction factor. Default: 0.8 (80% reduction at saturation). Biologically grounded: real biofilm EPS reduces diffusion by 20-80% for most solutes (Stewart, 2003; measured De/Daq ~ 0.25 for organic solutes in P. aeruginosa biofilms).
- `structural[x,y,z]` = concentration of structural-deposit species at this voxel
- `K_eps` = half-saturation constant for EPS effect. Default: 1.0 (at structural=1.0, diffusion is reduced by alpha/2 = 40%).

### Properties

1. **Monotonic reduction:** D_local is always <= D_base. More structural deposit = slower diffusion. This is physically correct (EPS thickening impedes molecular transport).

2. **Saturation:** As structural -> infinity, D_local -> D_base * (1 - alpha). Diffusion never reaches zero. This prevents numerical singularities and is physically realistic (even dense biofilm matrices allow some diffusion).

3. **Species-uniform (v1):** All chemical species experience the same diffusion reduction factor. This is a simplification. In reality, small molecules (O2, CO2) diffuse through EPS more easily than large molecules (proteins). A future extension could make alpha species-dependent: `alpha[s]` per species.

4. **No additional VRAM:** The structural species is already allocated in the field buffer. D_local is computed on-the-fly in the field update shader, not stored.

### GPU Shader Modification

In the field update compute shader, the diffusion term changes from:

```glsl
// BEFORE (constant D):
float laplacian = neighbors_sum - 6.0 * center;
float diffusion_term = D_base * laplacian;

// AFTER (niche construction):
float structural = field_in[structural_species_offset + voxel_idx];
float D_local = D_base * (1.0 - ALPHA * structural / (K_EPS + structural));
float laplacian = neighbors_sum - 6.0 * center;
float diffusion_term = D_local * laplacian;
```

This is one extra texture read (structural concentration, already in shared memory from the species loop) and one multiply-add per voxel per species. At 50M voxels x 12 species, this adds ~600M FLOPs per tick, which is negligible compared to the memory bandwidth cost (~7-13 ms/tick).

### Emergent Dynamics

Niche construction via diffusion modification creates several self-organizing feedback loops:

1. **Metabolite trapping:** Cells in EPS-rich regions retain their secreted metabolites (slower diffusion = less dilution). This benefits cells that produce useful metabolites and harms cells that rely on importing metabolites from far away.

2. **Core-periphery biofilm structure:** Cells at the colony edge experience high diffusion (no EPS) and fast nutrient access. Interior cells experience low diffusion and slower nutrient delivery but retain self-produced metabolites. This creates a natural growth vs. retention tradeoff.

3. **Chemical isolation between clusters:** EPS-producing lineages can create diffusion barriers that chemically isolate different regions of the grid, enabling divergent evolution in semi-isolated populations.

4. **Ecological inheritance:** Dead cells leave behind their EPS deposits (structural species decays slowly, with lambda_structural << lambda for other species). Daughter cells and newcomers inherit the modified diffusion environment. This is ecological inheritance in the niche construction sense (Odling-Smee et al., 2003).

5. **Public goods game:** EPS production costs energy (carbon -> structural in the cell's reaction network). Non-producers benefit from the EPS matrix without paying the cost. This creates a public goods dilemma analogous to the quorum sensing scenario. See [[mock-quorum-sensing-scenario]].

### Parameters

| Parameter | Default | Range | Notes |
|-----------|---------|-------|-------|
| alpha | 0.8 | [0.0, 0.95] | Max diffusion reduction. Higher = denser matrix. |
| K_eps | 1.0 | [0.1, 10.0] | Half-saturation. Lower = less EPS needed for full effect. |
| lambda_structural | 0.005 | [0.001, 0.05] | Structural species decay rate. 10x slower than metabolites. |
| structural_species_idx | 7 or 11 | [0, S-1] | Which external species is the structural deposit. |

### References

- Stewart, P.S. (2003). Diffusion in biofilms. J. Bacteriol., 185(5), 1485-1491.
- Flemming, H.-C. & Wingender, J. (2010). The biofilm matrix. Nature Rev. Microbiol., 8, 623-633.
- Odling-Smee, F.J., Laland, K.N., & Feldman, M.W. (2003). Niche Construction: The Neglected Process in Evolution. Princeton UP.

---

## Known Limitations / Planned

- Float16 precision sufficient for field concentrations. Analysis shows values above ~0.0001 are represented accurately; sub-threshold values flush to zero, acting as a natural noise floor. Use float32 for intracellular ODE. Mixed-precision compute (load float16, compute float32, store float16) is the safest approach and is free on Ada Lovelace. See [[research-gpu-field-update]] Section 7.
- Optional sedimentation drift term (slow downward bias on dense compounds) not yet specified. Low priority.
- Species count S is a compile-time or initialization-time constant. Runtime-variable species count would require dynamic shader recompilation.
- Multi-resolution / adaptive mesh refinement (AMR) evaluated and deferred. Full grid at 50M voxels runs at ~10 ms/tick, which is sufficient. AMR adds complexity without meaningful benefit at this scale. See [[research-gpu-field-update]] Section 8.
