---
<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
// cSpell: ignore linuxkms libinput libseat libudev libgbm libxkbcommon xkbcommon noseat keymap xkeyboard udevadm
title: LinuxKMS Backend
description: LinuxKMS Backend
---

The LinuxKMS backend runs only on Linux and eliminates the need for a windowing system such as Wayland or X11.
Instead it uses the following libraries and interface to render directly to the screen and react to touch, mouse,
and keyboard input.

 - OpenGL via KMS/DRI.
 - Vulkan via the Vulkan KHR Display Extension.
 - DRM dumb buffers for software rendering.
 - libinput/libudev for input event handling from mice, touch screens, or keyboards.
 - libseat for GPU and input device access without requiring root access. (optional)

## Dependencies

For compilation, pkg-config is used to determine the location of the following required system libraries:

| pkg-config package name | Package name on Debian based distros |
|-------------------------|--------------------------------------|
| `gbm`                   | `libgbm-dev`                         |
| `xkbcommon`             | `libxkbcommon-dev`                   |
| `libudev`               | `libudev-dev`                        |
| `libseat`               | `libseat-dev`                        |

:::note{Note}
If you don't have `libseat` available on your target system, then instead of selecting `backend-linuxkms`, select
`backend-linuxkms-noseat`. This variant of the LinuxKMS backend eliminates the need to have libseat installed, but
in exchange requires running the application as a user that's privileged to access all input and DRM/KMS device
files; typically that's the root user.
:::

## Renderers

The LinuxKMS backend supports different renderers. They can be explicitly selected for use through the
`SLINT_BACKEND` environment variable.

| Renderer name | Required Graphics APIs | `SLINT_BACKEND` value to select renderer                                    |
|---------------|------------------------|-----------------------------------------------------------------------------|
| FemtoVG       | OpenGL ES 2.0          | `linuxkms-femtovg`                                                          |
| Skia          | OpenGL ES 2.0, Vulkan  | `linuxkms-skia-opengl`, `linuxkms-skia-vulkan`, or `linuxkms-skia-software` |
| Software      | None                   | `linuxkms-software`                                                         |

