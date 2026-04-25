# Plan: Unified Runtime Configuration for MARL

**Branch:** `feat/configurability`  
**Date:** 2026-04-25  
**Scope:** Move all non-array-size simulation parameters from compile-time `const` to a unified runtime config system (TOML file + CLI overrides). Grid dimensions, species counts, and array capacities remain compile-time. Everything else becomes configurable per-run without recompilation.

---

## 1. Goal

Eliminate hardcoded literals scattered across `config.rs`, `main.rs`, `cell.rs`, `field.rs`, `light.rs`, `snapshot.rs`, and `data.rs`. Replace them with a single `SimulationConfig` struct loaded from an optional TOML file and overridable via CLI flags. This enables parameter sweeps, reproducible scenario files, and faster calibration workflows.

**Non-goal:** Making array-size constants (grid dims, species counts, reaction slot counts) runtime-configurable. Those determine `Vec` allocations and stack arrays; keeping them `const` avoids dynamic allocation in hot loops and keeps the type system simple.

---

## 2. Current State

### What stays compile-time `const` (array-size determinants)
These must remain `const` because they size arrays and `Vec`s throughout the code:

| Constant | File | Reason |
|----------|------|--------|
| `GRID_X` | `config.rs` | Sizes `Field.data`, `LightField.data`, occupancy vectors |
| `GRID_Y` | `config.rs` | Same |
| `GRID_Z` | `config.rs` | Same |
| `S_EXT` | `config.rs` | Sizes species arrays in `Field`, neighbor env reads |
| `M_INT` | `config.rs` | Sizes `CellState.internal` |
| `R_MAX` | `config.rs` | Sizes `Ruleset.reactions` |
| `S_RECEPTORS` | `config.rs` | Sizes `Ruleset.receptors` |
| `S_TRANSPORTERS` | `config.rs` | Sizes `Ruleset.transport` |
| `S_EFFECTORS` | `config.rs` | Sizes `Ruleset.effectors` |

### What moves to runtime `SimulationConfig`
Everything else — physics, chemistry, cell parameters, mutation tuning, boundary fluxes, light model, seeding geometry, output tuning, and snapshot behavior.

### What moves to runtime `RunConfig` (already partially runtime)
Tick counts, intervals, seed count, output directory. These will be merged into the unified config.

---

## 3. Design Decisions

### 3.1 Config hierarchy (precedence, highest wins)
1. **Built-in defaults** — hardcoded in `SimulationConfig::default()` to guarantee the binary runs standalone.
2. **TOML file** — `marl.toml` in CWD, or path given by `--config <path>`.
3. **CLI flags** — override any field individually (e.g., `--source-rate-oxidant 0.8`).

### 3.2 Crate choice
Use the `toml` crate (add to `Cargo.toml`). It is the standard Rust TOML parser, zero-dependency beyond `toml` itself, and supports `serde` derive for trivial deserialization. `serde` is already pulled in transitively by `toml`.

### 3.3 Struct layout
Split into two top-level structs inside `config.rs`:

- `SimulationConfig` — all physics, chemistry, biology, and seeding parameters.
- `OutputConfig` — logging cadence, snapshot species selection, image toggles, output directory.

Both implement `Default` with values matching the *current* hardcoded literals exactly, preserving backward compatibility.

### 3.4 Passing config through the program
`SimulationConfig` and `OutputConfig` will be passed by reference (`&SimulationConfig`, `&OutputConfig`) into functions that currently read `const`s. `main()` owns the instances and passes `&cfg` and `&out` down the call chain. This is slightly more typing but makes dependencies explicit and avoids global mutable state.

**Exception:** `cell.rs` `CellState::tick()` and `Ruleset::mutate()` currently take no config parameter. They will receive `&SimulationConfig` as an additional argument. `main.rs` will pass the same reference on every tick.

---

## 4. Detailed Implementation Steps

### Phase 1: Add dependency and prepare `config.rs`

**Step 1.1** — Add `toml` and `serde` to `Cargo.toml`.
```toml
[dependencies]
rand = "0.9"
rand_distr = "0.5"
rayon = "1.10"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
```
Remove `half = "2"` (unused per `INFO.md`).

**Step 1.2** — In `config.rs`, keep the compile-time block at the top exactly as-is (lines 1–25). Everything below line 25 becomes runtime.

**Step 1.3** — Define `SimulationConfig` in `config.rs` with the following fields. All defaults must match current hardcoded values.

