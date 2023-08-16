<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial -->
# FFmpeg Example

This example application demonstrates the use of ffmpeg with Rust to play back video.

## Building

On Linux, you need to install ffmpeg and alsa. For example on Debian based systems:

```bash
sudo apt-get install clang libavcodec-dev libavformat-dev libavutil-dev libavfilter-dev libavdevice-dev libasound2-dev pkg-config
```

On macOS, you can use brew:

```bash
brew install pkg-config ffmpeg
```

On Windows:

 - install [vcpkg](https://github.com/microsoft/vcpkg#quick-start-windows)
 - `vcpkg install ffmpeg --triplet x64-windows`
 - Make sure `VCPKG_ROOT` is set to where `vcpkg` is installed
 - Make sure `%VCPKG_ROOT%\installed\x64-windows\bin` is in your path
