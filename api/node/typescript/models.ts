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
     * Implementations of this function must add a line to the model with the provided data.
     * @param _data new data item to store in a new row.
     */
    pushRow(_data: T): void {
        console.log(
            "pushRow called on a model which does not re-implement this method. This happens when trying to modify a read-only model",
        );
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
     * Add a new row to the array backing the model and notifies run-time about the added row.
     * @param data new data item to store in a new row.
     */
    pushRow(data: T) {
        this.push(data);
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
 * FilterModel provides a filtered view of rows from a source model, by applying
 * a filter function to each row of the source model. Only rows for which the
 * filter function returns `true` are visible in the FilterModel.
 *
 * Note that the FilterModel does not automatically observe modifications made
 * directly to the source model (for example calling {@link ArrayModel.push} or
 * {@link ArrayModel.splice} on the underlying source model). After such a
 * modification, call {@link FilterModel.reset} to re-apply the filter function
 * and refresh the filtered view. Modifications made through
 * {@link FilterModel.setRowData} are reflected automatically.
 *
 * Calling {@link FilterModel.setRowData} while the cached mapping is stale
 * (i.e. after the source model was mutated directly without a following
 * {@link FilterModel.reset}) does not just return stale *reads* — it writes to
 * whatever source row the stale mapping points at, which may no longer be the
 * row the caller intended. Always call {@link FilterModel.reset} after a
 * direct source mutation before calling {@link FilterModel.setRowData} again.
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
export class FilterModel<T> extends Model<T> {
    /**
     * The source model that this FilterModel filters rows from.
     */
    readonly sourceModel: Model<T>;
    #filterFunction: (data: T) => boolean;
    #acceptedRows: number[] | undefined;

    /**
     * Constructs a new FilterModel that provides a filtered view on the given
     * `sourceModel` by applying `filterFunction` on each of its rows.
     * @param sourceModel the wrapped model.
     * @param filterFunction returns true if a row should be visible in the FilterModel.
     */
    constructor(sourceModel: Model<T>, filterFunction: (data: T) => boolean) {
        super();
        this.sourceModel = sourceModel;
        this.#filterFunction = filterFunction;
    }

    #updateMapping(): number[] {
        if (this.#acceptedRows === undefined) {
            const acceptedRows: number[] = [];
            for (let row = 0; row < this.sourceModel.rowCount(); ++row) {
                const data = this.sourceModel.rowData(row);
                if (data !== undefined && this.#filterFunction(data)) {
                    acceptedRows.push(row);
                }
            }
            this.#acceptedRows = acceptedRows;
        }
        return this.#acceptedRows;
    }

    /**
     * Returns the number of entries in the filtered model.
     */
    rowCount(): number {
        return this.#updateMapping().length;
    }

    /**
     * Returns the data at the specified row.
     * @param row index in range 0..(rowCount() - 1).
     * @returns undefined if row is out of range otherwise the data.
     */
    rowData(row: number): T | undefined {
        const sourceRow = this.#updateMapping()[row];
        if (sourceRow === undefined) {
            return undefined;
        }
        return this.sourceModel.rowData(sourceRow);
    }

    /**
     * Stores the given data in the source model at the row that corresponds to
     * the given filtered row index, then re-reads it from the source model to
     * decide whether it still passes the filter, notifying either a row change
     * or a row removal accordingly.
     *
     * If the source model's `setRowData` does not synchronously commit the
     * value read back by `rowData` (for example a read-only model that ignores
     * the write), the cached mapping may go out of sync; call
     * {@link FilterModel.reset} in that case.
     * @param row index in range 0..(rowCount() - 1).
     * @param data new data item to store on the given row index.
     */
    setRowData(row: number, data: T): void {
        const acceptedRows = this.#updateMapping();
        const sourceRow = acceptedRows[row];
        if (sourceRow === undefined) {
            return;
        }
        this.sourceModel.setRowData(sourceRow, data);
        const committed = this.sourceModel.rowData(sourceRow);
        if (committed !== undefined && this.#filterFunction(committed)) {
            this.notifyRowDataChanged(row);
        } else {
            acceptedRows.splice(row, 1);
            this.notifyRowRemoved(row, 1);
        }
    }

    /**
     * Re-applies the filter function on each row of the source model and
     * notifies the run-time that the filtered view has changed. Call this
     * after modifying the source model directly, so that the filtered view
     * reflects the current state of the source model.
     */
    reset(): void {
        this.#acceptedRows = undefined;
        this.notifyReset();
    }

    /**
     * Given a `filteredRow` index into this FilterModel, returns the
     * corresponding row index in the source model.
     * @param filteredRow index in range 0..(rowCount() - 1).
     * @returns undefined if filteredRow is out of range otherwise the source row index.
     */
    unfilteredRow(filteredRow: number): number | undefined {
        return this.#updateMapping()[filteredRow];
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
 *
 * Since a MapModel shares row change notifications with its source model,
 * modifications made to the underlying model (for example through
 * {@link ArrayModel.setRowData}) are reflected automatically.
 *
 * Note that "sharing row change notifications" means a MapModel literally
 * reuses its source model's underlying notification channel — it does not
 * have an independent one. As a consequence, calling {@link MapModel.reset}
 * notifies *every* view bound to that channel, including views bound
 * directly to the source model and views bound to any other MapModel
 * wrapping the same source model instance, not just views bound to this
 * particular MapModel. Only rely on this sharing behavior when the source
 * model is not otherwise displayed on its own or wrapped by another MapModel.
 */
export class MapModel<T, U> extends Model<U> {
    /**
     * The source model that this MapModel maps rows from.
     */
    readonly sourceModel: Model<T>;
    #mapFunction: (data: T) => U;

    /**
     * Constructs a new MapModel that provides a mapped view on the given
     * `sourceModel` by applying `mapFunction` on each of its rows.
     * @param sourceModel the wrapped model.
     * @param mapFunction maps the data from T to U.
     */
    constructor(sourceModel: Model<T>, mapFunction: (data: T) => U) {
        super(sourceModel.modelNotify);
        this.sourceModel = sourceModel;
        this.#mapFunction = mapFunction;
    }

    /**
     * Returns the number of entries in the model.
     */
    rowCount(): number {
        return this.sourceModel.rowCount();
    }

    /**
     * Returns the data at the specified row.
     * @param row index in range 0..(rowCount() - 1).
     * @returns undefined if row is out of range otherwise the data.
     */
    rowData(row: number): U | undefined {
        const data = this.sourceModel.rowData(row);
        if (data === undefined) {
            return undefined;
        }
        return this.#mapFunction(data);
    }

    /**
     * Notifies the run-time that the mapped view has changed. Call this if
     * the map function's result depends on state external to the source
     * model and that state has changed, so that the mapped view reflects
     * the current data.
     *
     * Since MapModel shares its notification channel with the source model
     * (see the class documentation), this also triggers a full reload of any
     * other view bound to the same source model instance, including views
     * bound to the source model directly or to a sibling MapModel.
     */
    reset(): void {
        this.notifyReset();
    }
}

/**
 * SortModel acts as an adapter model for a given source model by sorting all its
 * rows according to the order given by `compareFunction`.
 *
 * Note that, like {@link FilterModel}, the SortModel does not automatically
 * observe modifications made directly to the source model. After such a
 * modification, call {@link SortModel.reset} to re-apply the sort order and
 * refresh the sorted view. Modifications made through {@link SortModel.setRowData}
 * are reflected automatically.
 *
 * Calling {@link SortModel.setRowData} while the cached sort order is stale
 * (i.e. after the source model was mutated directly without a following
 * {@link SortModel.reset}) does not just return stale *reads* — it writes to
 * whatever source row the stale order points at, which may no longer be the
 * row the caller intended. Always call {@link SortModel.reset} after a direct
 * source mutation before calling {@link SortModel.setRowData} again.
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
export class SortModel<T> extends Model<T> {
    /**
     * The source model that this SortModel sorts rows from.
     */
    readonly sourceModel: Model<T>;
    #compareFunction: (a: T, b: T) => number;
    #sortedRows: number[] | undefined;

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
        this.#compareFunction = compareFunction;
    }

    #updateMapping(): number[] {
        if (this.#sortedRows === undefined) {
            const sortedRows: number[] = [];
            for (let row = 0; row < this.sourceModel.rowCount(); ++row) {
                sortedRows.push(row);
            }
            sortedRows.sort((a, b) => {
                const dataA = this.sourceModel.rowData(a);
                const dataB = this.sourceModel.rowData(b);
                if (dataA === undefined || dataB === undefined) {
                    return 0;
                }
                return this.#compareFunction(dataA, dataB);
            });
            this.#sortedRows = sortedRows;
        }
        return this.#sortedRows;
    }

    // Returns the leftmost index in `sortedRows` at which `value` can be
    // inserted while keeping `sortedRows` sorted (mirrors `std::lower_bound`
    // with the same comparator convention as `compareFunction`).
    #lowerBound(sortedRows: number[], value: T): number {
        let lo = 0;
        let hi = sortedRows.length;
        while (lo < hi) {
            const mid = (lo + hi) >>> 1;
            const midData = this.sourceModel.rowData(sortedRows[mid]);
            if (
                midData !== undefined &&
                this.#compareFunction(midData, value) < 0
            ) {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        return lo;
    }

    /**
     * Returns the number of entries in the sorted model.
     */
    rowCount(): number {
        return this.#updateMapping().length;
    }

    /**
     * Returns the data at the specified row.
     * @param row index in range 0..(rowCount() - 1).
     * @returns undefined if row is out of range otherwise the data.
     */
    rowData(row: number): T | undefined {
        const sourceRow = this.#updateMapping()[row];
        if (sourceRow === undefined) {
            return undefined;
        }
        return this.sourceModel.rowData(sourceRow);
    }

    /**
     * Stores the given data in the source model at the row that corresponds to
     * the given sorted row index, then re-reads it from the source model to
     * find its new sorted position, notifying either a row change (position
     * unchanged) or a row move (position changed) accordingly.
     *
     * If the source model's `setRowData` does not synchronously commit the
     * value read back by `rowData` (for example a read-only model that ignores
     * the write), the cached sort order may go out of sync; call
     * {@link SortModel.reset} in that case.
     * @param row index in range 0..(rowCount() - 1).
     * @param data new data item to store on the given row index.
     */
    setRowData(row: number, data: T): void {
        const sortedRows = this.#updateMapping();
        const sourceRow = sortedRows[row];
        if (sourceRow === undefined) {
            return;
        }
        this.sourceModel.setRowData(sourceRow, data);
        const committed = this.sourceModel.rowData(sourceRow);
        sortedRows.splice(row, 1);
        const insertionPoint =
            committed === undefined
                ? sortedRows.length
                : this.#lowerBound(sortedRows, committed);
        sortedRows.splice(insertionPoint, 0, sourceRow);
        if (insertionPoint === row) {
            this.notifyRowDataChanged(row);
        } else {
            this.notifyRowRemoved(row, 1);
            this.notifyRowAdded(insertionPoint, 1);
        }
    }

    /**
     * Re-applies the sort order on the rows of the source model and notifies
     * the run-time that the sorted view has changed. Call this after modifying
     * the source model directly, so that the sorted view reflects the current
     * state of the source model.
     */
    reset(): void {
        this.#sortedRows = undefined;
        this.notifyReset();
    }

    /**
     * Given a `sortedRow` index into this SortModel, returns the corresponding
     * row index in the source model.
     * @param sortedRow index in range 0..(rowCount() - 1).
     * @returns undefined if sortedRow is out of range otherwise the source row index.
     */
    unsortedRow(sortedRow: number): number | undefined {
        return this.#updateMapping()[sortedRow];
    }
}

/**
 * ReverseModel acts as an adapter model for a given source model by reversing
 * all its rows. This means that the first row in the source model is the last
 * row of this model, the second row is the second last, and so on.
 *
 * Unlike {@link FilterModel} and {@link SortModel}, ReverseModel does not need
 * to cache any state derived from the whole source model, so it always reflects
 * the current row count and row data of the source model. However, since row
 * insertions and removals on the source model are not automatically observed,
 * call {@link ReverseModel.reset} after such a modification so that the
 * run-time re-renders the reversed view. Modifications made through
 * {@link ReverseModel.setRowData} are reflected automatically.
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
export class ReverseModel<T> extends Model<T> {
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
    }

    /**
     * Returns the number of entries in the model.
     */
    rowCount(): number {
        return this.sourceModel.rowCount();
    }

    /**
     * Returns the data at the specified row.
     * @param row index in range 0..(rowCount() - 1).
     * @returns undefined if row is out of range otherwise the data.
     */
    rowData(row: number): T | undefined {
        const count = this.sourceModel.rowCount();
        if (row < 0 || row >= count) {
            return undefined;
        }
        return this.sourceModel.rowData(count - row - 1);
    }

    /**
     * Stores the given data in the source model at the row that corresponds to
     * the given reversed row index, and notifies the run-time about the change.
     * @param row index in range 0..(rowCount() - 1).
     * @param data new data item to store on the given row index.
     */
    setRowData(row: number, data: T): void {
        const count = this.sourceModel.rowCount();
        if (row < 0 || row >= count) {
            return;
        }
        this.sourceModel.setRowData(count - row - 1, data);
        this.notifyRowDataChanged(row);
    }

    /**
     * Notifies the run-time that the reversed view must be reloaded. Call this
     * after modifying the source model directly (for example calling
     * {@link ArrayModel.push} or {@link ArrayModel.splice} on the underlying
     * source model), so that the reversed view reflects the current row count
     * of the source model.
     */
    reset(): void {
        this.notifyReset();
    }
}
