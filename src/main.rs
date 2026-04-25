use marl::config::*;
use marl::field::Field;
use marl::light::LightField;
use marl::cell::*;
use marl::data::DataLogger;
use marl::snapshot;

use std::collections::HashMap;
use rand::Rng;
use std::time::Instant;

fn main() {
    // Parse runtime config from CLI args and optional TOML file.
    // Grid dimensions are compile-time — change in config.rs and recompile.
    let cfg = Config::load();
    let mut rng = rand::rng();

    let mut field = Field::new();
    let mut light = LightField::new();

    // Create the data logger — opens ticks.csv and manages all file output.
    let mut logger = DataLogger::new(&cfg.output.output_dir)
        .expect("Failed to create data logger / output directory");

    // Start with empty field — let boundary sources build gradients organically.
    // Pre-load only a thin boundary layer so initial cells can bootstrap.
    init_field_boundaries(&mut field, &cfg.simulation);

    // Cell storage: Vec for contiguous iteration + HashMap for O(1) spatial lookup.
    // The map stores position -> index into the Vec.
    let mut cells: Vec<CellState> = Vec::new();
    let mut cell_map: HashMap<[u16; 3], usize> = HashMap::new();

    // Seed three metabolisms — small populations at appropriate depths.
    // z_scale maps the "canonical" 200-layer depth to our actual grid depth,
    // so metabolisms land at the right relative positions regardless of GRID_Z.
    let z_scale = GRID_Z as f32 / 200.0;
    let sim = &cfg.simulation;

    // Phototrophs: surface
    let photo_lo = (sim.phototroph_z_lo * z_scale) as u16;
    let photo_hi = (sim.phototroph_z_hi * z_scale).max(photo_lo as f32 + 1.0) as u16;
    seed_cells(&mut cells, &mut cell_map, &mut rng, cfg.output.seed_count,
        photo_lo, photo_hi, make_phototroph, sim);

    // Chemolithotrophs: chemocline — oxidize reductant using oxidant at the interface
    let chemo_lo = (sim.chemolithotroph_z_lo * z_scale) as u16;
    let chemo_hi = (sim.chemolithotroph_z_hi * z_scale).max(chemo_lo as f32 + 3.0) as u16;
    seed_cells(&mut cells, &mut cell_map, &mut rng, cfg.output.seed_count,
        chemo_lo, chemo_hi, make_chemolithotroph, sim);

    // Anaerobes: deep zone — use reductant, killed by oxidant
    let ana_lo = (sim.anaerobe_z_lo * z_scale) as u16;
    let ana_hi = (sim.anaerobe_z_hi * z_scale).max(ana_lo as f32 + 3.0) as u16;
    seed_cells(&mut cells, &mut cell_map, &mut rng, cfg.output.seed_count,
        ana_lo, ana_hi, make_anaerobe, sim);

    println!("MARL v0.3 — CPU Prototype (Winogradsky)");
    println!("Grid: {}x{}x{} ({:.1}M voxels), Species: {} ext / {} int",
        GRID_X, GRID_Y, GRID_Z,
        (GRID_X * GRID_Y * GRID_Z) as f64 / 1e6,
        S_EXT, M_INT);
    println!("Seeded {} cells (photo/chemo/anaerobe)", cells.len());
    println!("Output: {}", cfg.output.output_dir);
    println!("Plan: {} ticks, stats every {}, snapshots every {}, images every {}",
        cfg.output.max_ticks, cfg.output.stats_interval, cfg.output.snapshot_interval, cfg.output.image_interval);
    println!("---");

    let mut total_divisions: u64 = 0;
    let mut total_deaths: u64 = 0;
    let start = Instant::now();

    // Track per-tick division/death counts for the data logger
    let mut tick_divisions: u64;
    let mut tick_deaths: u64;

    for tick in 0..cfg.output.max_ticks {
        tick_divisions = 0;
        tick_deaths = 0;

        // === STEP 1: Boundary sources ===
        // Inject oxidant + carbon at top, reductant at bottom — the only
        // external energy inputs. Everything else is recycled by cells.
        field.apply_boundary_sources(sim);

        // === STEP 2: Diffusion ===
        // Sub-stepped forward Euler on 3D Laplacian. CFL-stable because
        // D * dt_sub < 1/6 for all species (see config.rs).
        // Build occupancy grid so the diffusion solver knows where cells are.
        // Occupied voxels are fully excluded from diffusion.
        let mut occupancy = vec![false; GRID_X * GRID_Y * GRID_Z];
        for pos in cell_map.keys() {
            let idx = pos[2] as usize * GRID_Y * GRID_X
                    + pos[1] as usize * GRID_X
                    + pos[0] as usize;
            occupancy[idx] = true;
        }
        field.diffuse_tick_with_cells(&occupancy, sim);

        // === STEP 3: Light attenuation ===
        // Beer-Lambert top-down sweep. Light enters at z=0, attenuated by
        // cells and chemical absorbers. Stored per-voxel so photosynthesis
        // reactions can reference it as a catalyst.
        light.update(&field, &cell_map, sim);

        // === STEP 4: Cell update pass ===
        // Each cell runs the 5-phase tick (receptor, transport, reactions,
        // effector, fate) and returns field deltas + a fate event.
        let mut events: Vec<(usize, CellEvent)> = Vec::with_capacity(cells.len());

        for (i, cell) in cells.iter_mut().enumerate() {
            let p = cell.pos;
            // Cells sense the extracellular medium via empty neighbors,
            // not their own voxel (which is excluded from diffusion).
            let ext = read_neighbor_environment(p, &field, &cell_map);
            let l = light.get(p[0] as usize, p[1] as usize, p[2] as usize);

            let (deltas, event) = cell.tick(&ext, l, sim);
            // Secretion/consumption distributed to neighboring empty voxels
            apply_deltas_to_neighbors(p, &mut field, &cell_map, &deltas);
            events.push((i, event));
        }

        // === STEP 5: Process fate events ===
        let mut births: Vec<CellState> = Vec::new();
        let mut deaths: Vec<usize> = Vec::new();

        for (i, event) in &events {
            match event {
                CellEvent::Division => {
                    let parent = &cells[*i];
                    if let Some(daughter_pos) = find_empty_neighbor(parent.pos, &cell_map, &mut rng, sim) {
                        let mut daughter = parent.clone();
                        daughter.pos = daughter_pos;
                        daughter.age = 0;
                        daughter.prep_remaining = 0; // daughter starts fresh, not in prep
                        daughter.lineage_id = rng.random::<u64>();
                        // CRITICAL: split ALL 16 internal species, not just energy.
                        // This prevents division from being a free-energy exploit.
                        for k in 0..M_INT {
                            daughter.internal[k] *= 0.5;
                            cells[*i].internal[k] *= 0.5;
                        }
                        daughter.ruleset.mutate(&mut rng, sim);
                        births.push(daughter);
                        tick_divisions += 1;
                    }
                }
                CellEvent::Death => {
                    deaths.push(*i);
                    tick_deaths += 1;
                }
                _ => {}
            }
        }

        total_divisions += tick_divisions;
        total_deaths += tick_deaths;

        // Remove dead cells (reverse order to preserve indices during swap_remove)
        deaths.sort_unstable();
        deaths.dedup();
        for &i in deaths.iter().rev() {
            let pos = cells[i].pos;
            cell_map.remove(&pos);
            cells.swap_remove(i);
            if i < cells.len() {
                cell_map.insert(cells[i].pos, i);
            }
        }

        // Add newborns
        for cell in births {
            let pos = cell.pos;
            if !cell_map.contains_key(&pos) {
                let idx = cells.len();
                cell_map.insert(pos, idx);
                cells.push(cell);
            }
        }

        // === STEP 6: Data logging and periodic output ===

        // Log every tick to ticks.csv (lightweight — just one CSV row)
        if let Err(e) = logger.log_tick(tick as u64, &cells, tick_divisions, tick_deaths) {
            eprintln!("Warning: failed to log tick {}: {}", tick, e);
        }

        // Print human-readable stats to stdout at configured interval
        if tick % cfg.output.stats_interval == 0 || tick == cfg.output.max_ticks - 1 {
            print_stats(tick, &cells, &field, &light, total_divisions, total_deaths, &start);
        }

        // Write detailed CSV snapshots (chemistry profiles + cell dumps + reactions)
        if tick % cfg.output.snapshot_interval == 0 || tick == cfg.output.max_ticks - 1 {
            let t = tick as u64;
            if let Err(e) = logger.snapshot_chemistry(t, &field, &light) {
                eprintln!("Warning: failed to write chemistry snapshot at tick {}: {}", tick, e);
            }
            if let Err(e) = logger.snapshot_cells(t, &cells) {
                eprintln!("Warning: failed to write cell snapshot at tick {}: {}", tick, e);
            }
            if let Err(e) = logger.snapshot_reactions(t, &cells) {
                eprintln!("Warning: failed to write reaction snapshot at tick {}: {}", tick, e);
            }
        }

        // Write PPM image snapshots (cross-sections, density maps)
        if tick % cfg.output.image_interval == 0 || tick == cfg.output.max_ticks - 1 {
            if let Err(e) = snapshot::write_all_snapshots(
                &field, &light, &cell_map, &cells, tick as u64, &cfg.output, sim,
            ) {
                eprintln!("Warning: failed to write image snapshots at tick {}: {}", tick, e);
            }
        }
    }

    println!("\n=== FINAL Z-LAYER PROFILE ===");
    print_z_profile(&cells, &field, &light);

    let runtime = start.elapsed().as_secs_f32();
    println!("\nDone. {} ticks in {:.1}s, final pop={}, div={}, death={}",
        cfg.output.max_ticks, runtime, cells.len(), total_divisions, total_deaths);

    // Write the post-run summary (lab notebook entry for this run)
    if let Err(e) = logger.write_summary(
        cfg.output.max_ticks, runtime, &cells, &field, &light,
        total_divisions, total_deaths, sim,
    ) {
        eprintln!("Warning: failed to write summary: {}", e);
    } else {
        println!("Summary written to {}/summary.md", cfg.output.output_dir);
    }

    // Write the reaction registry — maps IDs back to topologies for the CLI tool
    if let Err(e) = logger.write_registry() {
        eprintln!("Warning: failed to write reaction registry: {}", e);
    } else {
        println!("Reaction registry: {} unique topologies observed", logger.registry.count());
    }

    // Write ancestry-colored XZ cross-section (red=photo, green=chemo, blue=anaerobe)
    if cfg.output.write_ancestry_map {
        if let Err(e) = snapshot::write_ancestry_xz(&cells, &cell_map, cfg.output.max_ticks as u64, &cfg.output.output_dir) {
            eprintln!("Warning: failed to write ancestry map: {}", e);
        }
    }
}

