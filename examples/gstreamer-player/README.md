<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# GStreamer Example

This example application demonstrates a way to use GStreamer (with Rust bindings) to display a video stream in Slint and
communicate state changes between Slint and GStreamer. On Linux, this can take advantage of hardware accelerated rendering
and transfer the video to Slint via EGL. On other platforms, the video gets transferred
via CPU accessible buffers.

Current Status:
* The code has so far only been tested on Ubuntu and Windows.

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

On Opensuse you can use:

```bash
$ zypper in zypper in gstreamer-plugins-bad-devel gstreamer-devel gstreamer-plugins-base-devel \
      gstreamer-plugins-good
```

On windows:
- Install gstreamer using [official binaries](https://gstreamer.freedesktop.org/data/pkg/windows/) (we need to install both, e.g. `gstreamer-1.0-msvc-x86_64-1.24.11.msi` and `gstreamer-1.0-devel-msvc-x86_64-1.24.11.msi`), make sure to install full gstreamer in installer.
- And export it to path:
```bash
# For a UNIX-style shell:
$ export PATH="c:/gstreamer/1.0/msvc_x86_64/bin${PATH:+:$PATH}"

# For cmd.exe:
$ set PATH=C:\gstreamer\1.0\msvc_x86_64\bin;%PATH%
```


Once you have a working gstreamer-rs and slint install, `cargo run` should work.
