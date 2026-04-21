// ============================================================================
// GRID DIMENSIONS — compile-time constants
// ============================================================================
// These must be const because they determine array sizes throughout the code.
// To run at a different grid size, change these values and recompile:
//   cargo build --release    (~2 seconds incremental)
//
// Suggested sizes:
//   64x64x32   — quick debug runs (~7 ticks/sec)
//   128x128x64 — calibration runs (~1 tick/sec est.)
//   256x256x128 — production runs (needs rayon, ~0.1 tick/sec est.)
pub const GRID_X: usize = 128;
pub const GRID_Y: usize = 128;
pub const GRID_Z: usize = 64;

// Species counts
pub const S_EXT: usize = 12;  // external chemical species
pub const M_INT: usize = 16;  // internal chemical species

// Reaction network
pub const R_MAX: usize = 16;  // max reactions per cell
pub const S_RECEPTORS: usize = 8;
pub const S_TRANSPORTERS: usize = 8;
pub const S_EFFECTORS: usize = 8;

// Physics
pub const DX: f32 = 100.0e-6; // 100 um voxel size
pub const DT: f32 = 1.0;      // 1 tick = 1 day

// Decay rates per species (per tick, fraction)
pub const LAMBDA_DECAY: [f32; S_EXT] = [
    0.0,   // 0: light-energy-carrier (not in field)
    0.01,  // 1: oxidant
    0.01,  // 2: reductant
    0.005, // 3: carbon-source
    0.03,  // 4: organic-waste (transient)
    0.05,  // 5: signal-A
    0.05,  // 6: signal-B
    0.002, // 7: structural-deposit
    0.01, 0.01, 0.01, 0.01,
];

// Cell parameters
pub const EPSILON: f32 = 0.001;
pub const C_MAX: f32 = 10.0;
pub const LAMBDA_MAINTENANCE: f32 = 0.12; // 12% energy drain per tick

// Hard thermodynamic death floor — below this energy level, the cell
// physically disintegrates regardless of its evolved death_energy threshold.
// This models the minimum ATP required to maintain membrane integrity,
// protein folding, and basic cellular machinery. No amount of evolution
// can keep a cell alive without this minimum — it's physics, not fitness.
// Prevents "zombie cells" that evolve death_energy to near-zero and
// persist indefinitely at negligible energy.
pub const HARD_DEATH_FLOOR: f32 = 0.01;

// Per-reaction protein expression cost — each active enzyme drains this
// much energy per tick. Cells with bloated genomes (many active reactions)
// pay more, selecting against junk reactions. A cell with 5 reactions pays
// 5 * 0.003 = 0.015/tick; one with 12 pays 0.036/tick.
pub const REACTION_MAINTENANCE: f32 = 0.003;

// Cell division cycle — cells can't divide instantly. After reaching
// division_energy, they enter a preparation phase (DNA replication,
// organelle duplication, membrane synthesis) that takes time and costs
// extra energy. This prevents the "conveyor belt" problem where surface
// cells endlessly spawn daughters into the interior.
//
// BASE_DIVISION_PREP: default prep time in ticks. Cells can evolve this
// shorter, but pay a rush penalty.
// PREP_MAINTENANCE_MULTIPLIER: maintenance cost multiplier during prep.
// RUSH_PENALTY_RATE: additional maintenance multiplier per tick below baseline.
//   e.g., evolving prep from 20 to 10 ticks adds 10 * 0.05 = 0.5 to the
//   multiplier, making prep cost 2.5x instead of 2.0x.
pub const BASE_DIVISION_PREP: f32 = 20.0;
pub const PREP_MAINTENANCE_MULTIPLIER: f32 = 2.0;
pub const RUSH_PENALTY_RATE: f32 = 0.05;

// Light: no free energy. All energy comes from substrate-consuming reactions.
pub const LIGHT_EFFICIENCY: f32 = 0.0;

