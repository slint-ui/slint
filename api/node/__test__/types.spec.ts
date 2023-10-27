// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

import test from 'ava';

import { Brush, ArrayModel, Timer, private_api } from '../index'

test('Color from fromRgb', (t) => {
  let color = private_api.SlintColor.fromRgb(100, 110, 120);

  t.deepEqual(color.red, 100);
  t.deepEqual(color.green, 110);
  t.deepEqual(color.blue, 120);
})

test('Color from fromArgb', (t) => {
  let color = private_api.SlintColor.fromArgb(120, 100, 110, 120);

  t.deepEqual(color.red, 100);
  t.deepEqual(color.green, 110);
  t.deepEqual(color.blue, 120);
  t.deepEqual(color.asArgbEncoded, 2019847800);
})

test('Color from fromArgbEncoded', (t) => {
  let color = private_api.SlintColor.fromArgbEncoded(2019847800);

  t.deepEqual(color.red, 100);
  t.deepEqual(color.green, 110);
  t.deepEqual(color.blue, 120);
})

test('Color brighter', (t) => {
  let color = private_api.SlintColor.fromRgb(100, 110, 120).brighter(0.1);

  t.deepEqual(color.red, 110);
  t.deepEqual(color.green, 121);
  t.deepEqual(color.blue, 132);
})

test('Color darker', (t) => {
  let color = private_api.SlintColor.fromRgb(100, 110, 120).darker(0.1);

  t.deepEqual(color.red, 91);
  t.deepEqual(color.green, 100);
  t.deepEqual(color.blue, 109);
})

test('Brush from Color', (t) => {
  let brush = new Brush({ red: 100, green: 110, blue: 120, alpha: 255 });

  t.deepEqual(brush.color.red, 100);
  t.deepEqual(brush.color.green, 110);
  t.deepEqual(brush.color.blue, 120);

  t.throws(() => {
    new Brush({ red: -100, green: 110, blue: 120, alpha: 255 })
  },
    {
      code: "GenericFailure",
      message: "A channel of Color cannot be negative"
    }
  );
})

test('ArrayModel push', (t) => {
  let arrayModel = new ArrayModel([0]);

  t.is(arrayModel.rowCount(), 1);
  t.is(arrayModel.rowData(0), 0);

  arrayModel.push(2);
  t.is(arrayModel.rowCount(), 2);
  t.is(arrayModel.rowData(1), 2);
})

test('ArrayModel setRowData', (t) => {
  let arrayModel = new ArrayModel([0]);

  t.is(arrayModel.rowCount(), 1);
  t.is(arrayModel.rowData(0), 0);

  arrayModel.setRowData(0, 2);
  t.is(arrayModel.rowCount(), 1);
  t.is(arrayModel.rowData(0), 2);
})

test('ArrayModel remove', (t) => {
  let arrayModel = new ArrayModel([0, 2, 1]);

  t.is(arrayModel.rowCount(), 3);
  t.is(arrayModel.rowData(0), 0);
  t.is(arrayModel.rowData(1), 2);

  arrayModel.remove(0, 2);
  t.is(arrayModel.rowCount(), 1);
  t.is(arrayModel.rowData(0), 1);
})

test('Timer negative duration', (t) => {
  t.throws(() => {
    Timer.singleShot(-1, function () { })
  },
    {
      code: "GenericFailure",
      message: "Duration cannot be negative"
    }
  );
})

