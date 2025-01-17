// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::include_modules!();

use gst::prelude::*;

#[cfg(slint_gstreamer_egl)]
mod egl_integration;
#[cfg(not(slint_gstreamer_egl))]
mod software_rendering;

fn main() -> anyhow::Result<()> {
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .require_opengl_es()
        .select()
        .expect("Unable to create Slint backend with OpenGL ES renderer");

    let app = App::new().unwrap();

    gst::init().unwrap();

    let pipeline = gst::ElementFactory::make("playbin")
        .property("uri", "https://gstreamer.freedesktop.org/data/media/sintel_trailer-480p.webm")
        .build()?
        .downcast::<gst::Pipeline>()
        .unwrap();

    let new_frame_callback = |app: App, new_frame| {
        app.set_video_frame(new_frame);
    };

    #[cfg(not(slint_gstreamer_egl))]
    software_rendering::init(&app, &pipeline, new_frame_callback)?;
    #[cfg(slint_gstreamer_egl)]
    egl_integration::init(&app, &pipeline, new_frame_callback)?;

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

    let _ = pipeline.set_state(gst::State::Null);

    Ok(())
}
