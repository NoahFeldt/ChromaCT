use bevy::prelude::*;
use bevy::tasks::{block_on, poll_once, Task};
use bevy::window::PrimaryWindow;

use crate::camera::fit_camera_to_window;
use crate::dicom_loader::VolumeData;
use crate::material::{create_3d_texture, MprMaterial};
use crate::types::{CTVolumeMesh, CTWindow, ImageDimensions, ViewingPlane};

#[derive(Component)]
pub struct LoadDicomTask(pub Task<Option<VolumeData>>);

pub fn handle_loading_task(
    mut commands: Commands,
    mut tasks: Query<(Entity, &mut LoadDicomTask)>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<MprMaterial>>,
    mut camera_query: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    mut volume_query: Query<(Entity, &MeshMaterial2d<MprMaterial>, &mut Transform), (With<CTVolumeMesh>, Without<Camera2d>)>,
) {
    for (task_entity, mut task) in &mut tasks {
        if let Some(task_result) = block_on(poll_once(&mut task.0)) {
            if let Some(volume_data) = task_result {
                let texture_handle = images.add(create_3d_texture(&volume_data));

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

                // Check if a CT volume quad already exists in the world
                let mut existing_meshes = volume_query.iter_mut();
                if let Some((_, material_handle, mut transform)) = existing_meshes.next() {
                    // --- REUSE EXISTING QUAD ---
                    transform.scale = Vec3::new(native_cols, image_height, 1.0);

                    if let Some(mut material) = materials.get_mut(material_handle) {
                        material.volume = texture_handle;
                        material.set_window(soft_window);
                        material.depth_fraction = (current_slice as f32 + 0.5) / native_rows;
                        material.plane = ViewingPlane::Coronal.as_u32();
                        material.interpolation_mode = 0;
                        material.color_mode = 0; // Reset to Grayscale
                    }

                    // Clean up any stray extra entities if there were any
                    for (extra_entity, _, _) in existing_meshes {
                        commands.entity(extra_entity).despawn();
                    }
                } else {
                    // --- FIRST LOAD: SPAWN IT ---
                    commands.spawn((
                        CTVolumeMesh,
                        Mesh2d(meshes.add(Rectangle::new(1.0, 1.0))),
                        MeshMaterial2d(materials.add(MprMaterial {
                            volume: texture_handle,
                            window_level: soft_window.level,
                            window_width: soft_window.width,
                            depth_fraction: (current_slice as f32 + 0.5) / native_rows,
                            interpolation_mode: 0,
                            plane: ViewingPlane::Coronal.as_u32(),
                            color_mode: 0, // Default to Grayscale
                        })),
                        Transform::from_scale(Vec3::new(native_cols, image_height, 1.0)),
                    ));
                }

                // Reset camera to center and fit the new volume
                if let Ok(window) = window_query.single() {
                    if let Ok((mut cam_transform, mut projection)) = camera_query.single_mut() {
                        cam_transform.translation.x = 0.0;
                        cam_transform.translation.y = 0.0;
                        fit_camera_to_window(&dimensions, window, &mut projection);
                    }
                }

                commands.insert_resource(dimensions);
            }

            commands.entity(task_entity).despawn();
        }
    }
}