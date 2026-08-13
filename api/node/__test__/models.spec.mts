// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { test, expect, vi } from "vitest";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

import {
    loadFile,
    loadSource,
    CompileError,
    ArrayModel,
    FilterModel,
    MapModel,
    SortModel,
    ReverseModel,
    private_api,
    Model,
} from "../dist/index.js";

private_api.initTesting();

test("ArrayModel.splice inserts at start, middle, and end", () => {
    const m = new ArrayModel<number>([1, 2, 3]);
    m.splice(0, 0, 0);
    expect([...m.values()]).toEqual([0, 1, 2, 3]);
    m.splice(2, 0, 99);
    expect([...m.values()]).toEqual([0, 1, 99, 2, 3]);
    m.splice(m.rowCount(), 0, 100);
    expect([...m.values()]).toEqual([0, 1, 99, 2, 3, 100]);
    expect(m.rowCount()).toBe(6);
});

test("ArrayModel.splice removes and returns elements", () => {
    const m = new ArrayModel<number>([1, 2, 3, 4, 5]);
    expect(m.splice(1, 2)).toEqual([2, 3]);
    expect([...m.values()]).toEqual([1, 4, 5]);
    // Omitted deleteCount removes everything from `start` to the end.
    expect(m.splice(1)).toEqual([4, 5]);
    expect([...m.values()]).toEqual([1]);
});

test("ArrayModel.splice replaces elements", () => {
    const m = new ArrayModel<number>([1, 2, 3, 4]);
    expect(m.splice(1, 2, 20, 30)).toEqual([2, 3]);
    expect([...m.values()]).toEqual([1, 20, 30, 4]);
});

test("ArrayModel.splice handles out-of-range indices like Array.prototype.splice", () => {
    const m = new ArrayModel<number>([1, 2, 3]);
    m.splice(-1, 0, 7);
    expect([...m.values()]).toEqual([1, 2, 7, 3]);
    m.splice(-100, 0, 8);
    expect([...m.values()]).toEqual([8, 1, 2, 7, 3]);
    m.splice(100, 1, 9);
    expect([...m.values()]).toEqual([8, 1, 2, 7, 3, 9]);
});

test("ArrayModel.splice into empty model", () => {
    const m = new ArrayModel<number>([]);
    expect(m.splice(0, 0, 42)).toEqual([]);
    expect([...m.values()]).toEqual([42]);
});

test("ArrayModel.splice notifies the run-time", () => {
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
    m.splice(0, 0, 5);
    expect(instance.total).toBe(25);
    m.splice(m.rowCount(), 0, 100);
    expect(instance.total).toBe(105);
    m.splice(0, 1, 7);
    expect(instance.total).toBe(107);
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

    const mapModel = new MapModel(nameModel, (data) => {
        return data.last + ", " + data.first;
    });

    instance.model = mapModel;

    private_api.send_mouse_click(instance, 5, 5);

    nameModel.setRowData(0, { first: "Simon", last: "Hausmann" });
    nameModel.setRowData(1, { first: "Olivier", last: "Goffart" });

    private_api.send_mouse_click(instance, 5, 5);

    expect(instance.changed_items).toBe("Goffart, OlivierHausmann, Simon");
});

test("MapModel handles an empty source model", () => {
    const source = new ArrayModel<number>([]);
    const mapped = new MapModel(source, (x) => x * 2);
    expect(mapped.rowCount()).toBe(0);
    expect(mapped.rowData(0)).toBeUndefined();
});

test("MapModel shares its notification channel with the source model", () => {
    // MapModel intentionally reuses the source model's modelNotify to get
    // automatic row change propagation for free (see MapModel's class docs).
    // A consequence is that resetting one MapModel, or mutating the source,
    // notifies every view sharing that channel, including sibling MapModels
    // wrapping the same source model instance.
    const source = new ArrayModel([1, 2, 3]);
    const doubled = new MapModel(source, (x) => x * 2);
    const tripled = new MapModel(source, (x) => x * 3);
    expect(doubled.modelNotify).toBe(source.modelNotify);
    expect(tripled.modelNotify).toBe(source.modelNotify);
});

