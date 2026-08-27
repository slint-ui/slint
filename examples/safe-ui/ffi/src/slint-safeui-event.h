// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/**
 * @file slint-safeui-event.h
 * @brief FFI event types for dispatching input events from C firmware into
 *        the Rust/Slint UI library.
 *
 * All coordinates and sizes are in **physical pixels** — the raw values
 * from the touch controller or display hardware. The Rust conversion layer
 * divides by the scale factor to produce logical coordinates for Slint.
 * This keeps the FFI boundary integer-only for positions, which avoids
 * FPU usage in ISR context.
 *
 * ## Usage
 *
 * @code{.c}
 * #include "slint-safeui-event.h"
 *
 * void touch_isr_handler(uint16_t x, uint16_t y) {
 *     FfiEvent event = {0};
 *     event.tag = FfiEventTag_PointerPressed;
 *     event.payload.pos_x = (int32_t)x;
 *     event.payload.pos_y = (int32_t)y;
 *     event.payload.button = FfiPointerButton_Left;
 *     slint_safeui_dispatch_event(&event);
 * }
 * @endcode
 */

#ifndef SLINT_SAFEUI_EVENT_H
#define SLINT_SAFEUI_EVENT_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Event discriminator. Each variant (except Quit) maps to a Slint WindowEvent.
 *
 * Fill only the FfiEventPayload fields documented for each tag;
 * leave everything else zeroed.
 */
typedef enum {
    /** Request clean shutdown of the Slint event loop. No payload. */
    FfiEventTag_Quit = 0,
    /** Touch press / mouse down. Payload: pos_x, pos_y, button. */
    FfiEventTag_PointerPressed,
    /** Touch release / mouse up. Payload: pos_x, pos_y, button. */
    FfiEventTag_PointerReleased,
    /** Touch drag / mouse move. Payload: pos_x, pos_y. */
    FfiEventTag_PointerMoved,
    /** Scroll wheel / rotary encoder. Payload: pos_x, pos_y, delta_x, delta_y. */
    FfiEventTag_PointerScrolled,
    /** Pointer left the window area. No payload. */
    FfiEventTag_PointerExited,
    /** Key press (initial). Payload: key_code. */
    FfiEventTag_KeyPressed,
    /** Key press repeat (held). Payload: key_code. */
    FfiEventTag_KeyPressRepeated,
    /** Key release. Payload: key_code. */
    FfiEventTag_KeyReleased,
    /** Display resolution changed (e.g. rotation). Payload: width, height. */
    FfiEventTag_Resized,
} FfiEventTag;

/**
 * Pointer button identity for press/release events.
 */
typedef enum {
    /** Primary / left button. Use for single-touch panels. */
    FfiPointerButton_Left = 0,
    FfiPointerButton_Right,
    FfiPointerButton_Middle,
    FfiPointerButton_Other,
} FfiPointerButton;

/**
 * Flat event payload. Fill only the fields relevant to the FfiEventTag;
 * leave everything else zero-initialized.
 *
 * Positions and sizes are in **physical pixels** (raw hardware values).
 * Scroll deltas are unitless floats passed through to Slint as-is.
 */
typedef struct
{
    /** Physical X coordinate (pixels). Pointer events. */
    int32_t pos_x;
    /** Physical Y coordinate (pixels). Pointer events. */
    int32_t pos_y;
    /** Which pointer button. Pointer press/release events. */
    FfiPointerButton button;
    /** Horizontal scroll delta (unitless). PointerScrolled. */
    float delta_x;
    /** Vertical scroll delta (unitless). PointerScrolled. */
    float delta_y;
    /** Unicode code point (U+0000 to U+10FFFF). Key events. */
    uint32_t key_code;
    /** New physical width (pixels). Resized. */
    int32_t width;
    /** New physical height (pixels). Resized. */
    int32_t height;
    /** Reserved for modifier key flags. Must be set to 0. */
    uint32_t modifiers;
} FfiEventPayload;

/**
 * A single input event dispatched from C firmware into the Rust/Slint library.
 *
 * All coordinates are in physical pixels — no float conversion needed on the
 * C side. The Rust conversion layer handles the physical-to-logical mapping.
 *
 * Construction:
 * @code{.c}
 * FfiEvent event = {0};
 * event.tag = FfiEventTag_PointerPressed;
 * event.payload.pos_x = (int32_t)touch_x;
 * event.payload.pos_y = (int32_t)touch_y;
 * event.payload.button = FfiPointerButton_Left;
 * @endcode
 */
typedef struct
{
    FfiEventTag tag;
    FfiEventPayload payload;
} FfiEvent;

/*
 * ABI contract guards.
 *
 * Rust side assumes 32-bit enum storage and fixed payload/event sizes.
 * Fail compilation early if toolchain flags (for example short-enum modes)
 * would change the wire layout.
 */
#if defined(__cplusplus)
static_assert(sizeof(FfiEventTag) == 4, "FfiEventTag must be 32-bit");
static_assert(sizeof(FfiPointerButton) == 4, "FfiPointerButton must be 32-bit");
static_assert(sizeof(FfiEventPayload) == 36, "FfiEventPayload size mismatch");
static_assert(sizeof(FfiEvent) == 40, "FfiEvent size mismatch");
#elif defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(sizeof(FfiEventTag) == 4, "FfiEventTag must be 32-bit");
_Static_assert(sizeof(FfiPointerButton) == 4, "FfiPointerButton must be 32-bit");
_Static_assert(sizeof(FfiEventPayload) == 36, "FfiEventPayload size mismatch");
_Static_assert(sizeof(FfiEvent) == 40, "FfiEvent size mismatch");
#endif

#ifdef __cplusplus
}
#endif

#endif /* SLINT_SAFEUI_EVENT_H */
