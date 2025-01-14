// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::include_modules!();

use gst::prelude::*;

#[cfg(target_os = "linux")]
mod egl_integration;
#[cfg(not(target_os = "linux"))]
mod software_rendering;

fn main() -> anyhow::Result<()> {
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .require_opengl_es()
        .select()
        .expect("Unable to create Slint backend with OpenGL ES renderer");

    let app = App::new().unwrap();

    gst::init().unwrap();

    let playbin_builder = gst::ElementFactory::make("playbin")
        .property("uri", "https://gstreamer.freedesktop.org/data/media/sintel_trailer-480p.webm");

    #[cfg(not(target_os = "linux"))]
    let pipeline = software_rendering::init(&app, playbin_builder)?;
    #[cfg(target_os = "linux")]
    let pipeline = egl_integration::init(&app, playbin_builder)?;

    let pipeline_weak_for_callback = pipeline.downgrade();
    let app_weak = app.as_weak();
    app.on_toggle_pause_play(move || {
        if let Some(pipeline) = pipeline_weak_for_callback.upgrade() {
            let current_state = pipeline.state(gst::ClockTime::NONE).1;
            let result;
            let new_state = match current_state {
                gst::State::Playing => {
                    result = false;
                    gst::State::Paused
                }
                _ => {
                    result = true;
                    gst::State::Playing
                }
            };

            // Attempt to set the state of the pipeline
            let state_result = pipeline.set_state(new_state);
            match state_result {
                Ok(_) => {
                    app_weak.unwrap().set_playing(result);
                }
                Err(err) => {
                    eprintln!("Failed to set pipeline state to {:?}: {}", new_state, err);
                }
            }
        }
    });

    app.run().unwrap();

    Ok(())
}
