// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import * as napi from "../binding.cjs";

class ModelIterator<T> implements Iterator<T> {
    private row: number;
    private model: Model<T>;

    constructor(model: Model<T>) {
        this.model = model;
        this.row = 0;
    }

    public next(): IteratorResult<T> {
        if (this.row < this.model.rowCount()) {
            const row = this.row;
            this.row++;
            return {
                done: false,
                value: this.model.rowData(row) as T,
            };
        }
        return {
            done: true,
            value: undefined,
        };
    }
}

/**
 * Model<T> is the interface for feeding dynamic data into
 * `.slint` views.
 *
 * A model is organized like a table with rows of data. The
 * fields of the data type T behave like columns.
 *
 * @template T the type of the model's items.
 *
 * ### Example
 * As an example let's see the implementation of {@link ArrayModel}
 *
 * ```js
 * export class ArrayModel<T> extends Model<T> {
 *    private a: Array<T>
 *
 *   constructor(arr: Array<T>) {
 *        super();
 *        this.a = arr;
 *    }
 *
 *    rowCount() {
 *        return this.a.length;
 *    }
 *
 *    rowData(row: number) {
 *       return this.a[row];
 *    }
 *
 *    setRowData(row: number, data: T) {
 *        this.a[row] = data;
 *        this.notifyRowDataChanged(row);
 *    }
 *
 *    push(...values: T[]) {
 *        let size = this.a.length;
 *        Array.prototype.push.apply(this.a, values);
 *        this.notifyRowAdded(size, arguments.length);
 *    }
 *
 *    remove(index: number, size: number) {
 *        let r = this.a.splice(index, size);
 *        this.notifyRowRemoved(index, size);
 *    }
 *
 *    get length(): number {
 *        return this.a.length;
 *    }
 *
 *    values(): IterableIterator<T> {
 *        return this.a.values();
 *    }
 *
 *    entries(): IterableIterator<[number, T]> {
 *        return this.a.entries()
 *    }
 *}
 * ```
 */
export abstract class Model<T> implements Iterable<T> {
    /**
     * @hidden
     */
    modelNotify: napi.ExternalObject<napi.SharedModelNotify>;

    /**
     * @hidden
     */
    constructor(modelNotify?: napi.ExternalObject<napi.SharedModelNotify>) {
        this.modelNotify = modelNotify ?? napi.jsModelNotifyNew();
    }

    /**
     * Returns a new Model where all elements are mapped by the function `mapFunction`.
     * @param mapFunction maps the data from T to U.
     * @returns a new {@link MapModel} that wraps the current model.
     */
    map<U>(mapFunction: (data: T) => U): MapModel<T, U> {
        return new MapModel(this, mapFunction);
    }

    /**
     * Returns a new Model that only contains the rows of this model for which
     * `filterFunction` returns true.
     * @param filterFunction returns true if a row should be visible.
     * @returns a new {@link FilterModel} that wraps the current model.
     */
    filter(filterFunction: (data: T) => boolean): FilterModel<T> {
        return new FilterModel(this, filterFunction);
    }

    /**
     * Returns a new Model with the same rows as this model, ordered according to
     * `compareFunction`.
     * @param compareFunction compares two rows the same way the callback passed to
     *                         {@link Array.prototype.sort} does.
     * @returns a new {@link SortModel} that wraps the current model.
     */
    sort(compareFunction: (a: T, b: T) => number): SortModel<T> {
        return new SortModel(this, compareFunction);
    }

    /**
     * Returns a new Model with the same rows as this model, in reverse order.
     * @returns a new {@link ReverseModel} that wraps the current model.
     */
    reverse(): ReverseModel<T> {
        return new ReverseModel(this);
    }

    /**
     * Implementations of this function must return the current number of rows.
     */
    abstract rowCount(): number;
    /**
     * Implementations of this function must return the data at the specified row.
     * @param row index in range 0..(rowCount() - 1).
     * @returns undefined if row is out of range otherwise the data.
     */
    abstract rowData(row: number): T | undefined;

    /**
     * Implementations of this function must store the provided data parameter
     * in the model at the specified row.
     * @param _row index in range 0..(rowCount() - 1).
     * @param _data new data item to store on the given row index
     */
    setRowData(_row: number, _data: T): void {
        console.log(
            "setRowData called on a model which does not re-implement this method. This happens when trying to modify a read-only model",
        );
    }

    /**
     * Adds a line to the model with the provided data.
     * The default implementation calls {@link Model.insertRow} with the row count.
     * @param data new data item to store in a new row.
     */
    pushRow(data: T): void {
        this.insertRow(this.rowCount(), data);
    }

