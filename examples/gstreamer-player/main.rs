// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::include_modules!();

use futures::stream::StreamExt;
use gst::{prelude::*, MessageView};

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

    // Handle messages from the GStreamer pipeline bus.
    // For most GStreamer objects with buses, you can use `while let Some(msg) = bus.next().await`
    // inside an async closure passed to `slint::spawn_local` to read messages from the bus.
    // However, that does not work for this pipeline's bus because gst::BusStream calls
    // gst::Bus::set_sync_handler internally and gst::Bus::set_sync_handler also must be called
    // on the pipeline's bus in the egl_integration. To work around this, send messages from the
    // sync handler over an async channel, then receive them here.
    let (bus_sender, mut bus_receiver) = futures::channel::mpsc::unbounded::<gst::Message>();
    slint::spawn_local(async move {
        while let Some(msg) = bus_receiver.next().await {
            match msg.view() {
                MessageView::Eos(..) => {
                    slint::quit_event_loop().unwrap();
                    break;
                }
                MessageView::Error(err) => {
                    eprintln!(
                        "Error from {:?}: {} ({:?})",
                        err.src().map(|s| s.path_string()),
                        err.error(),
                        err.debug()
                    );
                    break;
                }
                _ => (),
            }
        }
    })
    .unwrap();

    #[cfg(not(slint_gstreamer_egl))]
    software_rendering::init(&app, &pipeline, new_frame_callback, &bus_sender)?;
    #[cfg(slint_gstreamer_egl)]
    egl_integration::init(&app, &pipeline, new_frame_callback, &bus_sender)?;

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
