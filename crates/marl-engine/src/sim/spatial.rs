use crate::config::{GRID_X, GRID_Y, GRID_Z, S_EXT, SimulationConfig};
use crate::field::Field;
use rand::Rng;
use std::collections::HashMap;

/// The 6 face-neighbor offsets in 3D (±x, ±y, ±z).
/// Shared by all neighbor-scanning functions.
const FACE_OFFSETS: [(i16, i16, i16); 6] = [
    (1, 0, 0),
    (-1, 0, 0),
    (0, 1, 0),
    (0, -1, 0),
    (0, 0, 1),
    (0, 0, -1),
];

/// Collect the positions of all in-bounds, empty face-neighbors of a voxel.
pub fn empty_neighbors(pos: [u16; 3], cell_map: &HashMap<[u16; 3], usize>) -> Vec<[u16; 3]> {
    FACE_OFFSETS
        .iter()
        .filter_map(|&(dx, dy, dz)| {
            let nx = pos[0] as i16 + dx;
            let ny = pos[1] as i16 + dy;
            let nz = pos[2] as i16 + dz;
            if nx >= 0
                && nx < GRID_X as i16
                && ny >= 0
                && ny < GRID_Y as i16
                && nz >= 0
                && nz < GRID_Z as i16
            {
                let p = [nx as u16, ny as u16, nz as u16];
                if !cell_map.contains_key(&p) {
                    Some(p)
                } else {
                    None
                }
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
pub fn read_neighbor_environment(
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
pub fn apply_deltas_to_neighbors(
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
        field.apply_deltas(
            npos[0] as usize,
            npos[1] as usize,
            npos[2] as usize,
            &split_deltas,
        );
    }
}

/// Find an empty voxel `division_neighbor_distance` steps away along a face axis.
/// Daughters bud off and drift — they don't accrete like plant tissue.
/// Falls back to adjacent placement if no long-range positions are available.
pub fn find_empty_neighbor(
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
        if nx >= 0
            && nx < GRID_X as i16
            && ny >= 0
            && ny < GRID_Y as i16
            && nz >= 0
            && nz < GRID_Z as i16
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
pub fn find_cell_neighbor(pos: [u16; 3], cell_map: &HashMap<[u16; 3], usize>) -> Option<usize> {
    let offsets: [(i16, i16, i16); 6] = [
        (1, 0, 0),
        (-1, 0, 0),
        (0, 1, 0),
        (0, -1, 0),
        (0, 0, 1),
        (0, 0, -1),
    ];
    for &(dx, dy, dz) in &offsets {
        let nx = pos[0] as i16 + dx;
        let ny = pos[1] as i16 + dy;
        let nz = pos[2] as i16 + dz;
        if nx >= 0
            && nx < GRID_X as i16
            && ny >= 0
            && ny < GRID_Y as i16
            && nz >= 0
            && nz < GRID_Z as i16
        {
            let npos = [nx as u16, ny as u16, nz as u16];
            if let Some(&idx) = cell_map.get(&npos) {
                return Some(idx);
            }
        }
    }
    None
}
