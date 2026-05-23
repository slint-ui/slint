// Smoke test (Node-side): load the wasm-bindgen bundle and call into the
// Rust API without a browser. Validates the JS↔Rust bridge and the Slint
// compiler. Cannot test rendering (no canvas, no winit).

import init, {
    compile_from_string_with_style,
    wasm_model_notify_new,
    wasm_model_notify_row_added,
    wasm_model_notify_reset,
} from "./pkg/slint_wasm_interpreter.js";
import { readFile } from "node:fs/promises";

const wasmBytes = await readFile("./pkg/slint_wasm_interpreter_bg.wasm");
const mod = await WebAssembly.compile(wasmBytes);

try {
    await init({ module_or_path: mod });
} catch (e) {
    // The wasm-bindgen `start` callback calls `init()` which tries to create
    // a winit backend. That fails outside a browser; we ignore the error and
    // continue — module-level state is still initialized.
    console.log("init threw (expected outside browser):", String(e).slice(0, 200));
}

console.log("Calling compile_from_string_with_style…");
const result = await compile_from_string_with_style(
    `export component App inherits Window {
        in-out property <int> counter: 0;
        in-out property <[{ name: string, value: int }]> items: [{ name: "a", value: 1 }];
    }`,
    "test.slint",
    "",
    null,
);

console.log("error_string:", JSON.stringify(result.error_string));
console.log("diagnostics:", result.diagnostics.length);
console.log("definitions keys:", Object.keys(result.definitions));
console.log("structs keys:", Object.keys(result.structs));
console.log("enums keys:", Object.keys(result.enums));

const appDef = result.definitions.App;
console.log("App.name:", appDef.name);
console.log("App.properties:", appDef.properties.map((p) => p.name));

console.log("\nModel notify smoke:");
const notify = wasm_model_notify_new();
console.log("notify.id:", notify.id);
wasm_model_notify_row_added(notify, 0, 3);
wasm_model_notify_reset(notify);
console.log("notify calls succeeded");

console.log("\nOK");
