<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Qt Backend

The Qt backend uses the [Qt](https://www.qt.io) library to interact with the windowing system, for
rendering, as well as widget style for a native look and feel.

The Qt backend is only available on Linux-like operating systems.

The Qt backend only supports software rendering at the moment. That means it runs with any graphics driver,
but it does not utilize GPU hardware acceleration.

## Configuration Options

The Qt backend reads and interprets the following environment variables:

| Name               | Accepted Values | Description                                                        |
|--------------------|-----------------|--------------------------------------------------------------------|
| `SLINT_FULLSCREEN` | any value       | If this variable is set, every window is shown in fullscreen mode. |

## How To Disable the Qt Backend

By setting the `SLINT_NO_QT` environment variable when building Slint, the Qt backend won't be compiled and
no attempt will be made to find Qt on the system. This will also disable the warning stating that Qt wasn't found.
