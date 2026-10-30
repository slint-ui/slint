// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { startPreview } from "../src/plugin/preview-main";

const capture = vi.hoisted(() => ({
    read: vi.fn(),
    variables: vi.fn(),
    dependencies: [] as {
        changed: () => void;
        stop: ReturnType<typeof vi.fn>;
    }[],
}));
vi.mock("../src/plugin/capture", () => ({
    captureSelectionSource: capture.read,
    captureColorProfile: () => "LEGACY",
    sourceVariablesUnchanged: capture.variables,
    collectSelectionNodeIds: (nodes: { id: string }[]) =>
        nodes.map((node) => node.id),
    observeComponentDependencies: (
        _ids: string[],
        _nodes: Set<string>,
        changed: () => void,
    ) => {
        const stop = vi.fn();
        capture.dependencies.push({ changed, stop });
        return stop;
    },
}));

beforeEach(() => {
    vi.resetAllMocks();
    capture.dependencies.length = 0;
    capture.variables.mockResolvedValue(true);
    capture.read.mockImplementation(async (selection: { id: string }[]) => ({
        ok: true,
        source: {
            sourceVersion: 1,
            root: {
                id: selection[0].id,
                name: selection[0].id,
                type: "FRAME",
                properties: {},
                errors: [],
                children: [],
            },
            images: {},
            exportScale: 1,
            pngEnabled: true,
        },
        nodeIds: [selection[0].id],
        captureMetrics: {
            durationMs: 0,
            requests: 0,
            exports: 0,
            cacheHits: 0,
            work: {
                capturedNodes: 1,
                componentFamilies: 0,
                componentVariants: 0,
                pngExports: 2,
                svgExports: 3,
                textCacheHits: 0,
            },
        },
    }));
    vi.stubGlobal("__html__", "");
});
afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
});

function setup() {
    const handlers = new Map<string, () => void>();
    let changed: (event: unknown) => void = () => {};
    const host = {
        currentPage: {
            id: "page",
            selection: [{ id: "Icon Button", name: "Icon Button" }],
            on: (_type: string, callback: typeof changed) => {
                changed = callback;
            },
            off: vi.fn(),
        },
        clientStorage: { getAsync: vi.fn(), setAsync: vi.fn() },
        ui: {
            onmessage: undefined as ((message: unknown) => void) | undefined,
            postMessage: vi.fn(),
        },
        on: (type: string, callback: () => void) =>
            handlers.set(type, callback),
        showUI: vi.fn(),
        mixed: Symbol("mixed"),
    };
    vi.stubGlobal("figma", host);
    startPreview();
    host.ui.onmessage?.({ type: "ui-ready", devicePixelRatio: 1 });
    const captures = () =>
        host.ui.postMessage.mock.calls
            .map(([message]) => message)
            .filter(
                (message) =>
                    message.type === "preview-capture" ||
                    message.type === "preview-reuse",
            );
    const select = async (id: string, expected: number) => {
        host.currentPage.selection = [{ id, name: id }];
        handlers.get("selectionchange")?.();
        await vi.waitFor(() => expect(captures()).toHaveLength(expected));
    };
    return {
        host,
        handlers,
        captures,
        select,
        nodeChange: () => changed({ nodeChanges: [] }),
    };
}

test("first and repeat selections retain the complete tree without capture fallback", async () => {
    const p = setup();
    await vi.waitFor(() => expect(p.captures()).toHaveLength(1));
    await p.select("Buttons", 2);
    await p.select("Icon Button", 3);
    expect(capture.read).toHaveBeenCalledTimes(2);
    expect(p.captures()[2]).toMatchObject({
        type: "preview-reuse",
        captureId: p.captures()[0].captureId,
    });
    expect(p.captures()[2].captureJson).toBeUndefined();
    expect(p.captures()[2].trace.phases.captureSerialization).toBe(0);
    const metrics = p.captures()[2].trace.captureMetrics;
    expect(metrics).toMatchObject({
        sourceCacheHit: true,
        work: { capturedNodes: 1, pngExports: 0, svgExports: 0 },
    });
    expect(capture.dependencies[0].stop).not.toHaveBeenCalled();
});

