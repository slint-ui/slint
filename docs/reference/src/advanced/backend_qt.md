<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Qt Backend

The Qt backend uses the [Qt](https://www.qt.io) library to interact with the windowing system, for
rendering, as well as widget style for a native look and feel.

The Qt backend supports practically all relevant operating systems and windowing systems, including
macOS, Windows, Linux with Wayland and X11, and direct full-screen rendering via KMS or proprietary drivers.

The Qt backend only supports software rendering at the moment. That means it runs with any graphics driver,
but it does not utilize GPU hardware acceleration.

The compilation step will detect whether Qt is installed or not using the qttype crate.
See the instructions in the [qttypes documentation](https://docs.rs/qttypes/latest/qttypes/#finding-qt)
on how to set environment variables to point to the Qt installation.

If Qt is not installed, the backend will be disabled, and Slint will fallback to another backend, usually the [Winit backend](backend_winit.md).

## Configuration Options

The Qt backend reads and interprets the following environment variables:

| Name               | Accepted Values | Description                                                        |
|--------------------|-----------------|--------------------------------------------------------------------|
| `SLINT_FULLSCREEN` | any value       | If this variable is set, every window is shown in fullscreen mode. |
