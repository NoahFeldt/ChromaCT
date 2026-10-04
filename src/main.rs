use bevy::asset::RenderAssetUsages;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, TextureDimension, TextureFormat};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{Material2d, Material2dPlugin};
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};
use bevy::window::{CursorMoved, PresentMode, PrimaryWindow, WindowPlugin, WindowResized};
use bevy::winit::{UpdateMode, WinitSettings};
use dicom::dictionary_std::tags;
use dicom::object;
use dicom::pixeldata::PixelDecoder;
use ndarray::Array3;
use rayon::prelude::*;
use bevy_framepace::{FramepacePlugin, FramepaceSettings, Limiter};

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A global resource so the engine remembers how big the CT image is
#[derive(Resource)]
struct ImageDimensions {
    width: f32,
    height: f32,
    zoom: f32,
    z_height: f32,
    slices_len: usize,
    native_cols: f32, // The actual DICOM X resolution
    native_rows: f32, // The actual DICOM Y resolution
    current_slice: usize,
}

/// Struct to organize window
#[derive(Clone, Copy)]
struct CTWindow {
    /// Level is the middle of the window
    level: f32,
    /// Width is the width of the window
    width: f32,
}

/// Stores information on 3D pixel data and physical dimensions
struct VolumeData {
    cols: usize,
    rows: usize,
    slices_len: usize,
    cube: Array3<f32>,
    z_positions: Vec<f32>,
    pixel_spacing: f32,
}

#[derive(Component)]
struct LoadDicomTask(Task<Option<VolumeData>>);

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct MprMaterial {
    #[texture(0, dimension = "3d")]
    #[sampler(1)]
    volume: Handle<Image>,

    #[uniform(2)]
    window_level: f32,
    #[uniform(2)]
    window_width: f32,
    #[uniform(2)]
    depth_fraction: f32,
    #[uniform(2)]
    interpolation_mode: u32,

    // 0 = Transverse, 1 = Coronal, 2 = Sagittal
    #[uniform(2)]
    plane: u32,
}

// Tell Bevy where to find the WGSL code for this material
impl Material2d for MprMaterial {
    fn fragment_shader() -> ShaderRef {
        // "shaders/mpr_shader.wgsl".into()
        // "shaders/false_color.wgsl".into()
        // "shaders/subtraction.wgsl".into()
        // "shaders/lazy.wgsl".into()
        // "shaders/gemini.wgsl".into()
        "shaders/addition.wgsl".into()
    }
}

fn main() {
    App::new()
        .insert_resource(WinitSettings {
            focused_mode: UpdateMode::reactive(Duration::from_millis(0)),
            unfocused_mode: UpdateMode::reactive_low_power(Duration::from_millis(200)),
        })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(bevy::window::Window {
                title: "CT Viewer".into(),
                present_mode: PresentMode::AutoNoVsync,
                // present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(Material2dPlugin::<MprMaterial>::default())
        // .add_plugins(FramepacePlugin)
        // .add_systems(Startup, (setup, setup_framepace))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                handle_loading_task,
                scroll_slices,
                adjust_to_window_resize,
                pan_camera,
                handle_keyboard_inputs,
            ),
        )
        .run();
}

// fn setup_framepace(mut settings: ResMut<FramepaceSettings>) {
//     settings.limiter = Limiter::from_framerate(60.0);
// }

fn setup(mut commands: Commands) {
    // commands.spawn((Camera2d, Tonemapping::None));

    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        Tonemapping::None,
    ));

    // let dir_path = PathBuf::from("C257/reconstructed/full_dose");
    let dir_path = PathBuf::from("ST000000/SE000001");
    // let dir_path = PathBuf::from("ST000000/SE000000");
    let thread_pool = AsyncComputeTaskPool::get();

    let task = thread_pool.spawn(async move {
        match load_dicom(&dir_path) {
            Ok(data) => Some(data),
            Err(e) => {
                eprintln!("Failed to load DICOM files: {}", e);
                None
            }
        }
    });

    commands.spawn(LoadDicomTask(task));
}

