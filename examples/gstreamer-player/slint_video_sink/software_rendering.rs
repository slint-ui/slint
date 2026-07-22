// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use futures::channel::mpsc::UnboundedSender;
use gst::prelude::*;
use gst_video::video_frame::VideoFrameExt;

pub fn init<App: slint::ComponentHandle + 'static>(
    app: &App,
    pipeline: &gst::Pipeline,
    new_frame_callback: fn(App, slint::Image),
    bus_sender: UnboundedSender<gst::Message>,
) -> gst::Element {
    pipeline.bus().unwrap().set_sync_handler(move |_, message| {
        let _ = bus_sender.unbounded_send(message.to_owned());
        gst::BusSyncReply::Drop
    });

    let appsink = gst_app::AppSink::builder()
        .caps(&gst_video::VideoCapsBuilder::new().format(gst_video::VideoFormat::Rgb).build())
        .build();

    pipeline.set_property("video-sink", &appsink);

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
                let slint_frame = try_gstreamer_video_frame_to_pixel_buffer(&video_frame);

                app_weak
                    .upgrade_in_event_loop(move |app| {
                        new_frame_callback(app, slint::Image::from_rgb8(slint_frame))
                    })
                    .unwrap();

                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );

    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");

    appsink.into()
}

fn try_gstreamer_video_frame_to_pixel_buffer(
    frame: &gst_video::VideoFrame<gst_video::video_frame::Readable>,
) -> slint::SharedPixelBuffer<slint::Rgb8Pixel> {
    match frame.format() {
        gst_video::VideoFormat::Rgb => {
            let mut slint_pixel_buffer =
                slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(frame.width(), frame.height());
            frame
                .buffer()
                .copy_to_slice(0, slint_pixel_buffer.make_mut_bytes())
                .expect("Unable to copy to slice!"); // Copies!
            slint_pixel_buffer
        }
        _ => panic!(
            "Cannot convert frame to a slint RGB frame because it is format {}",
            frame.format().to_str()
        ),
    }
}
