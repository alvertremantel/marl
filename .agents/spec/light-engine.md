# Module: light-engine

## Purpose

Computes a per-voxel light availability scalar using Beer-Lambert attenuation integrated along the Z axis (top-down). This is the only directionally asymmetric element in the simulation and is the primary driver of emergent vertical zonation. Runs once per tick after the field update pass and before the cell update pass.

## Public Interface

```
light_update(
    cell_density:   [N³] float,          // cell presence/density per voxel
    absorbers:      [N³ × A] float,      // absorber chemical concentrations, A species
    params:         LightParams,         // I₀, α_cell, α_absorber[]
) -> light_field:  [N³] float           // light availability [0, I₀] per voxel
```

`LightParams`: surface intensity I₀, per-cell attenuation coefficient α_cell, per-species absorber coefficients α_absorber[].

## Internal Design

For each (x, y) column, compute a prefix sum along Z from z=0 (surface) downward:

```
attenuation(x,y,z) = Σ_{z'=0}^{z-1} [α_cell × ρ(x,y,z') + Σᵢ α_i × cᵢ(x,y,z')]
light(x,y,z) = I₀ × exp(−attenuation(x,y,z))
```

This is a parallel prefix scan along Z — O(N² × Z) total, trivially parallelizable by (x,y) column. Each column is independent.

## Biological Implications

- Dense phototroph populations near the surface self-shade — population growth is self-limiting via light competition.
- Photosynthetic cells that produce an oxidant proxy (e.g. "O2 analog" species) as a light-dependent output will drive a vertical redox gradient. Oxidant-intolerant cells are naturally excluded from the surface zone without explicit rules.
- Emergent layering is the Winogradsky column in simulation form: phototrophs at top, oxidant-tolerant chemolithotrophs below, anaerobes at depth.

## Dependencies

- Reads: cell density field (derived from cell registry), absorber species concentrations from [[Modules/field-update]]
- Produces: light availability field consumed by [[Modules/cell-agent]]

## Known Limitations / Planned

- Which chemical species are designated absorbers is a simulation initialization parameter. At least one must be set. Default suggestion: a "biomass pigment" species secreted by all cells.
- Diurnal cycling (oscillating I₀) not currently planned but trivial to add — expose I₀ as a function of tick count modulo cycle length.
- Spectral differentiation (different wavelengths absorbed by different compounds, multiple pigment strategies) is out of scope for initial implementation but is a natural extension for a paper follow-up.