// === FIELD INITIALIZATION ===

/// Initialize field with thin boundary layers only.
/// The bulk of the field starts empty — gradients build from boundary sources + diffusion.
fn init_field_boundaries(field: &mut Field, sim: &SimulationConfig) {
    // Prime only the boundary faces (configurable layers deep) so initial cells
    // have a local substrate source but the bulk field is empty.
    let layers = sim.boundary_prime_layers;
    for y in 0..GRID_Y {
        for x in 0..GRID_X {
            // Top layers: some oxidant and carbon (atmosphere analog)
            for z in 0..layers {
                field.set(x, y, z, 1, sim.boundary_prime_oxidant); // oxidant
                field.set(x, y, z, 3, sim.boundary_prime_carbon); // carbon
            }
            // Bottom layers: some reductant (geological source analog)
            for z in (GRID_Z - layers)..GRID_Z {
                field.set(x, y, z, 2, sim.boundary_prime_reductant); // reductant
            }
        }
    }
}

// === CELL SEEDING ===

fn seed_cells(
    cells: &mut Vec<CellState>,
    cell_map: &mut HashMap<[u16; 3], usize>,
    rng: &mut impl Rng,
    count: usize,
    z_lo: u16,
    z_hi: u16,
    factory: fn([u16; 3], u64) -> CellState,
    sim: &SimulationConfig,
) {
    let margin = sim.seed_margin;
    let mut seeded = 0;
    for _ in 0..count * 8 {
        if seeded >= count { break; }
        let x = rng.random_range(margin..GRID_X as u16 - margin);
        let y = rng.random_range(margin..GRID_Y as u16 - margin);
        let z = rng.random_range(z_lo..z_hi.min(GRID_Z as u16));
        let pos = [x, y, z];
        if cell_map.contains_key(&pos) { continue; }

        let cell = factory(pos, rng.random::<u64>());
        let idx = cells.len();
        cell_map.insert(pos, idx);
        cells.push(cell);
        seeded += 1;
    }
}

