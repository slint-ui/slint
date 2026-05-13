// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/**
 *  Represents a two-dimensional point.
 */
export interface Point {
    /**
     * Defines the x coordinate of the point.
     */
    x: number;

    /**
     * Defines the y coordinate of the point.
     */
    y: number;
}

/**
 *  Represents a two-dimensional size.
 */
export interface Size {
    /**
     * Defines the width length of the size.
     */
    width: number;

    /**
     * Defines the height length of the size.
     */
    height: number;
}

/**
 * This type represents a window towards the windowing system, that's used to render the
 * scene of a component. It provides API to control windowing system specific aspects such
 * as the position on the screen.
 */
export interface Window {
    /** Gets or sets the logical position of the window on the screen. */
    logicalPosition: Point;

    /** Gets or sets the physical position of the window on the screen. */
    physicalPosition: Point;

    /** Gets or sets the logical size of the window on the screen, */
    logicalSize: Size;

    /** Gets or sets the physical size of the window on the screen, */
    physicalSize: Size;

    /** Gets or sets the window's fullscreen state **/
    fullscreen: boolean;

    /** Gets or sets the window's maximized state **/
    maximized: boolean;

    /** Gets or sets the window's minimized state **/
    minimized: boolean;

    /**
     * Returns the visibility state of the window. This function can return false even if you previously called show()
     * on it, for example if the user minimized the window.
     */
    get visible(): boolean;

    /**
     * Shows the window on the screen. An additional strong reference on the
     * associated component is maintained while the window is visible.
     */
    show(): void;

    /** Hides the window, so that it is not visible anymore. */
    hide(): void;

    /** Issues a request to the windowing system to re-render the contents of the window. */
    requestRedraw(): void;
}

/**
 * An image data type that can be displayed by the Image element.
 *
 * This interface is inspired by the web [ImageData](https://developer.mozilla.org/en-US/docs/Web/API/ImageData) interface.
 */
export interface ImageData {
    /**
     * Returns the path of the image, if it was loaded from disk. Otherwise
     * the property is undefined.
     */
    readonly path?: string;

    /**
     *  Returns the image as buffer.
     */
    get data(): Uint8Array;

    /**
     * Returns the width of the image in pixels.
     */
    get width(): number;

    /**
     *  Returns the height of the image in pixels.
     */
    get height(): number;
}

/**
 * This interface describes the public API of a Slint component that is common to all instances. Use this to
 * show() the window on the screen, access the window and subsequent window properties, or start the
 * Slint event loop with run().
 */
export interface ComponentHandle {
    /**
     * Shows the window and runs the event loop. The returned promise is resolved when the event loop
     * is terminated, for example when the last window is closed and the last visible system tray
     * icon is hidden, or when the event loop is quit.
     *
     * This function is a convenience for calling show(), followed by runEventLoop(), and
     * hide() when the event loop's promise is resolved.
     */
    run(): Promise<unknown>;

    /**
     * Shows the component's window on the screen.
     */
    show(): void;

    /**
     * Hides the component's window, so that it is not visible anymore.
     */
    hide(): void;

    /**
     * Returns the Window associated with this component instance.
     * The window API can be used to control different aspects of the integration into the windowing system, such as the position on the screen.
     *
     * Not present on non-windowed components such as ones inheriting from `SystemTrayIcon`.
     */
    readonly window?: Window;
}

/**
 * LoadFileOptions are used to defines different optional parameters that can be used to configure the compiler.
 */
export interface LoadFileOptions {
    /**
     * If set to true warnings from the compiler will not be printed to the console.
     */
    quiet?: boolean;

    /**
     * Sets the widget style the compiler is currently using when compiling .slint files.
     */
    style?: string;

    /**
     * Sets the include paths used for looking up `.slint` imports to the specified vector of paths.
     */
    includePaths?: Array<string>;

    /**
     * Sets library paths used for looking up `@library` imports to the specified map of library names to paths.
     */
    libraryPaths?: Record<string, string>;

    /**
     * @hidden
     */
    fileLoader?: (path: string) => string;
}
