# 004 — Vertical Light Attenuation as Primary Energy Source

**Date:** 2026-03-15  
**Status:** Accepted

## Context

The simulation grid is spatially isotropic except where a physical asymmetry is imposed. Without an asymmetry source, vertical zonation (the most characteristic feature of real microbial mats) cannot emerge. Light is the natural candidate: it is directional, it attenuates with depth, and it is the primary energy source for surface-dwelling microbial communities. Adding it as a field variable creates the condition for phototroph stratification, oxidant gradients, and chemolithotroph niches to emerge without programming them.

## Options Considered

- **Option A — No light, uniform energy:** Simplest. No vertical structure emerges unless chemical gradients happen to self-organize it. Less biologically interesting. Omits the primary driver of real mat community structure.
- **Option B — Light as a global Z-depth proxy:** Cells at depth Z receive intensity I(Z) = I₀ × exp(−αZ) where α is a fixed constant. Simple, cheap, but doesn't respond to local cell density or chemical absorbers — a deep dense layer looks the same as a deep sparse one.
- **Option C — Beer-Lambert with local absorbers:** Intensity at each voxel is computed by integrating attenuation along the Z column above it, where attenuation is a function of local cell density and concentration of absorber chemicals. I(x,y,z) = I₀ × exp(−∫₀ᶻ [α_cell × ρ_cell(x,y,z') + Σᵢ α_i × cᵢ(x,y,z')] dz'). Captures self-shading by dense populations. Phototrophs that outcompete each other vertically shade out their own descendants. Correct physics.

## Decision

Option C. Beer-Lambert attenuation with local absorbers. The column integral is a prefix sum along Z — O(N² × Z) and trivially parallelizable per column on GPU. Light availability is written as a scalar field channel read by cells during the cell update pass as an energy input.

## Consequences

- A dedicated light attenuation pass is added to the per-tick compute pipeline, between field update and cell update.
- At least one chemical species must be designated as a light absorber (can be a secreted compound, a cell-density proxy, or both).
- Photosynthetic cells that produce an oxidant (e.g. a proxy oxygen species) as a light-dependent output will naturally drive a vertical redox gradient — oxidants near surface, anoxic conditions at depth. Obligate anaerobe analogs will be selected against near the surface without any explicit rule encoding this.
- Self-shading dynamics mean dense phototroph populations are self-limiting — a natural population control mechanism.
- Light intensity I₀ and attenuation coefficients α are simulation parameters, exposable to the user.

## Notes

This is the mechanism that makes MARL simulate a Winogradsky column in emergent form. The Winogradsky column is one of the most pedagogically legible demonstrations of chemical niche partitioning in microbiology — its emergence here without explicit programming is a strong result for a paper.
