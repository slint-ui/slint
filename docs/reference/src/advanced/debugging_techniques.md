<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Debugging Techniques

On this page we share different techniques and tools we've built into Slint that help you track down different issues you may be running into, during the design and development.

## Debugging Property Values

Use the [`debug()`](../language/builtins/functions.md#debug) function to print the values of properties to stderr.

## Slow Motion Animations

Animations in the user interface need to be carefully designed to have the correct duration and changes in element positioning or size need to follow an easing curve.

To inspect the animations in your application, set the `SLINT_SLOW_ANIMATIONS` environment variable before running the program. This variable accepts an unsigned integer value that is the factor by which to globally slow down the steps of all animations, automatically. This means that you don't have to make any manual changes to the `.slint` markup and recompile. For example,`SLINT_SLOW_ANIMATIONS=4` slows down animations by a factor of four.

## User Interface Scaling

The use of logical pixel lengths throughout `.slint` files lets Slint compute the number of physical pixels, dynamically, depending on the device-pixel ratio of the screen. To get an impression of how the individual elements look like when rendered on a screen with a different device-pixel ratio, set the `SLINT_SCALE_FACTOR` environment variable before running the program. This variable accepts a floating pointer number that is used to convert logical pixel lengths to physical pixel lengths. For example, `SLINT_SCALE_FACTOR=2` renders the user interface in a way where every logical pixel has twice the width and height.

_Note_: Currently, only the FemtoVG and Skia renderers support this environment variable.

## Debugging for Performance Improvements

Slint attempts to use hardware-acceleration to ensure that rendering the user interface consumes a minimal amount of CPU resources while maintaining smooth animations. However, depending on the complexity of the user interface, quality of the graphics drivers, or the power of the GPU in your system, you may hit limits and experience slowness. To address this
issue, set the `SLINT_DEBUG_PERFORMANCE` environment variable before running the program, to inspect the frame rate. The following options affect the frame rate inspection and reporting:

-   `refresh_lazy`: The frame rate is measured only when an actual frame is rendered, for example due to a running animation, user interaction, or some other state change that results in a visual difference in the user interface. If
there is no change, a low frame rate is reported. Use this option to verify that no unnecessary repainting happens when there are no visual changes. For example, in a user interface that shows a text input field with a cursor that blinks once per second, the reported frame rate should be two.
-   `refresh_full_speed`: The user interface is continuously refreshed, even if nothing is changed. This continuous refresh results in a higher load on the system. Use this option to identify any bottlenecks that prevent you from achieving smooth animations. Also disables partial rendering with the software renderer.
-   `console`: The frame rate is printed to `stderr` on the console.
-   `overlay`: The frame rate is as an overlay text label on top of the user interface in each window.

Use these options in combination, separated by a comma. You must select a combination of one frame rate measurement method and a reporting method. For example, `SLINT_DEBUG_PERFORMANCE=refresh_full_speed,overlay` repeatedly re-renders the entire user interface in each window and prints the achieved frame rate in the top-left corner. In comparison, `SLINT_DEBUG_PERFORMANCE=refresh_lazy,console,overlay` measures the frame rate only when something in the user interface changes and the measured value is printed to `stderr` as well as rendered as an overlay text label.

The environment variable must be set before running the program. If the application runs on a microcontroller without the standard library, the environment variable must be set during compilation.

## Tuning Rendering Performance

If you're not satisfied with the performance, it might be worthwhile to descend into a low-level investigation. Tools such as [RenderDoc](https://renderdoc.org) permit recording the rendering output
of your application and can give you detailed insight into the OpenGL/Vulkan commands Slint's renderers produce for your user interface.

As a general rule of thumb, it's best to minimize the number of commands per frame. With OpenGL you'll see `glDrawArrays()` and `glDrawElementsInstanced*` calls filling the color buffers. Reducing
the number of calls tends to improve performance.

For example, if you draw a series of bar charts by filling rectangles with a gradient, you'll observe that each rectangle is a draw call on its own:

```slint
component BarChart inherits Rectangle {
    background: black;
    height: 150px;
    HorizontalLayout {
        spacing: 10px;
        for i in 5: Rectangle {
            height: 10px + i * 20px;
            width: 20px;
            background: @linear-gradient(180deg, #f00 0%, #0f0);
        }
    }
}
```

For example when using the Skia renderer, if you replace the gradient with a plain color, the calls will all be batched together into one call. But when the
UI design requires the use of a gradient, that's not possible. But there might be another way. If the underlying data for your bar chart rarely changes, it
might be worthwhile to render the entire chart once into a texture and therefore replace multiple calls with just one. This can be done by setting the
[cache-rendering-hint](../language/builtins/elements.md#miscellaneous) to `true` on the `BarChar` itself, which surrounds and captures the individual bar charts.

Note that extensive use of this technique comes at the expense of increased GPU memory usage and memory throughput.

