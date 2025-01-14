// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use anyhow::bail;
use gst::prelude::*;
use gst_video::video_frame::VideoFrameExt;

use slint::ComponentHandle;

pub fn init(
    app: &crate::App,
    pipeline_builder: gst::element_factory::ElementBuilder<'_>,
) -> anyhow::Result<gst::Pipeline> {
    let appsink = gst_app::AppSink::builder()
        .caps(&gst_video::VideoCapsBuilder::new().format(gst_video::VideoFormat::Rgb).build())
        .build();

    let app_weak = app.as_weak();

    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |appsink| {
                let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                let buffer = sample.buffer_owned().unwrap(); // Probably copies!
                let caps = sample.caps().unwrap();
                let video_info =
                    gst_video::VideoInfo::from_caps(caps).expect("couldn't build video info!");
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

    let pipeline = pipeline_builder
        .property("video-sink", appsink)
        .build()?
        .downcast::<gst::Pipeline>()
        .unwrap();

    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");

    Ok(pipeline)
}

fn try_gstreamer_video_frame_to_pixel_buffer(
    frame: &gst_video::VideoFrame<gst_video::video_frame::Readable>,
) -> anyhow::Result<slint::SharedPixelBuffer<slint::Rgb8Pixel>> {
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
