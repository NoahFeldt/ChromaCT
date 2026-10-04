#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var volume_texture: texture_3d<f32>;
@group(2) @binding(1) var volume_sampler: sampler;

struct MprUniforms {
    window_level: f32,
    window_width: f32,
    depth_fraction: f32,
    interpolation_mode: u32,
    plane: u32,
};
@group(2) @binding(2) var<uniform> uniforms: MprUniforms;

// Standard clinical windowing math
fn apply_window(hu: f32, level: f32, width: f32) -> f32 {
    let lower_bound = level - (width / 2.0);
    return clamp((hu - lower_bound) / width, 0.0, 1.0);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    
    // 1. Get the screen coordinates
    let u = in.uv.x;
    let v = 1.0 - in.uv.y; 
    let depth = uniforms.depth_fraction; 
    
    var sample_coord: vec3<f32>;
    
    // 2. Swizzle (swap) the X, Y, and Z axes depending on the plane
    if (uniforms.plane == 0u) {
        // Transverse
        sample_coord = vec3<f32>(u, v, depth);
    }
    else if (uniforms.plane == 1u) {
        // Coronal
        sample_coord = vec3<f32>(u, depth, v);
    } 
    else {
        // Sagittal
        sample_coord = vec3<f32>(depth, u, v);
    }
    
    var hounsfield_unit: f32;

    // 3. Look up the voxel
    if (uniforms.interpolation_mode == 1u) {
        // Nearest Neighbor
        let dim_u32 = textureDimensions(volume_texture);
        let dim = vec3<f32>(f32(dim_u32.x), f32(dim_u32.y), f32(dim_u32.z));
        
        let pixel_coord = sample_coord * dim;
        let clamped_pos = clamp(vec3<i32>(pixel_coord), vec3<i32>(0), vec3<i32>(dim_u32) - vec3<i32>(1));
        
        hounsfield_unit = textureLoad(volume_texture, clamped_pos, 0).r;
    } else {
        // Linear
        hounsfield_unit = textureSample(volume_texture, volume_sampler, sample_coord).r;
    }
    
    // 4. Calculate your original three windows!
    let r_bone   = apply_window(hounsfield_unit, 400.0, 1500.0);
    let g_tissue = apply_window(hounsfield_unit, 40.0, 400.0);
    let b_lung   = apply_window(hounsfield_unit, -600.0, 1500.0);

    let final_color = vec3<f32>(r_bone, g_tissue, b_lung);
    
    // 6. Gamma correction
    let gamma_corrected = pow(final_color, vec3<f32>(2.2));
    
    return vec4<f32>(gamma_corrected, 1.0);
}