```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SimulationConfig {
    // Spatiotemporal
    pub dx: f32,                    // 100.0e-6
    pub dt: f32,                    // 1.0
    pub diffusion_substeps: usize,  // 10

    // Diffusion & decay (arrays, length S_EXT)
    pub d_voxel: [f32; S_EXT],      // current D_VOXEL literal
    pub lambda_decay: [f32; S_EXT], // current LAMBDA_DECAY literal

    // Boundary sources
    pub source_rate_oxidant: f32,   // 0.4
    pub source_rate_carbon: f32,    // 0.15
    pub source_rate_reductant: f32, // 0.5

    // Cell metabolism
    pub epsilon: f32,               // 0.001
    pub c_max: f32,                 // 10.0
    pub lambda_maintenance: f32,    // 0.12
    pub hard_death_floor: f32,      // 0.01
    pub reaction_maintenance: f32,  // 0.003

    // Cell cycle
    pub base_division_prep: f32,    // 20.0
    pub prep_maintenance_multiplier: f32, // 2.0
    pub rush_penalty_rate: f32,     // 0.05

    // Niche construction
    pub alpha_eps: f32,             // 0.8
    pub k_eps: f32,                 // 2.0

    // Light
    pub light_efficiency: f32,      // 0.0
    pub surface_intensity: f32,     // 1.0
    pub cell_absorption: f32,       // 0.3
    pub chemical_absorption: f32,   // 0.05
    pub light_floor: f32,           // 1e-7

    // Mutation
    pub mutation_stddev: f32,       // 0.1
    pub structural_mutation_rate_mult: f32, // 0.1
    pub meta_mutation_rate: f32,    // 0.01
    pub meta_mutation_clamp_low: f32,  // 0.001
    pub meta_mutation_clamp_high: f32, // 0.5
    pub hill_exponent_clamp_low: f32,  // 0.5
    pub hill_exponent_clamp_high: f32, // 8.0
    pub active_reaction_threshold: f32, // 1e-9

    // Seeding geometry (canonical 200-layer units)
    pub seed_margin: u16,           // 5
    pub phototroph_z_lo: f32,       // 0.0
    pub phototroph_z_hi: f32,       // 3.0
    pub chemolithotroph_z_lo: f32,  // 80.0
    pub chemolithotroph_z_hi: f32,  // 130.0
    pub anaerobe_z_lo: f32,         // 120.0
    pub anaerobe_z_hi: f32,         // 180.0

    // Division neighbor search
    pub division_neighbor_distance: u8, // 2

    // Field initialization boundary priming
    pub boundary_prime_layers: usize, // 2
    pub boundary_prime_oxidant: f32,  // 0.5
    pub boundary_prime_carbon: f32,   // 0.3
    pub boundary_prime_reductant: f32,// 0.5
}
```

Implement `Default` for `SimulationConfig` using the current literal values.

**Step 1.4** — Define `OutputConfig` in `config.rs`:

```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct OutputConfig {
    pub max_ticks: u32,             // 5000
    pub stats_interval: u32,        // 100
    pub snapshot_interval: u32,     // 500
    pub image_interval: u32,        // 500
    pub seed_count: usize,          // 30
    pub output_dir: String,         // format!("output/run_{}x{}x{}", GRID_X, GRID_Y, GRID_Z)

    // Snapshot species indices for XZ cross-sections
    pub xz_snapshot_species: Vec<usize>, // vec![1, 2, 3, 4]

    // XY slice depths (as fractions, 0.0..1.0, resolved at runtime)
    pub xy_slice_depths_frac: Vec<f32>, // vec![0.0, 0.25, 0.5, 0.75]

    // Toggle image types
    pub write_ancestry_map: bool,   // true
    pub write_density_map: bool,    // true
}
```

Implement `Default` for `OutputConfig`.

**Step 1.5** — Define a unified `Config` struct:

```rust
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct Config {
    #[serde(default)]
    pub simulation: SimulationConfig,
    #[serde(default)]
    pub output: OutputConfig,
}
```

**Step 1.6** — Implement `Config::load()` with the hierarchy:

