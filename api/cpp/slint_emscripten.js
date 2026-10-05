// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore dppx HEAPF HEAPU

// Browser glue for Slint's Emscripten backend, linked with `--js-library`.
// The functions below are called from Rust; the `slint_em_on_*` functions they
// call back are exported by api/cpp/emscripten.rs.

addToLibrary({
    $SlintEm: {
        canvas: null,
        context2d: null,
        imageData: null,
    },

    slint_em_attach_canvas__deps: [
        "$SlintEm",
        "$specialHTMLTargets",
        "$stringToNewUTF8",
        "free",
        "slint_em_on_pointer",
        "slint_em_on_wheel",
        "slint_em_on_key",
        "slint_em_on_focus",
        "slint_em_on_resize",
    ],
    slint_em_attach_canvas: () => {
        const canvas = Module["canvas"] || document.querySelector("#canvas");
        if (!canvas) {
            return 0;
        }
        SlintEm.canvas = canvas;
        // A size in the style attribute means the page lays out the canvas; otherwise Slint sizes it.
        const pageSized = canvas.style.width !== "" || canvas.style.height !== "";
        // Lets emscripten_webgl_create_context find the canvas, whatever its id.
        specialHTMLTargets["!slint-canvas"] = canvas;

        // Receive keyboard events, and keep the browser from panning or zooming on touch.
        if (!canvas.hasAttribute("tabindex")) {
            canvas.tabIndex = 0;
        }
        canvas.style.touchAction = "none";
        canvas.style.outline = "none";
        canvas.addEventListener("contextmenu", (e) => e.preventDefault());

        const pointer = (kind) => (e) => {
            if (kind === 0) {
                canvas.focus();
                canvas.setPointerCapture(e.pointerId);
            }
            _slint_em_on_pointer(kind, e.offsetX, e.offsetY, e.button, e.pointerType === "touch");
            e.preventDefault();
        };
        canvas.addEventListener("pointerdown", pointer(0));
        canvas.addEventListener("pointerup", pointer(1));
        canvas.addEventListener("pointermove", pointer(2));
        canvas.addEventListener("pointerleave", pointer(3));
        canvas.addEventListener("pointercancel", pointer(3));

        canvas.addEventListener(
            "wheel",
            (e) => {
                // deltaMode: 0 = pixels, 1 = lines, 2 = pages
                const scale = e.deltaMode === 1 ? 20 : e.deltaMode === 2 ? canvas.clientHeight : 1;
                _slint_em_on_wheel(e.offsetX, e.offsetY, -e.deltaX * scale, -e.deltaY * scale);
                e.preventDefault();
            },
            { passive: false },
        );

        const key = (pressed) => (e) => {
            if (e.isComposing) {
                return;
            }
            const keyName = stringToNewUTF8(e.key);
            const handled = _slint_em_on_key(pressed, keyName, e.shiftKey, e.repeat);
            _free(keyName);
            // Keep keys such as Tab, Backspace and the arrows from moving focus or scrolling the page,
            // but leave shortcuts such as reloading the page to the browser.
            if (handled && !e.ctrlKey && !e.metaKey) {
                e.preventDefault();
            }
        };
        canvas.addEventListener("keydown", key(1));
        canvas.addEventListener("keyup", key(0));

        canvas.addEventListener("focus", () => _slint_em_on_focus(1));
        canvas.addEventListener("blur", () => _slint_em_on_focus(0));

        const reportSize = () => {
            _slint_em_on_resize(canvas.clientWidth, canvas.clientHeight, window.devicePixelRatio || 1);
        };
        new ResizeObserver(reportSize).observe(canvas);
        // The device pixel ratio changes without a resize when the page is zoomed
        // or moved to another screen.
        const watchPixelRatio = () => {
            matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`).addEventListener(
                "change",
                () => {
                    reportSize();
                    watchPixelRatio();
                },
                { once: true },
            );
        };
        watchPixelRatio();
        return pageSized ? 2 : 1;
    },

    slint_em_canvas_css_size__deps: ["$SlintEm"],
    slint_em_canvas_css_size: (width_ptr, height_ptr, ratio_ptr) => {
        const canvas = SlintEm.canvas;
        HEAPF32[width_ptr >> 2] = canvas.clientWidth;
        HEAPF32[height_ptr >> 2] = canvas.clientHeight;
        HEAPF32[ratio_ptr >> 2] = window.devicePixelRatio || 1;
    },

    slint_em_set_canvas_css_size__deps: ["$SlintEm"],
    slint_em_set_canvas_css_size: (width, height) => {
        SlintEm.canvas.style.width = `${width}px`;
        SlintEm.canvas.style.height = `${height}px`;
    },

    slint_em_set_canvas_buffer_size__deps: ["$SlintEm"],
    slint_em_set_canvas_buffer_size: (width, height) => {
        const canvas = SlintEm.canvas;
        if (canvas.width !== width) {
            canvas.width = width;
        }
        if (canvas.height !== height) {
            canvas.height = height;
        }
    },

    slint_em_set_cursor__deps: ["$SlintEm"],
    slint_em_set_cursor: (name) => {
        SlintEm.canvas.style.cursor = UTF8ToString(name);
    },

    slint_em_set_title: (title) => {
        document.title = UTF8ToString(title);
    },

    // Copies RGBA pixels (straight alpha) from the wasm heap into the canvas.
    slint_em_put_image_data__deps: ["$SlintEm"],
    slint_em_put_image_data: (pixels, width, height) => {
        if (!SlintEm.context2d) {
            SlintEm.context2d = SlintEm.canvas.getContext("2d");
        }
        let imageData = SlintEm.imageData;
        if (!imageData || imageData.width !== width || imageData.height !== height) {
            imageData = SlintEm.imageData = SlintEm.context2d.createImageData(width, height);
        }
        imageData.data.set(HEAPU8.subarray(pixels, pixels + width * height * 4));
        SlintEm.context2d.putImageData(imageData, 0, 0);
    },
});
