# Observability & Lineage Tracking — Design Notes

## The Problem

We can see emergent dynamics (middle-zone recolonization, competitive exclusion,
vertical zonation) but we can't trace *how* they happen. When the middle zone
gets recolonized after extinction, we don't know:

- Which lineage(s) colonized it
- What mutations enabled survival in the starvation zone
- Whether it was a single lucky mutant or a gradual evolutionary front
- Whether the colonizers came from above (phototroph descendants) or below (anaerobe descendants)

## What We Need

### 1. Lineage Tree (Parent-Child Tracking)

Currently `lineage_id` is a random u64 assigned at birth. It's useless for
tracing ancestry — it's just a tag with no tree structure.

**Proposed:** Each cell gets a `parent_lineage_id` field. On division, the daughter
gets a new `lineage_id` but records `parent_lineage_id = parent.lineage_id`. This
creates a forest (multiple roots from the 3 starter metabolisms).

**Data output:** Add a `lineages_<tick>.csv` snapshot:
```
lineage_id, parent_lineage_id, birth_tick, pos_x, pos_y, pos_z, starter_type
```

Where `starter_type` = {phototroph, chemolithotroph, anaerobe} is inherited from
the original seed and never mutated — it's a permanent ancestral marker.

### 2. Mutation Log

Record every mutation event as it happens, not just the final state.

**Proposed:** A `mutations.csv` file appended during the run:
```
tick, lineage_id, parent_lineage_id, mutation_type, target, old_value, new_value
```

Where `mutation_type` = {parametric, structural_substrate, structural_product,
structural_catalyst}, and `target` = which reaction/transporter/receptor index.

This is potentially huge (millions of rows) but compresses well and enables
post-hoc reconstruction of evolutionary trajectories.

### 3. Phenotype Fingerprinting

Instead of tracking every parameter, compute a compact "phenotype hash" from
the functionally relevant parts of the ruleset:

- **Active reaction signature:** Sort active reactions by (substrate, product, catalyst),
  hash the resulting tuple. Two cells with the same active reactions in the same
  configuration have the same phenotype, regardless of inactive slots or kinetic
  details.
- **Metabolic type:** Classify by which external species the cell net-consumes vs
  net-produces. A cell that consumes oxidant+carbon and produces organic waste is
  a "phototroph-like" regardless of its internal wiring.

**Data output:** Add `phenotype` and `metabolic_type` columns to `cells_<tick>.csv`.

### 4. Spatial Lineage Maps

For each snapshot, color-code cells by their `starter_type` ancestor. This produces
a visual map showing which original metabolism dominates each depth zone — and
crucially, whether middle-zone colonizers descend from top or bottom.

**Implementation:** PPM image where:
- Red = phototroph descendants
- Green = chemolithotroph descendants
- Blue = anaerobe descendants
- Brightness = energy level

### 5. Invasion Events

Automatically detect when a new lineage appears in a z-zone where it wasn't
present N ticks ago. Log these "invasion events" with:
```
tick, z_zone, invading_starter_type, invader_count, resident_count
```

This would have caught the middle-zone recolonization automatically.

## Implementation Priority

1. **Starter type inheritance** — trivial, add one u8 field to CellState. Highest ROI.
2. **Spatial lineage PPM** — easy once starter_type exists. Huge visual payoff.
3. **Phenotype fingerprinting** — moderate effort, needed for any serious analysis.
4. **Parent lineage tracking** — easy but the data volume is large.
5. **Mutation log** — most complex, most data, but essential for "how did this evolve" questions.
6. **Invasion detection** — post-processing script, not in the hot loop.

## What NOT to Do

- Don't try to build a full phylogenetic tree in real-time. Too expensive.
- Don't log every cell every tick. The cell snapshots every 500 ticks are enough
  for spatial analysis; the mutation log captures the evolutionary detail.
- Don't hash the entire ruleset — it changes too fast from parametric drift.
  Focus on the functionally meaningful bits (active reaction topology).
