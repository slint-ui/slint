// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore autocapitalize autocorrect compositionend compositionupdate dppx HEAPF HEAPU inputmode

// Browser glue for Slint's Emscripten backend, linked with `--js-library`.
// The functions below are called from Rust; the `slint_em_on_*` functions they
// call back are exported by api/cpp/emscripten.rs.

addToLibrary({
    $SlintEm: {
        canvas: null,
        // Has the focus while a Slint text input has it, see slint_em_show_text_input.
        input: null,
        // The text of a copy or cut, from slint_em_copied.
        copied: null,
        context2d: null,
        imageData: null,
    },

    slint_em_attach_canvas__deps: [
        "$SlintEm",
        "$stringToNewUTF8",
        "free",
        "slint_em_on_pointer",
        "slint_em_on_wheel",
        "slint_em_on_key",
        "slint_em_on_focus",
        "slint_em_on_resize",
        "slint_em_on_text",
        "slint_em_on_composition",
        "slint_em_on_paste",
        "slint_em_on_copy",
    ],
    slint_em_attach_canvas: () => {
        const canvas = Module["canvas"] || document.querySelector("#canvas");
        if (!canvas) {
            return 0;
        }
        SlintEm.canvas = canvas;
        // A size in the style attribute means the page lays out the canvas; otherwise Slint sizes it.
        const pageSized = canvas.style.width !== "" || canvas.style.height !== "";

        // Receive keyboard events, and keep the browser from panning or zooming on touch.
        if (!canvas.hasAttribute("tabindex")) {
            canvas.tabIndex = 0;
        }
        canvas.style.touchAction = "none";
        canvas.style.outline = "none";
        canvas.addEventListener("contextmenu", (e) => e.preventDefault());

        // A hidden <input> receives the keys while a Slint text input has the focus:
        // it shows the keyboard on phones, and gets the composition of input methods and the clipboard's events.
        const input = (SlintEm.input = document.createElement("input"));
        input.setAttribute("autocapitalize", "none");
        input.setAttribute("autocomplete", "off");
        input.setAttribute("autocorrect", "off");
        input.spellcheck = false;
        Object.assign(input.style, {
            position: "fixed",
            opacity: "0",
            pointerEvents: "none",
            width: "1px",
            height: "1px",
            padding: "0",
            border: "0",
        });
        canvas.after(input);

        const pointer = (kind) => (e) => {
            if (kind === 0 && document.activeElement !== input) {
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

        // Whether Slint got the last key press, so that the text it typed into the input isn't sent again.
        let keyHandled = false;
        const key = (pressed) => (e) => {
            if (e.isComposing) {
                return;
            }
            const keyName = stringToNewUTF8(e.key);
            const handled = _slint_em_on_key(pressed, keyName, e.shiftKey, e.repeat);
            _free(keyName);
            keyHandled = pressed && handled;
            // Keep keys such as Tab, Backspace and the arrows from moving focus or scrolling the page,
            // but leave shortcuts such as reloading the page, and copy and paste, to the browser.
            if (handled && !e.ctrlKey && !e.metaKey) {
                e.preventDefault();
            }
        };
        const withString = (text, f) => {
            const ptr = stringToNewUTF8(text);
            f(ptr);
            _free(ptr);
        };
        for (const target of [canvas, input]) {
            target.addEventListener("keydown", key(1));
            target.addEventListener("keyup", key(0));
            // Moving the focus between the canvas and the input keeps the window active.
            target.addEventListener("focus", () => _slint_em_on_focus(1));
            target.addEventListener("blur", (e) => {
                if (e.relatedTarget !== canvas && e.relatedTarget !== input) {
                    _slint_em_on_focus(0);
                }
            });
        }
        input.addEventListener("input", (e) => {
            if (e.isComposing || e.inputType === "insertCompositionText") {
                return;
            }
            // A phone's keyboard types without key events that Slint knows, see slint_em_on_key.
            if (e.data && !keyHandled) {
                withString(e.data, _slint_em_on_text);
            }
            keyHandled = false;
            input.value = "";
        });
        input.addEventListener("compositionupdate", (e) =>
            withString(e.data ?? "", (text) => _slint_em_on_composition(0, text)),
        );
        input.addEventListener("compositionend", (e) => {
            withString(e.data ?? "", (text) => _slint_em_on_composition(1, text));
            input.value = "";
        });
        input.addEventListener("paste", (e) => {
            e.preventDefault();
            withString(e.clipboardData?.getData("text") ?? "", _slint_em_on_paste);
        });
        const copy = (cut) => (e) => {
            e.preventDefault();
            SlintEm.copied = null;
            _slint_em_on_copy(cut);
            if (SlintEm.copied !== null) {
                e.clipboardData?.setData("text/plain", SlintEm.copied);
            }
        };
        input.addEventListener("copy", copy(0));
        input.addEventListener("cut", copy(1));

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

    // Creates the WebGL 2 context for FemtoVG and returns its handle, or 0.
    // Not with emscripten_webgl_create_context, whose attributes struct has int fields up to Emscripten 3.1.56,
    // which .NET 10 links with, and bool fields after.
    slint_em_create_webgl_context__deps: ["$SlintEm", "$GL"],
    slint_em_create_webgl_context: () =>
        GL.createContext(SlintEm.canvas, { majorVersion: 2, minorVersion: 0, stencil: true, antialias: false }),

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

    // Puts the hidden input over the text cursor, where input methods show their candidates, and focuses it.
    slint_em_show_text_input__deps: ["$SlintEm"],
    slint_em_show_text_input: (x, y, width, height, inputMode) => {
        const { canvas, input } = SlintEm;
        const rect = canvas.getBoundingClientRect();
        input.style.left = `${rect.left + x}px`;
        input.style.top = `${rect.top + y}px`;
        input.style.height = `${Math.max(height, 1)}px`;
        input.inputMode = UTF8ToString(inputMode);
        if (document.activeElement !== input) {
            input.focus({ preventScroll: true });
        }
    },

    slint_em_hide_text_input__deps: ["$SlintEm"],
    slint_em_hide_text_input: () => {
        const { canvas, input } = SlintEm;
        if (document.activeElement === input) {
            canvas.focus({ preventScroll: true });
        }
    },

    slint_em_copied__deps: ["$SlintEm"],
    slint_em_copied: (text) => {
        SlintEm.copied = UTF8ToString(text);
    },

    // Outside a copy event, the browser may refuse, such as without a user gesture.
    slint_em_write_clipboard: (text) => {
        navigator.clipboard?.writeText(UTF8ToString(text)).catch(() => {});
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
