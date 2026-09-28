// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { test, expect } from "vitest";

import { language, platform, private_api } from "../dist/index.js";
import type { Window } from "../dist/index.d.ts";

private_api.initTesting();

test("Window constructor", () => {
    let thrownError: any;
    try {
        new private_api.Window();
    } catch (error) {
        thrownError = error;
    }
    expect(thrownError).toBeDefined();
    expect(thrownError.code).toBe("GenericFailure");
    expect(thrownError.message).toBe(
        "Window can only be created by using a Component.",
    );
});

test("Window show / hide", () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `

    export component App inherits Window {
        width: 300px;
        height: 300px;
    }`,
        "",
    );
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

    const window = instance!.window();
    expect(window.visible).toBe(false);
    window.show();
    expect(window.visible).toBe(true);
    window.hide();
    expect(window.visible).toBe(false);
});

test("Window dispatch pointer event", () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
    import { Button } from "std-widgets.slint";
    export component App inherits Window {
        out property <bool> clicked;
        width: 300px;
        height: 300px;

        Button {
            x: 0;
            y: 0;
            width: 50px;
            height: 50px;
            clicked => { root.clicked = true; }
        }
    }`,
        "",
    );
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

    const window = instance!.window() as Window;
    window.dispatchEvent({
        type: "pointer-pressed",
        button: language.PointerEventButton.Left,
        position: { x: 51, y: 51 },
    });
    window.dispatchEvent({
        type: "pointer-released",
        button: language.PointerEventButton.Left,
        position: { x: 51, y: 51 },
    });
    expect(instance.getProperty("clicked")).toBe(false);

    window.dispatchEvent({
        type: "pointer-pressed",
        button: language.PointerEventButton.Left,
        position: { x: 49, y: 49 },
    });
    window.dispatchEvent({
        type: "pointer-released",
        button: language.PointerEventButton.Left,
        position: { x: 49, y: 49 },
    });
    expect(instance.getProperty("clicked")).toBe(true);
});

test("Window dispatch pointer moved event", () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
    export component App inherits Window {
        out property <length> mouse-x <=> ta.mouse-x;
        out property <length> mouse-y <=> ta.mouse-y;
        width: 300px;
        height: 300px;

        ta := TouchArea {
            width: 100%;
            height: 100%;
        }
    }`,
        "",
    );
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

    const window = instance!.window() as Window;

    window.dispatchEvent({
        type: "pointer-pressed",
        button: language.PointerEventButton.Left,
        position: { x: 1, y: 1 },
    });
    window.dispatchEvent({
        type: "pointer-moved",
        position: { x: 10, y: 20 },
    });
    expect(instance.getProperty("mouse-x")).toBe(10);
    expect(instance.getProperty("mouse-y")).toBe(20);

    window.dispatchEvent({
        type: "pointer-moved",
        position: { x: 30, y: 40 },
    });
    expect(instance.getProperty("mouse-x")).toBe(30);
    expect(instance.getProperty("mouse-y")).toBe(40);
});

test("Window dispatch pointer scrolled event", () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
    export component App inherits Window {
        out property <length> scroll-delta-x;
        out property <length> scroll-delta-y;
        width: 300px;
        height: 300px;

        TouchArea {
            width: 100%;
            height: 100%;
            scroll-event(event) => {
                root.scroll-delta-x = event.delta-x;
                root.scroll-delta-y = event.delta-y;
                accept
            }
        }
    }`,
        "",
    );
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

    const window = instance!.window() as Window;

    window.dispatchEvent({
        type: "pointer-scrolled",
        position: { x: 100, y: 100 },
        deltaX: 5,
        deltaY: -10,
    });
    expect(instance.getProperty("scroll-delta-x")).toBe(5);
    expect(instance.getProperty("scroll-delta-y")).toBe(-10);
});

test("Window dispatch key events", () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
    export component App inherits Window {
        out property <string> text-pressed;
        out property <string> text-released;
        width: 300px;
        height: 300px;
        forward-focus: fs;

        fs := FocusScope {
            width: 100%;
            height: 100%;
            key-pressed(event) => {
                root.text-pressed = event.text;
                accept
            }
            key-released(event) => {
                root.text-released = event.text;
                accept
            }
        }
    }`,
        "",
    );
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

    const window = instance!.window() as Window;
    expect(instance.getProperty("text-pressed")).toBe("");
    expect(instance.getProperty("text-released")).toBe("");

    window.dispatchEvent({
        type: "key-pressed",
        text: "a",
    });
    expect(instance.getProperty("text-pressed")).toBe("a");
    expect(instance.getProperty("text-released")).toBe("");

    window.dispatchEvent({
        type: "key-released",
        text: "b",
    });
    expect(instance.getProperty("text-released")).toBe("b");
});

test("Window dispatch event result", () => {
    const compiler = new private_api.ComponentCompiler();
    const definition = compiler.buildFromSource(
        `
    export component App inherits Window {
        width: 100px;
        height: 100px;

        TouchArea {
            x: 0;
            y: 0;
            width: 50px;
            height: 50px;
        }
    }`,
        "",
    );
    expect(definition.App).not.toBeNull();

    const instance = definition.App!.create();
    expect(instance).not.toBeNull();

    const window = instance!.window() as Window;

    expect(window.dispatchEvent({ type: "pointer-exited" })).toBe(
        platform.WindowEventDispatchResult.Accepted,
    );

    expect(
        window.dispatchEvent({
            type: "pointer-pressed",
            button: language.PointerEventButton.Left,
            position: { x: 10, y: 10 },
        }),
    ).toBe(platform.WindowEventDispatchResult.Accepted);
    window.dispatchEvent({
        type: "pointer-released",
        button: language.PointerEventButton.Left,
        position: { x: 10, y: 10 },
    });

    expect(
        window.dispatchEvent({
            type: "pointer-pressed",
            button: language.PointerEventButton.Left,
            position: { x: 90, y: 90 },
        }),
    ).toBe(platform.WindowEventDispatchResult.Rejected);

    expect(window.dispatchEvent({ type: "key-pressed", text: "a" })).toBe(
        platform.WindowEventDispatchResult.Rejected,
    );
});
