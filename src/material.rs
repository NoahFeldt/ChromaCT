use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, TextureDimension, TextureFormat};
use bevy::shader::ShaderRef;
use bevy::sprite_render::Material2d;

use crate::dicom_loader::VolumeData;
use crate::types::CTWindow;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct MprMaterial {
    #[texture(0, dimension = "3d")]
    #[sampler(1)]
    pub volume: Handle<Image>,

    #[uniform(2)]
    pub window_level: f32,
    #[uniform(2)]
    pub window_width: f32,
    #[uniform(2)]
    pub depth_fraction: f32,
    #[uniform(2)]
    pub interpolation_mode: u32,

    // 0 = Transverse, 1 = Coronal, 2 = Sagittal
    #[uniform(2)]
    pub plane: u32,
}

impl MprMaterial {
    /// Ergonomic helper: updates both level and width from a CTWindow struct
    pub fn set_window(&mut self, window: CTWindow) {
        self.window_level = window.level;
        self.window_width = window.width;
    }
}

impl Material2d for MprMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/mpr_shader.wgsl".into()
    }
}

pub fn create_3d_texture(volume_data: &VolumeData) -> Image {
    let raw_bytes: Vec<u8> = volume_data
        .cube
        .iter()
        .flat_map(|&val| val.to_ne_bytes())
        .collect();

    Image::new(
        Extent3d {
            width: volume_data.cols as u32,
            height: volume_data.rows as u32,
            depth_or_array_layers: volume_data.slices_len as u32,
        },
        TextureDimension::D3,
        raw_bytes,
        TextureFormat::R32Float,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
}