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

/// Opaque handle to a property. Matches `PropertyHandle` in Rust (Cell<usize> = one pointer-sized
/// word).
typedef struct
{
    uintptr_t _0;
} SlintPropertyHandleOpaque;

/// Opaque handle to a callback. Matches `Callback<()>` in Rust (two pointers).
typedef struct
{
    void *_0;
    void *_1;
} SlintCallbackOpaque;

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
// Testing backend (available when `backend-testing` feature is enabled)
// ---------------------------------------------------------------------------

/// Initializes the headless testing backend. Call before creating component
/// instances in environments without a display server (e.g. CI).
void slint_swift_testing_init(void);

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
// Property functions (from internal/core/properties/ffi.rs)
// ---------------------------------------------------------------------------

/// Initialises a property handle. `out` must point to uninitialised memory.
void slint_property_init(SlintPropertyHandleOpaque *out);

/// Destroys a property handle.
void slint_property_drop(SlintPropertyHandleOpaque *handle);

/// Must be called before reading the value. If a binding is set it is evaluated and the result
/// written into `val`. Also registers the current evaluation context as a dependent.
void slint_property_update(const SlintPropertyHandleOpaque *handle, void *val);

/// Notifies the property system that the value at `value` has changed, removes any active binding,
/// and marks dependents as dirty.
void slint_property_set_changed(const SlintPropertyHandleOpaque *handle, const void *value);

/// Sets a binding on the property. `binding(user_data, pointer_to_value)` is called to recompute
/// the value; it must write the new value into `pointer_to_value`. `drop_user_data` is called when
/// the binding is removed. `intercept_set` and `intercept_set_binding` may be NULL.
void slint_property_set_binding(const SlintPropertyHandleOpaque *handle,
                                void (*binding)(void *user_data, void *pointer_to_value),
                                void *user_data, void (*drop_user_data)(void *),
                                bool (*intercept_set)(void *user_data,
                                                      const void *pointer_to_value),
                                bool (*intercept_set_binding)(void *user_data, void *new_binding));

// ---------------------------------------------------------------------------
// Callback functions (from internal/core/callbacks.rs)
// ---------------------------------------------------------------------------

/// Initialises a callback. `out` must point to uninitialised memory.
void slint_callback_init(SlintCallbackOpaque *out);

/// Destroys a callback.
void slint_callback_drop(SlintCallbackOpaque *handle);

/// Sets the handler for the callback. `handler(user_data, arg, ret)` is called when the callback
/// is invoked. `drop_user_data` is called when the handler is replaced or the callback is
/// destroyed. `arg` and `ret` are opaque pointers to the argument and return value.
void slint_callback_set_handler(const SlintCallbackOpaque *handle,
                                void (*handler)(void *user_data, const void *arg, void *ret),
                                void *user_data, void (*drop_user_data)(void *));

/// Invokes the callback, passing `arg` and writing the return value into `ret`.
void slint_callback_call(const SlintCallbackOpaque *handle, const void *arg, void *ret);

// ---------------------------------------------------------------------------
// String utility functions (from api/swift/lib.rs)
// ---------------------------------------------------------------------------

/// Parses a SharedString as a float. Returns true on success.
bool slint_string_to_float(const SlintSharedStringOpaque *string, float *value);

/// Returns the number of grapheme clusters in a SharedString.
uintptr_t slint_string_character_count(const SlintSharedStringOpaque *string);

// ---------------------------------------------------------------------------
// Interpreter types
// These declarations are only used when the `interpreter` Rust feature is enabled.
// ---------------------------------------------------------------------------

/// Discriminant for SlintValue (matches ValueType in Rust, repr(i8)).
typedef enum {
    SLINT_VALUE_TYPE_VOID = 0,
    SLINT_VALUE_TYPE_NUMBER = 1,
    SLINT_VALUE_TYPE_STRING = 2,
    SLINT_VALUE_TYPE_BOOL = 3,
    SLINT_VALUE_TYPE_MODEL = 4,
    SLINT_VALUE_TYPE_STRUCT = 5,
    SLINT_VALUE_TYPE_BRUSH = 6,
    SLINT_VALUE_TYPE_IMAGE = 7,
    SLINT_VALUE_TYPE_OTHER = -1,
} SlintValueType;