// === STATS ===

fn print_stats(
    tick: u32,
    cells: &[CellState],
    field: &Field,
    _light: &LightField,
    div: u64,
    death: u64,
    start: &Instant,
) {
    if cells.is_empty() {
        println!("t={:>5} | EXTINCT | div={} death={}", tick, div, death);
        return;
    }

    let n = cells.len() as f32;
    let avg_energy: f32 = cells.iter().map(|c| c.internal[0]).sum::<f32>() / n;
    let avg_enzyme: f32 = cells.iter().map(|c| c.internal[5]).sum::<f32>() / n;
    let active_rxns: f32 = cells.iter()
        .map(|c| c.ruleset.reactions.iter().filter(|r| r.v_max.abs() > 1e-9).count() as f32)
        .sum::<f32>() / n;

    // Count cells per z-third (surface / middle / deep)
    let z_third = (GRID_Z / 3) as u16;
    let (mut n_top, mut n_mid, mut n_bot) = (0u32, 0u32, 0u32);
    for c in cells {
        if c.pos[2] < z_third { n_top += 1; }
        else if c.pos[2] < z_third * 2 { n_mid += 1; }
        else { n_bot += 1; }
    }

    // Sample chemistry at center column
    let cx = GRID_X / 2;
    let cy = GRID_Y / 2;
    let ox_top = field.get(cx, cy, 0, 1);
    let ox_mid = field.get(cx, cy, GRID_Z / 2, 1);
    let red_bot = field.get(cx, cy, GRID_Z - 1, 2);
    let org_mid = field.get(cx, cy, GRID_Z / 2, 4);

    let elapsed = start.elapsed().as_secs_f32();
    let tps = if elapsed > 0.0 { (tick + 1) as f32 / elapsed } else { 0.0 };

    println!(
        "t={:>5} | pop={:>6} (top:{:>5} mid:{:>5} bot:{:>5}) | E={:.2} enz={:.3} rxn={:.1} | ox={:.2}/{:.2} red={:.2} org={:.2} | {:.1} t/s",
        tick, cells.len(), n_top, n_mid, n_bot,
        avg_energy, avg_enzyme, active_rxns,
        ox_top, ox_mid, red_bot, org_mid,
        tps,
    );
}

