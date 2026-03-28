// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/// SlintCore.h — Pure C declarations of Rust FFI functions for the Swift bridge.
/// These functions are exported by the slint-swift static library (libslint_swift.a).

#ifndef SLINT_CORE_H
#define SLINT_CORE_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

// ---------------------------------------------------------------------------
// Opaque types (match the #[repr(C)] structs in Rust)
// ---------------------------------------------------------------------------

/// Opaque type matching `SharedString` in Rust.
/// SharedString contains a SharedVector<u8>, which is a NonNull pointer.
typedef struct
{
    void *_0;
} SlintSharedStringOpaque;

/// Opaque type matching `WindowAdapterRcOpaque` in Rust (two pointers for Rc<dyn WindowAdapter>).
typedef struct
{
    void *_0;
    void *_1;
} SlintWindowAdapterRcOpaque;

/// Color represented as 4 bytes: red, green, blue, alpha.
/// Matches `Color` in Rust which is #[repr(C)] with 4 u8 fields.
typedef struct
{
    uint8_t red;
    uint8_t green;
    uint8_t blue;
    uint8_t alpha;
} SlintColor;

/// Size with unsigned 32-bit width and height.
/// Matches `euclid::default::Size2D<u32>`.
typedef struct
{
    uint32_t width;
    uint32_t height;
} SlintIntSize;

/// 2D point with signed 32-bit coordinates.
/// Matches `euclid::default::Point2D<i32>`.
typedef struct
{
    int32_t x;
    int32_t y;
} SlintPoint2DI32;

/// 2D point with float coordinates.
/// Matches `euclid::default::Point2D<f32>`.
typedef struct
{
    float x;
    float y;
} SlintPoint2DF32;

/// 2D size with float coordinates.
typedef struct
{
    float width;
    float height;
} SlintSizeF32;

/// Timer mode: single-shot or repeated.
/// Matches `TimerMode` in Rust (#[repr(u8)]).
typedef enum {
    SLINT_TIMER_MODE_SINGLE_SHOT = 0,
    SLINT_TIMER_MODE_REPEATED = 1,
} SlintTimerMode;

// ---------------------------------------------------------------------------
// Image opaque type
// ---------------------------------------------------------------------------

/// Opaque handle to a heap-allocated Rust Image.
/// Created by `slint_swift_image_new` or `slint_swift_image_load_from_path`,
/// freed by `slint_swift_image_drop`.
typedef struct SlintImageOpaque SlintImageOpaque;

// ---------------------------------------------------------------------------
// SharedString functions (from internal/core/string.rs)
// ---------------------------------------------------------------------------

/// Returns a nul-terminated pointer to the string's UTF-8 bytes.
/// The returned pointer is owned by the string and must not be freed.
const char *slint_shared_string_bytes(const SlintSharedStringOpaque *ss);

/// Destroys a SharedString, releasing its reference count.
void slint_shared_string_drop(const SlintSharedStringOpaque *ss);

/// Clones a SharedString (increments reference count).
/// `out` must point to uninitialized memory of size SlintSharedStringOpaque.
void slint_shared_string_clone(SlintSharedStringOpaque *out, const SlintSharedStringOpaque *ss);

/// Creates a SharedString from UTF-8 bytes. `bytes` must be valid UTF-8 of length `len`.
/// `out` must point to uninitialized memory.
void slint_shared_string_from_bytes(SlintSharedStringOpaque *out, const char *bytes, uintptr_t len);

/// Creates a SharedString from a floating-point number.
/// `out` must point to uninitialized memory.
void slint_shared_string_from_number(SlintSharedStringOpaque *out, double n);

/// Appends UTF-8 bytes to an existing SharedString.
void slint_shared_string_append(SlintSharedStringOpaque *self_, const char *bytes, uintptr_t len);

// ---------------------------------------------------------------------------
// Color functions (from internal/core/graphics/color.rs)
// ---------------------------------------------------------------------------