/// Diagnostic severity level.
typedef enum {
    SLINT_DIAGNOSTIC_LEVEL_ERROR = 0,
    SLINT_DIAGNOSTIC_LEVEL_WARNING = 1,
    SLINT_DIAGNOSTIC_LEVEL_NOTE = 2,
} SlintDiagnosticLevel;

/// Opaque heap-allocated Value. Created by `slint_swift_value_*` functions.
/// Freed by `slint_swift_value_drop`.
typedef struct SlintValueOpaque SlintValueOpaque;

/// Opaque heap-allocated Struct. Created by `slint_swift_struct_new`.
/// Freed by `slint_swift_struct_drop`.
typedef struct SlintInterpreterStructOpaque SlintInterpreterStructOpaque;

/// Opaque heap-allocated ComponentCompiler.
typedef struct SlintCompilerOpaque SlintCompilerOpaque;

/// Opaque heap-allocated ComponentDefinition.
typedef struct SlintComponentDefinitionOpaque SlintComponentDefinitionOpaque;

/// Opaque heap-allocated ComponentInstance.
typedef struct SlintComponentInstanceOpaque SlintComponentInstanceOpaque;

// ---------------------------------------------------------------------------
// Interpreter — Value functions (slint_swift_value_*)
// ---------------------------------------------------------------------------

/// Creates a void Value on the heap.
SlintValueOpaque *slint_swift_value_new_void(void);

/// Creates a number Value on the heap.
SlintValueOpaque *slint_swift_value_new_double(double d);

/// Creates a bool Value on the heap.
SlintValueOpaque *slint_swift_value_new_bool(bool b);

/// Creates a string Value on the heap from UTF-8 bytes.
SlintValueOpaque *slint_swift_value_new_string(const char *bytes, uintptr_t len);

/// Creates a Value wrapping a clone of `stru` on the heap.
SlintValueOpaque *slint_swift_value_new_struct(const SlintInterpreterStructOpaque *stru);

/// Clones a Value on the heap.
SlintValueOpaque *slint_swift_value_clone(const SlintValueOpaque *val);

/// Frees a heap-allocated Value.
void slint_swift_value_drop(SlintValueOpaque *val);

/// Returns the type discriminant of a Value.
SlintValueType slint_swift_value_type(const SlintValueOpaque *val);

/// Copies the number out of a Value. Returns true on success.
bool slint_swift_value_to_double(const SlintValueOpaque *val, double *out);

/// Copies the bool out of a Value. Returns true on success.
bool slint_swift_value_to_bool(const SlintValueOpaque *val, bool *out);

/// Writes the UTF-8 string data pointer and byte length. Returns true on success.
bool slint_swift_value_to_string(const SlintValueOpaque *val, const char **out_ptr,
                                 uintptr_t *out_len);

/// Returns a heap-allocated clone of the Struct inside a Value, or NULL if not a struct.
/// The caller must free it with `slint_swift_struct_drop`.
SlintInterpreterStructOpaque *slint_swift_value_to_struct(const SlintValueOpaque *val);

// ---------------------------------------------------------------------------
// Interpreter — Struct functions (slint_swift_struct_*)
// ---------------------------------------------------------------------------

/// Creates a heap-allocated default Struct.
SlintInterpreterStructOpaque *slint_swift_struct_new(void);

/// Clones a heap-allocated Struct.
SlintInterpreterStructOpaque *slint_swift_struct_clone(const SlintInterpreterStructOpaque *stru);

/// Frees a heap-allocated Struct.
void slint_swift_struct_drop(SlintInterpreterStructOpaque *stru);

/// Returns a heap-allocated clone of the named field, or NULL if absent.
SlintValueOpaque *slint_swift_struct_get_field(const SlintInterpreterStructOpaque *stru,
                                               const char *name, uintptr_t name_len);

/// Sets the named field on `stru` to a clone of `value`.
void slint_swift_struct_set_field(SlintInterpreterStructOpaque *stru, const char *name,
                                  uintptr_t name_len, const SlintValueOpaque *value);