fn print_z_profile(cells: &[CellState], field: &Field, light: &LightField) {
    println!("{:>3} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}",
        "z", "cells", "light", "oxidnt", "reduct", "carbon", "organic");
    let cx = GRID_X / 2;
    let cy = GRID_Y / 2;
    for z in 0..GRID_Z {
        let n = cells.iter().filter(|c| c.pos[2] == z as u16).count();
        let l = light.get(cx, cy, z);
        let ox = field.get(cx, cy, z, 1);
        let re = field.get(cx, cy, z, 2);
        let ca = field.get(cx, cy, z, 3);
        let og = field.get(cx, cy, z, 4);
        println!("{:>3} {:>6} {:>6.3} {:>6.3} {:>6.3} {:>6.3} {:>6.3}",
            z, n, l, ox, re, ca, og);
    }
}

// === NEIGHBOR UTILITIES ===

/// The 6 face-neighbor offsets in 3D (±x, ±y, ±z).
/// Shared by all neighbor-scanning functions.
const FACE_OFFSETS: [(i16, i16, i16); 6] = [
    (1, 0, 0), (-1, 0, 0),
    (0, 1, 0), (0, -1, 0),
    (0, 0, 1), (0, 0, -1),
];

/// Collect the positions of all in-bounds, empty face-neighbors of a voxel.
fn empty_neighbors(
    pos: [u16; 3],
    cell_map: &HashMap<[u16; 3], usize>,
) -> Vec<[u16; 3]> {
    FACE_OFFSETS.iter()
        .filter_map(|&(dx, dy, dz)| {
            let nx = pos[0] as i16 + dx;
            let ny = pos[1] as i16 + dy;
            let nz = pos[2] as i16 + dz;
            if nx >= 0 && nx < GRID_X as i16
                && ny >= 0 && ny < GRID_Y as i16
                && nz >= 0 && nz < GRID_Z as i16
            {
                let p = [nx as u16, ny as u16, nz as u16];
                if !cell_map.contains_key(&p) { Some(p) } else { None }
            } else {
                None
            }
        })
        .collect()
}