fn handle_loading_task(
    mut commands: Commands,
    mut tasks: Query<(Entity, &mut LoadDicomTask)>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<MprMaterial>>,
    mut camera_query: Query<&mut Projection, With<Camera2d>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
) {
    for (entity, mut task) in &mut tasks {
        if let Some(task_result) = block_on(poll_once(&mut task.0)) {
            // If the task was successful, unpack the volume data
            if let Some(volume_data) = task_result {
                let texture_handle = images.add(create_3d_texture(&volume_data));

                // Safely get Z boundaries, defaulting to 0.0 if empty
                let z_first = volume_data.z_positions.first().copied().unwrap_or(0.0);
                let z_last = volume_data.z_positions.last().copied().unwrap_or(0.0);
                let z_span = (z_last - z_first).abs();

                let image_height = (z_span / volume_data.pixel_spacing).round() as f32;
                let native_cols = volume_data.cols as f32;
                let native_rows = volume_data.rows as f32;

                let current_slice = (native_rows / 2.0) as usize;

                let dimensions = ImageDimensions {
                    width: native_cols,
                    height: image_height,
                    zoom: 1.0,
                    z_height: image_height,
                    slices_len: volume_data.slices_len,
                    native_cols,
                    native_rows,
                    current_slice,
                };

                let soft_window = CTWindow {
                    level: 40.0,
                    width: 400.0,
                };

                commands.spawn((
                    Mesh2d(meshes.add(Rectangle::new(1.0, 1.0))),
                    MeshMaterial2d(materials.add(MprMaterial {
                        volume: texture_handle,
                        window_level: soft_window.level,
                        window_width: soft_window.width,
                        depth_fraction: (current_slice as f32 + 0.5) / native_rows,
                        interpolation_mode: 0,
                        plane: 1, // Start Coronal
                    })),
                    Transform::from_scale(Vec3::new(native_cols, image_height, 1.0)),
                ));

                // Use our new DRY helper function!
                if let Ok(window) = window_query.single() {
                    if let Ok(mut projection) = camera_query.single_mut() {
                        fit_camera_to_window(&dimensions, window, &mut projection);
                    }
                }

                commands.insert_resource(dimensions);
            }

            commands.entity(entity).despawn();
        }
    }
}