/// Returns a brighter version of the color.
void slint_color_brighter(const SlintColor *col, float factor, SlintColor *out);

/// Returns a darker version of the color.
void slint_color_darker(const SlintColor *col, float factor, SlintColor *out);

/// Returns a more transparent version of the color.
void slint_color_transparentize(const SlintColor *col, float factor, SlintColor *out);

/// Mixes two colors. `factor` is clamped to [0, 1].
void slint_color_mix(const SlintColor *col1, const SlintColor *col2, float factor, SlintColor *out);

/// Returns the color with a new alpha value (0.0 = transparent, 1.0 = opaque).
void slint_color_with_alpha(const SlintColor *col, float alpha, SlintColor *out);

/// Converts a color to HSVA components (each in [0, 1]).
void slint_color_to_hsva(const SlintColor *col, float *h, float *s, float *v, float *a);

/// Creates a color from HSVA components (each in [0, 1]).
SlintColor slint_color_from_hsva(float h, float s, float v, float a);

/// Creates a color from OKLCh components.
SlintColor slint_color_from_oklch(float l, float c, float h, float a);

/// Converts a color to OKLCh components.
void slint_color_to_oklch(const SlintColor *col, float *l, float *c, float *h, float *a);

// ---------------------------------------------------------------------------
// Image functions (heap-allocated via api/swift/lib.rs + internal/core/graphics/image.rs)
// ---------------------------------------------------------------------------

/// Allocates a new default (empty) Image on the heap.
/// The caller must call `slint_swift_image_drop` to free it.
SlintImageOpaque *slint_swift_image_new(void);

/// Drops a heap-allocated Image.
void slint_swift_image_drop(SlintImageOpaque *image);

/// Clones a heap-allocated Image. Returns a new heap-allocated Image.
SlintImageOpaque *slint_swift_image_clone(const SlintImageOpaque *image);

/// Loads an image from a file path. Returns a new heap-allocated Image.
SlintImageOpaque *slint_swift_image_load_from_path(const SlintSharedStringOpaque *path);

/// Returns the size of the image in pixels.
SlintIntSize slint_image_size(const SlintImageOpaque *image);

/// Returns a pointer to the image's path (as a SharedString), or NULL if no path.
const SlintSharedStringOpaque *slint_image_path(const SlintImageOpaque *image);

/// Compares two images for equality.
bool slint_image_compare_equal(const SlintImageOpaque *image1, const SlintImageOpaque *image2);

/// Sets the nine-slice edges on an image.
void slint_image_set_nine_slice_edges(SlintImageOpaque *image, uint16_t top, uint16_t right,
                                      uint16_t bottom, uint16_t left);

// ---------------------------------------------------------------------------
// Window functions (from internal/core/window.rs + api/swift/lib.rs)
// ---------------------------------------------------------------------------

/// Creates a new window adapter. `out` must point to uninitialized memory.
void slint_windowrc_init(SlintWindowAdapterRcOpaque *out);

/// Destroys a window adapter, releasing its reference count.
void slint_windowrc_drop(SlintWindowAdapterRcOpaque *handle);

/// Clones a window adapter (increments reference count).
void slint_windowrc_clone(const SlintWindowAdapterRcOpaque *source,
                          SlintWindowAdapterRcOpaque *target);

/// Shows the window.
void slint_windowrc_show(const SlintWindowAdapterRcOpaque *handle);

/// Hides the window.
void slint_windowrc_hide(const SlintWindowAdapterRcOpaque *handle);

/// Returns whether the window is visible.
bool slint_windowrc_is_visible(const SlintWindowAdapterRcOpaque *handle);

/// Returns the window size in physical pixels.
SlintIntSize slint_windowrc_size(const SlintWindowAdapterRcOpaque *handle);

/// Sets the window size in physical pixels.
void slint_windowrc_set_physical_size(const SlintWindowAdapterRcOpaque *handle,
                                      const SlintIntSize *size);