    /**
     * Implementations of this function must remove the row at the specified index.
     * @param _index index of the row to remove.
     */
    removeRow(_index: number): void {
        console.log(
            "removeRow called on a model which does not re-implement this method. This happens when trying to modify a read-only model",
        );
    }

    /**
     * Implementations of this function must add a row at the specified index, pushing all next
     * rows to the right.
     * @param _index index of the row to insert.
     * @param _data new data item to store in a new row.
     */
    insertRow(_index: number, _data: T): void {
        console.log(
            "insertRow called on a model which does not re-implement this method. This happens when trying to modify a read-only model",
        );
    }

    [Symbol.iterator](): Iterator<T> {
        return new ModelIterator(this);
    }

    /**
     * Notifies the view that the data of the current row is changed.
     * @param row index of the changed row.
     */
    protected notifyRowDataChanged(row: number): void {
        napi.jsModelNotifyRowDataChanged(this.modelNotify, row);
    }

    /**
     * Notifies the view that multiple rows are added to the model.
     * @param row index of the first added row.
     * @param count the number of added items.
     */
    protected notifyRowAdded(row: number, count: number): void {
        napi.jsModelNotifyRowAdded(this.modelNotify, row, count);
    }

    /**
     * Notifies the view that multiple rows are removed to the model.
     * @param row index of the first removed row.
     * @param count the number of removed items.
     */
    protected notifyRowRemoved(row: number, count: number): void {
        napi.jsModelNotifyRowRemoved(this.modelNotify, row, count);
    }

    /**
     * Notifies the view that the complete data must be reload.
     */
    protected notifyReset(): void {
        napi.jsModelNotifyReset(this.modelNotify);
    }
}

/**
 * ArrayModel wraps a JavaScript array for use in `.slint` views. The underlying
 * array can be modified with the [[ArrayModel.push]], [[ArrayModel.remove]], and
 * [[ArrayModel.splice]] methods.
 */
export class ArrayModel<T> extends Model<T> {
    /**
     * @hidden
     */
    #array: Array<T>;

    /**
     * Creates a new ArrayModel.
     *
     * @param arr
     */
    constructor(arr: Array<T>) {
        super();
        this.#array = arr;
    }

    /**
     * Returns the number of entries in the array model.
     */
    get length(): number {
        return this.#array.length;
    }

    /**
     * Returns the number of entries in the array model.
     */
    rowCount() {
        return this.#array.length;
    }

    /**
     * Returns the data at the specified row.
     * @param row index in range 0..(rowCount() - 1).
     * @returns undefined if row is out of range otherwise the data.
     */
    rowData(row: number) {
        return this.#array[row];
    }

    /**
     * Stores the given data on the given row index and notifies run-time about the changed row.
     * @param row index in range 0..(rowCount() - 1).
     * @param data new data item to store on the given row index
     */
    setRowData(row: number, data: T) {
        this.#array[row] = data;
        this.notifyRowDataChanged(row);
    }

    /**
     * Remove a row from the array backing the model and notifies run-time about the removed row.
     * @param _index index of the row to remove.
     */
    removeRow(_index: number) {
        // Validate index range to prevent out-of-bounds access as this method is used by
        // the `array.remove(index)` slint method.
        if (_index < 0 || _index >= this.#array.length) {
            return;
        }
        this.remove(_index, 1);
    }