/// Returns the number of fields in the struct.
uintptr_t slint_swift_struct_field_count(const SlintInterpreterStructOpaque *stru);

/// Writes the name of field at `index` into `out_ptr`/`out_len`.
/// Returns true if `index` is valid.
bool slint_swift_struct_field_name_at(const SlintInterpreterStructOpaque *stru, uintptr_t index,
                                      const char **out_ptr, uintptr_t *out_len);

// ---------------------------------------------------------------------------
// Interpreter — ComponentCompiler functions (slint_swift_compiler_*)
// ---------------------------------------------------------------------------

/// Creates a heap-allocated ComponentCompiler. The caller must call `slint_swift_compiler_drop`.
SlintCompilerOpaque *slint_swift_compiler_new(void);

/// Frees a heap-allocated ComponentCompiler.
void slint_swift_compiler_drop(SlintCompilerOpaque *compiler);

/// Sets the style name for the compiler.
void slint_swift_compiler_set_style(SlintCompilerOpaque *compiler, const char *style,
                                    uintptr_t style_len);

/// Compiles `.slint` source code. Returns a heap-allocated ComponentDefinition on success,
/// or NULL on failure. The caller must call `slint_swift_definition_drop` to free it.
SlintComponentDefinitionOpaque *
slint_swift_compiler_build_from_source(SlintCompilerOpaque *compiler, const char *source,
                                       uintptr_t source_len, const char *path, uintptr_t path_len);

/// Returns the number of diagnostics from the last compilation.
uintptr_t slint_swift_compiler_diagnostics_count(const SlintCompilerOpaque *compiler);

/// Returns true if any diagnostic has error level.
bool slint_swift_compiler_has_errors(const SlintCompilerOpaque *compiler);

/// Reads diagnostic fields at `index`. Returns true if `index` is valid.
bool slint_swift_compiler_get_diagnostic(const SlintCompilerOpaque *compiler, uintptr_t index,
                                         const char **message_ptr, uintptr_t *message_len,
                                         const char **file_ptr, uintptr_t *file_len,
                                         uintptr_t *line, uintptr_t *column,
                                         SlintDiagnosticLevel *level);

// ---------------------------------------------------------------------------
// Interpreter — ComponentDefinition functions (slint_swift_definition_*)
// ---------------------------------------------------------------------------

/// Clones a heap-allocated ComponentDefinition.
SlintComponentDefinitionOpaque *
slint_swift_definition_clone(const SlintComponentDefinitionOpaque *def);

/// Frees a heap-allocated ComponentDefinition.
void slint_swift_definition_drop(SlintComponentDefinitionOpaque *def);

/// Returns the component name as a newly created SharedString.
/// The caller must call `slint_shared_string_drop` on `name_out`.
void slint_swift_definition_name(const SlintComponentDefinitionOpaque *def,
                                 SlintSharedStringOpaque *name_out);

/// Returns the number of public properties.
uintptr_t slint_swift_definition_properties_count(const SlintComponentDefinitionOpaque *def);

/// Writes the name and type of property at `index`. Returns true if `index` is valid.
/// The caller must call `slint_shared_string_drop` on `name_out` on success.
bool slint_swift_definition_property_at(const SlintComponentDefinitionOpaque *def, uintptr_t index,
                                        SlintSharedStringOpaque *name_out,
                                        SlintValueType *type_out);

/// Returns the number of public callbacks.
uintptr_t slint_swift_definition_callbacks_count(const SlintComponentDefinitionOpaque *def);

/// Writes the name of callback at `index` as a SharedString into `name_out`.
/// Returns true if `index` is valid.
/// The caller must call `slint_shared_string_drop` on `name_out` on success.
bool slint_swift_definition_callback_at(const SlintComponentDefinitionOpaque *def, uintptr_t index,
                                        SlintSharedStringOpaque *name_out);

/// Creates a heap-allocated ComponentInstance from this definition.
/// Returns NULL if creation fails. The caller must call `slint_swift_instance_drop`.
SlintComponentInstanceOpaque *
slint_swift_definition_create_instance(const SlintComponentDefinitionOpaque *def);

