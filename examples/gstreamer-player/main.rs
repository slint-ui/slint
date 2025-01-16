// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::include_modules!();

use anyhow::{bail, Result};

use gst::prelude::*;
use gst_video::video_frame::VideoFrameExt;

fn try_gstreamer_video_frame_to_pixel_buffer(
    frame: &gst_video::VideoFrame<gst_video::video_frame::Readable>,
) -> Result<slint::SharedPixelBuffer<slint::Rgb8Pixel>> {
    match frame.format() {
        gst_video::VideoFormat::Rgb => {
            let mut slint_pixel_buffer =
                slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(frame.width(), frame.height());
            frame
                .buffer()
                .copy_to_slice(0, slint_pixel_buffer.make_mut_bytes())
                .expect("Unable to copy to slice!"); // Copies!
            Ok(slint_pixel_buffer)
        }
        _ => {
            bail!(
                "Cannot convert frame to a slint RGB frame because it is format {}",
                frame.format().to_str()
            )
        }
    }
}

fn main() {
    let app = App::new().unwrap();
    let app_weak = app.as_weak();

    gst::init().unwrap();
    let source = gst::ElementFactory::make("videotestsrc")
        .name("source")
        .property_from_str("pattern", "smpte")
        .build()
        .expect("Could not create source element.");

    let width: u32 = 1024;
    let height: u32 = 1024;

    let appsink = gst_app::AppSink::builder()
        .caps(
            &gst_video::VideoCapsBuilder::new()
                .format(gst_video::VideoFormat::Rgb)
                .width(width as i32)
                .height(height as i32)
                .build(),
        )
        .build();

    let pipeline = gst::Pipeline::with_name("test-pipeline");

    pipeline.add_many([&source, &appsink.upcast_ref()]).unwrap();
    source.link(&appsink).expect("Elements could not be linked.");

    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |appsink| {
                let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                let buffer = sample.buffer_owned().unwrap(); // Probably copies!
                let video_info =
                    gst_video::VideoInfo::builder(gst_video::VideoFormat::Rgb, width, height)
                        .build()
                        .expect("couldn't build video info!");
                let video_frame =
                    gst_video::VideoFrame::from_buffer_readable(buffer, &video_info).unwrap();
                let slint_frame = try_gstreamer_video_frame_to_pixel_buffer(&video_frame)
                    .expect("Unable to convert the video frame to a slint video frame!");

                app_weak
                    .upgrade_in_event_loop(|app| {
                        app.set_video_frame(slint::Image::from_rgb8(slint_frame))
                    })
                    .unwrap();

                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );

    let result = pipeline.set_state(gst::State::Playing);
    match result {
        Ok(_) => {
            app.set_playing(true);
        }
        Err(err) => {
            eprintln!("Failed to set pipeline state to Playing: {}", err);
        }
    }

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
                    app_weak.upgrade_in_event_loop(move |app| app.set_playing(result)).unwrap();
                }
                Err(err) => {
                    eprintln!("Failed to set pipeline state to {:?}: {}", new_state, err);
                }
            }
        }
    });

    app.run().unwrap();
}
