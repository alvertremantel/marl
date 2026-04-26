use crate::cell::CellState;
use crate::config::{GRID_X, GRID_Y, GRID_Z, SimulationConfig};
use crate::field::Field;
use rand::Rng;
use std::collections::HashMap;

/// Initialize field with thin boundary layers only.
/// The bulk of the field starts empty — gradients build from boundary sources + diffusion.
pub fn init_field_boundaries(field: &mut Field, sim: &SimulationConfig) {
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

pub fn seed_cells(
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
        if seeded >= count {
            break;
        }
        let x = rng.random_range(margin..GRID_X as u16 - margin);
        let y = rng.random_range(margin..GRID_Y as u16 - margin);
        let z = rng.random_range(z_lo..z_hi.min(GRID_Z as u16));
        let pos = [x, y, z];
        if cell_map.contains_key(&pos) {
            continue;
        }

        let cell = factory(pos, rng.random::<u64>());
        let idx = cells.len();
        cell_map.insert(pos, idx);
        cells.push(cell);
        seeded += 1;
    }
}