// Boundary source rates — the sole external inputs.
// Top face = ocean surface (dissolved O2 + CO2 from atmosphere, strong).
// Bottom face = hydrothermal vent field (H2S/reductant, very strong).
// The middle of the column gets nothing — nutrients must diffuse from edges.
pub const SOURCE_RATE_OXIDANT: f32 = 0.4;
pub const SOURCE_RATE_CARBON: f32 = 0.15;
pub const SOURCE_RATE_REDUCTANT: f32 = 0.5;  // vents are chemically intense

// Niche construction
pub const ALPHA_EPS: f32 = 0.8;
pub const K_EPS: f32 = 2.0;

// NOTE: No longer used by diffusion — occupied voxels are now fully excluded
// from the diffusion solver. Chemicals only exist in empty (extracellular)
// space. Kept for potential future use (e.g., partial permeability).
#[allow(dead_code)]
pub const CELL_DIFFUSION_FACTOR: f32 = 0.1;

// Diffusion sub-stepping
pub const DIFFUSION_SUBSTEPS: usize = 10;

// D in voxels^2/tick (before sub-stepping). CFL: D*dt_sub < 1/6
pub const D_VOXEL: [f32; S_EXT] = [
    0.0,   // 0: light (no diffusion)
    1.5,   // 1: oxidant (fast)
    1.0,   // 2: reductant
    1.2,   // 3: carbon
    0.8,   // 4: organic waste
    0.5,   // 5: signal-A
    0.5,   // 6: signal-B
    0.1,   // 7: structural (EPS)
    0.3, 0.3, 0.3, 0.3,
];

// ============================================================================
// RUNTIME CONFIGURATION — parsed from CLI args, controls run behavior
// ============================================================================
// These parameters don't affect array sizes, so they can vary per-run without
// recompilation. Parse from command-line args in main().

/// Runtime configuration for a single simulation run.
/// Separates "how long / how often to log" from the physics constants above.
pub struct RunConfig {
    /// Total number of simulation ticks to execute
    pub max_ticks: u32,
    /// How often to print stats to stdout (every N ticks)
    pub stats_interval: u32,
    /// How often to write chemistry profiles and cell snapshots (every N ticks)
    pub snapshot_interval: u32,
    /// How often to write PPM image snapshots (every N ticks)
    pub image_interval: u32,
    /// How many cells of each starter metabolism to seed
    pub seed_count: usize,
    /// Output directory for this run's data files
    pub output_dir: String,
}

impl RunConfig {
    /// Sensible defaults for a calibration run at the current grid size.
    pub fn default() -> Self {
        Self {
            max_ticks: 5000,
            stats_interval: 100,
            snapshot_interval: 500,
            image_interval: 500,
            seed_count: 30,
            output_dir: format!("output/run_{}x{}x{}", GRID_X, GRID_Y, GRID_Z),
        }
    }

    /// Parse from command-line args. Unrecognized args are ignored.
    /// Format: `--ticks 10000 --snapshot 200 --images 500 --seed 50 --output path`
    pub fn from_args() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let mut cfg = Self::default();

        let mut i = 1; // skip binary name
        while i < args.len() {
            match args[i].as_str() {
                "--ticks" if i + 1 < args.len() => {
                    if let Ok(v) = args[i + 1].parse() { cfg.max_ticks = v; }
                    i += 2;
                }
                "--stats" if i + 1 < args.len() => {
                    if let Ok(v) = args[i + 1].parse() { cfg.stats_interval = v; }
                    i += 2;
                }
                "--snapshot" if i + 1 < args.len() => {
                    if let Ok(v) = args[i + 1].parse() { cfg.snapshot_interval = v; }
                    i += 2;
                }
                "--images" if i + 1 < args.len() => {
                    if let Ok(v) = args[i + 1].parse() { cfg.image_interval = v; }
                    i += 2;
                }
                "--seed" if i + 1 < args.len() => {
                    if let Ok(v) = args[i + 1].parse() { cfg.seed_count = v; }
                    i += 2;
                }
                "--output" if i + 1 < args.len() => {
                    cfg.output_dir = args[i + 1].clone();
                    i += 2;
                }
                _ => { i += 1; }
            }
        }
        cfg
    }
}
