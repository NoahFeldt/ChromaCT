use bevy::prelude::*;

/// A global resource so the engine remembers how big the CT image is
#[derive(Resource, Clone, Debug)]
pub struct ImageDimensions {
    pub width: f32,
    pub height: f32,
    pub zoom: f32,
    pub z_height: f32,
    pub slices_len: usize,
    pub native_cols: f32, // The actual DICOM X resolution
    pub native_rows: f32, // The actual DICOM Y resolution
    pub current_slice: usize,
}

/// Struct to organize window
#[derive(Debug, Clone, Copy)]
pub struct CTWindow {
    /// Level is the middle of the window
    pub level: f32,
    /// Width is the width of the window
    pub width: f32,
}

impl CTWindow {
    pub const fn new(level: f32, width: f32) -> Self {
        Self { level, width }
    }

    // Standard clinical CT window presets:
    pub const SOFT_TISSUE: Self = Self::new(40.0, 400.0);
    pub const BONE: Self = Self::new(400.0, 1500.0);
    pub const LUNG: Self = Self::new(-600.0, 1500.0);
    pub const BRAIN: Self = Self::new(40.0, 80.0);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewingPlane {
    #[default]
    Axial = 0,    // Transverse (looking top-down / craniocaudal)
    Coronal = 1,  // Frontal (looking front-to-back / anteroposterior)
    Sagittal = 2, // Lateral (looking side-to-side / mediolateral)
}

impl ViewingPlane {
    /// Converts the enum to the index expected by the WGSL shader
    pub fn as_u32(self) -> u32 {
        self as u32
    }

    /// Converts a u32 from the shader/material back into our type-safe enum
    pub fn from_u32(val: u32) -> Self {
        match val {
            1 => ViewingPlane::Coronal,
            2 => ViewingPlane::Sagittal,
            _ => ViewingPlane::Axial,
        }
    }
}