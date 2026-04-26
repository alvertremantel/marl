struct Params {
    grid: vec4<u32>,
    render: vec4<u32>,
    transfer: vec4<f32>,
};

@group(0) @binding(0) var field_tex: texture_3d<f32>;
@group(0) @binding(1) var<uniform> params: Params;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );

    var out: VertexOut;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    return out;
}

fn palette(value: f32, z_frac: f32) -> vec3<f32> {
    let cold = vec3<f32>(0.03, 0.18, 0.45);
    let warm = vec3<f32>(0.95, 0.62, 0.18);
    let hot = vec3<f32>(0.85, 0.95, 0.92);
    let mid = mix(cold, warm, clamp(value * 1.8, 0.0, 1.0));
    return mix(mid, hot, clamp(value - 0.55, 0.0, 1.0)) * (0.65 + 0.35 * z_frac);
}

@fragment
fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let viewport = vec2<f32>(f32(params.render.x), f32(params.render.y));
    let uv = frag_coord.xy / max(viewport, vec2<f32>(1.0, 1.0));

    let grid_x = params.grid.x;
    let grid_y = params.grid.y;
    let grid_z = params.grid.z;
    let species_count = params.grid.w;
    let species = min(params.render.z, species_count - 1u);
    let steps = max(params.render.w, 1u);
    let exposure = params.transfer.x;
    let density_scale = params.transfer.y;

    let voxel_x = min(u32(uv.x * f32(grid_x)), grid_x - 1u);
    let voxel_y = min(u32((1.0 - uv.y) * f32(grid_y)), grid_y - 1u);

    var rgb = vec3<f32>(0.0, 0.0, 0.0);
    var alpha = 0.0;

    for (var step = 0u; step < steps; step = step + 1u) {
        let z_frac = (f32(step) + 0.5) / f32(steps);
        let voxel_z = min(u32(z_frac * f32(grid_z)), grid_z - 1u);
        let tex_x = voxel_x * species_count + species;
        let raw = textureLoad(
            field_tex,
            vec3<i32>(i32(tex_x), i32(voxel_y), i32(voxel_z)),
            0,
        ).r;
        let density = clamp(raw * density_scale, 0.0, 1.0);
        let sample_alpha = 1.0 - exp(-density * exposure / f32(steps));
        let sample_rgb = palette(density, z_frac);

        rgb = rgb + (1.0 - alpha) * sample_alpha * sample_rgb;
        alpha = alpha + (1.0 - alpha) * sample_alpha;

        if (alpha > 0.985) {
            break;
        }
    }

    let background = vec3<f32>(0.008, 0.01, 0.014);
    return vec4<f32>(rgb + (1.0 - alpha) * background, 1.0);
}
