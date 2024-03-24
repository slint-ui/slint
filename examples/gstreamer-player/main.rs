// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::include_modules!();

use gst::prelude::*;
use gst_app::AppSink;

#[derive(thiserror::Error, Debug)]
enum GStreamerSlintError {
    #[error("wrong video type")]
    WrongVideoFormatError(String)
}

//  This code would be moved to a rust crate that
// TODO make width and height work dynamically without passing them
fn set_callback_that_updates_a_slint_image<F>(app_sink: &mut AppSink, callback: F, width:u32, height:u32) -> Result<(), GStreamerSlintError>
where F: FnOnce(&slint::Image) + Send
{
    app_sink.set_callbacks(
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

                let slint_frame = match video_frame.format() {
                    gst_video::VideoFormat::Rgb => {
                        let mut slint_pixel_buffer =
                            slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(video_frame.width(), video_frame.height());
                        video_frame
                            .buffer()
                            .copy_to_slice(0, slint_pixel_buffer.make_mut_bytes())
                            .expect("Unable to copy to slice!"); // Copies!
                        Ok(slint_pixel_buffer)
                    }
                    _ => {
                        Err(format!("Cannot convert frame to a slint RGB8 images because it is format {}", video_frame.format().to_str()))
                    }
                }.expect("Unable to convert the video frame to a slint video frame!");
                let slint_image = slint::Image::from_rgb8(slint_frame);
                callback(&slint_image);

                Ok(gst::FlowSuccess::Ok)
            })
            .build()
    );
    Ok(())
}



fn main() -> Result<(), GStreamerSlintError> {
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

    let mut app_sink = AppSink::builder()
        .caps(
            &gst_video::VideoCapsBuilder::new()
                .format(gst_video::VideoFormat::Rgb)
                .width(width as i32)
                .height(height as i32)
                .build(),
        )
        .build();

    let pipeline = gst::Pipeline::with_name("test-pipeline");

    pipeline.add_many([&source, &app_sink.upcast_ref()]).unwrap();
    source.link(&app_sink).expect("Elements could not be linked.");


    // Library user has to write this closure because app and set_video_frame are both codegenned.
    // Captures app_weak
    let set_video_frame = |image: & slint::Image| {
        // app.set_video_frame(image);
        app_weak
            .upgrade_in_event_loop(|app| {
                app.set_video_frame(*image)
            })
            .unwrap();
    };
    set_callback_that_updates_a_slint_image(&mut app_sink, set_video_frame, 256, 256)?;

    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");

    app.run().unwrap();
    Ok(())
}