:::note{Note}
This backend is still experimental. The backend has not undergone a great variety of testing on different devices
and there are [known issues](https://github.com/slint-ui/slint/labels/a%3Abackend-linuxkms).
:::

:::note{Note}
A mouse is supported as input device, but rendering of the mouse cursor only works with the Skia and FemtoVG renderers,
not with the Slint software renderer.
:::

## Display Selection with OpenGL or Skia Software

FemtoVG uses OpenGL, and Skia - unless Vulkan is enabled - uses OpenGL, too. Linux's direct rendering manager
(DRM) subsystem is used to configure display outputs. Slint defaults to selecting the first connected
display and configures it at either its preferred resolution (if available) or its highest. Set the `SLINT_DRM_OUTPUT`
environment variable to select a specific display. To get a list of available outputs, set `SLINT_DRM_OUTPUT`
to `list`, run your program, and observe the output.

For example, the output may look like this on a laptop with a built-in screen (eDP-1) and an externally
connected monitor (DP-3):

```
DRM Output List Requested:
eDP-1 (connected: true)
DP-1 (connected: false)
DP-2 (connected: false)
DP-3 (connected: true)
DP-4 (connected: false)
```

Setting `SLINT_DRM_OUTPUT` to `DP-3` will render on the second monitor.

To select a specific resolution and refresh rate (mode), set the `SLINT_DRM_MODE` variable. Set it to `list` and
run your program to get a list of available modes. For example the program output could look like this:

```
DRM Mode List Requested:
Index: 0 Width: 3840 Height: 2160 Refresh Rate: 60
Index: 1 Width: 3840 Height: 2160 Refresh Rate: 50
Index: 2 Width: 3840 Height: 2160 Refresh Rate: 30
Index: 3 Width: 2560 Height: 1440 Refresh Rate: 59
Index: 4 Width: 1920 Height: 1080 Refresh Rate: 60
Index: 5 Width: 1680 Height: 1050 Refresh Rate: 59
...
```

Set `SLINT_DRM_MODE` to `4` to select 1920x1080@60.

## Display Selection with Vulkan

When Skia's Vulkan feature is enabled, Skia will attempt use Vulkan's KHR Display extension to render
directly to a connected screen. Slint defaults to selecting the first connected display and configures it at
its highest available resolution and refresh rate. Set the `SLINT_VULKAN_DISPLAY` environment variable
to select a specific display. To get a list of available outputs, set `SLINT_VULKAN_DISPLAY` to `list`,
run your program, and observe the output.

For example, the output may look like this on a laptop with a built-in screen (index 0) and an externally
connected monitor (index 1):

```
Vulkan Display List Requested:
Index: 0 Name: monitor
Index: 1 Name: monitor
```

Setting `SLINT_VULKAN_DISPLAY` to `1` will render on the second monitor.

To select a specific resolution and refresh rate (mode), set the `SLINT_VULKAN_MODE` variable. Set it
to `list` and run your program to get a list of available modes. For example the program output could look like this:

```
Vulkan Mode List Requested:
Index: 0 Width: 3840 Height: 2160 Refresh Rate: 60
Index: 1 Width: 3840 Height: 2160 Refresh Rate: 50
Index: 2 Width: 3840 Height: 2160 Refresh Rate: 30
Index: 3 Width: 2560 Height: 1440 Refresh Rate: 59
Index: 4 Width: 1920 Height: 1080 Refresh Rate: 60
Index: 5 Width: 1680 Height: 1050 Refresh Rate: 59
...
```

Set `SLINT_VULKAN_MODE` to `4` to select 1920x1080@60.

## Configuring the Keyboard

By default the keyboard layout and model is assumed to be a US model and layout. Set the following
environment variables to configure support for different keyboards:

* `XKB_DEFAULT_LAYOUT`: A comma separated list of layouts (languages) to include in the keymap.
  See the layouts section in [xkeyboard-config(7)](https://manpages.debian.org/testing/xkb-data/xkeyboard-config.7.en.html) for a list of accepted language codes.
  for a list of supported layouts.
* `XKB_DEFAULT_MODEL`: The keyboard model by which to interpreter keys. See the models section in
  [xkeyboard-config(7)](https://manpages.debian.org/testing/xkb-data/xkeyboard-config.7.en.html) for a list of accepted model codes.
* `XKB_DEFAULT_VARIANT`: A comma separated list of variants, one per layout, which configures layout specific variants. See the values in parentheses in the layouts section in [xkeyboard-config(7)](https://manpages.debian.org/testing/xkb-data/xkeyboard-config.7.en.html) for a list of accepted variant codes.
* `XKB_DEFAULT_OPTIONS`: A comma separated list of options to configure layout-independent key combinations. See the
  options section in
  [xkeyboard-config(7)](https://manpages.debian.org/testing/xkb-data/xkeyboard-config.7.en.html) for a list of accepted option codes.

## Display Rotation

If your display's default orientation does not match the desired orientation of your user interface, then you can
set the `SLINT_KMS_ROTATION` environment variable to instruct Slint to rotate at rendering time. Supported values
are the rotation in degrees: `0`, `90`, `180`, and `270`.

Note that this variable merely rotates the rendering output. If you're using a touch screen attached to the same
display, then you may need to configure it to also apply a rotation on the touch events generated. For configuring
libinput's `LIBINPUT_CALIBRATION_MATRIX` see the [libinput Documentation](https://wayland.freedesktop.org/libinput/doc/latest/device-configuration-via-udev.html#static-device-configuration-via-udev)
for a list of valid values. Values can typically be set by writing them into a rules file under `/etc/udev/rules.d`.

The following example configures libinput to apply a 90 degree clockwise rotation for any attached touch screen:

```bash
echo 'ENV{LIBINPUT_CALIBRATION_MATRIX}="0 -1 1 1 0 0"' > /etc/udev/rules.d/libinput.rules
udevadm control --reload-rules
udevadm trigger
```