test("MapModel forwards row-added and row-removed notifications from the source model", () => {
    // Unlike FilterModel/SortModel/ReverseModel, MapModel needs no reset()
    // after a direct source mutation: since it shares the source's
    // notification channel directly, row-added/row-removed events reach the
    // run-time immediately.
    const source = `
    export component App {
      in-out property <[int]> data;
      out property <int> count: data.length;
      out property <int> last: data.length > 0 ? data[data.length - 1] : 0;
    }`;

    const demo = loadSource(source, "test.slint") as any;
    const instance = new demo.App();
    const sourceModel = new ArrayModel<number>([1, 2, 3]);
    const mapped = new MapModel(sourceModel, (x) => x * 10);
    instance.data = mapped;
    expect(instance.count).toBe(3);
    expect(instance.last).toBe(30);

    sourceModel.push(4);
    expect(instance.count).toBe(4);
    expect(instance.last).toBe(40);

    sourceModel.remove(0, 2);
    expect(instance.count).toBe(2);
    expect(instance.last).toBe(40);
});

test("FilterModel filters rows from the source model", () => {
    const source = new ArrayModel([1, 2, 3, 4, 5, 6]);
    const even = new FilterModel(source, (x) => x % 2 === 0);
    expect(even.rowCount()).toBe(3);
    expect(Array.from(even)).toEqual([2, 4, 6]);
});

test("FilterModel does not evaluate the filter function until the mapping is first needed", () => {
    const source = new ArrayModel([1, 2, 3]);
    let called = false;
    const filtered = new FilterModel(source, (x) => {
        called = true;
        return x % 2 === 0;
    });
    expect(called).toBe(false);
    filtered.rowCount();
    expect(called).toBe(true);
});

test("FilterModel.unfilteredRow maps back to the source index", () => {
    const source = new ArrayModel(["a", "b", "c", "d"]);
    const filtered = new FilterModel(source, (s) => s !== "b");
    expect(Array.from(filtered)).toEqual(["a", "c", "d"]);
    expect(filtered.unfilteredRow(0)).toBe(0);
    expect(filtered.unfilteredRow(1)).toBe(2);
    expect(filtered.unfilteredRow(2)).toBe(3);
});

test("FilterModel.setRowData updates the source model and re-applies the filter", () => {
    const source = new ArrayModel([1, 2, 3, 4]);
    const even = new FilterModel(source, (x) => x % 2 === 0);
    expect(Array.from(even)).toEqual([2, 4]);
    even.setRowData(0, 5);
    expect(Array.from(even)).toEqual([4]);
    expect(Array.from(source)).toEqual([1, 5, 3, 4]);
});

test("FilterModel.setRowData notifies a row change (not a full reset) when the row stays visible", () => {
    const source = new ArrayModel([2, 4, 6]);
    const even = new FilterModel(source, (x) => x % 2 === 0);
    const resetSpy = vi.spyOn(even as any, "notifyReset");
    const changedSpy = vi.spyOn(even as any, "notifyRowDataChanged");
    even.setRowData(1, 40);
    expect(resetSpy).not.toHaveBeenCalled();
    expect(changedSpy).toHaveBeenCalledWith(1);
    expect(Array.from(even)).toEqual([2, 40, 6]);
});

test("FilterModel.setRowData notifies a row removal (not a full reset) when the row is filtered out", () => {
    const source = new ArrayModel([2, 4, 6]);
    const even = new FilterModel(source, (x) => x % 2 === 0);
    const resetSpy = vi.spyOn(even as any, "notifyReset");
    const removedSpy = vi.spyOn(even as any, "notifyRowRemoved");
    even.setRowData(1, 5);
    expect(resetSpy).not.toHaveBeenCalled();
    expect(removedSpy).toHaveBeenCalledWith(1, 1);
    expect(Array.from(even)).toEqual([2, 6]);
    expect(Array.from(source)).toEqual([2, 5, 6]);
});

