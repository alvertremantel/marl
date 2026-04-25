use crate::config::*;
use crate::field::Field;
use std::collections::HashMap;

/// Light availability field. One scalar per voxel.
pub struct LightField {
    pub data: Vec<f32>,
}

impl LightField {
    pub fn new() -> Self {
        Self {
            data: vec![0.0; GRID_X * GRID_Y * GRID_Z],
        }
    }

    #[inline]
    fn idx(x: usize, y: usize, z: usize) -> usize {
        (z * GRID_Y + y) * GRID_X + x
    }

    pub fn get(&self, x: usize, y: usize, z: usize) -> f32 {
        self.data[Self::idx(x, y, z)]
    }

    /// Beer-Lambert top-down sweep.
    /// Light enters at z=0 with intensity 1.0, attenuates with depth
    /// based on cell density and absorber concentrations.
    pub fn update(
        &mut self,
        field: &Field,
        cells: &HashMap<[u16; 3], usize>, // pos -> cell index (for density)
        sim: &SimulationConfig,
    ) {
        for y in 0..GRID_Y {
            for x in 0..GRID_X {
                let mut intensity = sim.surface_intensity;

                for z in 0..GRID_Z {
                    self.data[Self::idx(x, y, z)] = intensity;

                    // Attenuation from cells
                    let pos = [x as u16, y as u16, z as u16];
                    if cells.contains_key(&pos) {
                        intensity *= (-sim.cell_absorption).exp();
                    }

                    // Attenuation from chemical absorbers (e.g., organic waste = species 4)
                    let absorber = field.get(x, y, z, 4);
                    intensity *= (-sim.chemical_absorption * absorber).exp();

                    // Floor to prevent denormals
                    if intensity < sim.light_floor { intensity = 0.0; }
                }
            }
        }
    }
}
