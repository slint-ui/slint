// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { test, expect } from "vitest";

import { private_api, ArrayModel } from "../dist/index.js";

test("SlintColor from fromRgb", () => {
    const color = private_api.SlintRgbaColor.fromRgb(100, 110, 120);

    expect(color.red).toStrictEqual(100);
    expect(color.green).toStrictEqual(110);
    expect(color.blue).toStrictEqual(120);
});

test("SlintColor from fromArgb", () => {
    const color = private_api.SlintRgbaColor.fromArgb(120, 100, 110, 120);

    expect(color.red).toStrictEqual(100);
    expect(color.green).toStrictEqual(110);
    expect(color.blue).toStrictEqual(120);
});

test("SlintColor brighter", () => {
    const color = private_api.SlintRgbaColor.fromRgb(100, 110, 120).brighter(
        0.1,
    );

    expect(color.red).toStrictEqual(110);
    expect(color.green).toStrictEqual(121);
    expect(color.blue).toStrictEqual(132);
});

test("SlintColor darker", () => {
    const color = private_api.SlintRgbaColor.fromRgb(100, 110, 120).darker(0.1);

    expect(color.red).toStrictEqual(91);
    expect(color.green).toStrictEqual(100);
    expect(color.blue).toStrictEqual(109);
});

test("private_api.SlintBrush from RgbaColor", () => {
    const brush = new private_api.SlintBrush({
        red: 100,
        green: 110,
        blue: 120,
        alpha: 255,
    });

    expect(brush.color.red).toStrictEqual(100);
    expect(brush.color.green).toStrictEqual(110);
    expect(brush.color.blue).toStrictEqual(120);

    let thrownError: any;
    try {
        new private_api.SlintBrush({
            red: -100,
            green: 110,
            blue: 120,
            alpha: 255,
        });
    } catch (error) {
        thrownError = error;
    }
    expect(thrownError).toBeDefined();
    expect(thrownError.code).toBe("GenericFailure");
    expect(thrownError.message).toBe("A channel of Color cannot be negative");
});

test("private_api.SlintBrush from Brush", () => {
    const brush = private_api.SlintBrush.fromBrush({
        color: { red: 100, green: 110, blue: 120, alpha: 255 },
    });

    expect(brush.color.red).toStrictEqual(100);
    expect(brush.color.green).toStrictEqual(110);
    expect(brush.color.blue).toStrictEqual(120);

    let thrownError: any;
    try {
        private_api.SlintBrush.fromBrush({
            color: { red: -100, green: 110, blue: 120, alpha: 255 },
        });
    } catch (error) {
        thrownError = error;
    }
    expect(thrownError).toBeDefined();
    expect(thrownError.code).toBe("GenericFailure");
    expect(thrownError.message).toBe("A channel of Color cannot be negative");
});

test("ArrayModel push", () => {
    const arrayModel = new ArrayModel([0]);

    expect(arrayModel.rowCount()).toBe(1);
    expect(arrayModel.rowData(0)).toBe(0);

    arrayModel.push(2);
    expect(arrayModel.rowCount()).toBe(2);
    expect(arrayModel.rowData(1)).toBe(2);
});

test("ArrayModel setRowData", () => {
    const arrayModel = new ArrayModel([0]);

    expect(arrayModel.rowCount()).toBe(1);
    expect(arrayModel.rowData(0)).toBe(0);

    arrayModel.setRowData(0, 2);
    expect(arrayModel.rowCount()).toBe(1);
    expect(arrayModel.rowData(0)).toBe(2);
});

test("ArrayModel remove", () => {
    const arrayModel = new ArrayModel([0, 2, 1]);

    expect(arrayModel.rowCount()).toBe(3);
    expect(arrayModel.rowData(0)).toBe(0);
    expect(arrayModel.rowData(1)).toBe(2);

    arrayModel.remove(0, 2);
    expect(arrayModel.rowCount()).toBe(1);
    expect(arrayModel.rowData(0)).toBe(1);
});
