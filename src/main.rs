//! Humanoid walk prototype entry point.
//!
//! The character manifest is validated before Bevy starts. If that bootstrap
//! fails the application still opens: it enters the terminal `Failed` state so
//! the reason is visible on screen and in the log, rather than exiting with a
//! bare process error.

use bevy::{asset::AssetPlugin, prelude::*};
use bevy_concept_world::{
    asset_root,
    character::CharacterPlugin,
    config::load_character_config,
    diagnostics::DiagnosticsPlugin,
    inspection::InspectionPlugin,
    state::{FailureReport, PrototypeState},
};

fn main() -> AppExit {
    let asset_root = asset_root();
    let config = load_character_config(&asset_root);

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: asset_root.to_string_lossy().into_owned(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Bevy Concept World — humanoid walk".into(),
                    resolution: (1280, 720).into(),
                    ..default()
                }),
                ..default()
            }),
    )
    .add_plugins((InspectionPlugin, CharacterPlugin, DiagnosticsPlugin))
    .init_resource::<FailureReport>();

    // The state is inserted exactly once, after `DefaultPlugins` has created
    // the `StateTransition` schedule and after the outcome of bootstrap is
    // known.
    match config {
        Ok(config) => {
            app.insert_resource(config);
            app.insert_state(PrototypeState::Loading);
        }
        Err(error) => {
            app.insert_resource(FailureReport::new(
                "Character bootstrap failed",
                vec![
                    format!("asset root: {}", asset_root.display()),
                    error.to_string(),
                ],
            ));
            app.insert_state(PrototypeState::Failed);
        }
    }

    app.run()
}
