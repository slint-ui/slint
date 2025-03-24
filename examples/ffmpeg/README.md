<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

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
 - Set`VCPKG_ROOT` to where `vcpkg` is installed
 - Add `%VCPKG_ROOT%\installed\x64-windows\bin` to your path
 - Run `vcpkg install llvm[clang,target-x86]:x64-windows`
 - Set `LIBCLANG_PATH` to where clang is installed: `%VCPKG_ROOT%\installed\x64-windows\bin`

 ![Screenshot of the FFmpeg Example on macOS](https://github.com/slint-ui/slint/assets/1486/5a1fad32-611a-478e-ab8f-576b4b4bdaf3 "FFmpeg Example")
