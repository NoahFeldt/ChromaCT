use bevy::prelude::*;
use bevy::sprite_render::Material2dPlugin;
use bevy::window::{PresentMode, WindowPlugin};
use bevy::winit::{UpdateMode, WinitSettings};
// use bevy_framepace::{FramepacePlugin, FramepaceSettings, Limiter};

use std::time::Duration;

mod dicom_loader;
mod camera;
mod material;
mod input;
mod types;
mod viewer;

use camera::{adjust_to_window_resize, setup_camera};
use material::MprMaterial;
use input::{handle_drag_and_drop, handle_keyboard_inputs, pan_camera, scroll_slices};
use viewer::handle_loading_task;

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
        .add_systems(Startup, setup_camera)
        // .add_systems(Startup, setup_framepace)
        .add_systems(
            Update,
            (
                handle_loading_task,
                handle_drag_and_drop,
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
