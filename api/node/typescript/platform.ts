// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import type { Size, Point } from "./index.ts";
import type { language } from "./generated/language";

/** A pointer was pressed. */
export interface PointerPressedEvent {
    type: "pointer-pressed";
    position: Point;
    /** The button that was pressed. */
    button: language.PointerEventButton;
}

/** A pointer was released. */
export interface PointerReleasedEvent {
    type: "pointer-released";
    position: Point;
    /** The button that was released. */
    button: language.PointerEventButton;
}

/** The position of the pointer has changed. */
export interface PointerMovedEvent {
    type: "pointer-moved";
    position: Point;
}

/** The wheel button of a mouse was rotated to initiate scrolling. */
export interface PointerScrolledEvent {
    type: "pointer-scrolled";
    /** The position of the pointer when the scroll occurred. */
    position: Point;
    /** The amount of logical pixels to scroll in the horizontal direction. */
    deltaX: number;
    /** The amount of logical pixels to scroll in the vertical direction. */
    deltaY: number;
}

/** The pointer exited the window. */
export interface PointerExitedEvent {
    type: "pointer-exited";
}

/** A key was pressed. */
export interface KeyPressedEvent {
    type: "key-pressed";
    /** The unicode representation of the key pressed. */
    text: string;
}

/** A key press was auto-repeated. */
export interface KeyPressRepeatedEvent {
    type: "key-press-repeated";
    /** The unicode representation of the key pressed. */
    text: string;
}

/** A key was released. */
export interface KeyReleasedEvent {
    type: "key-released";
    /** The unicode representation of the key released. */
    text: string;
}

/**
 * The window's scale factor has changed. This can happen for example when the display's resolution
 *  changes, the user selects a new scale factor in the system settings, or the window is moved to a
 * different screen.
 */
export interface ScaleFactorChangedEvent {
    type: "scale-factor-changed";
    /** The window system provided scale factor to map logical pixels to physical pixels. */
    scaleFactor: number;
}

/** The window was resized. */
export interface ResizedEvent {
    type: "resized";
    /** The new logical size of the window */
    size: Size;
}

/** The user requested to close the window. */
export interface CloseRequestedEvent {
    type: "close-requested";
}

/** The Window was activated or de-activated. */
export interface WindowActiveChangedEvent {
    type: "window-active-changed";
    active: boolean;
}

/** A event that describes user input or windowing system events. */
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
