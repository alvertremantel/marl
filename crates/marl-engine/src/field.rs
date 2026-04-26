use crate::config::*;
use rayon::prelude::*;

/// 3D chemical concentration field.
///
/// Stores concentrations of all S_EXT chemical species at every voxel in
/// a flat array laid out as [z][y][x][species]. This memory order gives
/// cache-friendly access when iterating z-columns (the dominant access
/// pattern for diffusion and light attenuation).
///
/// A pre-allocated scratch buffer (`scratch`) eliminates per-substep
/// allocation during diffusion. The two buffers are swapped each substep
/// so no copying is ever needed.
#[derive(Clone)]
pub struct Field {
    /// Concentration data: GRID_Z * GRID_Y * GRID_X * S_EXT floats
    pub data: Vec<f32>,
    /// Double-buffer for diffusion solver — same size as `data`.
    /// Swapped with `data` each substep to avoid allocation in the hot loop.
    scratch: Vec<f32>,
}

impl Field {
    pub fn new() -> Self {
        let n = GRID_X * GRID_Y * GRID_Z * S_EXT;
        Self {
            data: vec![0.0; n],
            scratch: vec![0.0; n],
        }
    }

    #[inline]
    fn idx(&self, x: usize, y: usize, z: usize, s: usize) -> usize {
        ((z * GRID_Y + y) * GRID_X + x) * S_EXT + s
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize, s: usize) -> f32 {
        self.data[self.idx(x, y, z, s)]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: usize, s: usize, val: f32) {
        let i = self.idx(x, y, z, s);
        self.data[i] = val;
    }

    /// Read all species at a voxel
    pub fn read_voxel(&self, x: usize, y: usize, z: usize) -> [f32; S_EXT] {
        let mut out = [0.0f32; S_EXT];
        let base = self.idx(x, y, z, 0);
        out.copy_from_slice(&self.data[base..base + S_EXT]);
        out
    }

    /// Apply cell secretion/consumption deltas to a voxel
    pub fn apply_deltas(&mut self, x: usize, y: usize, z: usize, deltas: &[f32; S_EXT]) {
        let base = self.idx(x, y, z, 0);
        for s in 0..S_EXT {
            self.data[base + s] = (self.data[base + s] + deltas[s]).max(0.0);
        }
    }

    /// Helper: compute flat index without &self (needed for parallel closures).
    #[inline]
    fn idx_static(x: usize, y: usize, z: usize, s: usize) -> usize {
        ((z * GRID_Y + y) * GRID_X + x) * S_EXT + s
    }

    /// Read a concentration from a raw data slice (used in parallel diffusion).
    #[inline]
    fn get_from(src: &[f32], x: usize, y: usize, z: usize, s: usize) -> f32 {
        src[Self::idx_static(x, y, z, s)]
    }

