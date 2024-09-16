// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import test from "ava";

import { private_api, ArrayModel } from "../dist/index.js";

test("SlintColor from fromRgb", (t) => {
    const color = private_api.SlintRgbaColor.fromRgb(100, 110, 120);

    t.deepEqual(color.red, 100);
    t.deepEqual(color.green, 110);
    t.deepEqual(color.blue, 120);
});

test("SlintColor from fromArgb", (t) => {
    const color = private_api.SlintRgbaColor.fromArgb(120, 100, 110, 120);

    t.deepEqual(color.red, 100);
    t.deepEqual(color.green, 110);
    t.deepEqual(color.blue, 120);
});

test("SlintColor brighter", (t) => {
    const color = private_api.SlintRgbaColor.fromRgb(100, 110, 120).brighter(
        0.1,
    );

    t.deepEqual(color.red, 110);
    t.deepEqual(color.green, 121);
    t.deepEqual(color.blue, 132);
});

test("SlintColor darker", (t) => {
    const color = private_api.SlintRgbaColor.fromRgb(100, 110, 120).darker(0.1);

    t.deepEqual(color.red, 91);
    t.deepEqual(color.green, 100);
    t.deepEqual(color.blue, 109);
});

test("private_api.SlintBrush from RgbaColor", (t) => {
    const brush = new private_api.SlintBrush({
        red: 100,
        green: 110,
        blue: 120,
        alpha: 255,
    });

    t.deepEqual(brush.color.red, 100);
    t.deepEqual(brush.color.green, 110);
    t.deepEqual(brush.color.blue, 120);

    t.throws(
        () => {
            new private_api.SlintBrush({
                red: -100,
                green: 110,
                blue: 120,
                alpha: 255,
            });
        },
        {
            code: "GenericFailure",
            message: "A channel of Color cannot be negative",
        },
    );
});

test("private_api.SlintBrush from Brush", (t) => {
    const brush = private_api.SlintBrush.fromBrush({
        color: { red: 100, green: 110, blue: 120, alpha: 255 },
    });

    t.deepEqual(brush.color.red, 100);
    t.deepEqual(brush.color.green, 110);
    t.deepEqual(brush.color.blue, 120);

    t.throws(
        () => {
            private_api.SlintBrush.fromBrush({
                color: { red: -100, green: 110, blue: 120, alpha: 255 },
            });
        },
        {
            code: "GenericFailure",
            message: "A channel of Color cannot be negative",
        },
    );
});

test("ArrayModel push", (t) => {
    const arrayModel = new ArrayModel([0]);

    t.is(arrayModel.rowCount(), 1);
    t.is(arrayModel.rowData(0), 0);

    arrayModel.push(2);
    t.is(arrayModel.rowCount(), 2);
    t.is(arrayModel.rowData(1), 2);
});

test("ArrayModel setRowData", (t) => {
    const arrayModel = new ArrayModel([0]);

    t.is(arrayModel.rowCount(), 1);
    t.is(arrayModel.rowData(0), 0);

    arrayModel.setRowData(0, 2);
    t.is(arrayModel.rowCount(), 1);
    t.is(arrayModel.rowData(0), 2);
});

test("ArrayModel remove", (t) => {
    const arrayModel = new ArrayModel([0, 2, 1]);

    t.is(arrayModel.rowCount(), 3);
    t.is(arrayModel.rowData(0), 0);
    t.is(arrayModel.rowData(1), 2);

    arrayModel.remove(0, 2);
    t.is(arrayModel.rowCount(), 1);
    t.is(arrayModel.rowData(0), 1);
});
