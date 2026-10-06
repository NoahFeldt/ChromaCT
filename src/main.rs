mod dicom_loader;
mod camera;
mod material;
mod input;
mod types;

use dicom_loader::{load_dicom, VolumeData};
use camera::{adjust_to_window_resize, fit_camera_to_window, setup_camera};
use material::{create_3d_texture, MprMaterial};
use input::{handle_keyboard_inputs, pan_camera, scroll_slices};
use types::{CTWindow, ImageDimensions, ViewingPlane};

use bevy::prelude::*;
use bevy::sprite_render::Material2dPlugin;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};
use bevy::window::{PresentMode, PrimaryWindow, WindowPlugin};
use bevy::winit::{UpdateMode, WinitSettings};
// use bevy_framepace::{FramepacePlugin, FramepaceSettings, Limiter};

use std::path::PathBuf;
use std::time::Duration;

#[derive(Component)]
struct LoadDicomTask(Task<Option<VolumeData>>);

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
        .add_systems(Startup, (setup_camera, setup))
        // .add_systems(Startup, setup_framepace)
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

                let soft_window = CTWindow::SOFT_TISSUE;

                commands.spawn((
                    Mesh2d(meshes.add(Rectangle::new(1.0, 1.0))),
                    MeshMaterial2d(materials.add(MprMaterial {
                        volume: texture_handle,
                        window_level: soft_window.level,
                        window_width: soft_window.width,
                        depth_fraction: (current_slice as f32 + 0.5) / native_rows,
                        interpolation_mode: 0,
                        plane: ViewingPlane::Coronal.as_u32(),
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