/// Sets the window size in logical pixels.
void slint_windowrc_set_logical_size(const SlintWindowAdapterRcOpaque *handle,
                                     const SlintSizeF32 *size);

/// Gets the window position in physical pixels.
void slint_windowrc_position(const SlintWindowAdapterRcOpaque *handle, SlintPoint2DI32 *pos);

/// Sets the window position in physical pixels.
void slint_windowrc_set_physical_position(const SlintWindowAdapterRcOpaque *handle,
                                          const SlintPoint2DI32 *pos);

/// Sets the window position in logical pixels.
void slint_windowrc_set_logical_position(const SlintWindowAdapterRcOpaque *handle,
                                         const SlintPoint2DF32 *pos);

/// Returns whether the window is in fullscreen mode.
bool slint_windowrc_is_fullscreen(const SlintWindowAdapterRcOpaque *handle);

/// Sets fullscreen mode.
void slint_windowrc_set_fullscreen(const SlintWindowAdapterRcOpaque *handle, bool value);

/// Returns whether the window is minimized.
bool slint_windowrc_is_minimized(const SlintWindowAdapterRcOpaque *handle);

/// Sets minimized state.
void slint_windowrc_set_minimized(const SlintWindowAdapterRcOpaque *handle, bool value);

/// Returns whether the window is maximized.
bool slint_windowrc_is_maximized(const SlintWindowAdapterRcOpaque *handle);

/// Sets maximized state.
void slint_windowrc_set_maximized(const SlintWindowAdapterRcOpaque *handle, bool value);

/// Returns the scale factor.
float slint_windowrc_get_scale_factor(const SlintWindowAdapterRcOpaque *handle);

/// Requests a redraw.
void slint_windowrc_request_redraw(const SlintWindowAdapterRcOpaque *handle);

// ---------------------------------------------------------------------------
// Timer functions (from internal/core/timers.rs)
// ---------------------------------------------------------------------------

/// Starts or restarts a timer.
/// `id` should be 0 for a new timer. Returns the timer ID.
uintptr_t slint_timer_start(uintptr_t id, SlintTimerMode mode, uint64_t duration,
                            void (*callback)(void *user_data), void *user_data,
                            void (*drop_user_data)(void *));

/// Fires a single-shot timer after `delay` milliseconds.
void slint_timer_singleshot(uint64_t delay, void (*callback)(void *user_data), void *user_data,
                            void (*drop_user_data)(void *));

/// Destroys a timer by ID.
void slint_timer_destroy(uintptr_t id);

/// Stops a timer (does not destroy it).
void slint_timer_stop(uintptr_t id);

/// Restarts a stopped timer.
void slint_timer_restart(uintptr_t id);

/// Returns whether a timer is currently running.
bool slint_timer_running(uintptr_t id);

/// Returns the timer's interval in milliseconds.
uint64_t slint_timer_interval(uintptr_t id);

// ---------------------------------------------------------------------------
// Event loop functions (from api/swift/lib.rs)
// ---------------------------------------------------------------------------

/// Ensures a backend is initialized.
void slint_ensure_backend(void);

/// Runs the event loop. Blocks until `slint_quit_event_loop` is called.
/// If `quit_on_last_window_closed` is true, quits when the last window is closed.
void slint_run_event_loop(bool quit_on_last_window_closed);

/// Quits the event loop.
void slint_quit_event_loop(void);

/// Posts an event to be executed on the main thread.
void slint_post_event(void (*event)(void *user_data), void *user_data,
                      void (*drop_user_data)(void *));

// ---------------------------------------------------------------------------
// String utility functions (from api/swift/lib.rs)
// ---------------------------------------------------------------------------

/// Parses a SharedString as a float. Returns true on success.
bool slint_string_to_float(const SlintSharedStringOpaque *string, float *value);

/// Returns the number of grapheme clusters in a SharedString.
uintptr_t slint_string_character_count(const SlintSharedStringOpaque *string);

#endif // SLINT_CORE_H
