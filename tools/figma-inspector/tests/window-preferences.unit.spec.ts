// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import {
    WindowPreferences,
    isWindowSize,
} from "../src/plugin/window-preferences";

test("restores only valid saved dimensions and recovers from storage failures", async () => {
    for (const value of [
        undefined,
        null,
        {},
        { width: 499, height: 600 },
        { width: 640.5, height: 600 },
        { width: 640, height: Infinity },
        { width: 9000, height: 600 },
    ]) {
        const preferences = new WindowPreferences({
            getAsync: async () => value,
            setAsync: async () => {},
        });
        expect(await preferences.load()).toEqual({ width: 640, height: 640 });
        expect(isWindowSize(value)).toBe(false);
    }
    const preferences = new WindowPreferences({
        getAsync: async () => {
            throw Error("Unavailable");
        },
        setAsync: async () => {},
    });
    expect(await preferences.load()).toEqual({ width: 640, height: 640 });
    const saved = new WindowPreferences({
        getAsync: async () => ({ width: 920, height: 700 }),
        setAsync: async () => {},
    });
    expect(await saved.load()).toEqual({ width: 920, height: 700 });
});

test("resize writes stay ordered and a rejected write does not block subsequent sizes", async () => {
    const writes: unknown[] = [];
    let release: () => void = () => {};
    const first = new Promise<void>((resolve) => {
        release = resolve;
    });
    const preferences = new WindowPreferences({
        getAsync: async () => undefined,
        setAsync: async (_key, value) => {
            writes.push(value);
            if (writes.length === 1) {
                await first;
                throw Error("Failed");
            }
        },
    });
    preferences.save({ width: 700, height: 500 });
    preferences.save({ width: 900, height: 700 });
    await Promise.resolve();
    expect(writes).toEqual([{ width: 700, height: 500 }]);
    release();
    await expect.poll(() => writes.length).toBe(2);
    expect(writes[1]).toEqual({ width: 900, height: 700 });
});