test("FilterModel.setRowData notifies with the filtered row index, not the source row index", () => {
    // Regression test: source row 3 (value 4) maps to filtered row 1, since
    // source rows 0 and 2 (values 1, 3) are filtered out. A bug that passed
    // the source-space index to the notify calls instead of the filtered-space
    // index would not be caught by a source model where row === sourceRow.
    const source = new ArrayModel([1, 2, 3, 4, 5, 6]);
    const even = new FilterModel(source, (x) => x % 2 === 0);
    expect(Array.from(even)).toEqual([2, 4, 6]);

    const changedSpy = vi.spyOn(even as any, "notifyRowDataChanged");
    even.setRowData(1, 40);
    expect(changedSpy).toHaveBeenCalledWith(1);
    expect(Array.from(even)).toEqual([2, 40, 6]);
    expect(Array.from(source)).toEqual([1, 2, 3, 40, 5, 6]);

    const removedSpy = vi.spyOn(even as any, "notifyRowRemoved");
    even.setRowData(1, 7);
    expect(removedSpy).toHaveBeenCalledWith(1, 1);
    expect(Array.from(even)).toEqual([2, 6]);
    expect(Array.from(source)).toEqual([1, 2, 3, 7, 5, 6]);
});

test("FilterModel.setRowData re-reads the committed value instead of trusting the caller-supplied data", () => {
    // A source model that ignores writes (like the base Model.setRowData
    // default) must not desync the cached mapping: the filter decision has to
    // be based on what the source model actually reports afterwards.
    class ReadOnlyModel extends Model<number> {
        #array: number[];
        constructor(arr: number[]) {
            super();
            this.#array = arr;
        }
        rowCount(): number {
            return this.#array.length;
        }
        rowData(row: number): number | undefined {
            return this.#array[row];
        }
        // setRowData intentionally left as the read-only no-op default.
    }
    const source = new ReadOnlyModel([2, 4, 6]);
    const even = new FilterModel(source, (x) => x % 2 === 0);
    expect(Array.from(even)).toEqual([2, 4, 6]);
    // The write is ignored by the source model, so despite passing an odd
    // number, the committed value (4) is still even and the row must stay.
    even.setRowData(1, 5);
    expect(Array.from(even)).toEqual([2, 4, 6]);
});

test("FilterModel treats an in-range row that returns undefined as filtered out", () => {
    // Mirrors Rust/C++'s handling of a "broken" model that reports a row
    // count larger than the data it can actually provide.
    class BrokenModel extends Model<number> {
        rowCount(): number {
            return 3;
        }
        rowData(row: number): number | undefined {
            return row === 1 ? undefined : row * 10;
        }
    }
    const broken = new BrokenModel();
    const filtered = new FilterModel(broken, () => true);
    expect(Array.from(filtered)).toEqual([0, 20]);
});

test("FilterModel handles an empty source model", () => {
    const source = new ArrayModel<number>([]);
    const even = new FilterModel(source, (x) => x % 2 === 0);
    expect(even.rowCount()).toBe(0);
    expect(even.rowData(0)).toBeUndefined();
    expect(even.unfilteredRow(0)).toBeUndefined();
});

test("FilterModel.rowData and setRowData ignore out-of-range rows", () => {
    const source = new ArrayModel([2, 4, 6]);
    const even = new FilterModel(source, (x) => x % 2 === 0);
    expect(even.rowData(-1)).toBeUndefined();
    expect(even.rowData(10)).toBeUndefined();
    even.setRowData(-1, 100);
    even.setRowData(10, 100);
    expect(Array.from(source)).toEqual([2, 4, 6]);
});

test("FilterModel.reset refreshes the view after the source model is mutated directly", () => {
    const source = new ArrayModel([1, 2, 3]);
    const even = new FilterModel(source, (x) => x % 2 === 0);
    expect(Array.from(even)).toEqual([2]);
    source.push(4, 6);
    // The filtered view is cached until reset() is called.
    expect(Array.from(even)).toEqual([2]);
    even.reset();
    expect(Array.from(even)).toEqual([2, 4, 6]);
});

test("FilterModel notifies the run-time", () => {
    const source = `
    export component App {
      in-out property <[int]> data;
      out property <int> total: data.length > 0 ? data[0] + data[data.length - 1] : 0;
    }`;

    const demo = loadSource(source, "test.slint") as any;
    const instance = new demo.App();
    const sourceModel = new ArrayModel<number>([1, 2, 3, 4]);
    const evens = new FilterModel(sourceModel, (x) => x % 2 === 0);
    instance.data = evens;
    expect(instance.total).toBe(6);

    evens.setRowData(0, 10);
    expect(instance.total).toBe(14);

    sourceModel.push(5, 6);
    evens.reset();
    expect(instance.total).toBe(16);
});

