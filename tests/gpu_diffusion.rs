#![cfg(feature = "gpu")]

use marl::config::{GRID_X, GRID_Y, GRID_Z, S_EXT, SimulationConfig};
use marl::field::Field;
use marl::gpu::{GpuError, GpuFieldDiffuser};

fn init_field(field: &mut Field) {
    for z in 0..GRID_Z {
        for y in 0..GRID_Y {
            for x in 0..GRID_X {
                for s in 0..S_EXT {
                    let wave = ((x * 17 + y * 31 + z * 43 + s * 59) % 997) as f32 / 997.0;
                    let eps_boost = if s == 7 && x > GRID_X / 3 && x < GRID_X / 3 + 16 {
                        2.0
                    } else {
                        0.0
                    };
                    field.set(x, y, z, s, wave * (s as f32 + 1.0) * 0.02 + eps_boost);
                }
            }
        }
    }
}

fn occupancy_empty() -> Vec<bool> {
    vec![false; GRID_X * GRID_Y * GRID_Z]
}

fn occupancy_center() -> Vec<bool> {
    let mut occupancy = occupancy_empty();
    occupancy[voxel_idx(GRID_X / 2, GRID_Y / 2, GRID_Z / 2)] = true;
    occupancy
}

fn occupancy_boundary_adjacent() -> Vec<bool> {
    let mut occupancy = occupancy_empty();
    occupancy[voxel_idx(GRID_X / 2, GRID_Y / 2, 1)] = true;
    occupancy
}

fn occupancy_dense_cluster() -> Vec<bool> {
    let mut occupancy = occupancy_empty();
    let cx = GRID_X / 2;
    let cy = GRID_Y / 2;
    let cz = GRID_Z / 2;
    for z in cz - 1..=cz + 1 {
        for y in cy - 1..=cy + 1 {
            for x in cx - 1..=cx + 1 {
                occupancy[voxel_idx(x, y, z)] = true;
            }
        }
    }
    occupancy
}

fn voxel_idx(x: usize, y: usize, z: usize) -> usize {
    z * GRID_Y * GRID_X + y * GRID_X + x
}

fn compare_fields(cpu: &Field, gpu: &Field, tolerance: f32) {
    let mut worst_idx = 0;
    let mut max_abs = 0.0f32;
    let mut max_rel = 0.0f32;
    for (i, (&a, &b)) in cpu.data.iter().zip(&gpu.data).enumerate() {
        let abs = (a - b).abs();
        if abs > max_abs {
            max_abs = abs;
            worst_idx = i;
        }
        let denom = a.abs().max(b.abs());
        if denom > 1e-6 {
            max_rel = max_rel.max(abs / denom);
        }
    }

    let flat = worst_idx / S_EXT;
    let s = worst_idx % S_EXT;
    let z = flat / (GRID_X * GRID_Y);
    let y = (flat % (GRID_X * GRID_Y)) / GRID_X;
    let x = flat % GRID_X;
    assert!(
        max_abs <= tolerance,
        "CPU/GPU diffusion mismatch: max_abs={max_abs:e}, max_rel={max_rel:e}, worst=(x={x}, y={y}, z={z}, s={s}), cpu={}, gpu={}",
        cpu.data[worst_idx],
        gpu.data[worst_idx]
    );
}

fn run_case(label: &str, occupancy: Vec<bool>, substeps: usize) {
    let mut sim = SimulationConfig::default();
    sim.diffusion_substeps = substeps;

    let mut cpu_field = Field::new();
    init_field(&mut cpu_field);
    let mut gpu_field = cpu_field.clone();

    cpu_field.diffuse_tick_with_cells(&occupancy, &sim);

    let mut diffuser = match GpuFieldDiffuser::new() {
        Ok(diffuser) => diffuser,
        Err(GpuError::NoAdapter) => {
            eprintln!("skipping {label}: no compatible GPU adapter");
            return;
        }
        Err(err) => panic!("failed to initialize GPU diffuser for {label}: {err}"),
    };
    diffuser
        .diffuse_tick_with_cells(&mut gpu_field, &occupancy, &sim)
        .unwrap_or_else(|err| panic!("GPU diffusion failed for {label}: {err}"));

    compare_fields(&cpu_field, &gpu_field, 1e-4);
}

#[test]
fn gpu_matches_cpu_empty_occupancy() {
    run_case("empty occupancy", occupancy_empty(), 2);
}

#[test]
fn gpu_matches_cpu_center_occupancy() {
    run_case("center occupancy", occupancy_center(), 2);
}

#[test]
fn gpu_matches_cpu_boundary_adjacent_occupancy() {
    run_case(
        "boundary-adjacent occupancy",
        occupancy_boundary_adjacent(),
        2,
    );
}

#[test]
fn gpu_matches_cpu_dense_cluster_occupancy() {
    run_case("dense cluster occupancy", occupancy_dense_cluster(), 2);
}

#[test]
fn gpu_matches_cpu_single_substep_ping_pong() {
    run_case("single substep ping-pong", occupancy_center(), 1);
}

#[test]
fn gpu_matches_cpu_default_substeps() {
    run_case(
        "default substeps",
        occupancy_empty(),
        SimulationConfig::default().diffusion_substeps,
    );
}
