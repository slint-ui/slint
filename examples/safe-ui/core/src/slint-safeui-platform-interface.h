// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#ifndef SLINT_SAFEUI_PLATFORM_INTERFACE
#define SLINT_SAFEUI_PLATFORM_INTERFACE

/**
 * Implement this function to suspend the current task. The function should return if one of the two
 * conditions are met:
 *
 * 1. If `max_wait_milliseconds` is positive and `max_wait_milliseconds` have elapsed since the
 * invocation, wake up and return.
 * 2. If `slint_safeui_platform_wake()` was invoked.
 *
 * In practice, this function marks to FreeRTOS' TaskNotifyTake function(s), like this:
 *
 * ```cpp
 * TickType_t ticks_to_wait = portMAX_DELAY;
 * if (max_wait_milliseconds >= 0) {
 *     ticks_to_wait = pdMS_TO_TICKS(max_wait_milliseconds);
 * }
 * ulTaskNotifyTake(pdTRUE, ticks_to_wait);
 * ```
 */
void slint_safeui_platform_wait_for_events(int max_wait_milliseconds);

/**
 * Implement this function to wake up the suspend slint task.
 *
 * With FreeRTOS, this typically maps to `vTaskNotifyGiveFromISR()`.
 */
void slint_safeui_platform_wake(void);

/**
 * Implement this function to provide Slint with temporary access to the framebuffer, for rendering.
 *
 * The framebuffer is expected to be in BGRA8888 format (blue in the lower 8 bit, alpha in the upper
 * most, etc.)
 *
 * The implementation is typically three-fold:
 *
 * 1. Obtain a pointer to the framebuffer to render into.
 * 2. Invoke `render_fn()` with the provided `user_data()`, as well as a pointer to the frame
 * buffer, the size of the buffer in bytes, as well as the number of pixels per line. Slint is
 * expected to write to all bytes of the buffer.
 * 3. Flush the framebuffer to the display.
 */
void slint_safeui_platform_render(const void *user_data,
                                  void (*render_fn)(const void *user_data, char *frame_buffer,
                                                    unsigned int buffer_size_bytes,
                                                    unsigned int pixel_stride));

/**
 * Implement this function to provide Slint with a "sense of time". This is used to driver
 * animations as well as timers.
 *
 * A FreeRTOS-based implementation is typically a two-liner:
 *
 * ```cpp
 * TickType_t ticks = xTaskGetTickCount();
 * return ticks * portTICK_PERIOD_MS;
 * ```
 */
int slint_safeui_platform_duration_since_start(void);

/**
 * Implement this function to provide Slint with the dimensions of the frame buffer in pixels.
 *
 * This function is called only once. Resizing of the frame buffer is not implemented right now.
 */
void slint_safeui_platform_get_screen_size(unsigned int *width, unsigned int *height);

/**
 * This function is provided by the `SlintSafeUi` CMake target. It's implemented in
 * [./lib.rs](./lib.rs); Invoke this from your UI task to spin the Slint event loop.
 */
void slint_app_main(void);

/**
 * Describes the phase of a touch or pointer interaction.
 *
 * The phases correspond to a typical touch sequence:
 *  - START: A new touch begins (finger or pointer pressed).
 *  - MOVE:  The touch position changes while the touch is active.
 *  - END:   The touch ends (finger lifted or pointer released).
 *
 * These phases are translated internally to Slint pointer events.
 */
typedef enum {
    START = 0,
    MOVE = 1,
    END = 2,
} TouchPhase;

/**
 * Inject a touch (or pointer) event into the Slint event loop.
 *
 * The coordinates are specified in physical display pixels. They are
 * automatically converted to logical coordinates using the scale factor
 * configured during platform initialization.
 *
 * This function is safe to call from contexts outside the Slint event loop
 * (for example from input drivers or interrupt-driven tasks). The event is
 * queued and dispatched asynchronously by the UI thread.
 *
 * @param display_x  X position in physical display pixels.
 * @param display_y  Y position in physical display pixels.
 * @param phase      The phase of the touch interaction.
 */
void slint_safeui_inject_touch_event(float display_x, float display_y, TouchPhase phase);

#endif
