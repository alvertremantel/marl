# Module: cell-agent

## Purpose

Defines the data structure and per-tick update logic for individual cellular agents. Cells are sparse — stored in a hashmap keyed on voxel coordinates, not in the dense grid. Each cell maintains an internal chemical concentration vector, a ruleset (see [[Decisions/003-ruleset-representation]]), and a scalar energy budget. Per tick, each cell reads the local external chemical field, runs its ruleset, updates internal concentrations, and emits: secretion/consumption deltas to the field, and optionally a reproduction or death event.

## Public Interface

```
cell_update(
    cells:      CellRegistry,           // sparse hashmap: voxel_coord → CellState
    field:      [N³ × S] float buffer,  // external chemical field (read-only this pass)
    light:      [N²] float buffer,      // light availability per (x,y,z) from light-engine
) -> (
    deltas:     [N³ × S] float buffer,  // secretion/consumption to fold into next field tick
    events:     EventQueue,             // reproduction, death, quiescence events
)
```

## CellState Structure

```
CellState {
    voxel:          (u16, u16, u16),     // grid position
    internal_conc:  [M] float,           // internal chemical concentrations, M species
    energy:         float,               // current energy budget
    ruleset:        Ruleset,             // see Decisions/003
    age:            u32,                 // ticks since birth
    lineage_id:     u64,                 // for phylogenetic tracking
}
```

M (internal species count) is a global constant, likely 4–8. Keeping it fixed across all cells preserves uniform ODE structure — all cells run the same integration code with different parameters, which is GPU-batchable.

## Ruleset Evaluation (B+E Hybrid — see [[mock-hybrid-cell-tick]] for complete pseudocode)

The B+E hybrid combines parametric receptor/effector layers (Option B) with an intracellular catalytic reaction network (Option E). Each tick per cell:

1. **Receptor pass (Option B):** For each external species i, compute activation Aᵢ = gain × cᵢ_ext^n / (kᵢ^n + cᵢ_ext^n). Hill function with evolvable parameters (k_half, n_hill, gain). Light availability is an additional input.
2. **Transport pass:** Move chemicals across the interstitial/intracellular boundary. Uptake and secretion rates are Michaelis-Menten-like, governed by evolvable per-species transport parameters. Mass is conserved between domains.
3. **Intracellular reaction network (Option E):** Evaluate R_max=16 reaction rules. Each rule is an "abstract enzyme" with substrate, product, catalyst, and kinetic parameters. Rate = v_max × [S]/(k_m + [S]) × [C]/(k_cat + [C]). Catalyst is NOT consumed (true catalysis). Internal concentrations updated via Forward Euler ODE step. Autocatalytic loops emerge naturally.
4. **Effector pass (Option B):** Internal concentrations above evolvable thresholds drive secretion of external species.
5. **Fate decision:** Internal species 0 ("energy carrier") compared against evolvable thresholds for death, quiescence, and division. Energy is a hardcoded distinguished species (see [[Decisions/007-energy-currency]]): all cells use `internal_conc[0]` for fate decisions, and a fixed per-tick maintenance drain (`lambda_maintenance = 0.02`) ensures that cells must actively produce energy to survive.

## Dependencies

- Reads: [[Modules/field-update]] output, [[Modules/light-engine]] output
- Produces: delta buffer consumed by [[Modules/field-update]], event queue consumed by [[Modules/hgt-engine]]

## Sizing

```
Per-cell ruleset:  ~314 bytes (48 receptor + 48 transport + 160 reactions + 48 effector + 6 fate + 4 meta)
Per-cell internal:  32 bytes (8 species x float32)
Per-cell total:    ~346 bytes
At 100K cells:      35 MB (negligible vs. 800 MB field)
At 1M cells:       346 MB (still fits in 8 GB VRAM with field)
```

## Known Limitations / Planned

- ~~Ruleset representation format gated on [[Decisions/003-ruleset-representation]].~~ B+E hybrid is the leading candidate. See [[mock-hybrid-cell-tick]] for complete pseudocode.
- GPU batching strategy: all cells execute the same fixed-length loop (S=8 receptors, S=8 transporters, R_MAX=16 reactions, S=8 effectors). No branching on cell identity. Inactive reactions (v_max=0) are no-ops within the fixed loop. This is GPU-friendly by construction.
- Quiescence state: energy below quiescence_threshold. Cell remains alive, receptors and transport still run, but effector output is suppressed. Maintenance decay continues, so quiescent cells slowly die unless they accumulate energy from light or transported nutrients.
- Direct cell-cell contact signaling deliberately omitted. If contact-mediated behavior emerges as a research priority, add a contact-detection pass reading the cell registry for adjacent occupied voxels.
- Cell deltas are written to a separate float32 accumulation buffer, then added to the float16 field in a single pass (avoids scattered atomics). See [[research-gpu-field-update]].