test("SortModel sorts rows from the source model", () => {
    const source = new ArrayModel([5, 3, 1, 4, 2]);
    const sorted = new SortModel(source, (a, b) => a - b);
    expect(sorted.rowCount()).toBe(5);
    expect(Array.from(sorted)).toEqual([1, 2, 3, 4, 5]);
});

test("SortModel does not evaluate the compare function until the mapping is first needed", () => {
    const source = new ArrayModel([3, 1, 2]);
    let called = false;
    const sorted = new SortModel(source, (a, b) => {
        called = true;
        return a - b;
    });
    expect(called).toBe(false);
    sorted.rowCount();
    expect(called).toBe(true);
});

test("SortModel.unsortedRow maps back to the source index", () => {
    const source = new ArrayModel(["banana", "apple", "cherry"]);
    const sorted = new SortModel(source, (a, b) => a.localeCompare(b));
    expect(Array.from(sorted)).toEqual(["apple", "banana", "cherry"]);
    expect(sorted.unsortedRow(0)).toBe(1);
    expect(sorted.unsortedRow(1)).toBe(0);
    expect(sorted.unsortedRow(2)).toBe(2);
});

test("SortModel.setRowData updates the source model and re-applies the sort order", () => {
    const source = new ArrayModel([3, 1, 2]);
    const sorted = new SortModel(source, (a, b) => a - b);
    expect(Array.from(sorted)).toEqual([1, 2, 3]);
    sorted.setRowData(0, 10);
    expect(Array.from(sorted)).toEqual([2, 3, 10]);
    expect(Array.from(source)).toEqual([3, 10, 2]);
});

test("SortModel.setRowData notifies a row change (not a full reset) when the sorted position is unchanged", () => {
    const source = new ArrayModel([1, 2, 3, 4, 5]);
    const sorted = new SortModel(source, (a, b) => a - b);
    const resetSpy = vi.spyOn(sorted as any, "notifyReset");
    const changedSpy = vi.spyOn(sorted as any, "notifyRowDataChanged");
    // Row 2 holds value 3; changing it to 3.5 still sorts between 2 and 4.
    sorted.setRowData(2, 3.5);
    expect(resetSpy).not.toHaveBeenCalled();
    expect(changedSpy).toHaveBeenCalledWith(2);
    expect(Array.from(sorted)).toEqual([1, 2, 3.5, 4, 5]);
});

test("SortModel.setRowData notifies a row move (not a full reset) when the sorted position changes", () => {
    const source = new ArrayModel([1, 2, 3, 4, 5]);
    const sorted = new SortModel(source, (a, b) => a - b);
    const resetSpy = vi.spyOn(sorted as any, "notifyReset");
    const removedSpy = vi.spyOn(sorted as any, "notifyRowRemoved");
    const addedSpy = vi.spyOn(sorted as any, "notifyRowAdded");
    // Row 2 holds value 3; changing it to 10 moves it to the end.
    sorted.setRowData(2, 10);
    expect(resetSpy).not.toHaveBeenCalled();
    expect(removedSpy).toHaveBeenCalledWith(2, 1);
    expect(addedSpy).toHaveBeenCalledWith(4, 1);
    expect(Array.from(sorted)).toEqual([1, 2, 4, 5, 10]);
});

test("SortModel.setRowData notifies with the sorted row index, not the source row index", () => {
    // Regression test: source is deliberately not already in sorted order, so
    // the sorted-space row index diverges from the source-space row index at
    // every position. A bug that passed the source-space index to the notify
    // calls instead of the sorted-space index would not be caught by an
    // already-sorted source model.
    const source = new ArrayModel([30, 10, 20]);
    const sorted = new SortModel(source, (a, b) => a - b);
    expect(Array.from(sorted)).toEqual([10, 20, 30]);

    // Sorted row 1 (value 20) maps to source row 2. Changing it to 21 keeps
    // its sorted position unchanged.
    const changedSpy = vi.spyOn(sorted as any, "notifyRowDataChanged");
    sorted.setRowData(1, 21);
    expect(changedSpy).toHaveBeenCalledWith(1);
    expect(Array.from(sorted)).toEqual([10, 21, 30]);
    expect(Array.from(source)).toEqual([30, 10, 21]);

    // Sorted row 0 (value 10) maps to source row 1. Changing it to 100 moves
    // it to the end (sorted row 2), and neither notified index equals the
    // source row (1) that was actually written to.
    const removedSpy = vi.spyOn(sorted as any, "notifyRowRemoved");
    const addedSpy = vi.spyOn(sorted as any, "notifyRowAdded");
    sorted.setRowData(0, 100);
    expect(removedSpy).toHaveBeenCalledWith(0, 1);
    expect(addedSpy).toHaveBeenCalledWith(2, 1);
    expect(Array.from(sorted)).toEqual([21, 30, 100]);
    expect(Array.from(source)).toEqual([30, 100, 21]);
});

