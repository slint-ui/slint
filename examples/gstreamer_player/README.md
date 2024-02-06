<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# GStreamer Example

This example application demonstrates the use of gstreamer with Rust to play back video in Slint.

## Building

You will need to have the gstreamer libraries used by gstreamer-rs installed.

https://gstreamer.pages.freedesktop.org/gstreamer-rs/stable/latest/docs/gstreamer/

On Debian/Ubuntu you can use:

```bash
$ apt-get install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
      gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
      gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly \
      gstreamer1.0-libav libgstrtspserver-1.0-dev libges-1.0-dev
```
