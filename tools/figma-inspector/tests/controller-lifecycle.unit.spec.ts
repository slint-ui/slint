// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { TimingTraceBuilder } from "../src/performance/timing";
import { PreviewController } from "../src/preview/controller";

const runtime = vi.hoisted(() => ({
    initialize: vi.fn(),
    compile: vi.fn(),
    run: vi.fn(),
    registerFont: vi.fn(),
}));

vi.mock("@interpreter/slint_wasm_interpreter_bg.wasm?url&inline", () => ({
    default: "data:application/wasm;base64,AA==",
}));
vi.mock("@interpreter/slint_wasm_interpreter.js", () => ({
    default: runtime.initialize,
    compile_from_string: runtime.compile,
    run_event_loop: runtime.run,
    register_font_from_memory: runtime.registerFont,
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
        width: 240,
        height: 72,
        style: { visibility: "", setProperty: vi.fn() },
        dispatchEvent: vi.fn(),
        getContext: vi.fn().mockReturnValue(context),
    };
    const status = { dataset: {} as Record<string, string>, textContent: "" };
    const diagnostics = {
        dataset: {} as Record<string, string>,
        hidden: true,
        textContent: "",
        replaceChildren: vi.fn(),
    };
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

test("export diagnostics preserve the preview, release compiler wrappers, and are cached", async () => {
    const { controller, canvas, status, diagnostics } = setup();
    vi.stubGlobal("document", {
        createElement: () => ({ append: vi.fn(), textContent: "" }),
    });
    const preview = compiled();
    const validation = {
        component: undefined,
        error_string: "Invalid export",
        free: vi.fn(),
    };
    const compilation = deferred<typeof validation>();
    runtime.compile
        .mockResolvedValueOnce(preview.result)
        .mockImplementationOnce(() => {
            expect(preview.component.free).toHaveBeenCalledOnce();
            expect(preview.result.free).toHaveBeenCalledOnce();
            expect(canvas.style.visibility).toBe("visible");
            return compilation.promise;
        });
    await controller.initialize("preview", 1);
    const exportPackage = {};
    const initial = controller.validateExport(
        "export",
        1,
        [],
        undefined,
        exportPackage,
    );
    await vi.waitFor(() => expect(runtime.compile).toHaveBeenCalledTimes(2));
    expect(status.dataset).toEqual({ state: "compiling", revision: "1" });
    compilation.resolve(validation);
    await initial;
    expect(validation.free).toHaveBeenCalledOnce();
    expect(status.dataset).toEqual({ state: "ready", revision: "1" });
    expect(diagnostics.dataset.severity).toBe("warning");
    controller.requestRender("preview", 2);
    await vi.waitFor(() =>
        expect(status.dataset).toEqual({ state: "ready", revision: "2" }),
    );
    expect(
        await controller.validateExport(
            undefined,
            2,
            [],
            undefined,
            exportPackage,
        ),
    ).toBe(false);
    expect(runtime.compile).toHaveBeenCalledTimes(2);
    const revalidation = deferred<typeof validation>();
    runtime.compile.mockReturnValueOnce(revalidation.promise);
    controller.requestRender("preview", 3);
    await vi.waitFor(() =>
        expect(status.dataset).toEqual({ state: "ready", revision: "3" }),
    );
    const revalidated = controller.validateExport("different export", 3, []);
    await vi.waitFor(() => expect(runtime.compile).toHaveBeenCalledTimes(3));
    expect(status.dataset).toEqual({ state: "compiling", revision: "3" });
    revalidation.resolve(validation);
    expect(await revalidated).toBe(false);
    await vi.waitFor(() =>
        expect(status.dataset).toEqual({ state: "ready", revision: "3" }),
    );
});

test("export compiler rejection reports a runtime failure and blocks further runtime calls", async () => {
    const { controller, canvas, status, diagnostics } = setup();
    const preview = compiled();
    runtime.compile
        .mockResolvedValueOnce(preview.result)
        .mockRejectedValueOnce(new WebAssembly.RuntimeError("unreachable"));
    await controller.initialize("preview", 1);
    expect(await controller.validateExport("export", 1, [])).toBe(false);
    expect(status.dataset).toEqual({ state: "error", revision: "1" });
    expect(canvas.style.visibility).toBe("hidden");
    expect(diagnostics.textContent).toContain("Export compiler failed");
    expect(diagnostics.textContent).toContain("Restart the plugin");
    controller.requestRender("next", 2);
    await vi.waitFor(() =>
        expect(status.dataset).toEqual({ state: "error", revision: "2" }),
    );
    expect(diagnostics.textContent).toContain("Export compiler failed");
    expect(runtime.compile).toHaveBeenCalledTimes(2);
});

test("registers the bundled font before compiling selected text", async () => {
    const { controller } = setup();
    runtime.compile.mockResolvedValue(compiled().result);
    await controller.initialize(
        'export component Preview inherits Window { Text { text: "Label"; font-family: "Roboto"; } }',
        1,
    );
    expect(runtime.registerFont).toHaveBeenCalledOnce();
    expect(
        (runtime.registerFont.mock.calls[0][0] as Uint8Array).byteLength,
    ).toBeGreaterThan(100_000);
    expect(runtime.registerFont.mock.invocationCallOrder[0]).toBeLessThan(
        runtime.compile.mock.invocationCallOrder[0],
    );
});

test("export validation preserves the shown preview and serializes a newer render", async () => {
    const { controller, canvas, status } = setup();
    const preview = compiled(),
        later = compiled();
    const validation = deferred<ReturnType<typeof compiled>["result"]>();
    runtime.compile
        .mockResolvedValueOnce(preview.result)
        .mockReturnValueOnce(validation.promise)
        .mockResolvedValueOnce(later.result);
    await controller.initialize("preview", 1);
    const result = controller.validateExport("native export", 1, []);
    await vi.waitFor(() => expect(runtime.compile).toHaveBeenCalledTimes(2));
    expect(canvas.style.visibility).toBe("visible");
    expect(status.dataset.state).toBe("compiling");
    controller.requestRender("newer preview", 2);
    expect(runtime.compile).toHaveBeenCalledTimes(2);
    validation.resolve(compiled().result);
    expect(await result).toBe(false);
    await vi.waitFor(() => expect(later.instance.show).toHaveBeenCalledOnce());
    expect(runtime.compile).toHaveBeenCalledTimes(3);
});

test("export timings distinguish compilation, cached validation, and skipped validation", async () => {
    const { controller } = setup();
    let now = 0;
    const clock = vi.spyOn(performance, "now").mockImplementation(() => now);
    try {
        runtime.compile.mockResolvedValue(compiled().result);
        await controller.initialize("preview", 1);
        const onTiming = vi.fn();
        const pkg = {};
        runtime.compile.mockImplementationOnce(async () => {
            now += 17;
            return compiled().result;
        });
        expect(
            await controller.validateExport("export", 1, [], onTiming, pkg),
        ).toBe(true);
        expect(onTiming).toHaveBeenLastCalledWith(
            expect.objectContaining({
                compilationMs: 17,
                totalMs: 17,
                cacheHit: false,
                outcome: "valid",
            }),
        );
        expect(
            await controller.validateExport("export", 1, [], onTiming, pkg),
        ).toBe(true);
        expect(onTiming).toHaveBeenLastCalledWith(
            expect.objectContaining({
                compilationMs: 0,
                cacheHit: true,
                outcome: "valid",
            }),
        );
        expect(
            await controller.validateExport(undefined, 1, [], onTiming),
        ).toBe(true);
        expect(onTiming).toHaveBeenLastCalledWith(
            expect.objectContaining({
                compilationMs: 0,
                cacheHit: false,
                outcome: "skipped",
            }),
        );
        expect(runtime.compile).toHaveBeenCalledTimes(2);
    } finally {
        clock.mockRestore();
    }
});

test("runtime warmup overlaps capture and the first source shares its initialization", async () => {
    const { controller } = setup();
    const runtimeReady = deferred<void>();
    runtime.initialize.mockReturnValueOnce(runtimeReady.promise);
    const warm = controller.warmup();
    runtime.compile.mockResolvedValueOnce(compiled().result);
    const render = controller.initialize("preview", 1);
    expect(runtime.initialize).toHaveBeenCalledOnce();
    expect(runtime.compile).not.toHaveBeenCalled();
    runtimeReady.resolve();
    await warm;
    await render;
    expect(runtime.registerFont).toHaveBeenCalledOnce();
    expect(controller.currentInstance).toBeDefined();
});

test("compiled previews reuse definitions and retain assets until bounded eviction", async () => {
    const { controller } = setup();
    const a = compiled(),
        b = compiled(),
        c = compiled();
    const releaseA = vi.fn(),
        releaseB = vi.fn(),
        releaseC = vi.fn();
    runtime.compile
        .mockResolvedValueOnce(a.result)
        .mockResolvedValueOnce(b.result)
        .mockResolvedValueOnce(c.result);
    await controller.initialize("A", 1, undefined, [], () => releaseA);
    controller.requestRender("B", 2, undefined, [], () => releaseB);
    await vi.waitFor(() => expect(b.instance.show).toHaveBeenCalledOnce());
    controller.requestRender("A", 3, undefined, [], () => releaseA);
    await vi.waitFor(() =>
        expect(a.component.create_with_existing_window).toHaveBeenCalledOnce(),
    );
    expect(runtime.compile).toHaveBeenCalledTimes(2);
    expect(a.component.free).not.toHaveBeenCalled();
    controller.requestRender("C", 4, undefined, [], () => releaseC);
    await vi.waitFor(() => expect(c.instance.show).toHaveBeenCalledOnce());
    expect(releaseB).toHaveBeenCalledOnce();
    expect(b.component.free).toHaveBeenCalledOnce();
    controller.releaseCompiledPreviews();
    expect(releaseA).toHaveBeenCalledOnce();
    expect(releaseC).toHaveBeenCalledOnce();
});

test("immutable export package validation is reused across section changes", async () => {
    const { controller } = setup();
    runtime.compile.mockResolvedValue(compiled().result);
    await controller.initialize("preview", 1);
    const a = {},
        b = {};
    const timing = vi.fn();
    expect(await controller.validateExport("native A", 1, [], timing, a)).toBe(
        true,
    );
    expect(await controller.validateExport("native B", 1, [], timing, b)).toBe(
        true,
    );
    expect(controller.hasValidatedExport(a)).toBe(true);
    expect(await controller.validateExport(undefined, 1, [], timing, a)).toBe(
        true,
    );
    expect(timing).toHaveBeenLastCalledWith(
        expect.objectContaining({
            cacheHit: true,
            compilationMs: 0,
            outcome: "valid",
        }),
    );
    expect(runtime.compile).toHaveBeenCalledTimes(3);
});

test("the current canvas is visible during presentation and a newer selection hides it", async () => {
    const { controller, canvas, status } = setup();
    runtime.compile.mockResolvedValue(compiled().result);
    const frames: FrameRequestCallback[] = [];
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
        frames.push(callback);
        return frames.length;
    });
    const trace = new TimingTraceBuilder(1, "initial").toTrace();
    const initializing = controller.initialize("first", 1, trace);
    await vi.waitFor(() => expect(frames).toHaveLength(1));
    expect(canvas.style.visibility).toBe("visible");
    expect(status.dataset.state).toBe("compiling");
    frames.shift()!(0);
    await vi.waitFor(() => expect(frames).toHaveLength(1));
    controller.reserveRevision(2);
    expect(canvas.style.visibility).toBe("hidden");
    frames.shift()!(0);
    await initializing;
    expect(canvas.style.visibility).toBe("hidden");
    const events = canvas.dispatchEvent.mock.calls.map(([event]) => event);
    const stale = events.find((event) => event.type === "preview-rendered");
    expect(stale).toBeUndefined();
});
