// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { rgbToHex } from "./property-parsing";
import { expect, test } from "vitest";

test("converts rgb to hex #ffffff", () => {
    const color = rgbToHex({ r: 1, g: 1, b: 1 });
    expect(color).toBe("#ffffff");
});

test("converts rgb to hex floating #ffffff", () => {
    const color = rgbToHex({ r: 1.0, g: 1.0, b: 1.0 });
    expect(color).toBe("#ffffff");
});

test("converts rgb to hex #000000", () => {
    const color = rgbToHex({ r: 0, g: 0, b: 0 });
    expect(color).toBe("#000000");
});

test("converts rgb to hex floating #000000", () => {
    const color = rgbToHex({ r: 0.0, g: 0.0, b: 0.0 });
    expect(color).toBe("#000000");
});

