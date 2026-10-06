use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::window::{CursorMoved, PrimaryWindow};

use crate::camera::fit_camera_to_window;
use crate::material::MprMaterial;
use crate::types::{CTWindow, ImageDimensions, ViewingPlane};

/// Handles middle-mouse-button panning
pub fn pan_camera(
    mut last_cursor_pos: Local<Option<Vec2>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut cursor_moved_events: MessageReader<CursorMoved>,
    mut camera_query: Query<(&mut Transform, &Projection), With<Camera2d>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    dimensions: Option<Res<ImageDimensions>>,
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
        if let (Ok(window), Some(dim)) = (window_query.single(), dimensions) {
            for (mut transform, projection) in &mut camera_query {
                if let Projection::Orthographic(ref ortho) = *projection {
                    transform.translation.x -= delta.x * ortho.scale;
                    transform.translation.y += delta.y * ortho.scale;

                    clamp_camera_position(&mut transform, projection, window, &dim);
                }
            }
        }
    }
    *last_cursor_pos = current_cursor_pos;
}

/// Clamps the camera translation so the viewport never pans past the CT image boundaries
pub fn clamp_camera_position(
    transform: &mut Transform,
    projection: &Projection,
    window: &Window,
    dim: &ImageDimensions,
) {
    if let Projection::Orthographic(ref ortho) = *projection {
        let half_image_w = dim.width / 2.0;
        let half_image_h = dim.height / 2.0;

        let half_view_w = (window.width() * ortho.scale) / 2.0;
        let half_view_h = (window.height() * ortho.scale) / 2.0;

        // If the viewport is larger than the image, max is 0.0 (stays centered)
        let max_x = (half_image_w - half_view_w).max(0.0);
        let max_y = (half_image_h - half_view_h).max(0.0);

        transform.translation.x = transform.translation.x.clamp(-max_x, max_x);
        transform.translation.y = transform.translation.y.clamp(-max_y, max_y);
    }
}

pub fn scroll_slices(
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

                    clamp_camera_position(&mut transform, &projection, window, &dim);
                }
            }
        }
    } else {
        if let Ok(material_handle) = material_query.single() {
            if let Some(mut material) = materials.get_mut(material_handle) {
                if let Some(mut dim) = dimensions {
                    // Determine how many integer slices exist based on the active plane
                    let plane = ViewingPlane::from_u32(material.plane);
                    let max_slices = match plane {
                        ViewingPlane::Axial => dim.slices_len,
                        ViewingPlane::Coronal => dim.native_rows as usize,
                        ViewingPlane::Sagittal => dim.native_cols as usize,
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

pub fn handle_keyboard_inputs(
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
    if keyboard_input.just_pressed(KeyCode::Digit1) { new_plane = Some(ViewingPlane::Axial); }
    if keyboard_input.just_pressed(KeyCode::Digit2) { new_plane = Some(ViewingPlane::Coronal); }
    if keyboard_input.just_pressed(KeyCode::Digit3) { new_plane = Some(ViewingPlane::Sagittal); }

    if let Some(plane) = new_plane {
        if let Ok((material_handle, mut mesh_transform)) = mesh_query.single_mut() {
            if let Some(mut dim) = dimensions {
                dim.width = match plane {
                    ViewingPlane::Sagittal => dim.native_rows,
                    _ => dim.native_cols,
                };
                dim.height = match plane {
                    ViewingPlane::Axial => dim.native_rows,
                    _ => dim.z_height,
                };

                mesh_transform.scale = Vec3::new(dim.width, dim.height, 1.0);
                dim.zoom = 1.0;

                let max_slices = match plane {
                    ViewingPlane::Axial => dim.slices_len,
                    ViewingPlane::Coronal => dim.native_rows as usize,
                    ViewingPlane::Sagittal => dim.native_cols as usize,
                };
                dim.current_slice = max_slices / 2;

                if let Some(mut material) = materials.get_mut(material_handle) {
                    material.plane = plane.as_u32();
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
    if keyboard_input.just_pressed(KeyCode::KeyS) { new_window = Some(CTWindow::SOFT_TISSUE); }
    if keyboard_input.just_pressed(KeyCode::KeyB) { new_window = Some(CTWindow::BONE); }
    if keyboard_input.just_pressed(KeyCode::KeyL) { new_window = Some(CTWindow::LUNG); }
    if keyboard_input.just_pressed(KeyCode::KeyH) { new_window = Some(CTWindow::BRAIN); }

    if let Some(window) = new_window {
        if let Ok((material_handle, _)) = mesh_query.single() {
            if let Some(mut material) = materials.get_mut(material_handle) {
                material.set_window(window); // <-- Beautiful single line!
            }
        }
    }
}