    /// Run one diffusion substep for all species, parallelized over z-layers.
    ///
    /// Uses forward Euler integration of the 3D discrete Laplacian with
    /// Neumann (zero-flux) boundary conditions. Each voxel's new
    /// concentration is:
    ///
    ///   c' = c + dt * (D_local * laplacian(c) - lambda * c)
    ///
    /// where D_local is reduced by:
    ///   1. Niche construction (structural EPS deposits slow diffusion,
    ///      mimicking biofilm matrix)
    ///   2. Cell body exclusion — occupied voxels are **completely skipped**.
    ///      In a real microbial column, the extracellular medium (water/gel
    ///      between cells) is where diffusion happens. Cells are physical
    ///      objects that exclude the liquid phase. A voxel occupied by a
    ///      cell has no free water for chemicals to diffuse through.
    ///
    /// Occupied neighbors are treated as Neumann boundaries (zero-flux),
    /// identical to wall boundaries. This means chemicals cannot diffuse
    /// into, out of, or through cell-occupied space. Cells access the
    /// external medium through their transport machinery (see cell.rs),
    /// reading from adjacent empty voxels.
    ///
    /// This is the key mechanism that prevents grid saturation: interior
    /// cells in a dense colony are cut off from nutrients because the
    /// surrounding occupied voxels block diffusion. Only cells on the
    /// colony surface have access to the medium.
    ///
    /// The z-loop is embarrassingly parallel: each layer reads from the
    /// source buffer (immutable) and writes to its own slice of the
    /// scratch buffer. Rayon splits this across all available CPU cores.
    fn diffusion_step_inner(
        &mut self,
        dt_sub: f32,
        occupancy: Option<&[bool]>,
        sim: &SimulationConfig,
    ) {
        let src = &self.data;
        let layer_size = GRID_Y * GRID_X * S_EXT;
        let voxel_layer_size = GRID_Y * GRID_X;

        self.scratch
            .par_chunks_mut(layer_size)
            .enumerate()
            .for_each(|(z, dst_layer)| {
                for y in 0..GRID_Y {
                    for x in 0..GRID_X {
                        let occ_here =
                            occupancy.map_or(false, |o| o[z * voxel_layer_size + y * GRID_X + x]);

                        // Occupied voxels are excluded from diffusion entirely.
                        // Their field concentrations are meaningless (chemicals
                        // inside cells are tracked in cell.internal[], not here).
                        // Just copy unchanged to maintain buffer consistency.
                        if occ_here {
                            for s in 0..S_EXT {
                                let local_idx = (y * GRID_X + x) * S_EXT + s;
                                dst_layer[local_idx] = Self::get_from(src, x, y, z, s);
                            }
                            continue;
                        }

                        // --- Empty voxel: compute diffusion normally ---

                        // Niche construction: EPS deposits slow diffusion locally
                        let structural = Self::get_from(src, x, y, z, 7);
                        let niche_factor =
                            1.0 - sim.alpha_eps * structural / (sim.k_eps + structural);

                        // Check which neighbors are occupied or walls.
                        // Occupied neighbors are treated identically to walls:
                        // Neumann BC (zero flux) by substituting center value.
                        let occ_check = |nx: usize, ny: usize, nz: usize| -> bool {
                            occupancy.map_or(false, |o| o[nz * voxel_layer_size + ny * GRID_X + nx])
                        };

                        for s in 0..S_EXT {
                            let c = Self::get_from(src, x, y, z, s);
                            let d = sim.d_voxel[s] * niche_factor;

                            // 6-neighbor Laplacian. Walls AND occupied neighbors
                            // both get Neumann treatment (use center value c).
                            let xm = if x == 0 || occ_check(x - 1, y, z) {
                                c
                            } else {
                                Self::get_from(src, x - 1, y, z, s)
                            };
                            let xp = if x >= GRID_X - 1 || occ_check(x + 1, y, z) {
                                c
                            } else {
                                Self::get_from(src, x + 1, y, z, s)
                            };
                            let ym = if y == 0 || occ_check(x, y - 1, z) {
                                c
                            } else {
                                Self::get_from(src, x, y - 1, z, s)
                            };
                            let yp = if y >= GRID_Y - 1 || occ_check(x, y + 1, z) {
                                c
                            } else {
                                Self::get_from(src, x, y + 1, z, s)
                            };
                            let zm = if z == 0 || occ_check(x, y, z - 1) {
                                c
                            } else {
                                Self::get_from(src, x, y, z - 1, s)
                            };
                            let zp = if z >= GRID_Z - 1 || occ_check(x, y, z + 1) {
                                c
                            } else {
                                Self::get_from(src, x, y, z + 1, s)
                            };

                            let laplacian = xm + xp + ym + yp + zm + zp - 6.0 * c;
                            let decay = sim.lambda_decay[s] * c;
                            let new_c = c + dt_sub * (d * laplacian - decay);

                            let local_idx = (y * GRID_X + x) * S_EXT + s;
                            dst_layer[local_idx] = new_c.max(0.0);
                        }
                    }
                }
            });

        // Swap buffers — the scratch buffer becomes the live data,
        // and the old data buffer becomes scratch for the next substep.
        std::mem::swap(&mut self.data, &mut self.scratch);
    }

