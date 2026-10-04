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

// This function automatically maps specific density values (HU) to specific colors
// so you can see lungs, soft tissue, and bones all at the same time.
fn get_hdr_color(hu: f32) -> vec3<f32> {
    // 1. Lung range (-1000 to -200) -> Dark Blue to Cyan
    let lung_weight = smoothstep(-1000.0, -200.0, hu);
    let lung_color = mix(vec3<f32>(0.0, 0.0, 0.15), vec3<f32>(0.0, 0.8, 1.0), lung_weight);
    
    // 2. Soft Tissue range (-100 to 250) -> Grayscale (so organs look normal)
    let tissue_weight = smoothstep(-100.0, 250.0, hu);
    let tissue_gray = mix(0.1, 0.8, tissue_weight); 
    let tissue_color = vec3<f32>(tissue_gray, tissue_gray, tissue_gray);
    
    // 3. Bone range (300 to 1500) -> Warm Orange to White
    let bone_weight = smoothstep(300.0, 1500.0, hu);
    let bone_color = mix(vec3<f32>(1.0, 0.6, 0.1), vec3<f32>(1.0, 1.0, 1.0), bone_weight);

    // Blend the zones together smoothly so there are no harsh pixelated edges
    if (hu < -150.0) {
        return lung_color;
    } else if (hu < 250.0) {
        // Smooth transition from lung cyan to tissue gray
        let blend = smoothstep(-150.0, -50.0, hu);
        return mix(lung_color, tissue_color, blend); 
    } else {
        // Smooth transition from tissue gray to bone orange
        let blend = smoothstep(250.0, 350.0, hu);
        return mix(tissue_color, bone_color, blend);
    }
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    
    // 1. Get the screen coordinates
    let u = in.uv.x;
    let v = 1.0 - in.uv.y; 
    let depth = uniforms.depth_fraction; 
    
    var sample_coord: vec3<f32>;
    
    // 2. Swizzle (swap) the X, Y, and Z axes depending on which plane we want
    if (uniforms.plane == 0u) {
        sample_coord = vec3<f32>(u, v, depth);
    }
    else if (uniforms.plane == 1u) {
        sample_coord = vec3<f32>(u, depth, v);
    } 
    else {
        sample_coord = vec3<f32>(depth, u, v);
    }
    
    var hounsfield_unit: f32;

    // 3. Look up the voxel using the coordinate
    if (uniforms.interpolation_mode == 1u) {
        // --- NEAREST NEIGHBOR ---
        let dim_u32 = textureDimensions(volume_texture);
        let dim = vec3<f32>(f32(dim_u32.x), f32(dim_u32.y), f32(dim_u32.z));
        
        let pixel_coord = sample_coord * dim;
        let clamped_pos = clamp(vec3<i32>(pixel_coord), vec3<i32>(0), vec3<i32>(dim_u32) - vec3<i32>(1));
        
        hounsfield_unit = textureLoad(volume_texture, clamped_pos, 0).r;
    } else {
        // --- LINEAR ---
        hounsfield_unit = textureSample(volume_texture, volume_sampler, sample_coord).r;
    }
    
    // 4. Apply our custom color mapping instead of standard windowing!
    let final_color = get_hdr_color(hounsfield_unit);
    
    // 5. Bypass hardware Gamma tollbooth
    let gamma_corrected = pow(final_color, vec3<f32>(2.2));
    
    return vec4<f32>(gamma_corrected, 1.0);
}