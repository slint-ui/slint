// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import type { Size, Point, Window } from "./index.ts";
import type { language } from "./generated/language";

/**
 * A pointer was pressed.
 *
 * @see {@link Window.dispatchEvent}
 */
export interface PointerPressedEvent {
    type: "pointer-pressed";
    /** The position of the pointer, in logical pixels relative to the top left corner of the window. */
    position: Point;
    /** The button that was pressed. */
    button: language.PointerEventButton;
}

/**
 * A pointer was released.
 *
 * @see {@link Window.dispatchEvent}
 */
export interface PointerReleasedEvent {
    type: "pointer-released";
    /** The position of the pointer, in logical pixels relative to the top left corner of the window. */
    position: Point;
    /** The button that was released. */
    button: language.PointerEventButton;
}

/**
 * The position of the pointer has changed.
 *
 * @see {@link Window.dispatchEvent}
 */
export interface PointerMovedEvent {
    type: "pointer-moved";
    /** The new position of the pointer, in logical pixels relative to the top left corner of the window. */
    position: Point;
}

/**
 * The wheel button of a mouse was rotated to initiate scrolling.
 *
 * @see {@link Window.dispatchEvent}
 */
export interface PointerScrolledEvent {
    type: "pointer-scrolled";
    /** The position of the pointer when the scroll occurred. */
    position: Point;
    /** The amount of logical pixels to scroll in the horizontal direction. */
    deltaX: number;
    /** The amount of logical pixels to scroll in the vertical direction. */
    deltaY: number;
}

/**
 * The pointer exited the window.
 *
 * Dispatching this event always returns `Accepted`.
 *
 * @see {@link Window.dispatchEvent}
 */
export interface PointerExitedEvent {
    type: "pointer-exited";
}

/**
 * A key was pressed.
 *
 * @see {@link Window.dispatchEvent}
 */
export interface KeyPressedEvent {
    type: "key-pressed";
    /** The unicode representation of the key pressed. */
    text: string;
}

/**
 * A key press was auto-repeated.
 *
 * @see {@link Window.dispatchEvent}
 */
export interface KeyPressRepeatedEvent {
    type: "key-press-repeated";
    /** The unicode representation of the key pressed. */
    text: string;
}

/**
 * A key was released.
 *
 * @see {@link Window.dispatchEvent}
 */
export interface KeyReleasedEvent {
    type: "key-released";
    /** The unicode representation of the key released. */
    text: string;
}

/**
 * The window's scale factor has changed.
 * This can happen for example when the display's resolution changes,
 * the user selects a new scale factor in the system settings,
 * or the window is moved to a different screen.
 *
 * @see {@link Window.dispatchEvent}
 */
export interface ScaleFactorChangedEvent {
    type: "scale-factor-changed";
    /** The window system provided scale factor to map logical pixels to physical pixels. */
    scaleFactor: number;
}

/**
 * The window was resized.
 *
 * Dispatching this event updates the `width` and `height` properties of the root window element.
 *
 * @see {@link Window.dispatchEvent}
 */
export interface ResizedEvent {
    type: "resized";
    /** The new logical size of the window. */
    size: Size;
}

/**
 * The user requested to close the window.
 *
 * Dispatching this event invokes the `close-requested` callback of the window element,
 * and hides the window unless that callback returns `reject`.
 *
 * @see {@link Window.dispatchEvent}
 */
export interface CloseRequestedEvent {
    type: "close-requested";
}

/**
 * The window was activated or de-activated.
 *
 * @see {@link Window.dispatchEvent}
 */
export interface WindowActiveChangedEvent {
    type: "window-active-changed";
    /** True when the window gained focus, false when it lost focus. */
    active: boolean;
}

/**
 * An event that describes user input or a windowing system change.
 *
 * The `type` field selects the variant and determines which other fields apply.
 * Dispatch an event to a window with `Window.dispatchEvent`,
 * which reports whether the scene accepted, rejected, or ignored it.
 *
 * @example
 * ```js
 * import * as slint from "slint-ui";
 *
 * const result = window.dispatchEvent({
 *     type: "pointer-pressed",
 *     position: { x: 51, y: 51 },
 *     button: "left",
 * });
 *
 * if (result === slint.WindowEventDispatchResult.Accepted) {
 *     console.log("the scene handled the press");
 * }
 * ```
 */
export type WindowEvent =
    | PointerPressedEvent
    | PointerReleasedEvent
    | PointerMovedEvent
    | PointerScrolledEvent
    | PointerExitedEvent
    | KeyPressedEvent
    | KeyPressRepeatedEvent
    | KeyReleasedEvent
    | ScaleFactorChangedEvent
    | ResizedEvent
    | CloseRequestedEvent
    | WindowActiveChangedEvent;
