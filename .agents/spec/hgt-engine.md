# Module: hgt-engine

## Purpose

Processes reproduction events from the cell update pass and generates daughter cell rulesets. This is the evolutionary engine of MARL. It handles both vertical inheritance (parent → daughter with mutation) and horizontal gene transfer (partial ruleset adoption from neighboring cells at the time of division). HGT propensity is itself a mutable parameter, allowing evolutionary dynamics to act on the rate of lateral transfer — a second-order evolvable property.

## Public Interface

```
hgt_process(
    events:     ReproductionEventQueue, // from cell-agent pass
    registry:   CellRegistry,           // for neighbor lookups
    rng:        RngState,
) -> new_cells: Vec<CellState>
```

## Reproduction Logic

For each reproduction event (parent cell P dividing into daughter D at adjacent empty voxel V):

1. **Site selection:** If multiple adjacent empty voxels exist, select by lowest local chemical concentration of a configurable "crowding signal" species — cells preferentially bud into lower-density space.

2. **Vertical inheritance:** Copy parent ruleset to daughter. All parameters inherited exactly before mutation.

3. **Mutation pass:** For each mutable parameter in daughter ruleset, apply Gaussian perturbation with probability p_mut (per-parameter mutation rate, itself evolvable). Parameter values clamped to valid ranges.

4. **HGT pass:** Sample neighbor cells within 1-voxel radius of V (the daughter's position, not the parent's). For each neighbor N with a different lineage (skip clonemates): with probability p_hgt (from *parent's* ruleset — parent's HGT propensity governs, not daughter's), select a random **active reaction rule** (v_max > 0) from N's catalytic network and copy it into a random reaction slot in the daughter's catalytic network, overwriting whatever was there. This transfers a complete metabolic capability — substrate, product, catalyst, and kinetic parameters — not just a single scalar. The transferred reaction is immediately functional because all cells share the same internal species namespace (M=8 species, same indices). Whether the transferred reaction is *useful* depends on whether the daughter has the required catalyst at sufficient concentration — creating a natural compatibility filter. At most one HGT event per reproduction. See [[mock-hybrid-cell-tick]] for complete pseudocode.

5. **Lineage bookkeeping:** Assign daughter a new lineage_id encoding parent lineage + generation counter. Used for phylogenetic reconstruction.

## HGT Parameter Notes

`p_hgt` is the per-parameter horizontal transfer probability. It is a component of the ruleset and therefore evolvable. This means:
- Rulesets that "benefit" from HGT (i.e., neighboring rulesets are compatible or synergistic) may evolve higher p_hgt.
- Rulesets in a well-adapted niche surrounded by poorly-adapted neighbors may evolve lower p_hgt to preserve successful parameter sets.
- This is an uninstructed analog of restriction-modification systems.

## Dependencies

- Consumes: [[Modules/cell-agent]] reproduction event queue
- Produces: new CellState entries returned to simulation controller for registry insertion
- Reads: CellRegistry for neighbor lookups

## Known Limitations / Planned

- ~~Parameter block definition for HGT copy not yet specified.~~ **Resolved:** HGT transfers a complete reaction rule from the donor's catalytic network (Option E layer). This parallels real bacterial HGT of metabolic operons. The receptor/effector layers (Option B) are NOT transferred by HGT — they are inherited vertically and modified only by point mutation. This reflects the biological distinction between core regulatory machinery (receptors) and transferable metabolic capabilities (operons/enzymes).
- Mutation rate p_mut evolvability: making mutation rate itself mutable is biologically realistic (mutator strains) but introduces the risk of mutation rate runaway. Consider soft upper bound.
- No recombination (two-parent sexual analog) currently planned. Could be added as a distinct mechanism for cells in direct contact.
