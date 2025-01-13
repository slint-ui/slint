// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import * as napi from "../rust-module.cjs";

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
                value: this.model.rowData(row),
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

    // /**
    //  * Returns a new Model where all elements are mapped by the function `mapFunction`.
    //  * @template T the type of the source model's items.
    //  * @param mapFunction functions that maps
    //  * @returns a new {@link MapModel} that wraps the current model.
    //  */
    // map<U>(
    //     mapFunction: (data: T) => U
    // ): MapModel<T, U> {
    //     return new MapModel(this, mapFunction);
    // }

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
 * array can be modified with the [[ArrayModel.push]] and [[ArrayModel.remove]] methods.
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

    // FIXME: should this be named splice and have the splice api?
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
