// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { test, expect } from "vitest";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

import {
    loadFile,
    loadSource,
    CompileError,
    ArrayModel,
    private_api,
    Model,
} from "../dist/index.js";

private_api.initTesting();

test("ArrayModel.insert at start, middle, and end", () => {
    const m = new ArrayModel<number>([1, 2, 3]);
    m.insert(0, 0);
    expect([...m.values()]).toEqual([0, 1, 2, 3]);
    m.insert(2, 99);
    expect([...m.values()]).toEqual([0, 1, 99, 2, 3]);
    m.insert(m.rowCount(), 100);
    expect([...m.values()]).toEqual([0, 1, 99, 2, 3, 100]);
    expect(m.rowCount()).toBe(6);
});

test("ArrayModel.insert clamps out-of-range indices", () => {
    const m = new ArrayModel<number>([1, 2, 3]);
    m.insert(-5, 7);
    expect([...m.values()]).toEqual([7, 1, 2, 3]);
    m.insert(100, 8);
    expect([...m.values()]).toEqual([7, 1, 2, 3, 8]);
});

test("ArrayModel.insert accepts multiple values", () => {
    const m = new ArrayModel<number>([1, 4]);
    m.insert(1, 2, 3);
    expect([...m.values()]).toEqual([1, 2, 3, 4]);
});

test("ArrayModel.insert into empty model", () => {
    const m = new ArrayModel<number>([]);
    m.insert(0, 42);
    expect([...m.values()]).toEqual([42]);
});

test("ArrayModel.insert notifies the run-time", () => {
    const source = `
    export component App {
      in-out property <[int]> data;
      out property <int> total: data.length > 0 ? data[0] + data[data.length - 1] : 0;
    }`;

    const demo = loadSource(source, "test.slint") as any;
    const instance = new demo.App();
    const m = new ArrayModel<number>([10, 20]);
    instance.data = m;
    expect(instance.total).toBe(30);
    m.insert(0, 5);
    expect(instance.total).toBe(25);
    m.insert(m.rowCount(), 100);
    expect(instance.total).toBe(105);
});

test("MapModel notify rowChanged", () => {
    const source = `
    export component App {

      in-out property <[string]> model;
      in-out property <string> changed-items;

      for item in root.model : Text {
          text: item;

          changed text => {
              root.changed-items += self.text;
          }
      }
    }`;

    const path = "api.spec.ts";

    const demo = loadSource(source, path) as any;
    const instance = new demo.App();

    interface Name {
        first: string;
        last: string;
    }

    const nameModel: ArrayModel<Name> = new ArrayModel([
        { first: "Hans", last: "Emil" },
        { first: "Max", last: "Mustermann" },
        { first: "Roman", last: "Tisch" },
    ]);

    const mapModel = new private_api.MapModel(nameModel, (data) => {
        return data.last + ", " + data.first;
    });

    instance.model = mapModel;

    private_api.send_mouse_click(instance, 5, 5);

    nameModel.setRowData(0, { first: "Simon", last: "Hausmann" });
    nameModel.setRowData(1, { first: "Olivier", last: "Goffart" });

    private_api.send_mouse_click(instance, 5, 5);

    expect(instance.changed_items).toBe("Goffart, OlivierHausmann, Simon");
});
