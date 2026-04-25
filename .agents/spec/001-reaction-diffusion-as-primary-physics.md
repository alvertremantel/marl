# 001 — Reaction-Diffusion Field as Primary Physics Layer

**Date:** 2026-03-15  
**Status:** Accepted

## Context

The naive GoL-derived approach would make cell geometry the primary driver — cells count neighbors, apply rules, live or die. This forces structural assumptions (interstitial spacing, fixed neighborhood topology) that are architecturally limiting and biologically arbitrary. The alternative is to make the chemical field the primary substrate and reduce cells to sparse readers/writers of that field.

## Options Considered

- **Option A — GoL-derived geometry:** Cells count neighbors, rules are topological. Simple, well-precedented, but cell-cell interaction is direct and spatial; chemistry is secondary or absent.
- **Option B — Reaction-diffusion primary, cells sparse:** The field evolves by standard diffusion+reaction equations every tick regardless of cell presence. Cells read local concentrations, run chemical response rulesets, write secretion/consumption deltas back. Cell-cell interaction is fully mediated by the field.

## Decision

Option B. The field is the primary physics layer. Cells interact only through chemistry.

## Consequences

- Cell-cell contact is no longer structurally presumed — it can only emerge from chemical motivation. This is biologically honest for the targeted regime (biofilm).
- Field update is the dominant compute cost but is maximally GPU-parallelizable (uniform math, no branching, no heterogeneity).
- Cell update cost is proportional to live cell count, not grid size — sparsity is directly rewarded.
- Ruleset heterogeneity (the interesting part) is isolated to the sparse cell pass, where it is manageable.
- The system is now in conversation with the artificial chemistry and PhysiCell literature rather than GoL extensions.

## Notes

PhysiCell (Ghaffarizadeh et al., 2018, PLOS Comp Bio) uses a similar field+sparse-agent architecture but with fixed researcher-defined phenotypes and no evolution. MARL's evolvable-ruleset layer is the differentiating feature.