// ---------------------------------------------------------------------------
// Interpreter — ComponentInstance functions (slint_swift_instance_*)
// ---------------------------------------------------------------------------

/// Frees a heap-allocated ComponentInstance.
void slint_swift_instance_drop(SlintComponentInstanceOpaque *inst);

/// Shows or hides the component window.
void slint_swift_instance_show(const SlintComponentInstanceOpaque *inst, bool visible);

/// Returns a heap-allocated Value for the named property, or NULL on failure.
SlintValueOpaque *slint_swift_instance_get_property(const SlintComponentInstanceOpaque *inst,
                                                    const char *name, uintptr_t name_len);

/// Sets the named property. Returns true on success.
bool slint_swift_instance_set_property(const SlintComponentInstanceOpaque *inst, const char *name,
                                       uintptr_t name_len, const SlintValueOpaque *value);

/// Invokes a callback or function with the given argument array.
/// `args` is an array of `SlintValueOpaque*` pointers of length `args_count`.
/// Returns a heap-allocated Value on success, NULL on failure.
SlintValueOpaque *slint_swift_instance_invoke(const SlintComponentInstanceOpaque *inst,
                                              const char *name, uintptr_t name_len,
                                              const SlintValueOpaque *const *args,
                                              uintptr_t args_count);

// ---------------------------------------------------------------------------
// Phase 4: Platform Integration — Event dispatch
// ---------------------------------------------------------------------------

/// Pointer button identifiers matching PointerEventButton in Rust.
typedef enum {
    SLINT_POINTER_BUTTON_OTHER = 0,
    SLINT_POINTER_BUTTON_LEFT = 1,
    SLINT_POINTER_BUTTON_RIGHT = 2,
    SLINT_POINTER_BUTTON_MIDDLE = 3,
    SLINT_POINTER_BUTTON_BACK = 4,
    SLINT_POINTER_BUTTON_FORWARD = 5,
} SlintPointerButton;

/// Dispatches a pointer-pressed event to the window.
void slint_swift_dispatch_pointer_pressed(const SlintWindowAdapterRcOpaque *handle, float x,
                                          float y, uint32_t button);

/// Dispatches a pointer-released event to the window.
void slint_swift_dispatch_pointer_released(const SlintWindowAdapterRcOpaque *handle, float x,
                                           float y, uint32_t button);

/// Dispatches a pointer-moved event to the window.
void slint_swift_dispatch_pointer_moved(const SlintWindowAdapterRcOpaque *handle, float x, float y);

/// Dispatches a pointer-scrolled event to the window.
void slint_swift_dispatch_pointer_scrolled(const SlintWindowAdapterRcOpaque *handle, float x,
                                           float y, float delta_x, float delta_y);

/// Dispatches a pointer-exited event to the window.
void slint_swift_dispatch_pointer_exited(const SlintWindowAdapterRcOpaque *handle);

/// Dispatches a key-pressed event to the window.
void slint_swift_dispatch_key_pressed(const SlintWindowAdapterRcOpaque *handle,
                                      const SlintSharedStringOpaque *text);

/// Dispatches a key-press-repeated event to the window.
void slint_swift_dispatch_key_press_repeated(const SlintWindowAdapterRcOpaque *handle,
                                             const SlintSharedStringOpaque *text);

/// Dispatches a key-released event to the window.
void slint_swift_dispatch_key_released(const SlintWindowAdapterRcOpaque *handle,
                                       const SlintSharedStringOpaque *text);

/// Dispatches a scale-factor-changed event to the window.
void slint_swift_dispatch_scale_factor_changed(const SlintWindowAdapterRcOpaque *handle,
                                               float scale_factor);

/// Dispatches a resized event to the window.
void slint_swift_dispatch_resized(const SlintWindowAdapterRcOpaque *handle, float width,
                                  float height);

/// Dispatches a close-requested event to the window.
void slint_swift_dispatch_close_requested(const SlintWindowAdapterRcOpaque *handle);