    /// Run a full tick of diffusion with cell-body exclusion.
    ///
    /// Occupied voxels are completely excluded from diffusion — chemicals
    /// only move through the extracellular medium (empty voxels). This is
    /// the physically correct model: cells are solid objects that displace
    /// the liquid phase. Interior cells in a dense colony are cut off
    /// from nutrients, creating natural carrying capacity.
    pub fn diffuse_tick_with_cells(&mut self, occupancy: &[bool], sim: &SimulationConfig) {
        let dt_sub = sim.dt / sim.diffusion_substeps as f32;
        for _ in 0..sim.diffusion_substeps {
            self.diffusion_step_inner(dt_sub, Some(occupancy), sim);
        }
    }

    #[allow(dead_code)] // TODO: occupancy-free version kept for testing/benchmarking
    /// Run a full tick of diffusion (multiple substeps for stability)
    pub fn diffuse_tick(&mut self, sim: &SimulationConfig) {
        let dt_sub = sim.dt / sim.diffusion_substeps as f32;
        for _ in 0..sim.diffusion_substeps {
            self.diffusion_step_inner(dt_sub, None, sim);
        }
    }

    /// Set boundary source terms (called once per tick before diffusion).
    /// Oxidant + carbon sourced from top (z=0), reductant from bottom (z=max).
    /// These are the only external inputs to the system — everything else is recycled.
    pub fn apply_boundary_sources(&mut self, sim: &SimulationConfig) {
        // Top face: oxidant (species 1) and carbon (species 3)
        for y in 0..GRID_Y {
            for x in 0..GRID_X {
                let ox = self.get(x, y, 0, 1);
                self.set(x, y, 0, 1, (ox + sim.source_rate_oxidant).min(sim.c_max));
                let ca = self.get(x, y, 0, 3);
                self.set(x, y, 0, 3, (ca + sim.source_rate_carbon).min(sim.c_max));
            }
        }

        // Bottom face: reductant (species 2)
        for y in 0..GRID_Y {
            for x in 0..GRID_X {
                let z = GRID_Z - 1;
                let re = self.get(x, y, z, 2);
                self.set(x, y, z, 2, (re + sim.source_rate_reductant).min(sim.c_max));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_deterministic_field(field: &mut Field) {
        for z in 0..GRID_Z {
            for y in 0..GRID_Y {
                for x in 0..GRID_X {
                    for s in 0..S_EXT {
                        let value = ((x * 13 + y * 17 + z * 19 + s * 23) % 541) as f32 * 0.001;
                        field.set(x, y, z, s, value);
                    }
                }
            }
        }
    }

    #[test]
    fn occupied_voxel_is_copied_unchanged() {
        let mut sim = SimulationConfig::default();
        sim.diffusion_substeps = 1;

        let mut field = Field::new();
        init_deterministic_field(&mut field);

        let x = GRID_X / 2;
        let y = GRID_Y / 2;
        let z = GRID_Z / 2;
        let before = field.read_voxel(x, y, z);

        let mut occupancy = vec![false; GRID_X * GRID_Y * GRID_Z];
        occupancy[z * GRID_Y * GRID_X + y * GRID_X + x] = true;

        field.diffuse_tick_with_cells(&occupancy, &sim);

        assert_eq!(field.read_voxel(x, y, z), before);
    }

    #[test]
    fn deterministic_diffusion_stays_finite_and_nonnegative() {
        let mut sim = SimulationConfig::default();
        sim.diffusion_substeps = 1;

        let mut field = Field::new();
        init_deterministic_field(&mut field);
        let occupancy = vec![false; GRID_X * GRID_Y * GRID_Z];

        field.diffuse_tick_with_cells(&occupancy, &sim);

        for (index, value) in field.data.iter().copied().enumerate() {
            assert!(
                value.is_finite(),
                "non-finite concentration at index {index}"
            );
            assert!(
                value >= 0.0,
                "negative concentration at index {index}: {value}"
            );
        }
    }
}
