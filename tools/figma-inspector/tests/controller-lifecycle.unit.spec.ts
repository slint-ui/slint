// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { PreviewController } from "../src/preview/controller";

const runtime = vi.hoisted(() => ({
    initialize: vi.fn(),
    compile: vi.fn(),
    run: vi.fn(),
}));

vi.mock("@interpreter/slint_wasm_interpreter_bg.wasm?url&inline", () => ({
    default: "data:application/wasm;base64,AA==",
}));
vi.mock("@interpreter/slint_wasm_interpreter.js", () => ({
    default: runtime.initialize,
    compile_from_string: runtime.compile,
    run_event_loop: runtime.run,
}));

function deferred<T>() {
    let resolve!: (value: T) => void;
    let reject!: (reason: Error) => void;
    const promise = new Promise<T>((a, b) => {
        resolve = a;
        reject = b;
    });
    return { promise, resolve, reject };
}

function compiled() {
    const instance = {
        show: vi.fn().mockResolvedValue(undefined),
        hide: vi.fn().mockResolvedValue(undefined),
        free: vi.fn(),
    };
    const component = {
        create: vi.fn().mockResolvedValue(instance),
        create_with_existing_window: vi.fn().mockResolvedValue(instance),
        free: vi.fn(),
    };
    return {
        instance,
        component,
        result: { component, error_string: "", free: vi.fn() },
    };
}

function setup() {
    const context = {
        clearColor: vi.fn(),
        clear: vi.fn(),
        COLOR_BUFFER_BIT: 1,
        DEPTH_BUFFER_BIT: 2,
        STENCIL_BUFFER_BIT: 4,
    };
    const canvas = {
        id: "preview",
        style: { visibility: "" },
        dispatchEvent: vi.fn(),
        getContext: vi.fn().mockReturnValue(context),
    };
    const status = { dataset: {} as Record<string, string>, textContent: "" };
    const diagnostics = { dataset: {}, hidden: true, textContent: "" };
    const controller = new PreviewController(
        canvas as unknown as HTMLCanvasElement,
        status as unknown as HTMLElement,
        diagnostics as unknown as HTMLElement,
    );
    return { controller, canvas, context, status, diagnostics };
}

beforeEach(() => {
    vi.resetAllMocks();
    runtime.initialize.mockResolvedValue(undefined);
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
        queueMicrotask(() => callback(0));
        return 1;
    });
});

afterEach(() => vi.unstubAllGlobals());

test("rejects runtime initialization and retries a later selection", async () => {
    const { controller, canvas, status, diagnostics } = setup();
    runtime.initialize.mockRejectedValueOnce(new Error("WASM unavailable"));
    await expect(controller.initialize("first", 1)).rejects.toThrow(
        "WASM unavailable",
    );
    expect(status.dataset).toEqual({ state: "error", revision: "1" });
    expect(diagnostics.textContent).toContain("Restart the plugin");
    expect(canvas.style.visibility).toBe("hidden");

    const next = compiled();
    runtime.compile.mockResolvedValueOnce(next.result);
    await controller.initialize("second", 2);
    expect(runtime.initialize).toHaveBeenCalledTimes(2);
    expect(runtime.initialize).toHaveBeenLastCalledWith({
        module_or_path: new Uint8Array([0]),
    });
    expect(runtime.compile).toHaveBeenCalledExactlyOnceWith(
        "second",
        "",
        undefined,
    );
    expect(status.dataset).toEqual({ state: "ready", revision: "2" });
    expect(diagnostics.hidden).toBe(true);
    expect(canvas.style.visibility).toBe("visible");
});

test("attributes shared initialization rejection to the newest queued selection", async () => {
    const { controller, canvas, status, diagnostics } = setup();
    const initialization = deferred<void>();
    runtime.initialize.mockReturnValueOnce(initialization.promise);
    const pending = controller.initialize("first", 1);
    controller.requestRender("second", 2);
    controller.requestRender("third", 3);
    initialization.reject(new Error("Initialization interrupted"));
    await expect(pending).rejects.toThrow("Initialization interrupted");
    expect(status.dataset).toEqual({ state: "error", revision: "3" });
    expect(diagnostics.textContent).toContain("Initialization interrupted");
    expect(canvas.dispatchEvent).toHaveBeenCalledWith(
        expect.objectContaining({
            type: "preview-error",
            detail: expect.objectContaining({ revision: 3 }),
        }),
    );
    expect(runtime.compile).not.toHaveBeenCalled();

    const recovery = compiled();
    runtime.compile.mockResolvedValueOnce(recovery.result);
    await controller.initialize("fourth", 4);
    expect(runtime.compile).toHaveBeenCalledExactlyOnceWith(
        "fourth",
        "",
        undefined,
    );
    expect(status.dataset).toEqual({ state: "ready", revision: "4" });
});

test("releases a stale compile result without creating or presenting its instance", async () => {
    const { controller, canvas, status } = setup();
    const compilation = deferred<ReturnType<typeof compiled>["result"]>();
    const stale = compiled();
    runtime.compile.mockReturnValueOnce(compilation.promise);
    const initial = controller.initialize("old", 1);
    await vi.waitFor(() => expect(runtime.compile).toHaveBeenCalledOnce());
    controller.showDiagnostic("New selection failed", 2);
    compilation.resolve(stale.result);
    await initial;
    expect(stale.result.free).toHaveBeenCalledOnce();
    expect(stale.component.create).not.toHaveBeenCalled();
    expect(status.dataset).toEqual({ state: "error", revision: "2" });
    expect(canvas.style.visibility).toBe("hidden");
    expect(
        canvas.dispatchEvent.mock.calls.map(([event]) => event.type),
    ).toEqual(["preview-error"]);
});

test("an older clear finishing after a new preview cannot erase the new canvas", async () => {
    const { controller, canvas, context, status } = setup();
    const first = compiled();
    const next = compiled();
    runtime.compile
        .mockResolvedValueOnce(first.result)
        .mockResolvedValueOnce(next.result);
    await controller.initialize("first", 1);
    const hidden = deferred<void>();
    first.instance.hide.mockReturnValueOnce(hidden.promise);
    controller.clearPreview(2);
    controller.requestRender("next", 3);
    await vi.waitFor(() =>
        expect(status.dataset).toEqual({ state: "ready", revision: "3" }),
    );
    const clears = context.clear.mock.calls.length;
    hidden.resolve();
    await vi.waitFor(() => expect(first.instance.free).toHaveBeenCalledOnce());
    expect(context.clear).toHaveBeenCalledTimes(clears);
    expect(canvas.style.visibility).toBe("visible");
    expect(controller.currentInstance).toBe(next.instance);
    expect(
        canvas.dispatchEvent.mock.calls.map(([event]) => event.type),
    ).not.toContain("preview-cleared");
});

test("releases the failed instance and compilation wrappers when show rejects", async () => {
    const { controller, canvas, status } = setup();
    const failure = compiled();
    failure.instance.show.mockRejectedValueOnce(
        new Error("Cannot show window"),
    );
    runtime.compile.mockResolvedValueOnce(failure.result);
    await controller.initialize("broken", 1);
    expect(failure.instance.free).toHaveBeenCalledOnce();
    expect(failure.component.free).toHaveBeenCalledOnce();
    expect(failure.result.free).toHaveBeenCalledOnce();
    expect(controller.currentInstance).toBeUndefined();
    expect(status.dataset).toEqual({ state: "error", revision: "1" });
    expect(canvas.style.visibility).toBe("hidden");
});
