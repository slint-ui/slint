// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::include_modules!();

use futures::stream::StreamExt;
use gst::{prelude::*, MessageView};

mod slint_video_sink;

fn main() {
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .require_opengl_es()
        .select()
        .expect("Unable to create Slint backend with OpenGL ES renderer");

    let app = App::new().unwrap();

    gst::init().unwrap();

    let pipeline = gst::ElementFactory::make("playbin")
        .property("uri", "https://gstreamer.freedesktop.org/data/media/sintel_trailer-480p.webm")
        .build()
        .unwrap()
        .downcast::<gst::Pipeline>()
        .unwrap();

    // Handle messages from the GStreamer pipeline bus.
    // For most GStreamer objects with buses, you can use `while let Some(msg) = bus.next().await`
    // inside an async closure passed to `slint::spawn_local` to read messages from the bus.
    // However, that does not work for this pipeline's bus because gst::BusStream calls
    // gst::Bus::set_sync_handler internally and gst::Bus::set_sync_handler also must be called
    // on the pipeline's bus in the egl_integration. To work around this, send messages from the
    // sync handler over an async channel, then receive them here.
    let (bus_sender, mut bus_receiver) = futures::channel::mpsc::unbounded::<gst::Message>();
    slint::spawn_local({
        // GStreamer Objects are GLib Objects, so they are reference counted. Cloning increments
        // the reference count, like cloning a std::rc::Rc.
        let pipeline = pipeline.clone();
        let app = app.as_weak();
        async move {
            while let Some(msg) = bus_receiver.next().await {
                match msg.view() {
                    MessageView::Buffering(b) => app.unwrap().set_buffering_percent(b.percent()),
                    // Only update the `playing` property of the GUI in response to GStreamer's state changing
                    // rather than updating it from GUI callbacks. This ensures that the state of the GUI stays
                    // in sync with GStreamer.
                    MessageView::StateChanged(s) => {
                        if *s.src().unwrap() == pipeline {
                            app.unwrap().set_playing(s.current() == gst::State::Playing);
                        }
                    }
                    // When the file is finished playing, close the program.
                    MessageView::Eos(..) => slint::quit_event_loop().unwrap(),
                    MessageView::Error(err) => {
                        eprintln!(
                            "Error from {:?}: {} ({:?})",
                            err.src().map(|s| s.path_string()),
                            err.error(),
                            err.debug()
                        );
                        slint::quit_event_loop().unwrap();
                    }
                    _ => (),
                }
            }
        }
    })
    .unwrap();

    // If your application needs a GStreamer pipeline that is anything more complex
    // than a single playbin element, you will need to link this gst::Element to some
    // other gst::Element in your application code.
    let _video_sink = slint_video_sink::init(&app, &pipeline, bus_sender);

    app.on_toggle_pause_play({
        let pipeline = pipeline.clone();
        move || {
            let current_state = pipeline.state(gst::ClockTime::NONE).1;
            let new_state = match current_state {
                gst::State::Playing => gst::State::Paused,
                _ => gst::State::Playing,
            };

            pipeline.set_state(new_state).unwrap_or_else(|err| {
                panic!("Failed to set pipeline state to {new_state:?}: {err}")
            });
        }
    });

    app.run().unwrap();

    let _ = pipeline.set_state(gst::State::Null);
}