/// Read the average chemical environment available to a cell from
/// surrounding extracellular space.
///
/// In a real microbial column, cells access dissolved chemicals from the
/// liquid medium around them — not from inside their own body. A cell
/// surrounded by other cells has limited access to nutrients: only empty
/// (liquid-filled) neighboring voxels contribute.
///
/// Returns the mean concentration across all empty face-neighbors. If
/// the cell is completely enclosed by other cells, returns all zeros —
/// the cell is cut off from external resources and will starve.
fn read_neighbor_environment(
    pos: [u16; 3],
    field: &Field,
    cell_map: &HashMap<[u16; 3], usize>,
) -> [f32; S_EXT] {
    let neighbors = empty_neighbors(pos, cell_map);
    if neighbors.is_empty() {
        return [0.0; S_EXT];
    }
    let mut sum = [0.0f32; S_EXT];
    for npos in &neighbors {
        let voxel = field.read_voxel(npos[0] as usize, npos[1] as usize, npos[2] as usize);
        for s in 0..S_EXT {
            sum[s] += voxel[s];
        }
    }
    let n = neighbors.len() as f32;
    for s in 0..S_EXT {
        sum[s] /= n;
    }
    sum
}

/// Distribute a cell's secretion/consumption deltas to surrounding empty voxels.
///
/// In the real world, metabolic byproducts are released into the liquid
/// medium around the cell, and consumed substrates are depleted from that
/// same medium. The deltas are split equally among all empty face-neighbors.
///
/// If no empty neighbors exist, deltas are lost — the cell cannot exchange
/// chemicals with a fully packed environment (waste heat / trapped products).
fn apply_deltas_to_neighbors(
    pos: [u16; 3],
    field: &mut Field,
    cell_map: &HashMap<[u16; 3], usize>,
    deltas: &[f32; S_EXT],
) {
    let neighbors = empty_neighbors(pos, cell_map);
    if neighbors.is_empty() {
        return; // enclosed cell — deltas are lost
    }
    let n = neighbors.len() as f32;
    let mut split_deltas = [0.0f32; S_EXT];
    for s in 0..S_EXT {
        split_deltas[s] = deltas[s] / n;
    }
    for npos in &neighbors {
        field.apply_deltas(npos[0] as usize, npos[1] as usize, npos[2] as usize, &split_deltas);
    }
}

/// Find an empty voxel `division_neighbor_distance` steps away along a face axis.
/// Daughters bud off and drift — they don't accrete like plant tissue.
/// Falls back to adjacent placement if no long-range positions are available.
fn find_empty_neighbor(
    pos: [u16; 3],
    occupied: &HashMap<[u16; 3], usize>,
    rng: &mut impl Rng,
    sim: &SimulationConfig,
) -> Option<[u16; 3]> {
    let dist = sim.division_neighbor_distance as i16;
    // Try distance-first (gap between parent and daughter)
    let mut candidates: Vec<[u16; 3]> = Vec::new();
    for &(dx, dy, dz) in &FACE_OFFSETS {
        let nx = pos[0] as i16 + dx * dist;
        let ny = pos[1] as i16 + dy * dist;
        let nz = pos[2] as i16 + dz * dist;
        if nx >= 0 && nx < GRID_X as i16
            && ny >= 0 && ny < GRID_Y as i16
            && nz >= 0 && nz < GRID_Z as i16
        {
            let npos = [nx as u16, ny as u16, nz as u16];
            if !occupied.contains_key(&npos) {
                candidates.push(npos);
            }
        }
    }
    if !candidates.is_empty() {
        let idx = rng.random_range(0..candidates.len());
        return Some(candidates[idx]);
    }

    // Fallback: adjacent placement if surrounded at distance
    let adjacent = empty_neighbors(pos, occupied);
    if adjacent.is_empty() {
        None
    } else {
        let idx = rng.random_range(0..adjacent.len());
        Some(adjacent[idx])
    }
}

#[allow(dead_code)] // TODO: used by HGT when re-enabled
fn find_cell_neighbor(
    pos: [u16; 3],
    cell_map: &HashMap<[u16; 3], usize>,
) -> Option<usize> {
    let offsets: [(i16, i16, i16); 6] = [
        (1, 0, 0), (-1, 0, 0),
        (0, 1, 0), (0, -1, 0),
        (0, 0, 1), (0, 0, -1),
    ];
    for &(dx, dy, dz) in &offsets {
        let nx = pos[0] as i16 + dx;
        let ny = pos[1] as i16 + dy;
        let nz = pos[2] as i16 + dz;
        if nx >= 0 && nx < GRID_X as i16
            && ny >= 0 && ny < GRID_Y as i16
            && nz >= 0 && nz < GRID_Z as i16
        {
            let npos = [nx as u16, ny as u16, nz as u16];
            if let Some(&idx) = cell_map.get(&npos) {
                return Some(idx);
            }
        }
    }
    None
}

