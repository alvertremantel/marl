# MARL

### Microbial Automata with Reaction-diffusion and Lineage

> marl, old term used to refer to an earthy mixture of fine-grained minerals. The term was applied to a great variety of sediments and rocks with a considerable range of composition. Calcareous marls grade into clays, by diminution in the amount of lime, and into clayey limestones. Greensand marls contain the green, potash-rich mica mineral glauconite; widely distributed along the Atlantic coast in the United States and Europe, they are used as water softeners.

-- *the encyclopedia brittanica, for some reason*

MARL is a CPU-based Rust research prototype for 3D reaction-diffusion cellular automata with simulated lineage. It models sparse microbial cells embedded in a continuous extracellular chemical field. Cells do not interact directly. They interact through transport, secretion, diffusion-limited access to neighboring empty voxels, and a depth-dependent light field.

The current codebase is a Winogradsky-column style simulation: oxidant and carbon are sourced at the top boundary, reductant is sourced at the bottom boundary, and three starter metabolisms are seeded at different depths. Birth, death, quiescence, and mutation emerge from local chemistry and per-cell rulesets rather than from any explicit fitness function.

## Current State

- Language: Rust
- Execution model: CPU only
- Default grid: `128 x 128 x 64`
- Transport medium: diffusion only
- Cell occupancy: one cell per voxel
- Light model: Beer-Lambert attenuation from the surface
- Evolution path: vertical mutation during division
- Horizontal gene transfer: code exists but is not currently wired into the main loop

This repository is a functional prototype, not a polished platform. The core simulation loop, field physics, lineage tracking, snapshots, and run summaries are implemented. Several intended extensions are present only in partial form.

## What The Simulation Does

- Maintains a dense 3D extracellular field with `12` external species
- Maintains a sparse cell population with `16` internal species and up to `16` reactions per cell
- Diffuses chemistry with cell-body exclusion and local diffusion slowdown from structural deposits
- Computes a per-voxel light field from top-down attenuation
- Updates each cell through receptor, transport, reaction, effector, and fate phases
- Supports mutation of kinetic parameters and rare structural rewiring of reactions
- Writes raw binary field/cell snapshots for viewer ingestion, plus optional legacy CSV/PPM diagnostics

## Architecture At A Glance

- `src/config.rs`: compile-time grid constants and runtime config structs (TOML + CLI)
- `src/field.rs`: 3D extracellular chemistry and parallel diffusion
- `src/cell.rs`: cell rulesets, cell tick, mutation logic, lineage state
- `src/light.rs`: top-down light attenuation field
- `src/hgt.rs`: horizontal gene transfer helper
- `src/data.rs`: CSV logging, reaction registry, end-of-run summary
- `src/snapshot.rs`: PPM cross-sections and ancestry images
- `src/main.rs`: seeding, simulation loop, births/deaths, orchestration

More detail lives in `INFO.md`.

## Important Model Choices

- There is no explicit fitness function.
- Occupied voxels are excluded from diffusion, so dense clusters starve internally.
- Cells sample only empty face-neighbor voxels, not their own occupied voxel.
- Division splits all internal species 50/50 to avoid division as a free-energy exploit.
- Light is used as a catalyst signal inside cells, not as a free energy source.
- Selection is thermodynamic and spatial, driven by local chemistry and access constraints.

## Known In-Progress Or Partial Areas

- Receptors are computed each tick but are not yet used to gate transport or reactions.
- `hgt.rs` is implemented, but HGT is currently disabled in the main loop.
- The code includes spare external and internal species capacity for future chemistry.
- The project is CPU-only today despite earlier GPU-oriented ambitions.


## Running

Build and run with Cargo:

```bash
cargo run --release -- --ticks 5000 --stats 100 --snapshot 500 --images 500
```

### Runtime Configuration

All physics, chemistry, biology, and output parameters are runtime-configurable via an optional TOML file and CLI overrides. Grid dimensions remain compile-time constants (they determine array sizes).

Load settings from a TOML file:

```bash
cargo run --release -- --config marl.toml
```

A sample `marl.toml` with all defaults is included in the repository. Copy it and modify values for parameter sweeps or reproducible scenarios. Partial TOML files work — missing keys fall back to built-in defaults.

Supported CLI flags (override TOML values):

- `--config <path>` — path to TOML config file (default: `marl.toml` in CWD)
- `--ticks <n>` — total simulation ticks
- `--stats <n>` — stdout stats interval
- `--snapshot <n>` — binary and optional CSV snapshot interval
- `--images <n>` — optional PPM image snapshot interval
- `--seed <n>` — cells to seed per starter metabolism
- `--output <dir>` — output directory

Grid dimensions are compile-time constants in `src/config.rs`, so changing grid size requires recompilation.

## Outputs

Runs write into an output directory like `output/run_128x128x64` and produce by default:

- `run_meta.json`: grid dimensions, species count, snapshot cadence, and binary layouts
- `tick_<T>.field.bin`: raw `f32` field data in `[z][y][x][species]` order
- `tick_<T>.cells.bin`: packed viewer cell records (`pos`, `lineage_id`, `starter_type`, `energy`)
- `summary.md`: end-of-run run summary

Legacy diagnostics are opt-in via `marl.toml`:

- `ticks.csv`: per-tick population and z-layer counts (`write_tick_log = true`)
- `chem_<tick>.csv`, `cells_<tick>.csv`, `reactions_<tick>.csv`, `reaction_registry.csv`: CSV snapshots (`write_csv_snapshots = true`)
- `*.ppm`: cross-sections, density maps, and ancestry maps (set `xz_snapshot_species`, `xy_slice_depths_frac`, `write_density_map`, or `write_ancestry_map`)

## Status Summary

The project is already a real simulation rather than a scaffold. Its current strengths are the field/cell split, the spatial exclusion model, the seeded ecological gradient, and the data products. Its main unfinished areas are adaptive receptor wiring, re-enabled HGT, and broader chemistry expansion.

## Citation

If you use this software in your scholarly work, please attribute it with this citation:

> Geosmin Jones. (2026). *Microbial Automata with Reaction-Diffusion and Lineage* (Version 0.2.0.1) [Desktop Software]. Github. https://github.com/alvertremantel/marl

If you're publishing in a big kid venue, you're free to shoot me an email at geosminjones@gmail.com for my real name and Google Scholar profile. 