```rust
impl Config {
    pub fn load() -> Self {
        let mut cfg = Self::default();

        // 2. TOML file override
        let args: Vec<String> = std::env::args().collect();
        let mut config_path: Option<String> = None;
        let mut i = 1;
        while i < args.len() {
            if args[i] == "--config" && i + 1 < args.len() {
                config_path = Some(args[i + 1].clone());
                i += 2;
            } else {
                i += 1;
            }
        }
        let toml_path = config_path.unwrap_or_else(|| "marl.toml".to_string());
        if let Ok(content) = std::fs::read_to_string(&toml_path) {
            if let Ok(parsed) = toml::from_str::<Config>(&content) {
                cfg = parsed;
            }
        }

        // 3. CLI override (individual flags)
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--config" => i += 2,
                "--ticks" if i + 1 < args.len() => {
                    if let Ok(v) = args[i+1].parse() { cfg.output.max_ticks = v; }
                    i += 2;
                }
                "--stats" if i + 1 < args.len() => {
                    if let Ok(v) = args[i+1].parse() { cfg.output.stats_interval = v; }
                    i += 2;
                }
                "--snapshot" if i + 1 < args.len() => {
                    if let Ok(v) = args[i+1].parse() { cfg.output.snapshot_interval = v; }
                    i += 2;
                }
                "--images" if i + 1 < args.len() => {
                    if let Ok(v) = args[i+1].parse() { cfg.output.image_interval = v; }
                    i += 2;
                }
                "--seed" if i + 1 < args.len() => {
                    if let Ok(v) = args[i+1].parse() { cfg.output.seed_count = v; }
                    i += 2;
                }
                "--output" if i + 1 < args.len() => {
                    cfg.output.output_dir = args[i+1].clone();
                    i += 2;
                }
                // Add per-parameter CLI overrides as needed, or defer to TOML-only for physics params.
                _ => { i += 1; }
            }
        }

        cfg
    }
}
```

**Decision:** For the first iteration, keep per-physics-parameter CLI flags minimal (or absent). The primary interface for physics tuning is the TOML file. CLI is for run-control (`--ticks`, `--output`, `--config`). This keeps the CLI parser simple. We can add `--set key=value` later if needed.

---

### Phase 2: Wire `SimulationConfig` through `main.rs`

**Step 2.1** — Replace `RunConfig::from_args()` with `Config::load()` in `main()`.

**Step 2.2** — Change `init_field_boundaries` signature to:
```rust
fn init_field_boundaries(field: &mut Field, sim: &SimulationConfig)
```
Use `sim.boundary_prime_layers`, `sim.boundary_prime_oxidant`, etc. instead of hardcoded `2`, `0.5`, `0.3`, `0.5`.

**Step 2.3** — Change `seed_cells` signature to:
```rust
fn seed_cells(
    cells: &mut Vec<CellState>,
    cell_map: &mut HashMap<[u16; 3], usize>,
    rng: &mut impl Rng,
    count: usize,
    z_lo: u16,
    z_hi: u16,
    factory: fn([u16; 3], u64) -> CellState,
    sim: &SimulationConfig,
)
```
Use `sim.seed_margin` instead of hardcoded `5` in the random range.

**Step 2.4** — In `main()`, compute seeding depth bands using `sim.phototroph_z_lo/hi`, `sim.chemolithotroph_z_lo/hi`, `sim.anaerobe_z_lo/hi` instead of the literal `80.0`, `130.0`, etc.

**Step 2.5** — Pass `&cfg.simulation` into `cell.tick()` and `ruleset.mutate()` calls (see Phase 3).

**Step 2.6** — Pass `&cfg.output` (or `&cfg.simulation` where needed) into `snapshot::write_all_snapshots` and `DataLogger` calls.

**Step 2.7** — Update the `println!` headers in `main()` to read grid dimensions from `const`s (still valid) but run parameters from `cfg.output`.

---

### Phase 3: Wire `SimulationConfig` through `cell.rs`

**Step 3.1** — Change `CellState::tick` signature:
```rust
pub fn tick(
    &mut self,
    ext_conc: &[f32; S_EXT],
    light: f32,
    sim: &SimulationConfig,
) -> ([f32; S_EXT], CellEvent)
```

**Step 3.2** — Replace all direct `const` references inside `tick()` with `sim.*`:
- `DT` → `sim.dt`
- `EPSILON` → `sim.epsilon`
- `C_MAX` → `sim.c_max`
- `LAMBDA_MAINTENANCE` → `sim.lambda_maintenance`
- `BASE_DIVISION_PREP` → `sim.base_division_prep`
- `PREP_MAINTENANCE_MULTIPLIER` → `sim.prep_maintenance_multiplier`
- `RUSH_PENALTY_RATE` → `sim.rush_penalty_rate`
- `HARD_DEATH_FLOOR` → `sim.hard_death_floor`
- `REACTION_MAINTENANCE` → `sim.reaction_maintenance`
- Active threshold `1e-9` → `sim.active_reaction_threshold`