// === STARTER RULESETS ===
// From mock-winogradsky-scenario.md Section 4

fn inactive_receptor() -> ReceptorParams {
    ReceptorParams { k_half: 1.0, n_hill: 2.0, gain: 0.0 }
}
fn inactive_transport() -> TransportParams {
    TransportParams { uptake_rate: 0.0, secrete_rate: 0.0, ext_species: 0, int_species: 0 }
}
fn inactive_reaction() -> Reaction {
    Reaction { substrate: 0, product: 0, catalyst: 0, cofactor: 0xFF, k_m: 1.0, v_max: 0.0, k_cat: 0.5 }
}
fn inactive_effector() -> EffectorParams {
    EffectorParams { threshold: 10.0, rate: 0.0, int_species: 0, ext_species: 0 }
}

/// Phototroph: uses light + reductant to produce energy and oxidant.
/// Surface dweller. Spec Section 4.1.
fn make_phototroph(pos: [u16; 3], lineage_id: u64) -> CellState {
    let mut receptors: [ReceptorParams; S_RECEPTORS] = std::array::from_fn(|_| inactive_receptor());
    let mut transport: [TransportParams; S_TRANSPORTERS] = std::array::from_fn(|_| inactive_transport());
    let mut reactions: [Reaction; R_MAX] = std::array::from_fn(|_| inactive_reaction());
    let mut effectors: [EffectorParams; S_EFFECTORS] = std::array::from_fn(|_| inactive_effector());

    // Sense reductant and carbon
    receptors[2] = ReceptorParams { k_half: 0.3, n_hill: 2.0, gain: 1.0 };
    receptors[3] = ReceptorParams { k_half: 0.5, n_hill: 2.0, gain: 1.0 };

    // Transport: carbon is the primary fuel, secretes oxidant + organic waste
    transport[0] = TransportParams { uptake_rate: 0.6, secrete_rate: 0.0, ext_species: 3, int_species: 3 }; // carbon in (primary)
    transport[1] = TransportParams { uptake_rate: 0.0, secrete_rate: 0.6, ext_species: 1, int_species: 1 }; // oxidant out
    transport[2] = TransportParams { uptake_rate: 0.0, secrete_rate: 0.3, ext_species: 4, int_species: 4 }; // organic waste out
    transport[3] = TransportParams { uptake_rate: 0.1, secrete_rate: 0.0, ext_species: 2, int_species: 2 }; // some reductant in (secondary)

    // Rxn 0: carbon(3) -> energy(0), cat=LIGHT(15)  — photosynthesis: CO2 + light -> energy
    reactions[0] = Reaction { substrate: 3, product: 0, catalyst: 15, cofactor: 0xFF, k_m: 0.2, v_max: 0.8, k_cat: 0.1 };
    // Rxn 1: carbon(3) -> oxidant(1), cat=LIGHT(15)  — O2 production (water splitting analog)
    reactions[1] = Reaction { substrate: 3, product: 1, catalyst: 15, cofactor: 0xFF, k_m: 0.2, v_max: 0.4, k_cat: 0.1 };
    // Rxn 2: carbon(3) -> organic(4), cat=energy(0)  — carbon fixation into biomass
    reactions[2] = Reaction { substrate: 3, product: 4, catalyst: 0, cofactor: 0xFF, k_m: 0.3, v_max: 0.3, k_cat: 0.3 };
    // Rxn 3: carbon(3) -> enzyme-A(5), cat=enzyme-B(6)  — autocatalytic pair
    reactions[3] = Reaction { substrate: 3, product: 5, catalyst: 6, cofactor: 0xFF, k_m: 0.3, v_max: 0.15, k_cat: 0.1 };
    // Rxn 4: carbon(3) -> enzyme-B(6), cat=enzyme-A(5)
    reactions[4] = Reaction { substrate: 3, product: 6, catalyst: 5, cofactor: 0xFF, k_m: 0.3, v_max: 0.1, k_cat: 0.1 };

    // Effectors: secrete oxidant and organic waste
    effectors[0] = EffectorParams { threshold: 0.5, rate: 0.8, int_species: 1, ext_species: 1 }; // oxidant out
    effectors[1] = EffectorParams { threshold: 1.0, rate: 0.2, int_species: 4, ext_species: 4 }; // waste out

    let ruleset = Ruleset {
        receptors, transport, reactions, effectors,
        fate: FateParams { division_energy: 1.2, death_energy: 0.05, quiescence_energy: 0.15, division_prep_ticks: 20.0 },
        hgt_propensity: 0.1,
        mutation_rate: 0.05,
    };

    let mut internal = [0.0f32; M_INT];
    internal[0] = 0.3;   // starting energy
    internal[3] = 0.3;   // starting carbon (photosynthesis substrate)
    internal[5] = 0.05;  // enzyme-A seed
    internal[6] = 0.05;  // enzyme-B seed

    CellState { pos, lineage_id, age: 0, internal, ruleset, quiescent: false, starter_type: 0, prep_remaining: 0 }
}