test("SortModel.setRowData re-reads the committed value instead of trusting the caller-supplied data", () => {
    // A source model that ignores writes (like the base Model.setRowData
    // default) must not desync the cached sort order: the new sort position
    // has to be based on what the source model actually reports afterwards.
    class ReadOnlyModel extends Model<number> {
        #array: number[];
        constructor(arr: number[]) {
            super();
            this.#array = arr;
        }
        rowCount(): number {
            return this.#array.length;
        }
        rowData(row: number): number | undefined {
            return this.#array[row];
        }
        // setRowData intentionally left as the read-only no-op default.
    }
    const source = new ReadOnlyModel([1, 2, 3]);
    const sorted = new SortModel(source, (a, b) => a - b);
    expect(Array.from(sorted)).toEqual([1, 2, 3]);
    // The write is ignored by the source model, so despite passing 100, the
    // committed value (1) is unchanged and the sort order must not move it.
    sorted.setRowData(0, 100);
    expect(Array.from(sorted)).toEqual([1, 2, 3]);
});

test("SortModel treats an in-range row that returns undefined as tied rather than reordering it", () => {
    // Mirrors Rust/C++'s handling of a "broken" model that reports a row
    // count larger than the data it can actually provide.
    class BrokenModel extends Model<number> {
        rowCount(): number {
            return 3;
        }
        rowData(row: number): number | undefined {
            return row === 1 ? undefined : row === 0 ? 20 : 10;
        }
    }
    const broken = new BrokenModel();
    const sorted = new SortModel(broken, (a, b) => a - b);
    // Row 1 (undefined) is treated as tied with everything by the comparator,
    // so it is not reliably moved out of its original relative position, but
    // rowData/rowCount must not throw and must stay in range.
    expect(sorted.rowCount()).toBe(3);
    expect(() => Array.from(sorted)).not.toThrow();
});

test("SortModel handles an empty source model", () => {
    const source = new ArrayModel<number>([]);
    const sorted = new SortModel(source, (a, b) => a - b);
    expect(sorted.rowCount()).toBe(0);
    expect(sorted.rowData(0)).toBeUndefined();
    expect(sorted.unsortedRow(0)).toBeUndefined();
});

test("SortModel.rowData and setRowData ignore out-of-range rows", () => {
    const source = new ArrayModel([3, 1, 2]);
    const sorted = new SortModel(source, (a, b) => a - b);
    expect(sorted.rowData(-1)).toBeUndefined();
    expect(sorted.rowData(10)).toBeUndefined();
    sorted.setRowData(-1, 100);
    sorted.setRowData(10, 100);
    expect(Array.from(source)).toEqual([3, 1, 2]);
});

test("SortModel.reset refreshes the view after the source model is mutated directly", () => {
    const source = new ArrayModel([3, 1, 2]);
    const sorted = new SortModel(source, (a, b) => a - b);
    expect(Array.from(sorted)).toEqual([1, 2, 3]);
    source.push(0);
    // The sorted view is cached until reset() is called.
    expect(Array.from(sorted)).toEqual([1, 2, 3]);
    sorted.reset();
    expect(Array.from(sorted)).toEqual([0, 1, 2, 3]);
});

