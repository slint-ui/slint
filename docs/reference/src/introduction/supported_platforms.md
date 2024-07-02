<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Supported Platforms

Slint runs on many desktop and embedded platforms and micro-controllers.

The platform descriptions below cover what has been tested for deployment. For the development environment,
we recommend using a recent desktop operating system and compiler version.

Contact [SixtyFPS GmbH](https://slint.dev/contact) if you need to support specific or older versions.

## Desktop Platforms

Generally, Slint runs on Windows, macOS, and popular Linux distributions. The following tables
cover versions that we specifically test. The general objective is to support the operating systems that
are supported by their vendors at the time of a Slint version release.

### Windows

| Operating System | Architecture |
| ---------------- | ------------ |
| Windows 10       | x86-64       |
| Windows 11       | x86-64       |

### macOS

| Operating System  | Architecture    |
| ----------------- | --------------- |
| macOS 12 Monterey | x86-64, aarch64 |
| macOS 13 Ventura  | x86-64, aarch64 |
| macOS 14 Sonoma   | x86-64, aarch64 |

### Linux

Linux desktop distribution present a diverse landscape, and Slint should run on any of them, provided that they
are using Wayland or X-Windows, glibc, and d-bus. If a Linux distribution provides Long Term Support (LTS),
Slint should run on the most recent LTS or newer, at the time of a Slint version release.

## Embedded Platforms

Slint runs on a variety of embedded platforms. Generally speaking, Slint requires a modern Linux userspace
with working OpenGL ES 2.0 (or newer) or Vulkan drivers. We've had success running Slint on

-   Yocto based distributions. For C++ applications see [meta-slint](https://github.com/slint-ui/meta-slint) for recipes. Rust application work out of the box with Yocto's rust support.
-   BuildRoot based distributions.
-   [TorizonCore](https://www.torizon.io/torizoncore-os).

### Microcontrollers

Slint's platform abstraction allows for integration into any Rust or C++ based Microcontroller development
environment. Developers need to implement functionality to feed input events such as touch or keyboard, as
well as displaying the pixels rendered by Slint into a frame- or linebuffer.
