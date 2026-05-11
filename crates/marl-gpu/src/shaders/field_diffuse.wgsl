// Naive f32 reaction-diffusion shader matching src/field.rs::diffusion_step_inner.
// Layout is AoS per voxel: [z][y][x][species].

const GRID_X: u32 = 128u;
const GRID_Y: u32 = 128u;
const GRID_Z: u32 = 64u;
const S_EXT: u32 = 12u;
const GRID_SIZE: u32 = GRID_X * GRID_Y * GRID_Z;
const STRUCTURAL_SPECIES: u32 = 7u;

struct DiffusionParams {
    dt_sub: f32,
    alpha_eps: f32,
    k_eps: f32,
    _pad0: f32,
    d_voxel: array<f32, 12>,
    lambda_decay: array<f32, 12>,
}

@group(0) @binding(0)
var<storage, read> field_in: array<f32>;

@group(0) @binding(1)
var<storage, read_write> field_out: array<f32>;

@group(0) @binding(2)
var<storage, read> occupancy: array<u32>;

@group(0) @binding(3)
var<storage, read> params: DiffusionParams;

fn field_idx(x: u32, y: u32, z: u32, s: u32) -> u32 {
    return ((z * GRID_Y + y) * GRID_X + x) * S_EXT + s;
}

fn voxel_idx(x: u32, y: u32, z: u32) -> u32 {
    return (z * GRID_Y + y) * GRID_X + x;
}

fn neighbor_or_center(nx: i32, ny: i32, nz: i32, s: u32, c: f32) -> f32 {
    if (nx < 0 || nx >= i32(GRID_X) || ny < 0 || ny >= i32(GRID_Y) || nz < 0 || nz >= i32(GRID_Z)) {
        return c;
    }

    let x = u32(nx);
    let y = u32(ny);
    let z = u32(nz);
    if (occupancy[voxel_idx(x, y, z)] != 0u) {
        return c;
    }

    return field_in[field_idx(x, y, z, s)];
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let voxel = gid.x;
    if (voxel >= GRID_SIZE) {
        return;
    }

    let x = voxel % GRID_X;
    let y = (voxel / GRID_X) % GRID_Y;
    let z = voxel / (GRID_X * GRID_Y);
    let base = voxel * S_EXT;

    if (occupancy[voxel] != 0u) {
        for (var s = 0u; s < S_EXT; s = s + 1u) {
            field_out[base + s] = field_in[base + s];
        }
        return;
    }

    let structural = field_in[base + STRUCTURAL_SPECIES];
    let niche_factor = 1.0 - params.alpha_eps * structural / (params.k_eps + structural);
    let xi = i32(x);
    let yi = i32(y);
    let zi = i32(z);

    for (var s = 0u; s < S_EXT; s = s + 1u) {
        let c = field_in[base + s];
        let xm = neighbor_or_center(xi - 1, yi, zi, s, c);
        let xp = neighbor_or_center(xi + 1, yi, zi, s, c);
        let ym = neighbor_or_center(xi, yi - 1, zi, s, c);
        let yp = neighbor_or_center(xi, yi + 1, zi, s, c);
        let zm = neighbor_or_center(xi, yi, zi - 1, s, c);
        let zp = neighbor_or_center(xi, yi, zi + 1, s, c);

        let laplacian = xm + xp + ym + yp + zm + zp - 6.0 * c;
        let d = params.d_voxel[s] * niche_factor;
        let decay = params.lambda_decay[s] * c;
        let new_c = c + params.dt_sub * (d * laplacian - decay);
        field_out[base + s] = max(new_c, 0.0);
    }
}
