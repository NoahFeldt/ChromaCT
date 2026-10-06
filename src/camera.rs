use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResized};
use crate::types::ImageDimensions;

/// Spawns the 2D orthographic camera with clinical display settings
pub fn setup_camera(mut commands: Commands)
{
    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        Tonemapping::None, // linear math instead of cinematic HDR curves
    ));
}

/// Helper function to fit the CT image dimensions inside the window
pub fn fit_camera_to_window(dim: &ImageDimensions, window: &Window, projection: &mut Projection) {
    let scale_w = dim.width / window.width();
    let scale_h = dim.height / window.height();

    if let Projection::Orthographic(ref mut ortho) = *projection {
        ortho.scale = scale_w.max(scale_h) * dim.zoom;
    }
}

pub fn adjust_to_window_resize(
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