/// Dispatches a window-active-changed event to the window.
void slint_swift_dispatch_window_active_changed(const SlintWindowAdapterRcOpaque *handle,
                                                bool active);

// ---------------------------------------------------------------------------
// Phase 4: Platform Integration — Custom WindowAdapter
// ---------------------------------------------------------------------------

/// Opaque reference to a `&dyn Renderer` (two pointers: data + vtable).
typedef struct
{
    const void *_0;
    const void *_1;
} SlintRendererRefOpaque;

/// Creates a custom window adapter backed by function pointers.
/// `renderer` must be a valid renderer ref (e.g. from `slint_software_renderer_handle`).
/// Writes the result into `target`.
void slint_swift_window_adapter_new(
        void *user_data, void (*drop_fn)(void *), void (*set_visible_fn)(void *, bool),
        void (*request_redraw_fn)(void *), void (*size_fn)(void *, uint32_t *, uint32_t *),
        void (*set_size_fn)(void *, uint32_t, uint32_t),
        bool (*position_fn)(void *, int32_t *, int32_t *),
        void (*set_position_fn)(void *, int32_t, int32_t),
        void (*update_window_properties_fn)(void *, const SlintSharedStringOpaque *, bool, bool,
                                            bool),
        SlintRendererRefOpaque renderer, SlintWindowAdapterRcOpaque *target);

// ---------------------------------------------------------------------------
// Phase 4: Platform Integration — Custom Platform
// ---------------------------------------------------------------------------

/// Opaque task handle for platform `invoke_from_event_loop` callbacks.
typedef struct
{
    const void *_0;
    const void *_1;
} SlintPlatformTaskOpaque;

/// Registers a custom platform with the Slint runtime. Must be called before any window.
void slint_swift_platform_register(void *user_data, void (*drop_fn)(void *),
                                   void (*window_factory_fn)(void *, SlintWindowAdapterRcOpaque *),
                                   void (*run_event_loop_fn)(void *),
                                   void (*quit_event_loop_fn)(void *),
                                   void (*invoke_from_event_loop_fn)(void *,
                                                                     SlintPlatformTaskOpaque));

/// Runs a platform task received from `invoke_from_event_loop`.
void slint_swift_platform_task_run(SlintPlatformTaskOpaque task);

/// Drops a platform task without running it.
void slint_swift_platform_task_drop(SlintPlatformTaskOpaque task);

/// Updates all timers and animations. Call from your event loop.
void slint_swift_platform_update_timers_and_animations(void);

/// Returns milliseconds until next timer fires, or UINT64_MAX if none pending.
uint64_t slint_swift_platform_duration_until_next_timer_update(void);

/// Returns whether the window has active animations.
bool slint_swift_window_has_active_animations(const SlintWindowAdapterRcOpaque *handle);

// ---------------------------------------------------------------------------
// Phase 4: Platform Integration — Software Renderer (for custom platforms)
// ---------------------------------------------------------------------------

/// Opaque handle to a SoftwareRenderer.
typedef struct SlintSoftwareRendererOpaque SlintSoftwareRendererOpaque;

/// Buffer age for software renderer: 0 = new buffer, 1 = reused, 2 = swapped.
/// Creates a new software renderer. Free with `slint_software_renderer_drop`.
SlintSoftwareRendererOpaque *slint_software_renderer_new(uint32_t buffer_age);

/// Drops a software renderer.
void slint_software_renderer_drop(SlintSoftwareRendererOpaque *renderer);

/// Returns the renderer ref handle for use with `slint_swift_window_adapter_new`.
SlintRendererRefOpaque slint_software_renderer_handle(SlintSoftwareRendererOpaque *renderer);

/// Renders into an RGB8 pixel buffer. Returns the dirty physical region.
/// `buffer` is a pointer to `buffer_len` Rgb8Pixels (3 bytes each, R, G, B).
/// `pixel_stride` is the number of pixels per row.
void slint_software_renderer_render_rgb8(SlintSoftwareRendererOpaque *renderer, void *buffer,
                                         uintptr_t buffer_len, uintptr_t pixel_stride);

#endif // SLINT_CORE_H
