<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# GStreamer Example

This example application demonstrates a way to use gstreamer (with Rust bindings) to display a video stream in Slint.

Current Status: This started as a fork of the ffmpeg example, but doesn't implement everything in the ffmpeg example yet:
* Play/Pause functionaly has not been implemented.
* The code has so far only been tested on Ubuntu.
* We use gstreamer's test source instead of streaming a video off the internet to save bandwidth and make the example more self contained.

## Building and Running

You will need to have the gstreamer libraries used by gstreamer-rs installed.

https://gstreamer.pages.freedesktop.org/gstreamer-rs/stable/latest/docs/gstreamer/

On Debian/Ubuntu you can use:

```bash
$ apt-get install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
      gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
      gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly \
      gstreamer1.0-libav libgstrtspserver-1.0-dev libges-1.0-dev
```

Once you have a working gstreamer-rs and slint install, `cargo run` should work.