test("SortModel notifies the run-time", () => {
    const source = `
    export component App {
      in-out property <[int]> data;
      out property <int> first: data.length > 0 ? data[0] : 0;
    }`;

    const demo = loadSource(source, "test.slint") as any;
    const instance = new demo.App();
    const sourceModel = new ArrayModel<number>([3, 1, 2]);
    const sorted = new SortModel(sourceModel, (a, b) => a - b);
    instance.data = sorted;
    expect(instance.first).toBe(1);

    sorted.setRowData(0, 10);
    expect(instance.first).toBe(2);

    sourceModel.push(0);
    sorted.reset();
    expect(instance.first).toBe(0);
});

test("ReverseModel reverses rows from the source model", () => {
    const source = new ArrayModel([1, 2, 3, 4, 5]);
    const reversed = new ReverseModel(source);
    expect(reversed.rowCount()).toBe(5);
    expect(Array.from(reversed)).toEqual([5, 4, 3, 2, 1]);
});

test("ReverseModel.setRowData updates the corresponding source row", () => {
    const source = new ArrayModel([1, 2, 3]);
    const reversed = new ReverseModel(source);
    reversed.setRowData(0, 30);
    expect(Array.from(source)).toEqual([1, 2, 30]);
    expect(Array.from(reversed)).toEqual([30, 2, 1]);
});

test("ReverseModel.rowData/rowCount reflect live source state even without reset()", () => {
    // Unlike FilterModel/SortModel, ReverseModel caches nothing, so reading
    // it directly always reflects the source model's current state. reset()
    // is still required to make the *run-time* re-render, see the next test.
    const source = new ArrayModel([1, 2, 3]);
    const reversed = new ReverseModel(source);
    source.push(4);
    expect(Array.from(reversed)).toEqual([4, 3, 2, 1]);
});

test("ReverseModel handles an empty source model", () => {
    const source = new ArrayModel<number>([]);
    const reversed = new ReverseModel(source);
    expect(reversed.rowCount()).toBe(0);
    expect(reversed.rowData(0)).toBeUndefined();
});

test("ReverseModel.rowData and setRowData ignore out-of-range rows without corrupting the source model", () => {
    const source = new ArrayModel([1, 2, 3]);
    const reversed = new ReverseModel(source);
    expect(reversed.rowData(-1)).toBeUndefined();
    expect(reversed.rowData(10)).toBeUndefined();
    reversed.setRowData(-1, 100);
    reversed.setRowData(10, 100);
    // Out-of-range rows must not silently grow or corrupt the source array.
    expect(Array.from(source)).toEqual([1, 2, 3]);
    expect(source.rowCount()).toBe(3);
});

test("Model.map/.filter/.sort/.reverse compose into a chained view", () => {
    const source = new ArrayModel([1, 2, 3, 4, 5, 6, 7, 8]);
    const chained = source
        .filter((x) => x % 2 === 0) // [2, 4, 6, 8]
        .map((x) => x * 10) // [20, 40, 60, 80]
        .sort((a, b) => b - a) // [80, 60, 40, 20]
        .reverse(); // [20, 40, 60, 80]
    expect(Array.from(chained)).toEqual([20, 40, 60, 80]);

    source.push(10); // filtered in as 10, mapped to 100
    // Only ReverseModel/SortModel/FilterModel need an explicit reset() to
    // refresh after a direct source mutation; MapModel is transparent to
    // notifications (it shares the source's channel), but the mapping's own
    // caches still need refreshing.
    const sortModel = (chained as any).sourceModel;
    const filterModel = sortModel.sourceModel.sourceModel;
    filterModel.reset();
    sortModel.reset();
    expect(Array.from(chained)).toEqual([20, 40, 60, 80, 100]);
});

test("ReverseModel notifies the run-time", () => {
    const source = `
    export component App {
      in-out property <[int]> data;
      out property <int> first: data.length > 0 ? data[0] : 0;
    }`;

    const demo = loadSource(source, "test.slint") as any;
    const instance = new demo.App();
    const sourceModel = new ArrayModel<number>([1, 2, 3]);
    const reversed = new ReverseModel(sourceModel);
    instance.data = reversed;
    expect(instance.first).toBe(3);

    reversed.setRowData(0, 30);
    expect(instance.first).toBe(30);

    sourceModel.push(4);
    // The run-time is not notified until reset() is called, even though
    // reversed.rowData() itself already reflects the pushed value.
    expect(instance.first).toBe(30);
    reversed.reset();
    expect(instance.first).toBe(4);
});