/// Chemolithotroph: oxidizes reductant using oxidant at the chemocline.
/// Models sulfur-oxidizing bacteria at the oxic-anoxic interface.
fn make_chemolithotroph(pos: [u16; 3], lineage_id: u64) -> CellState {
    let mut receptors: [ReceptorParams; S_RECEPTORS] = std::array::from_fn(|_| inactive_receptor());
    let mut transport: [TransportParams; S_TRANSPORTERS] = std::array::from_fn(|_| inactive_transport());
    let mut reactions: [Reaction; R_MAX] = std::array::from_fn(|_| inactive_reaction());
    let mut effectors: [EffectorParams; S_EFFECTORS] = std::array::from_fn(|_| inactive_effector());

    receptors[1] = ReceptorParams { k_half: 0.3, n_hill: 2.0, gain: 1.0 }; // oxidant
    receptors[2] = ReceptorParams { k_half: 0.3, n_hill: 2.0, gain: 1.0 }; // reductant

    // Transport: take in both oxidant and reductant, secrete organic waste
    transport[0] = TransportParams { uptake_rate: 0.7, secrete_rate: 0.0, ext_species: 1, int_species: 1 }; // oxidant in
    transport[1] = TransportParams { uptake_rate: 0.7, secrete_rate: 0.0, ext_species: 2, int_species: 2 }; // reductant in
    transport[2] = TransportParams { uptake_rate: 0.2, secrete_rate: 0.0, ext_species: 3, int_species: 3 }; // carbon in (for enzymes)
    transport[3] = TransportParams { uptake_rate: 0.0, secrete_rate: 0.3, ext_species: 4, int_species: 4 }; // organic waste out

    // Rxn 0: reductant(2) -> energy(0), cat=enzyme-A(5), cofactor=oxidant(1)  — sulfur oxidation
    reactions[0] = Reaction { substrate: 2, product: 0, catalyst: 5, cofactor: 1, k_m: 0.15, v_max: 0.7, k_cat: 0.1 };
    // Rxn 1: oxidant(1) -> organic(4), cat=energy(0)  — oxidant processing byproduct
    reactions[1] = Reaction { substrate: 1, product: 4, catalyst: 0, cofactor: 0xFF, k_m: 0.3, v_max: 0.3, k_cat: 0.2 };
    // Rxn 2: carb_reserve(7) -> energy(0), cat=enzyme-A(5)  — slow burn of internal carbon store
    reactions[2] = Reaction { substrate: 7, product: 0, catalyst: 5, cofactor: 0xFF, k_m: 0.3, v_max: 0.15, k_cat: 0.1 };
    // Rxn 3-4: autocatalytic enzyme loop
    reactions[3] = Reaction { substrate: 3, product: 5, catalyst: 6, cofactor: 0xFF, k_m: 0.3, v_max: 0.15, k_cat: 0.1 };
    reactions[4] = Reaction { substrate: 3, product: 6, catalyst: 5, cofactor: 0xFF, k_m: 0.3, v_max: 0.1, k_cat: 0.1 };

    // Effectors: secrete organic waste
    effectors[0] = EffectorParams { threshold: 0.5, rate: 0.3, int_species: 4, ext_species: 4 };

    let ruleset = Ruleset {
        receptors, transport, reactions, effectors,
        fate: FateParams { division_energy: 1.0, death_energy: 0.05, quiescence_energy: 0.12, division_prep_ticks: 20.0 },
        hgt_propensity: 0.1,
        mutation_rate: 0.05,
    };

    let mut internal = [0.0f32; M_INT];
    internal[0] = 1.5;   // substantial starting energy
    internal[1] = 0.5;   // starting oxidant
    internal[2] = 0.5;   // starting reductant
    internal[5] = 0.05;
    internal[6] = 0.05;
    internal[7] = 5.0;   // carb reserve — slow-burn fuel while waiting for gradients to form

    CellState { pos, lineage_id, age: 0, internal, ruleset, quiescent: false, starter_type: 1, prep_remaining: 0 }
}