**Step 3.3** — Change `Ruleset::mutate` signature:
```rust
pub fn mutate(&mut self, rng: &mut impl Rng, sim: &SimulationConfig)
```

**Step 3.4** — Replace mutation hardcodes with `sim.*`:
- `Normal::new(0.0f32, 0.1)` → `Normal::new(0.0f32, sim.mutation_stddev)`
- Structural mutation multiplier `0.1` → `sim.structural_mutation_rate_mult`
- Meta-mutation fixed rate `0.01` → `sim.meta_mutation_rate`
- Meta clamp `(0.001, 0.5)` → `(sim.meta_mutation_clamp_low, sim.meta_mutation_clamp_high)`
- Hill clamp `(0.5, 8.0)` → `(sim.hill_exponent_clamp_low, sim.hill_exponent_clamp_high)`
- Threshold `1e-9` → `sim.active_reaction_threshold`

**Step 3.5** — Update `main.rs` call sites: `cell.tick(&ext, l, &cfg.simulation)` and `daughter.ruleset.mutate(&mut rng, &cfg.simulation)`.

---

### Phase 4: Wire `SimulationConfig` through `field.rs`

**Step 4.1** — Change `Field::apply_boundary_sources` signature:
```rust
pub fn apply_boundary_sources(&mut self, sim: &SimulationConfig)
```

**Step 4.2** — Replace `SOURCE_RATE_OXIDANT`, `SOURCE_RATE_CARBON`, `SOURCE_RATE_REDUCTANT`, `C_MAX` with `sim.*`.

**Step 4.3** — Change `diffusion_step_inner` signature:
```rust
fn diffusion_step_inner(&mut self, dt_sub: f32, occupancy: Option<&[bool]>, sim: &SimulationConfig)
```

**Step 4.4** — Replace `LAMBDA_DECAY[s]` → `sim.lambda_decay[s]`, `D_VOXEL[s]` → `sim.d_voxel[s]`, `ALPHA_EPS` → `sim.alpha_eps`, `K_EPS` → `sim.k_eps`.

**Step 4.5** — Change `diffuse_tick_with_cells` signature:
```rust
pub fn diffuse_tick_with_cells(&mut self, occupancy: &[bool], sim: &SimulationConfig)
```

**Step 4.6** — Update `dt_sub` computation to use `sim.dt / sim.diffusion_substeps as f32`.

**Step 4.7** — Update `diffuse_tick` (the `#[allow(dead_code)]` test version) to take `sim: &SimulationConfig` as well.

**Step 4.8** — Update `main.rs` call sites: `field.apply_boundary_sources(&cfg.simulation)` and `field.diffuse_tick_with_cells(&occupancy, &cfg.simulation)`.

---

### Phase 5: Wire `SimulationConfig` through `light.rs`

**Step 5.1** — Change `LightField::update` signature:
```rust
pub fn update(&mut self, field: &Field, cells: &HashMap<[u16; 3], usize>, sim: &SimulationConfig)
```

**Step 5.2** — Replace hardcoded `surface_intensity = 1.0`, `cell_absorption = 0.3`, `chemical_absorption = 0.05`, `intensity < 1e-7` with `sim.surface_intensity`, `sim.cell_absorption`, `sim.chemical_absorption`, `sim.light_floor`.

**Step 5.3** — Update `main.rs` call site.

---

### Phase 6: Wire `OutputConfig` through `snapshot.rs` and `data.rs`

**Step 6.1** — Change `snapshot::write_all_snapshots` signature:
```rust
pub fn write_all_snapshots(
    field: &Field,
    light: &LightField,
    cells: &HashMap<[u16; 3], usize>,
    cell_vec: &[crate::cell::CellState],
    tick: u64,
    out: &OutputConfig,
    sim: &SimulationConfig,
) -> std::io::Result<()>
```

**Step 6.2** — Replace hardcoded species list `[1, 2, 3, 4]` with `out.xz_snapshot_species`. Replace hardcoded `z_depths` computation with `out.xy_slice_depths_frac` mapped through `GRID_Z`.

**Step 6.3** — Gate `write_ancestry_xz` behind `out.write_ancestry_map`.
Gate `write_cell_density_xz` behind `out.write_density_map`.

