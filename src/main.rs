//! Humanoid walk prototype entry point.
//!
//! Three things must succeed before Bevy can do anything useful: the asset
//! root must be found, the character manifest must validate, and — when one is
//! requested — the unattended-capture delay must parse. If any of them fails
//! the application still opens: it enters the terminal `Failed` state so the
//! reason is visible on screen and in the log, rather than exiting with a bare
//! process error.
//!
//! A scripted run is different. It has no one to read the overlay, so its
//! bootstrap failure must also be a nonzero exit. Setting the capture
//! environment variable *at all* marks the run as scripted, including when the
//! value itself is the thing that is wrong; the unattended systems in
//! `diagnostics` then observe `Failed` and exit nonzero instead of waiting for
//! a keypress nobody will make.

use std::{path::PathBuf, time::Duration};

use bevy::{asset::AssetPlugin, prelude::*};
use bevy_concept_world::{
    AssetRoot,
    character::CharacterPlugin,
    config::load_character_config,
    diagnostics::{DiagnosticsPlugin, capture_seconds_from_env},
    inspection::InspectionPlugin,
    resolve_asset_root,
    state::{FailureReport, PrototypeState},
};

fn main() -> AppExit {
    // Every bootstrap step is attempted before the app is built, so the first
    // failure — and only the first — is the one reported.
    let capture = capture_seconds_from_env();
    let asset_root = resolve_asset_root();

    let diagnostics = match &capture {
        Ok(Some(delay)) => DiagnosticsPlugin::unattended(*delay),
        Ok(None) => DiagnosticsPlugin::attended(),
        Err(_) => DiagnosticsPlugin::unattended(Duration::ZERO),
    };

    let (asset_path, root_note) = match &asset_root {
        Ok(AssetRoot { path, source }) => (
            path.clone(),
            format!("asset root: {} (from {source:?})", path.display()),
        ),
        // Nothing will load from it, but `AssetPlugin` still needs a path, and
        // naming the one that was tried is more useful than an empty string.
        Err(error) => (
            PathBuf::from("assets"),
            format!("asset root: unresolved ({error})"),
        ),
    };

    let config = asset_root
        .as_ref()
        .ok()
        .map(|root| load_character_config(&root.path));

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: asset_path.to_string_lossy().into_owned(),
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
    .add_plugins((InspectionPlugin, CharacterPlugin, diagnostics))
    .init_resource::<FailureReport>();

    let failure = match (&capture, &asset_root, config) {
        (Err(error), _, _) => Some(("Unattended capture request is invalid", error.to_string())),
        (_, Err(error), _) => Some(("Asset root could not be resolved", error.to_string())),
        (_, Ok(_), Some(Err(error))) => Some(("Character contract failed", error.to_string())),
        (_, Ok(_), Some(Ok(config))) => {
            app.insert_resource(config);
            None
        }
        // Unreachable: `config` is `None` only when `asset_root` is `Err`,
        // which the arm above already handled.
        (_, Ok(_), None) => Some((
            "Character contract was never attempted",
            "the asset root resolved but no manifest load was performed".to_string(),
        )),
    };

    // The state is inserted exactly once, after `DefaultPlugins` has created
    // the `StateTransition` schedule and after the outcome of bootstrap is
    // known.
    match failure {
        None => app.insert_state(PrototypeState::Loading),
        Some((summary, detail)) => {
            app.insert_resource(FailureReport::new(summary, vec![root_note, detail]));
            app.insert_state(PrototypeState::Failed)
        }
    };

    app.run()
}
