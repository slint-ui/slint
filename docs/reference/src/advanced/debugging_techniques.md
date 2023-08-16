<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Debugging Techniques

On this page we're presenting different techniques and tools we've built into Slint that may help you track down different issues you may be running into, during the design and development.

## Slow Motion Animations

Animations in the user interface need to be carefully designed to have the correct duration and changes in element positioning or size need to follow a suitable curve.

In order to inspect the animations in your application, you can can set the `SLINT_SLOW_ANIMATIONS` environment variable before running the program. The variable accepts an unsigned integer value that is interpreted as a factor to globally slow down the steps of all animations, without having to make any changes to the `.slint` markup and recompiling. For example `SLINT_SLOW_ANIMATIONS=4` will slow down animations by a factor of four.

## User Interface Scaling

The use of logical pixel lengths throughout `.slint` files allows Slint to dynamically compute the correct size of physical pixels, depending on the device-pixel ratio of the screen that is reported by the windowing system. If you want to get an impression of how the individual elements look like when rendered on a screen with a different device-pixel ratio, then you can set the `SLINT_SCALE_FACTOR` environment variable before running the program. The variable accepts a floating pointer number that is used to convert logical pixel lengths to physical pixel lengths by multiplication. For example `SLINT_SCALE_FACTOR=2` will render the user interface in a way where every logical pixel will have twice the width and height.

_Note_: At the moment this overriding environment variable is only supported when using the OpenGL rendering backend.

## Performance Debugging

Slint tries its best to use hardware-acceleration to ensure that rendering the user interface uses a minimal amount of CPU resources and animations appear smooth. However depending on the complexity of the user interface, the quality of the graphics drivers or the power of the graphics acceleration in your system, you may hit limits and experience a slow down. You can set the `SLINT_DEBUG_PERFORMANCE` environment variable running the program to inspect at what rate your application is rendering frames to the screen. The variable accepts a few comma-separated options that affect how the frame rate inspection is performed and reported:

-   `refresh_lazy`: The frame rate is measured only when an actual frame is rendered, for example due to a running animation, user interface or some state change that results in a visual difference in the user interface. When nothing changes, the reported frame rate will be low. This can be useful to verify that no unnecessary repainting happens when there are no visual changes. For example a user interface that shows a text input field with a cursor that blinks once a second, the reported frames per second should be two.
-   `refresh_full_speed`: The user interface is continuously refreshed, even if nothing is changed. This will result in a higher load on the system, but can be useful to identify any bottlenecks that prevent you from achieving smooth animations.
-   `console`: The measured frame per second rate is printed to stderr on the console.
-   `overlay`: The measured frame per second rate is as an overlay text label on top of the user interface in each window.

These options are combined. At least the method of frame rate measuring and one reporting method must be specified. For example `SLINT_DEBUG_PERFORMANCE=refresh_full_speed,overlay` will repeatedly re-render the entire user interface in each window and print the achieved frame rate in the top-left corner. `SLINT_DEBUG_PERFORMANCE=refresh_lazy,console,overlay` will measure the frame rate only when something in the user interface changes and the measured value will be printed to stderr as well as rendered as an overlay text label.