    /**
     * Insert a new row into the array backing the model at the specified index and notifies run-time about the added row.
     * @param _index index at which to insert the new row.
     * @param _data data item to store in the new row.
     */
    insertRow(_index: number, _data: T) {
        // Validate index range to prevent out-of-bounds access as this method is used by
        // the `array.insert(index, value)` slint method.
        if (_index < 0 || _index > this.#array.length) return;
        this.splice(_index, 0, _data);
    }

    /**
     * Pushes new values to the array that's backing the model and notifies
     * the run-time about the added rows.
     * @param values list of values that will be pushed to the array.
     */
    push(...values: T[]) {
        const size = this.#array.length;
        Array.prototype.push.apply(this.#array, values);
        this.notifyRowAdded(size, arguments.length);
    }

    /**
     * Removes the last element from the array and returns it.
     *
     * @returns The removed element or undefined if the array is empty.
     */
    pop(): T | undefined {
        const last = this.#array.pop();
        if (last !== undefined) {
            this.notifyRowRemoved(this.#array.length, 1);
        }
        return last;
    }

    /**
     * Removes the specified number of element from the array that's backing
     * the model, starting at the specified index.
     * @param index index of first row to remove.
     * @param size number of rows to remove.
     */
    remove(index: number, size: number) {
        const r = this.#array.splice(index, size);
        this.notifyRowRemoved(index, size);
    }

    /**
     * Removes elements from the array that's backing the model and, if
     * necessary, inserts new elements in their place, following the semantics
     * of `Array.prototype.splice`. The run-time is notified about the removed
     * and added rows.
     * @param start zero-based index at which to start changing the array; negative values count back from the end and out-of-range values are clamped.
     * @param deleteCount number of elements to remove starting at `start`; if omitted, all elements from `start` to the end are removed.
     * @param items elements to insert at `start`.
     * @returns an array containing the removed elements.
     */
    splice(start: number, deleteCount?: number, ...items: T[]): T[] {
        const len = this.#array.length;
        // Normalize `start` the way `Array.prototype.splice` does, so the
        // change notifications point at the actual mutation index.
        const actualStart =
            start < 0 ? Math.max(len + start, 0) : Math.min(start, len);
        const removed =
            deleteCount === undefined
                ? this.#array.splice(actualStart)
                : this.#array.splice(actualStart, deleteCount, ...items);
        if (removed.length > 0) {
            this.notifyRowRemoved(actualStart, removed.length);
        }
        if (items.length > 0) {
            this.notifyRowAdded(actualStart, items.length);
        }
        return removed;
    }

    /**
     * Returns an iterable of values in the array.
     */
    values(): IterableIterator<T> {
        return this.#array.values();
    }

    /**
     * Returns an iterable of key, value pairs for every entry in the array.
     */
    entries(): IterableIterator<[number, T]> {
        return this.#array.entries();
    }
}

/**
 * @hidden
 * Base class for the Reverse/Filter/Sort/MapModel adapters, whose row-mapping
 * bookkeeping runs natively in Rust behind a shared `napi.NativeModel`
 * handle. Centralizes the delegation to that handle, including the
 * `__slintNativeModel` property the native side uses to recognize a source
 * model as backed by a native adapter and reuse its backing directly
 * instead of treating it as an opaque JS model.
 */
abstract class NativeAdapterModel<T> extends Model<T> {
    #native!: napi.NativeModel;

    protected setNative(native: napi.NativeModel): void {
        this.#native = native;
    }

    protected get native(): napi.NativeModel {
        return this.#native;
    }

    /**
     * @hidden
     */
    get __slintNativeModel(): napi.NativeModel {
        return this.#native;
    }

    /**
     * Returns the number of entries in the model.
     */
    rowCount(): number {
        return this.#native.rowCount();
    }

    /**
     * Returns the data at the specified row.
     * @param row index in range 0..(rowCount() - 1).
     * @returns undefined if row is out of range otherwise the data.
     */
    rowData(row: number): T | undefined {
        return this.#native.rowData(row) as T | undefined;
    }

    /**
     * Stores the given data in the source model at the row that corresponds
     * to the given row index.
     * @param row index in range 0..(rowCount() - 1).
     * @param data new data item to store on the given row index.
     */
    setRowData(row: number, data: T): void {
        this.#native.setRowData(row, data);
    }
}

/**
 * FilterModel provides a filtered view of rows from a source model, by applying
 * a filter function to each row of the source model. Only rows for which the
 * filter function returns `true` are visible in the FilterModel.
 *
 * ### Example
 *
 * ```js
 * const source = new ArrayModel([1, 2, 3, 4, 5, 6]);
 * const evenNumbers = new FilterModel(source, (x) => x % 2 === 0);
 *
 * // prints 2, 4, 6
 * for (const x of evenNumbers) {
 *     console.log(x);
 * }
 * ```
 */
export class FilterModel<T> extends NativeAdapterModel<T> {
    /**
     * The source model that this FilterModel filters rows from.
     */
    readonly sourceModel: Model<T>;

    /**
     * Constructs a new FilterModel that provides a filtered view on the given
     * `sourceModel` by applying `filterFunction` on each of its rows.
     * @param sourceModel the wrapped model.
     * @param filterFunction returns true if a row should be visible in the FilterModel.
     */
    constructor(sourceModel: Model<T>, filterFunction: (data: T) => boolean) {
        super();
        this.sourceModel = sourceModel;
        this.setNative(
            napi.nativeFilterModelNew(
                sourceModel,
                filterFunction as (data: unknown) => boolean,
                this,
            ),
        );
    }

    /**
     * Re-applies the filter function on each row of the source model. Use
     * this if the filter function depends on state external to the source
     * model and that state has changed.
     */
    reset(): void {
        this.native.reset();
    }

    /**
     * Given a `filteredRow` index into this FilterModel, returns the
     * corresponding row index in the source model.
     * @param filteredRow index in range 0..(rowCount() - 1).
     * @returns undefined if filteredRow is out of range otherwise the source row index.
     */
    unfilteredRow(filteredRow: number): number | undefined {
        return this.native.unmappedRow(filteredRow) ?? undefined;
    }
}

/**
 * MapModel provides rows that are generated by a map function based on the
 * rows of another model.
 *
 * @template T item type of source model that is mapped to U.
 * @template U the type of the mapped items.
 *
 * ### Example
 *
 * Here we have an {@link ArrayModel} holding rows of a custom interface `Name`
 * and a MapModel that maps the name rows to single string rows.
 *
 * ```js
 * interface Name {
 *     first: string;
 *     last: string;
 * }
 *
 * const model = new ArrayModel<Name>([
 *     { first: "Hans", last: "Emil" },
 *     { first: "Max", last: "Mustermann" },
 *     { first: "Roman", last: "Tisch" },
 * ]);
 *
 * const mappedModel = new MapModel(model, (data) => data.last + ", " + data.first);
 *
 * // prints "Emil, Hans"
 * console.log(mappedModel.rowData(0));
 * ```
 */
export class MapModel<T, U> extends NativeAdapterModel<U> {
    /**
     * The source model that this MapModel maps rows from.
     */
    readonly sourceModel: Model<T>;

    /**
     * Constructs a new MapModel that provides a mapped view on the given
     * `sourceModel` by applying `mapFunction` on each of its rows.
     * @param sourceModel the wrapped model.
     * @param mapFunction maps the data from T to U.
     */
    constructor(sourceModel: Model<T>, mapFunction: (data: T) => U) {
        super();
        this.sourceModel = sourceModel;
        this.setNative(
            napi.nativeMapModelNew(
                sourceModel,
                mapFunction as (data: unknown) => unknown,
                this,
            ),
        );
    }
}

/**
 * SortModel acts as an adapter model for a given source model by sorting all its
 * rows according to the order given by `compareFunction`.
 *
 * ### Example
 *
 * ```js
 * const source = new ArrayModel(["lorem", "ipsum", "dolor"]);
 * const sorted = new SortModel(source, (a, b) => a.localeCompare(b));
 *
 * // prints dolor, ipsum, lorem
 * for (const x of sorted) {
 *     console.log(x);
 * }
 * ```
 */
export class SortModel<T> extends NativeAdapterModel<T> {
    /**
     * The source model that this SortModel sorts rows from.
     */
    readonly sourceModel: Model<T>;

    /**
     * Constructs a new SortModel that provides a sorted view on the given
     * `sourceModel` by applying the order given by `compareFunction`.
     * @param sourceModel the wrapped model.
     * @param compareFunction compares two rows the same way the callback passed to
     *                         {@link Array.prototype.sort} does.
     */
    constructor(
        sourceModel: Model<T>,
        compareFunction: (a: T, b: T) => number,
    ) {
        super();
        this.sourceModel = sourceModel;
        this.setNative(
            napi.nativeSortModelNew(
                sourceModel,
                compareFunction as (a: unknown, b: unknown) => number,
                this,
            ),
        );
    }

    /**
     * Re-applies the sort order on the rows of the source model. Use this if
     * the compare function depends on state external to the source model and
     * that state has changed.
     */
    reset(): void {
        this.native.reset();
    }

    /**
     * Given a `sortedRow` index into this SortModel, returns the corresponding
     * row index in the source model.
     * @param sortedRow index in range 0..(rowCount() - 1).
     * @returns undefined if sortedRow is out of range otherwise the source row index.
     */
    unsortedRow(sortedRow: number): number | undefined {
        return this.native.unmappedRow(sortedRow) ?? undefined;
    }
}

/**
 * ReverseModel acts as an adapter model for a given source model by reversing
 * all its rows. This means that the first row in the source model is the last
 * row of this model, the second row is the second last, and so on.
 *
 * ### Example
 *
 * ```js
 * const source = new ArrayModel([1, 2, 3, 4, 5]);
 * const reversed = new ReverseModel(source);
 *
 * // prints 5, 4, 3, 2, 1
 * for (const x of reversed) {
 *     console.log(x);
 * }
 * ```
 */
export class ReverseModel<T> extends NativeAdapterModel<T> {
    /**
     * The source model that this ReverseModel reverses rows from.
     */
    readonly sourceModel: Model<T>;

    /**
     * Constructs a new ReverseModel that provides a reversed view on the given
     * `sourceModel`.
     * @param sourceModel the wrapped model.
     */
    constructor(sourceModel: Model<T>) {
        super();
        this.sourceModel = sourceModel;
        this.setNative(napi.nativeReverseModelNew(sourceModel, this));
    }
}
