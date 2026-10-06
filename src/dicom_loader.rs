use dicom::dictionary_std::tags;
use dicom::object;
use dicom::pixeldata::PixelDecoder;
use ndarray::Array3;
use rayon::prelude::*;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

/// Stores information on 3D pixel data and physical dimensions
pub struct VolumeData {
    pub cols: usize,
    pub rows: usize,
    pub slices_len: usize,
    pub cube: Array3<f32>,
    pub z_positions: Vec<f32>,
    pub pixel_spacing: f32,
}

/// Creates a 3D ndarray for slice data voxels in Hounsfield units
pub fn load_dicom(input_directory: &Path) -> Result<VolumeData, Box<dyn Error + Send + Sync>> {
    let entries = fs::read_dir(input_directory)?;
    let paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();

    struct SliceData {
        cols: usize,
        rows: usize,
        pixel_spacing: f32,
        z_position: f32,
        instance_number: i32,
        raw_pixels: Vec<f32>
    }

    let mut slices: Vec<SliceData> = paths
        .into_par_iter()
        .filter_map(|path| {
            let object_file = object::open_file(&path).ok()?;

            // Extract exact native dimensions!
            let cols = object_file
                .element(tags::COLUMNS)
                .ok()?
                .to_int::<usize>()
                .ok()?;
            let rows = object_file
                .element(tags::ROWS)
                .ok()?
                .to_int::<usize>()
                .ok()?;
            let instance_number = object_file
                .element(tags::INSTANCE_NUMBER)
                .ok()?
                .to_int::<i32>()
                .ok()?;

            let z_position = object_file
                .element(tags::IMAGE_POSITION_PATIENT)
                .ok()?
                .to_multi_float32()
                .ok()?
                .get(2)
                .copied()?;
            let pixel_spacing = object_file
                .element(tags::PIXEL_SPACING)
                .ok()?
                .to_multi_float32()
                .ok()?
                .get(0)
                .copied()?;

            let matrix = object_file
                .decode_pixel_data()
                .ok()?
                .to_ndarray::<f32>()
                .ok()?;
            let (raw_pixels, _) = matrix.into_raw_vec_and_offset();

            Some(SliceData {
                cols,
                rows,
                pixel_spacing,
                z_position,
                instance_number,
                raw_pixels,
            })
        })
        .collect();

    if slices.is_empty() {
        return Err("No valid DICOM files found in directory".into());
    }

    // Safely sort the slices
    slices.sort_by_key(|a| a.instance_number);

    if slices.len() > 1 && slices[0].z_position > slices.last().unwrap().z_position {
        slices.reverse();
    }

    let slices_len = slices.len();
    let cols = slices[0].cols;
    let rows = slices[0].rows;
    let pixel_spacing = slices[0].pixel_spacing;

    let mut flat_data = Vec::with_capacity(slices_len * cols * rows);
    let mut z_positions = Vec::with_capacity(slices_len);

    for slice in slices {
        flat_data.extend(slice.raw_pixels);
        z_positions.push(slice.z_position);
    }

    let cube = Array3::from_shape_vec((slices_len, rows, cols), flat_data)?;

    Ok(VolumeData {
        cols,
        rows,
        slices_len,
        cube,
        z_positions,
        pixel_spacing,
    })
}