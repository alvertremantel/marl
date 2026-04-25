# MARL Overnight Runs — 2026-03-16/17

## Output Directory Index

| Run | Directory | Grid | Ticks | Physics | Key Result |
|-----|-----------|------|-------|---------|------------|
| Test | `test_pipeline/` | 128x128x64 | 50 | Old (free diffusion through cells) | Pipeline validation only |
| 1 | `run1_128x128x64_5k/` | 128x128x64 | ~500 of 5000 | Old + 10% diffusion blocking | **Saturated 99.9%** by tick 250. Killed early. |
| 2 | `run2_diffblock_2k/` | 128x128x64 | 2000 | 10% diffusion blocking | **Saturated 99.9%**, E=0.44, no zonation. 10% blocking insufficient. |
| Test | `test_newdiffusion/` | 128x128x64 | 200 | Full exclusion, old params | 77% at tick 200, still growing. Boundary sources too strong (C_MAX). |
| Test | `test_excluded_diffusion/` | 128x128x64 | 200 | Full exclusion, old params | Same — heading to saturation from edges inward. |
| 3 | `run3_fullphysics_500/` | 128x128x64 | 500 | Full exclusion + 12% maint + rxn cost | **66% at t=500**, E=0.01. Dead zone forming z=33-35. Still growing. |
| 4 | `run4_fullphysics_2k/` | 128x128x64 | 2000 | Full exclusion + 12% maint + rxn cost | **79% capacity, zombie equilibrium.** Cells evolved death_energy→0, survived at E=0.00008. No turnover after t=500. |
| 5 | `run5_deathfloor_500/` | 128x128x64 | 500 | + hard death floor (E<0.01=dead) | **20% capacity! Sparse dynamics!** 14.5M div, 8.7M death. Beautiful z-profile: packed surface, sparse middle (50-350/layer), packed bottom. |
| 6 | `run6_deathfloor_5k/` | 128x128x64 | 5000 | Same as Run 5 | **In progress** — 5000 ticks, checking long-term stability. |

## Physics Changes (chronological)

1. **Baseline (commits up to fa56876):** Free diffusion everywhere. Cells read own voxel. Grid saturates instantly.
2. **Cell diffusion blocking (e398d73):** Occupied voxels reduce D by CELL_DIFFUSION_FACTOR=0.1. Still saturates — 10% is too much.
3. **Full exclusion (2583075):** Occupied voxels completely excluded from diffusion. Cells read from empty neighbors. Cells surrounded by other cells get zero input.
4. **Evolution + parameter tuning (f94a407):** Maintenance 7%→12%, reaction cost 0.003/rxn/tick, gene duplication mutations, reduced boundary sources.
5. **Hard death floor (6cb1e4c):** HARD_DEATH_FLOOR=0.01. Cells can't evolve death threshold below this. Eliminates zombie cells, creates real turnover.

## Key Insight

The fundamental problem was that cells are physical objects but the diffusion solver treated them as transparent. Fixing this (full exclusion) creates natural carrying capacity: only surface cells access nutrients. Interior cells starve. This is how real biofilms work.

## How to View PPM Images

PPM files can be opened directly in most image viewers, or converted:
```bash
# On Windows with ImageMagick:
magick convert xz_oxidant_t500.ppm xz_oxidant_t500.png
```

## How to Read the Data

- `ticks.csv` — one row per tick, columns: tick, pop, avg_energy, avg_enzyme_a/b, avg_active_rxns, div, death, z0_cells..z63_cells
- `chem_<t>.csv` — one row per z-layer, columns: z, species_0..11_avg, light_avg
- `cells_<t>.csv` — one row per cell: x, y, z, energy, enzyme_a, enzyme_b, active_rxns, lineage_id, age, quiescent
- `summary.md` — automated post-run lab notebook entry
