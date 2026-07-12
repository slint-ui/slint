// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { test, expect } from "vitest";

import { DataTransfer, loadSource, private_api } from "../dist/index.js";

test("default DataTransfer is empty", () => {
    const dt = new DataTransfer();
    expect(dt.hasPlainText).toBe(false);
    expect(dt.hasImage).toBe(false);
    expect(dt.plainText).toBeNull();
    expect(dt.image).toBeNull();
    expect(dt.userData).toBeNull();
    expect(dt.isEmpty).toBe(true);
});

test("DataTransfer plain text round-trip", () => {
    const dt = new DataTransfer();
    dt.plainText = "Hello, World!";
    expect(dt.hasPlainText).toBe(true);
    expect(dt.plainText).toBe("Hello, World!");
    expect(dt.isEmpty).toBe(false);
});

test("DataTransfer isEmpty after image assignment", () => {
    const image = new private_api.SlintImageData(4, 4);
    const dt = new DataTransfer();
    dt.image = image;
    expect(dt.isEmpty).toBe(false);
});

test("DataTransfer isEmpty after userData", () => {
    const dt = new DataTransfer();
    dt.userData = { k: 1 };
    expect(dt.isEmpty).toBe(false);
});

test("DataTransfer plainText assignment overwrites", () => {
    const dt = new DataTransfer();
    dt.plainText = "first";
    dt.plainText = "second";
    expect(dt.plainText).toBe("second");
});

test("DataTransfer image round-trip", () => {
    const image = new private_api.SlintImageData(4, 4);
    const dt = new DataTransfer();
    dt.image = image;
    expect(dt.hasImage).toBe(true);
    const fetched = dt.image;
    expect(fetched).not.toBeNull();
    expect(fetched!.width).toBe(image.width);
    expect(fetched!.height).toBe(image.height);
});

test("DataTransfer assigning empty string clears plainText", () => {
    const dt = new DataTransfer();
    dt.plainText = "hello";
    dt.plainText = "";
    expect(dt.hasPlainText).toBe(false);
    expect(dt.plainText).toBeNull();
    expect(dt.isEmpty).toBe(true);
});

test("DataTransfer assigning undefined clears plainText", () => {
    const dt = new DataTransfer();
    dt.plainText = "hello";
    dt.plainText = undefined;
    expect(dt.hasPlainText).toBe(false);
    expect(dt.plainText).toBeNull();
});

test("DataTransfer assigning null clears plainText", () => {
    const dt = new DataTransfer();
    dt.plainText = "hello";
    dt.plainText = null;
    expect(dt.hasPlainText).toBe(false);
    expect(dt.plainText).toBeNull();
});

test("DataTransfer assigning undefined clears image", () => {
    const dt = new DataTransfer();
    dt.image = new private_api.SlintImageData(2, 2);
    dt.image = undefined;
    expect(dt.hasImage).toBe(false);
    expect(dt.image).toBeNull();
});

test("DataTransfer assigning null clears image", () => {
    const dt = new DataTransfer();
    dt.image = new private_api.SlintImageData(2, 2);
    dt.image = null;
    expect(dt.hasImage).toBe(false);
    expect(dt.image).toBeNull();
});

test("DataTransfer userData round-trip plain object", () => {
    const dt = new DataTransfer();
    const payload = { key: "value", n: 42 };
    dt.userData = payload;
    const fetched = dt.userData;
    expect(fetched).toEqual(payload);
    // Same object, not a copy.
    expect(fetched).toBe(payload);
});

test("DataTransfer userData round-trip class instance", () => {
    class Marker {
        constructor(public n: number) {}
    }
    const dt = new DataTransfer();
    const marker = new Marker(5);
    dt.userData = marker;
    const fetched = dt.userData;
    expect(fetched).toBeInstanceOf(Marker);
    expect(fetched).toBe(marker);
});

test("DataTransfer userData overwrites", () => {
    const dt = new DataTransfer();
    dt.userData = { v: "first" };
    dt.userData = { v: "second" };
    expect((dt.userData as { v: string }).v).toBe("second");
});

test("DataTransfer assigning null clears userData", () => {
    const dt = new DataTransfer();
    dt.userData = { k: 1 };
    expect(dt.userData).not.toBeNull();
    dt.userData = null;
    expect(dt.userData).toBeNull();
});

test("DataTransfer assigning undefined clears userData", () => {
    const dt = new DataTransfer();
    dt.userData = { k: 1 };
    dt.userData = undefined;
    expect(dt.userData).toBeNull();
});

test("DataTransfer plain text and userData coexist", () => {
    const dt = new DataTransfer();
    dt.plainText = "hello";
    dt.userData = { k: 1 };
    expect(dt.hasPlainText).toBe(true);
    expect(dt.plainText).toBe("hello");
    expect(dt.userData).toEqual({ k: 1 });
});

test("DataTransfer round-trips through Slint callbacks", () => {
    const ui = loadSource(
        `
        export global Api {
            pure callback identity(data-transfer) -> data-transfer;
            pure callback make-plain(string) -> data-transfer;
            pure callback get-plain(data-transfer) -> string;
        }
        export component App {}
        `,
        "data_transfer_callback.slint",
    );
    const app = new ui.App() as unknown as {
        Api: {
            identity: (dt: DataTransfer) => DataTransfer;
            make_plain: (text: string) => DataTransfer;
            get_plain: (dt: DataTransfer) => string;
        };
    };

    app.Api.identity = (dt) => dt;
    app.Api.make_plain = (text) => {
        const out = new DataTransfer();
        out.plainText = text;
        return out;
    };
    app.Api.get_plain = (dt) => dt.plainText ?? "";

    const source = new DataTransfer();
    source.plainText = "payload";
    const echoed = app.Api.identity(source);
    expect(echoed.plainText).toBe("payload");

    const built = app.Api.make_plain("constructed");
    expect(built.plainText).toBe("constructed");

    expect(app.Api.get_plain(built)).toBe("constructed");
});