/// Creates a 3D ndarray for slice data voxels in Hounsfield units
fn load_dicom(input_directory: &Path) -> Result<VolumeData, Box<dyn Error + Send + Sync>> {
    let entries = fs::read_dir(input_directory)?;
    let paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();

    struct SliceData {
        cols: usize,
        rows: usize,
        pixel_spacing: f32,
        z_position: f32,
        instance_number: i32, // <-- ADD THIS BACK
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

/// Packages the entire 3D array of Hounsfield units into a 3D GPU Texture
fn create_3d_texture(volume_data: &VolumeData) -> Image {
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

fn scroll_slices(
    mut mouse_wheel_events: MessageReader<MouseWheel>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    dimensions: Option<ResMut<ImageDimensions>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    mut camera_query: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
    material_query: Query<&MeshMaterial2d<MprMaterial>>,
    mut materials: ResMut<Assets<MprMaterial>>,
) {
    let mut scroll_amount = 0.0;
    for event in mouse_wheel_events.read() {
        scroll_amount += event.y;
    }
    if scroll_amount == 0.0 {
        return;
    }

    let shift_pressed =
        keyboard_input.pressed(KeyCode::ShiftLeft) || keyboard_input.pressed(KeyCode::ShiftRight);

    if shift_pressed {
        // [Keep your existing zoom logic exact as it is...]
        if let Some(mut dim) = dimensions {
            dim.zoom *= 1.1_f32.powf(-scroll_amount);
            dim.zoom = dim.zoom.clamp(0.05, 1.0);

            if let Ok(window) = window_query.single() {
                if let Ok((mut transform, mut projection)) = camera_query.single_mut() {
                    fit_camera_to_window(&dim, window, &mut projection);

                    if dim.zoom >= 1.0 {
                        transform.translation.x = 0.0;
                        transform.translation.y = 0.0;
                    }
                }
            }
        }
    } else {
        if let Ok(material_handle) = material_query.single() {
            if let Some(mut material) = materials.get_mut(material_handle) {
                if let Some(mut dim) = dimensions {
                    // Determine how many integer slices exist based on the active plane
                    let max_slices = match material.plane {
                        0 => dim.slices_len,           // Transverse
                        1 => dim.native_rows as usize, // Coronal
                        2 => dim.native_cols as usize, // Sagittal
                        _ => 1,
                    };

                    let scroll_steps = scroll_amount.signum() as isize;
                    
                    let mut new_slice = dim.current_slice as isize + scroll_steps;
                    new_slice = new_slice.clamp(0, max_slices as isize - 1);
                    
                    dim.current_slice = new_slice as usize;

                    // Convert the exact slice back into a precision float mapping for the GPU
                    material.depth_fraction = (dim.current_slice as f32 + 0.5) / max_slices as f32;

                    println!("Current slice: {}", dim.current_slice);
                }
            }
        }
    }
}

fn adjust_to_window_resize(
    mut resize_events: MessageReader<WindowResized>,
    mut camera_query: Query<&mut Projection, With<Camera2d>>,
    dimensions: Option<Res<ImageDimensions>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
) {
    for _ in resize_events.read() {
        if let Some(dim) = &dimensions {
            if let Ok(window) = window_query.single() {
                if let Ok(mut projection) = camera_query.single_mut() {
                    // DRY: Use helper
                    fit_camera_to_window(dim, window, &mut projection);
                }
            }
        }
    }
}

fn pan_camera(
    mut last_cursor_pos: Local<Option<Vec2>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut cursor_moved_events: MessageReader<CursorMoved>,
    mut camera_query: Query<(&mut Transform, &Projection), With<Camera2d>>,
) {
    let mut current_cursor_pos = *last_cursor_pos;
    for event in cursor_moved_events.read() {
        current_cursor_pos = Some(event.position);
    }

    if !mouse_buttons.pressed(MouseButton::Middle) {
        *last_cursor_pos = current_cursor_pos;
        return;
    }

    if let (Some(current), Some(last)) = (current_cursor_pos, *last_cursor_pos) {
        let delta = current - last;
        for (mut transform, projection) in &mut camera_query {
            if let Projection::Orthographic(ref ortho) = *projection {
                transform.translation.x -= delta.x * ortho.scale;
                transform.translation.y += delta.y * ortho.scale;
            }
        }
    }
    *last_cursor_pos = current_cursor_pos;
}

fn handle_keyboard_inputs(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut materials: ResMut<Assets<MprMaterial>>,
    mut mesh_query: Query<(&MeshMaterial2d<MprMaterial>, &mut Transform), Without<Camera2d>>,
    dimensions: Option<ResMut<ImageDimensions>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    mut camera_query: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    // 1. Interpolation Toggle
    if keyboard_input.just_pressed(KeyCode::KeyI) {
        if let Ok((material_handle, _)) = mesh_query.single() {
            if let Some(mut material) = materials.get_mut(material_handle) {
                material.interpolation_mode = if material.interpolation_mode == 0 {
                    1
                } else {
                    0
                };
            }
        }
    }

    // 2. Plane Switching
    let mut new_plane = None;
    if keyboard_input.just_pressed(KeyCode::Digit1) { new_plane = Some(0); } // Transverse
    if keyboard_input.just_pressed(KeyCode::Digit2) { new_plane = Some(1); } // Coronal
    if keyboard_input.just_pressed(KeyCode::Digit3) { new_plane = Some(2); } // Sagittal

    if let Some(plane_idx) = new_plane {
        if let Ok((material_handle, mut mesh_transform)) = mesh_query.single_mut() {
            if let Some(mut dim) = dimensions {
                dim.width = if plane_idx == 2 {
                    dim.native_rows
                } else {
                    dim.native_cols
                };
                dim.height = if plane_idx == 0 {
                    dim.native_rows
                } else {
                    dim.z_height
                };

                mesh_transform.scale = Vec3::new(dim.width, dim.height, 1.0);
                dim.zoom = 1.0;

                // Figure out new bounds for the chosen plane and reset to the middle slice
                let max_slices = match plane_idx {
                    0 => dim.slices_len,
                    1 => dim.native_rows as usize,
                    2 => dim.native_cols as usize,
                    _ => 1,
                };
                dim.current_slice = max_slices / 2;

                if let Some(mut material) = materials.get_mut(material_handle) {
                    material.plane = plane_idx;
                    // Apply the precise float calculated from the newly centered slice
                    material.depth_fraction = (dim.current_slice as f32 + 0.5) / max_slices as f32;
                }

                if let Ok((mut cam_transform, mut projection)) = camera_query.single_mut() {
                    cam_transform.translation.x = 0.0;
                    cam_transform.translation.y = 0.0;

                    if let Ok(window) = window_query.single() {
                        fit_camera_to_window(&dim, window, &mut projection);
                    }
                }
            }
        }
    }

    // 3. Window Presets
    let mut new_window = None;
    if keyboard_input.just_pressed(KeyCode::KeyS) {
        // Soft tissue window
        new_window = Some((40.0, 400.0));
    }
    if keyboard_input.just_pressed(KeyCode::KeyB) {
        // Bone window
        new_window = Some((400.0, 1500.0));
    }
    if keyboard_input.just_pressed(KeyCode::KeyL) {
        // Lung window
        new_window = Some((-600.0, 1500.0));
    }
    if keyboard_input.just_pressed(KeyCode::KeyH) {
        // Brain (head) window
        new_window = Some((40.0, 80.0));
    }

    if let Some((level, width)) = new_window {
        if let Ok((material_handle, _)) = mesh_query.single() {
            if let Some(mut material) = materials.get_mut(material_handle) {
                material.window_level = level;
                material.window_width = width;
            }
        }
    }
}

fn fit_camera_to_window(dim: &ImageDimensions, window: &Window, projection: &mut Projection) {
    let scale_w = dim.width / window.width();
    let scale_h = dim.height / window.height();

    if let Projection::Orthographic(ref mut ortho) = *projection {
        ortho.scale = scale_w.max(scale_h) * dim.zoom;
    }
}
