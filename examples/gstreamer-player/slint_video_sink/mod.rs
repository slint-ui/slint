// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use crate::App;
use futures::channel::mpsc::UnboundedSender;

#[cfg(slint_gstreamer_egl)]
mod egl_integration;
#[cfg(not(slint_gstreamer_egl))]
mod software_rendering;

pub fn init(
    app: &App,
    pipeline: &gst::Pipeline,
    bus_sender: UnboundedSender<gst::Message>,
) -> gst::Element {
    let new_frame_callback = |app: App, new_frame| {
        app.set_video_frame(new_frame);
    };

    #[cfg(not(slint_gstreamer_egl))]
    return software_rendering::init(app, pipeline, new_frame_callback, bus_sender);
    #[cfg(slint_gstreamer_egl)]
    return egl_integration::init(app, pipeline, new_frame_callback, bus_sender);
}