/// Anaerobe: uses reductant for energy. Killed by oxidant. Deep zone.
/// Spec Section 4.3.
fn make_anaerobe(pos: [u16; 3], lineage_id: u64) -> CellState {
    let mut receptors: [ReceptorParams; S_RECEPTORS] = std::array::from_fn(|_| inactive_receptor());
    let mut transport: [TransportParams; S_TRANSPORTERS] = std::array::from_fn(|_| inactive_transport());
    let mut reactions: [Reaction; R_MAX] = std::array::from_fn(|_| inactive_reaction());
    let mut effectors: [EffectorParams; S_EFFECTORS] = std::array::from_fn(|_| inactive_effector());

    receptors[2] = ReceptorParams { k_half: 0.3, n_hill: 2.0, gain: 1.0 }; // reductant

    // Transport — higher uptake rates to match the strong vent chemistry
    transport[0] = TransportParams { uptake_rate: 0.9, secrete_rate: 0.0, ext_species: 2, int_species: 2 }; // reductant in (primary fuel)
    transport[1] = TransportParams { uptake_rate: 0.3, secrete_rate: 0.0, ext_species: 3, int_species: 3 }; // carbon in
    transport[2] = TransportParams { uptake_rate: 0.0, secrete_rate: 0.5, ext_species: 4, int_species: 4 }; // organic waste out
    transport[3] = TransportParams { uptake_rate: 0.1, secrete_rate: 0.0, ext_species: 1, int_species: 1 }; // oxidant in (inadvertent!)

    // Rxn 0: reductant(2) -> energy(0), cat=enzyme-A(5)  — anaerobic respiration (BUFFED v_max)
    reactions[0] = Reaction { substrate: 2, product: 0, catalyst: 5, cofactor: 0xFF, k_m: 0.15, v_max: 0.8, k_cat: 0.1 };
    // Rxn 1: carbon(3) -> organic(4), cat=energy(0)  — fermentation
    reactions[1] = Reaction { substrate: 3, product: 4, catalyst: 0, cofactor: 0xFF, k_m: 0.2, v_max: 0.3, k_cat: 0.2 };
    // Rxn 2: OXIDANT TOXICITY — energy(0) -> carbon(3), cat=oxidant(1)
    //   High v_max + low k_m = even trace oxidant is lethal
    reactions[2] = Reaction { substrate: 0, product: 3, catalyst: 1, cofactor: 0xFF, k_m: 0.01, v_max: 2.0, k_cat: 0.01 };
    // Rxn 3-4: autocatalytic enzyme loop
    reactions[3] = Reaction { substrate: 3, product: 5, catalyst: 6, cofactor: 0xFF, k_m: 0.3, v_max: 0.15, k_cat: 0.1 };
    reactions[4] = Reaction { substrate: 3, product: 6, catalyst: 5, cofactor: 0xFF, k_m: 0.3, v_max: 0.1, k_cat: 0.1 };

    // Effectors: secrete organic waste
    effectors[0] = EffectorParams { threshold: 0.5, rate: 0.5, int_species: 4, ext_species: 4 };

    let ruleset = Ruleset {
        receptors, transport, reactions, effectors,
        fate: FateParams { division_energy: 0.8, death_energy: 0.05, quiescence_energy: 0.1, division_prep_ticks: 20.0 },
        hgt_propensity: 0.1,
        mutation_rate: 0.05,
    };

    let mut internal = [0.0f32; M_INT];
    internal[0] = 0.5;   // more starting energy (needs to survive the 20-tick prep phase)
    internal[2] = 0.7;   // more starting reductant (vents are chemically rich, help bootstrap)
    internal[5] = 0.05;
    internal[6] = 0.05;

    CellState { pos, lineage_id, age: 0, internal, ruleset, quiescent: false, starter_type: 2, prep_remaining: 0 }
}
