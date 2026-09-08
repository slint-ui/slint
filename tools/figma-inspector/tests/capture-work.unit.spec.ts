// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { describe, expect, test } from "vitest";

import { CaptureCache } from "../src/plugin/capture-work";
import {
    captureBusyTime,
    captureScheduler,
    mapCaptureChildren,
} from "../src/plugin/capture-work";

describe("capture-cache", () => {
    test("capture cache reuses successes, evicts to its byte budget and retries failures", async () => {
        const cache = new CaptureCache(12);
        let reads = 0;
        const read = (key: string) =>
            cache.get(
                key,
                async () => {
                    reads++;
                    return new Uint8Array(4);
                },
                (v) => v.byteLength,
            );
        const first = await read("a");
        expect(await read("a")).toBe(first);
        await read("b");
        await read("c");
        await read("a");
        expect(reads).toBe(4);
        await expect(
            cache.get(
                "error",
                async () => {
                    throw Error("retry");
                },
                () => 0,
            ),
        ).rejects.toThrow("retry");
        expect(
            await cache.get(
                "error",
                async () => 42,
                () => 0,
            ),
        ).toBe(42);
    });

    test("invalidation prevents in-flight results from repopulating the cache", async () => {
        const cache = new CaptureCache();
        let finish: (value: number) => void = () => {};
        const pending = cache.get(
            "a",
            () =>
                new Promise<number>((resolve) => {
                    finish = resolve;
                }),
            () => 1,
        );
        cache.clear();
        finish(1);
        await pending;
        expect(
            await cache.get(
                "a",
                async () => 2,
                () => 1,
            ),
        ).toBe(2);
        expect(cache.hits).toBe(0);
    });

    test("capture cache retains false values", async () => {
        const cache = new CaptureCache();
        let loads = 0;
        const read = () =>
            cache.get(
                "false",
                async () => {
                    loads++;
                    return false;
                },
                () => 1,
            );
        expect(await read()).toBe(false);
        expect(await read()).toBe(false);
        expect(loads).toBe(1);
        expect(cache.hits).toBe(1);
    });

    test("replacing an entry accounts for its old size before eviction", () => {
        const cache = new CaptureCache(12);
        expect(cache.put("a", 1, 4)).toBe(true);
        expect(cache.put("b", 2, 4)).toBe(true);
        expect(cache.put("b", 3, 4)).toBe(true);
        expect(cache.peek("a")).toBe(1);
        expect(cache.peek("b")).toBe(3);
        expect(cache.retainedBytes).toBe(12);
    });

    test("text export reuse is invalidated by content, ancestor context, scale and explicit reset", async () => {
        const { captureSource } = await import("../src/plugin/capture");
        const { readFile } = await import("node:fs/promises");
        const png = new Uint8Array(
            await readFile("fixtures/authored/odd-size.png"),
        );
        const text = {
            id: "text",
            name: "Text",
            type: "TEXT",
            visible: true,
            characters: "Hello",
            fontName: { family: "Example Font", style: "Regular" },
            fills: [],
            width: 10,
            height: 10,
        };
        const root = {
            id: "root",
            name: "Root",
            type: "FRAME",
            visible: true,
            opacity: 1,
            children: [text],
        };
        const cache = new CaptureCache();
        let exports = 0;
        const capture = (scale = 1) =>
            captureSource(
                root as unknown as SceneNode,
                Symbol("mixed"),
                async () => "",
                async () => undefined,
                async () => {
                    exports++;
                    return png;
                },
                scale,
                false,
                undefined,
                undefined,
                4,
                true,
                cache,
            );
        const first = await capture();
        expect(exports).toBe(1);
        expect(first.fontMetrics).toEqual({
            requests: 1,
            exports: 1,
            cacheHits: 0,
        });
        const repeated = await capture();
        expect(repeated.source).toEqual(first.source);
        expect(repeated.fontMetrics).toEqual({
            requests: 1,
            exports: 0,
            cacheHits: 1,
        });
        expect(exports).toBe(1);
        text.characters = "Changed";
        await capture();
        expect(exports).toBe(2);
        root.opacity = 0.5;
        await capture();
        expect(exports).toBe(3);
        await capture(2);
        expect(exports).toBe(4);
        cache.clear();
        await capture(2);
        expect(exports).toBe(5);
        Object.defineProperty(text, "resolvedVariableModes", {
            get() {
                throw Error("Unavailable context");
            },
        });
        await capture(2);
        await capture(2);
        expect(exports).toBe(7);
    });
});

describe("capture-scheduler", () => {
    test("capture scheduler bounds overlapping exports and releases failures", async () => {
        const schedule = captureScheduler(2);
        const releases: (() => void)[] = [];
        let active = 0;
        let peak = 0;
        const calls = Array.from({ length: 6 }, (_, index) =>
            schedule(async () => {
                active++;
                peak = Math.max(peak, active);
                await new Promise<void>((resolve) => releases.push(resolve));
                active--;
                if (index === 1) throw Error("export failed");
                return index;
            }),
        );
        const done = Promise.allSettled(calls);
        expect(active).toBe(2);
        for (let index = 0; index < 6; index++) {
            while (!releases.length) await Promise.resolve();
            releases.shift()?.();
            await Promise.resolve();
        }
        expect(
            (await done).filter((r) => r.status === "fulfilled"),
        ).toHaveLength(5);
        expect(peak).toBe(2);
    });

    test("concurrent traversal preserves sibling order and stops scheduling on cancellation", async () => {
        let cancel = false;
        const visited: number[] = [];
        const result = await mapCaptureChildren(
            [0, 1, 2, 3],
            2,
            async (n) => {
                visited.push(n);
                await Promise.resolve();
                cancel = true;
                return n;
            },
            () => cancel,
        );
        expect(visited).toEqual([0, 1]);
        expect(result).toEqual([0, 1]);
    });

    test("concurrent export timing counts overlapping wall time once", () => {
        let now = 0;
        const timing = captureBusyTime(() => now);
        const a = timing.start();
        now = 3;
        const b = timing.start();
        now = 10;
        a();
        now = 15;
        b();
        expect(timing.duration()).toBe(15);
        now = 20;
        const c = timing.start();
        now = 25;
        c();
        expect(timing.duration()).toBe(20);
    });
});
