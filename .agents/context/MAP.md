# MARL Repo Map

## Cargo Workspace
- `Cargo.toml`: virtual workspace manifest with 9 crates under `crates/`
- `crates/marl-config/`: compile-time grid constants + runtime `SimulationConfig`/`OutputConfig`/`Config` (zero simulation deps, only serde + toml)
- `crates/marl-cell/`: cellular biology — `CellState`, `Ruleset`, `Reaction`, all param types, 5-phase `tick()`, `mutate()`, HGT (depends on `marl-config` + rand)
- `crates/marl-field/`: extracellular physics — `Field` (CPU diffusion), `LightField` (Beer-Lambert attenuation) (depends on `marl-config` + rayon)
- `crates/marl-sim/`: simulation orchestration — tick loop (`run()`), seeding, spatial helpers, starter metabolisms, stats printing (depends on `marl-config` + `marl-cell` + `marl-field` + `marl-output`; optional `marl-gpu`)
- `crates/marl-output/`: I/O sidecar — binary dumps, CSV diagnostics (`DataLogger`, `ReactionRegistry`), PPM snapshots, run summaries (depends on `marl-config` + `marl-cell` + `marl-field` + `marl-format`)
- `crates/marl-gpu/`: optional GPU diffusion — `GpuFieldDiffuser`, `GpuContext`, WGSL shader, CPU/GPU equivalence tests (depends on `marl-config` + `marl-field` + wgpu)
- `crates/marl-engine/`: thin binary crate (~10 lines) — loads config, forwards to `marl_sim::run()`
- `crates/marl-format/`: shared binary schema — `RunMeta`, `ViewerCellRecord`, field layout constants (unchanged, bridge between engine and viewer)
- `crates/marl-viewer-rs/`: standalone `wgpu` viewer with `egui` GUI (depends on `marl-format` only)

## Dependency DAG
```
marl-config        ← zero internal deps (serde + toml)
    ↓
marl-cell          ← depends on marl-config + rand + rand_distr
marl-field         ← depends on marl-config + rayon
    ↓
marl-sim           ← depends on marl-config + marl-cell + marl-field + marl-output (+ optional marl-gpu)
marl-output        ← depends on marl-config + marl-cell + marl-field + marl-format
marl-gpu           ← depends on marl-config + marl-field + wgpu + bytemuck + pollster
    ↓
marl-engine (bin)  ← depends on marl-sim (+ forwards gpu feature to marl-sim)
```

## Simulation Code
- `crates/marl-config/src/lib.rs`: compile-time dimensions/species counts plus runtime `SimulationConfig` and `OutputConfig` with `Config::load()`
- `crates/marl-cell/src/cell.rs`: cell state, rulesets, reactions, transport, fate, mutation
- `crates/marl-cell/src/hgt.rs`: horizontal gene transfer helper (not currently wired into the main loop)
- `crates/marl-field/src/field.rs`: extracellular field storage, CPU diffusion, boundary sources, field tests
- `crates/marl-field/src/light.rs`: top-down light attenuation field
- `crates/marl-sim/src/lib.rs`: `run()` function — full tick loop (boundary sources → diffusion → light → cell updates → fate processing → logging/snapshots)
- `crates/marl-sim/src/seeding.rs`: field boundary initialization and cell seeding
- `crates/marl-sim/src/spatial.rs`: face-neighbor spatial utilities (empty neighbors, environment reading, division placement)
- `crates/marl-sim/src/starter_metabolisms.rs`: phototroph, chemolithotroph, and anaerobe factory functions
- `crates/marl-sim/src/stats.rs`: per-tick stats printing and final z-layer profile

## Output And Viewer Data
- `crates/marl-output/src/binary_dump.rs`: raw viewer files (`tick_<T>.field.bin`, `tick_<T>.cells.bin`) and `run_meta.json`
- `crates/marl-output/src/data.rs`: optional CSV diagnostics, reaction registry, end-of-run `summary.md`
- `crates/marl-output/src/snapshot.rs`: optional PPM cross-sections, density maps, and ancestry maps
- `crates/marl-format/`: shared binary schema constants, `RunMeta`, field byte-length helper, and packed `ViewerCellRecord`

## Standalone Viewer
- `crates/marl-viewer-rs/src/main.rs`: thin viewer binary entrypoint
- `crates/marl-viewer-rs/src/args.rs`: viewer CLI parsing with isometric/cell mode flags and GUI label helpers
- `crates/marl-viewer-rs/src/io.rs`: `run_meta.json`, field snapshot, cell record loading, and tick discovery
- `crates/marl-viewer-rs/src/camera.rs`: deterministic orthographic camera basis for iso/top views
- `crates/marl-viewer-rs/src/renderer.rs`: `wgpu` renderer with reloadable snapshot resources, 3D field/cell texture upload, egui overlay pass, and action processing
- `crates/marl-viewer-rs/src/gui.rs`: `egui` GUI state, action enum, toolbar/sidebar layout, tick navigation helpers, view settings controls, and unit tests
- `crates/marl-viewer-rs/src/app.rs`: `winit` application/event loop handling with egui event forwarding
- `crates/marl-viewer-rs/src/viewer_raymarch.wgsl`: full-screen raymarch shader with isometric ray/AABB traversal, chemical field sampling, and cell voxel compositing

## GPU Prototype
- `crates/marl-gpu/src/context.rs`: GPU device/queue creation with error types
- `crates/marl-gpu/src/field_diffusion.rs`: `GpuFieldDiffuser` with synchronous upload/dispatch/readback
- `crates/marl-gpu/src/shaders/field_diffuse.wgsl`: WGSL compute shader with duplicated grid/species constants
- `crates/marl-gpu/tests/gpu_diffusion.rs`: CPU/GPU equivalence tests (compiled with `gpu` feature on `marl-gpu`)

## Documentation And Agent Context
- `README.md`: project landing page (overview, architecture, model choices, status, citation)
- `docs/USAGE.md`: comprehensive usage guide (build, engine, viewer, outputs, workflows, troubleshooting)
- `docs/INFO.md`: deep technical characterization and architecture reference
- `docs/SCRIPTS.md`: documentation for project utility scripts
- `scripts/check_binary_dump.py`: quick binary output sanity checker
- `marl.toml`: sample runtime config with binary outputs on and legacy diagnostics off by default
- `.agents/plans/`: implementation plans
- `.agents/context/STATUS.md`: current project status and completed work
- `.agents/context/NOTES.md`: durable implementation decisions and gotchas
- `.agents/context/MAP.md`: this concise structure map
