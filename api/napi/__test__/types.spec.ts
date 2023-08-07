// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

import test from 'ava';

import { Brush, Color } from '../index'

test('Color from fromRgb', (t) => {
    let color = Color.fromRgb(100, 110, 120);

    t.deepEqual(color.red, 100);
    t.deepEqual(color.green, 110);
    t.deepEqual(color.blue, 120);
})

test('Color from fromArgb', (t) => {
    let color = Color.fromArgb(120, 100, 110, 120);

    t.deepEqual(color.red, 100);
    t.deepEqual(color.green, 110);
    t.deepEqual(color.blue, 120);
    t.deepEqual(color.asArgbEncoded, 2019847800);
})

test('Color from fromArgbEncoded', (t) => {
    let color = Color.fromArgbEncoded(2019847800);

    t.deepEqual(color.red, 100);
    t.deepEqual(color.green, 110);
    t.deepEqual(color.blue, 120);
})

test('Color brighter', (t) => {
    let color = Color.fromRgb(100, 110, 120).brighter(0.1);

    t.deepEqual(color.red, 110);
    t.deepEqual(color.green, 121);
    t.deepEqual(color.blue, 132);
})

test('Color darker', (t) => {
    let color = Color.fromRgb(100, 110, 120).darker(0.1);

    t.deepEqual(color.red, 91);
    t.deepEqual(color.green, 100);
    t.deepEqual(color.blue, 109);
})

test('Brush from Color', (t) => {
    let brush = Brush.fromColor(Color.fromRgb(100, 110, 120));

    t.deepEqual(brush.color.red, 100);
    t.deepEqual(brush.color.green, 110);
    t.deepEqual(brush.color.blue, 120);
})