**Step 6.4** — `DataLogger::new` currently takes `output_dir: &str`. Keep that; `OutputConfig` will pass `output_dir` when constructing it. No signature change needed for `DataLogger` methods, but `write_summary` and `snapshot_chemistry` currently reference `GRID_*` constants directly (fine) and some diagnostic thresholds indirectly. Leave `data.rs` mostly untouched except for removing any stray physics `const` references if they exist.

**Step 6.5** — Update `main.rs` call site for `write_all_snapshots`.

---

### Phase 7: Remove dead `const`s and verify

**Step 7.1** — In `config.rs`, delete every `const` that has been moved to `SimulationConfig` or `OutputConfig` (everything below line 25 except the compile-time block). Keep only:
```rust
pub const GRID_X: usize = 128;
pub const GRID_Y: usize = 128;
pub const GRID_Z: usize = 64;
pub const S_EXT: usize = 12;
pub const M_INT: usize = 16;
pub const R_MAX: usize = 16;
pub const S_RECEPTORS: usize = 8;
pub const S_TRANSPORTERS: usize = 8;
pub const S_EFFECTORS: usize = 8;
```

**Step 7.2** — Remove `CELL_DIFFUSION_FACTOR` if it is truly dead (it is `#[allow(dead_code)]` and not referenced anywhere).

**Step 7.3** — Remove `LIGHT_EFFICIENCY` if it is not referenced anywhere after the refactor (check `cell.rs` — it is not currently used).

**Step 7.4** — Run `cargo check`. Fix any remaining "cannot find value" errors by either wiring the parameter or, if it was missed, adding it to `SimulationConfig`.

**Step 7.5** — Run `cargo build --release`.

**Step 7.6** — Run the sim with defaults to verify zero behavioral change:
```bash
cargo run --release -- --ticks 100 --stats 10
```
Confirm it produces the same stdout pattern as before (population counts, chemistry values).

---

### Phase 8: Add a sample TOML and documentation

**Step 8.1** — Create `marl.toml` at repo root with all fields set to their default values. Include comments mapping each field to its previous `const` name for discoverability.

```toml
[simulation]
dx = 0.0001                     # 100 um voxel size
dt = 1.0                        # 1 tick = 1 day
diffusion_substeps = 10

# Diffusion coefficients per external species (voxels^2/tick)
d_voxel = [0.0, 1.5, 1.0, 1.2, 0.8, 0.5, 0.5, 0.1, 0.3, 0.3, 0.3, 0.3]

# Decay rates per species (fraction per tick)
lambda_decay = [0.0, 0.01, 0.01, 0.005, 0.03, 0.05, 0.05, 0.002, 0.01, 0.01, 0.01, 0.01]

source_rate_oxidant = 0.4
source_rate_carbon = 0.15
source_rate_reductant = 0.5

epsilon = 0.001
c_max = 10.0
lambda_maintenance = 0.12
hard_death_floor = 0.01
reaction_maintenance = 0.003

base_division_prep = 20.0
prep_maintenance_multiplier = 2.0
rush_penalty_rate = 0.05

alpha_eps = 0.8
k_eps = 2.0

light_efficiency = 0.0
surface_intensity = 1.0
cell_absorption = 0.3
chemical_absorption = 0.05
light_floor = 0.0000001

mutation_stddev = 0.1
structural_mutation_rate_mult = 0.1
meta_mutation_rate = 0.01
meta_mutation_clamp_low = 0.001
meta_mutation_clamp_high = 0.5
hill_exponent_clamp_low = 0.5
hill_exponent_clamp_high = 8.0
active_reaction_threshold = 0.000000001

seed_margin = 5
phototroph_z_lo = 0.0
phototroph_z_hi = 3.0
chemolithotroph_z_lo = 80.0
chemolithotroph_z_hi = 130.0
anaerobe_z_lo = 120.0
anaerobe_z_hi = 180.0

division_neighbor_distance = 2

boundary_prime_layers = 2
boundary_prime_oxidant = 0.5
boundary_prime_carbon = 0.3
boundary_prime_reductant = 0.5

[output]
max_ticks = 5000
stats_interval = 100
snapshot_interval = 500
image_interval = 500
seed_count = 30
output_dir = "output/run_128x128x64"

xz_snapshot_species = [1, 2, 3, 4]
xy_slice_depths_frac = [0.0, 0.25, 0.5, 0.75]
write_ancestry_map = true
write_density_map = true
```