test("page, style, and cached off-page component changes invalidate full captures", async () => {
    const p = setup();
    await vi.waitFor(() => expect(p.captures()).toHaveLength(1));
    await p.select("Buttons", 2);
    capture.dependencies[0].changed();
    expect(capture.dependencies[0].stop).toHaveBeenCalledOnce();
    await p.select("Icon Button", 3);
    expect(capture.read).toHaveBeenCalledTimes(3);
    p.nodeChange();
    await p.select("Icon Button", 4);
    expect(capture.read).toHaveBeenCalledTimes(4);
    p.handlers.get("stylechange")?.();
    await p.select("Icon Button", 5);
    expect(capture.read).toHaveBeenCalledTimes(5);
});

test("changed variable values force a fresh capture on repeat selection", async () => {
    const p = setup();
    await vi.waitFor(() => expect(p.captures()).toHaveLength(1));
    capture.variables.mockResolvedValueOnce(false);
    await p.select("Icon Button", 2);
    expect(capture.read).toHaveBeenCalledTimes(2);
    expect(p.captures()[1].trace.captureMetrics.sourceCacheHit).toBe(false);
});

test("a capture exceeding the former deadline finishes once without restarting or rasterizing", async () => {
    vi.useFakeTimers();
    const complete = capture.read.getMockImplementation();
    if (!complete) throw Error("Missing capture implementation");
    capture.read.mockImplementationOnce(async (...args: unknown[]) => {
        await new Promise((resolve) => setTimeout(resolve, 35000));
        return complete?.(...args);
    });
    const p = setup();
    await vi.advanceTimersByTimeAsync(21000);
    expect(capture.read).toHaveBeenCalledOnce();
    expect(p.captures()).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(14000);
    expect(capture.read).toHaveBeenCalledOnce();
    expect(p.captures()).toHaveLength(1);
});

test("a repeated selection cannot inherit the cancelled result of an older in-flight capture", async () => {
    const complete = capture.read.getMockImplementation();
    if (!complete) throw Error("Missing capture implementation");
    let finish: () => void = () => {};
    capture.read.mockImplementationOnce(
        () =>
            new Promise((resolve) => {
                finish = () =>
                    resolve({
                        ok: false,
                        nodeIds: [],
                        captureMetrics: {
                            durationMs: 0,
                            requests: 0,
                            exports: 0,
                            cacheHits: 0,
                        },
                        diagnostics: [
                            {
                                code: "CAPTURE_ERROR",
                                severity: "error",
                                message: "Capture cancelled",
                            },
                        ],
                    });
            }),
    );
    const p = setup();
    expect(capture.read).toHaveBeenCalledOnce();
    capture.read.mockImplementation(complete);
    await p.select("Icon Button", 1);
    finish();
    await Promise.resolve();
    expect(capture.read).toHaveBeenCalledTimes(2);
    expect(p.captures()).toHaveLength(1);
    expect(p.captures()[0].revision).toBe(2);
    expect(
        p.host.ui.postMessage.mock.calls
            .map(([message]) => message)
            .filter((message) => message.type === "preview-diagnostics"),
    ).toHaveLength(0);
});

test("an evicted UI result requests the full current capture without reading Figma again", async () => {
    const p = setup();
    await vi.waitFor(() => expect(p.captures()).toHaveLength(1));
    await p.select("Icon Button", 2);
    const reused = p.captures()[1];
    p.host.ui.onmessage?.({
        type: "capture-reuse-miss",
        revision: 1,
        captureId: reused.captureId,
    });
    expect(p.captures()).toHaveLength(2);
    p.host.ui.onmessage?.({
        type: "capture-reuse-miss",
        revision: 2,
        captureId: reused.captureId,
    });
    expect(p.captures()[2]).toMatchObject({
        type: "preview-capture",
        revision: 2,
        captureId: reused.captureId,
    });
    expect(capture.read).toHaveBeenCalledOnce();
});
