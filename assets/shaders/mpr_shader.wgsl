#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var volume_texture: texture_3d<f32>;
@group(2) @binding(1) var volume_sampler: sampler;

struct MprUniforms {
    window_level: f32,
    window_width: f32,
    depth_fraction: f32,
    interpolation_mode: u32,
    plane: u32,
    color_mode: u32, // 0 = Standard Grayscale, 1 = Multi-Window False Color
};
@group(2) @binding(2) var<uniform> uniforms: MprUniforms;

fn apply_window(hu: f32, level: f32, width: f32) -> f32 {
    let lower_bound = level - (width / 2.0);
    return clamp((hu - lower_bound) / width, 0.0, 1.0);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // 1. Screen coordinates
    let u = in.uv.x;
    let v = 1.0 - in.uv.y; 
    let depth = uniforms.depth_fraction; 

    var sample_coord: vec3<f32>;

    // 2. Swizzle axes depending on viewing plane
    if (uniforms.plane == 0u) {
        // Transverse / Axial
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

    // 3. Voxel sampling
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

    var final_color: vec3<f32>;

    // 1. True clinical grayscale luminosity
    let base_grayscale = apply_window(hounsfield_unit, uniforms.window_level, uniforms.window_width);

    if (uniforms.color_mode == 1u) {
        // 2. Your Narrower Chromaticity Window!
        let c = 0.42; // Tweak between 0.3 (super punchy) and 0.6 (wider)
        let chroma_width = uniforms.window_width * c;
        let chroma_lower = uniforms.window_level - (chroma_width / 2.0);
        let chroma_norm = clamp((hounsfield_unit - chroma_lower) / chroma_width, 0.0, 1.0);

        // 3. Inverted hue mapped across this tighter window
        let inverted_hue = (1.0 - chroma_norm) * 0.833;
        let raw_color = hue_to_rgb(inverted_hue);

        // 4. Background & Air Mask:
        // Ensures ambient air (< -900 HU) stays pitch black in Lung Window,
        // and dark tissues stay black in Bone Window
        let air_cutoff = smoothstep(-950.0, -800.0, hounsfield_unit);
        let lum_mask = smoothstep(0.01, 0.08, base_grayscale);
        let effective_saturation = 0.8 * air_cutoff * lum_mask;

        // 5. Paint chromaticity while preserving base luminosity
        final_color = apply_chromaticity(raw_color, base_grayscale, effective_saturation);
    } else {
        // Standard Grayscale
        final_color = vec3<f32>(base_grayscale, base_grayscale, base_grayscale);
    }

    let gamma_corrected = pow(final_color, vec3<f32>(2.2));
    return vec4<f32>(gamma_corrected, 1.0);
}


// Transfers chromaticity from a color source onto a target grayscale luminance
fn apply_chromaticity(color: vec3<f32>, target_lum: f32, saturation: f32) -> vec3<f32> {
    // Perceptual luminance weights (Rec. 709)
    let weights = vec3<f32>(0.2126, 0.7152, 0.0722);
    let color_lum = dot(color, weights);
    
    // Extract pure chromaticity (zero-luminance color offset)
    let chroma = color - vec3<f32>(color_lum);
    
    // Anchor chromaticity directly to the CT grayscale intensity
    let result = vec3<f32>(target_lum) + chroma * saturation;
    
    return clamp(result, vec3<f32>(0.0), vec3<f32>(1.0));
}

// Converts a normalized hue angle [0.0, 1.0] to an RGB vector
fn hue_to_rgb(hue: f32) -> vec3<f32> {
    let r = abs(hue * 6.0 - 3.0) - 1.0;
    let g = 2.0 - abs(hue * 6.0 - 2.0);
    let b = 2.0 - abs(hue * 6.0 - 4.0);
    return clamp(vec3<f32>(r, g, b), vec3<f32>(0.0), vec3<f32>(1.0));
}

// Maps Hounsfield Units (-1000 to +1000) to a continuous Rainbow spectrum
fn hu_to_rainbow(hu: f32) -> vec3<f32> {
    // 1. Clamp and normalize HU into a 0.0 to 1.0 range
    let norm = clamp((hu - (-150.0)) / (150.0 - (-150.0)), 0.0, 1.0);
    
    // 2. Scale to 0.833 so it ends at Purple/Magenta without looping back into Red
    let hue = (1.0 - norm) * 0.833;

    return hue_to_rgb(hue);
}