**Step 8.2** — Add `marl.toml` to `.gitignore` if we want to avoid committing user-local config. **Decision:** Do NOT add to `.gitignore`. The sample `marl.toml` is documentation and a reproducible default. Users can copy it and modify. (Add a comment in the file: "Copy this file and pass `--config myrun.toml` to use your own settings.")

**Step 8.3** — Update `README.md`:
- Change the "Grid dimensions are compile-time constants" line to note that physics parameters are now runtime-configurable via TOML.
- Add a brief section showing:
  ```bash
  cargo run --release -- --config marl.toml
  ```
- List the supported CLI flags.

**Step 8.4** — Update `.agents/context/STATUS.md` with current progress.

**Step 8.5** — Update `.agents/context/NOTES.md` with any gotchas discovered during implementation.

---

## 5. Verification Checklist

- [ ] `cargo check` passes with zero errors and zero warnings.
- [ ] `cargo build --release` succeeds.
- [ ] `cargo run --release -- --ticks 100 --stats 10` produces the same stdout as before the refactor (to within RNG variance; use `--seed` if deterministic testing is available).
- [ ] `cargo run --release -- --config marl.toml --ticks 100` runs successfully.
- [ ] Deleting `marl.toml` and running with CLI flags only still works (falls back to defaults).
- [ ] Modifying `source_rate_oxidant` in `marl.toml` produces visibly different chemistry in the first 100 ticks (e.g., higher oxidant at surface).
- [ ] No `const` physics parameters remain in `config.rs` below the compile-time block.
- [ ] `half` dependency removed from `Cargo.toml`.

---

## 6. Risks & Mitigations

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Missing a `const` reference in a file not covered above | Medium | `cargo check` will catch every unresolved identifier. Fix iteratively. |
| Performance regression from passing `&SimulationConfig` by reference | Low | The struct is ~200 bytes; passing a pointer has zero overhead vs. reading a static. The hot loop (diffusion) still uses inline indexing into `Vec<f32>`. |
| `serde` + `toml` bloat compile time | Low | Both are standard, widely cached deps. `toml` is already in most Rust toolchains. |
| Default values drift from current behavior | Medium | Copy-paste every literal exactly. Verify with a short run comparing stdout before/after. |
| Array-size `const`s accidentally made runtime | Low | Only the eight listed constants remain. Everything else is a struct field. Review diff before commit. |

---

## 7. Open Questions (for the implementing agent to resolve)

1. **Should `OutputConfig` also control which CSV columns are written?** For now, no — keep `ticks.csv` and snapshot formats unchanged to avoid breaking downstream analysis scripts.

2. **Should we add a `--set key=value` generic CLI override?** Defer to a follow-up. The TOML file is the primary physics interface.

3. **Should starter metabolism factory functions (`make_phototroph`, etc.) also be parameterized?** Yes, but partially. The factory functions contain many literal kinetic parameters (`k_m`, `v_max`, `uptake_rate`, etc.). For this plan, **do not** parameterize every reaction coefficient — that balloons scope. Instead, parameterize the high-level tuning knobs (seed counts, depth bands, margin) and leave the detailed biochemistry of the three starter metabolisms as hardcoded factory defaults. A future plan can introduce "scenario definitions" for custom starter metabolisms.

---

## 8. Files to Touch

| File | Change Type | Notes |
|------|-------------|-------|
| `Cargo.toml` | edit | Add `serde`, `toml`; remove `half` |
| `src/config.rs` | heavy rewrite | Keep compile-time block; replace rest with structs, `Default`, `serde::Deserialize`, `Config::load()` |
| `src/main.rs` | moderate edit | Wire `Config` through initialization, seeding, tick loop, output calls |
| `src/cell.rs` | moderate edit | Add `sim: &SimulationConfig` to `tick()` and `mutate()`; replace const refs |
| `src/field.rs` | moderate edit | Add `sim: &SimulationConfig` to diffusion and boundary methods; replace const refs |
| `src/light.rs` | light edit | Add `sim: &SimulationConfig` to `update()`; replace const refs |
| `src/snapshot.rs` | light edit | Add `out: &OutputConfig`, `sim: &SimulationConfig` to `write_all_snapshots`; replace hardcoded lists |
| `src/data.rs` | none / minimal | No physics consts here; may need `OutputConfig` if snapshot toggles affect it |
| `marl.toml` | create | Sample config with all defaults |
| `README.md` | edit | Document TOML config and CLI flags |
| `.agents/context/STATUS.md` | create/update | Record completion |
| `.agents/context/NOTES.md` | create/update | Record gotchas |
