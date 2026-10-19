// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using System.Collections;
using System.Runtime.InteropServices;
using Slint.Interop;

namespace Slint;

/// <summary>What the native side and <see cref="ValueConverter"/> call on any <see cref="Model{T}"/>.</summary>
internal interface INativeModel
{
    /// <summary>Returns a new native value with the model.</summary>
    nint ToNativeValue();

    int RowCount { get; }

    /// <summary>Returns a new native value with the row, or 0 if there's none.</summary>
    nint RowDataForNative(int row);

    void SetRowDataFromNative(int row, nint value);
}

/// <summary>
/// The rows of a <c>for</c> repeater or an array property.
/// Derive from it to provide rows from your own data, and call the <c>Notify</c> methods when the data changes;
/// <see cref="ListModel{T}"/> covers the common case of a list.
/// </summary>
/// <remarks>
/// While Slint uses the model, Slint keeps it alive.
/// Use it from the Slint thread only.
/// </remarks>
public abstract unsafe class Model<T> : IReadOnlyList<T>, INativeModel
{
    private ModelWeakHandle? _weak;

    /// <summary>The number of rows.</summary>
    public abstract int RowCount { get; }

    /// <summary>Returns the row <paramref name="row"/>, between 0 and <see cref="RowCount"/>.</summary>
    public abstract T RowData(int row);

    /// <summary>Called when Slint changes a row, such as through a two-way binding; ignores the change by default.</summary>
    public virtual void SetRowData(int row, T value)
    {
    }

    /// <inheritdoc/>
    public int Count => RowCount;

    /// <inheritdoc/>
    public T this[int index] => RowData(index);

    /// <inheritdoc/>
    public IEnumerator<T> GetEnumerator()
    {
        for (var i = 0; i < RowCount; i++)
        {
            yield return RowData(i);
        }
    }

    IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();

    /// <summary>Tells Slint that the row <paramref name="row"/> changed.</summary>
    protected void NotifyRowChanged(int row) =>
        Notify(weak => NativeMethods.slint_dotnet_model_notify_row_changed(weak, (nuint)row));

    /// <summary>Tells Slint that <paramref name="count"/> rows were inserted at <paramref name="index"/>.</summary>
    protected void NotifyRowAdded(int index, int count) =>
        Notify(weak => NativeMethods.slint_dotnet_model_notify_row_added(weak, (nuint)index, (nuint)count));

    /// <summary>Tells Slint that <paramref name="count"/> rows were removed at <paramref name="index"/>.</summary>
    protected void NotifyRowRemoved(int index, int count) =>
        Notify(weak => NativeMethods.slint_dotnet_model_notify_row_removed(weak, (nuint)index, (nuint)count));

    /// <summary>Tells Slint that all rows changed.</summary>
    protected void NotifyReset() => Notify(NativeMethods.slint_dotnet_model_notify_reset);

    private void Notify(Func<ModelWeakHandle, bool> notify)
    {
        if (_weak != null)
        {
            NativeError.Check(notify(_weak));
        }
    }

    private protected virtual nint ToNativeValue()
    {
        if (_weak != null)
        {
            var existing = NativeMethods.slint_dotnet_model_weak_upgrade(_weak);
            NativeError.ThrowPending();
            if (existing != 0)
            {
                return existing;
            }
            _weak.Dispose();
            _weak = null;
        }

        var vtable = new ModelVTable
        {
            RowCount = &ModelTrampoline.RowCount,
            RowData = &ModelTrampoline.RowData,
            SetRowData = &ModelTrampoline.SetRowData,
            DropUserData = &ModelTrampoline.Release,
        };
        // The native model owns this handle and keeps the model alive while Slint uses it.
        var userData = GCHandle.ToIntPtr(GCHandle.Alloc(this));
        nint weak;
        var value = NativeError.CheckValue(NativeMethods.slint_dotnet_model_new(vtable, userData, &weak));
        _weak = new ModelWeakHandle(weak);
        return value;
    }

    nint INativeModel.ToNativeValue() => ToNativeValue();

    nint INativeModel.RowDataForNative(int row) =>
        row < RowCount ? ValueConverter.ToNative(RowData(row)) : 0;

    void INativeModel.SetRowDataFromNative(int row, nint value) =>
        SetRowData(row, ValueConverter.ConvertTo<T>(ValueConverter.FromNative(value)));
}

internal static unsafe class ModelTrampoline
{
    private static INativeModel Get(nint userData) => (INativeModel)GCHandle.FromIntPtr(userData).Target!;

    [UnmanagedCallersOnly]
    public static nuint RowCount(nint userData)
    {
        try
        {
            return (nuint)Get(userData).RowCount;
        }
        catch (Exception e)
        {
            NativeError.SetPending(e);
            return 0;
        }
    }

    [UnmanagedCallersOnly]
    public static void RowData(nint userData, nuint row, nint* result)
    {
        try
        {
            *result = Get(userData).RowDataForNative((int)row);
        }
        catch (Exception e)
        {
            NativeError.SetPending(e);
        }
    }

    [UnmanagedCallersOnly]
    public static void SetRowData(nint userData, nuint row, nint value)
    {
        try
        {
            Get(userData).SetRowDataFromNative((int)row, value);
        }
        catch (Exception e)
        {
            NativeError.SetPending(e);
        }
    }

    [UnmanagedCallersOnly]
    public static void Release(nint userData) => GCHandle.FromIntPtr(userData).Free();
}

/// <summary>A model backed by a list, which tells Slint about every change made through it.</summary>
public class ListModel<T> : Model<T>, IList<T>
{
    private readonly List<T> _rows;

    /// <summary>Creates an empty model.</summary>
    public ListModel()
    {
        _rows = [];
    }

    /// <summary>Creates a model with a copy of <paramref name="rows"/>.</summary>
    public ListModel(IEnumerable<T> rows)
    {
        _rows = [.. rows];
    }

    /// <inheritdoc/>
    public override int RowCount => _rows.Count;

    /// <inheritdoc/>
    public override T RowData(int row) => _rows[row];

    /// <summary>Stores a row that Slint changed.</summary>
    public override void SetRowData(int row, T value) => this[row] = value;

    /// <inheritdoc/>
    public new T this[int index]
    {
        get => _rows[index];
        set
        {
            _rows[index] = value;
            NotifyRowChanged(index);
        }
    }

    bool ICollection<T>.IsReadOnly => false;

    /// <inheritdoc/>
    public void Add(T item)
    {
        _rows.Add(item);
        NotifyRowAdded(_rows.Count - 1, 1);
    }

    /// <summary>Appends <paramref name="items"/> with a single notification.</summary>
    public void AddRange(IEnumerable<T> items)
    {
        var index = _rows.Count;
        _rows.AddRange(items);
        if (_rows.Count > index)
        {
            NotifyRowAdded(index, _rows.Count - index);
        }
    }

    /// <inheritdoc/>
    public void Insert(int index, T item)
    {
        _rows.Insert(index, item);
        NotifyRowAdded(index, 1);
    }

    /// <inheritdoc/>
    public void RemoveAt(int index)
    {
        _rows.RemoveAt(index);
        NotifyRowRemoved(index, 1);
    }

    /// <inheritdoc/>
    public bool Remove(T item)
    {
        var index = _rows.IndexOf(item);
        if (index < 0)
        {
            return false;
        }
        RemoveAt(index);
        return true;
    }

    /// <inheritdoc/>
    public void Clear()
    {
        _rows.Clear();
        NotifyReset();
    }

    /// <inheritdoc/>
    public int IndexOf(T item) => _rows.IndexOf(item);

    /// <inheritdoc/>
    public bool Contains(T item) => _rows.Contains(item);

    /// <inheritdoc/>
    public void CopyTo(T[] array, int arrayIndex) => _rows.CopyTo(array, arrayIndex);
}

/// <summary>A model that Slint created, such as an array literal; changes made in Slint aren't reported.</summary>
internal sealed class NativeModel(ValueHandle value) : Model<object?>
{
    public override int RowCount =>
        (int)Math.Max(0, NativeMethods.slint_dotnet_value_model_row_count(value.DangerousGetHandle()));

    public override object? RowData(int row)
    {
        var data = NativeMethods.slint_dotnet_value_model_row_data(value.DangerousGetHandle(), (nuint)row);
        NativeError.ThrowPending();
        if (data == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(row));
        }
        return ValueConverter.TakeFromNative(data);
    }

    public override void SetRowData(int row, object? data)
    {
        var native = ValueConverter.ToNative(data);
        try
        {
            NativeError.Check(NativeMethods.slint_dotnet_value_model_set_row_data(value, (nuint)row, native));
        }
        finally
        {
            NativeMethods.slint_dotnet_value_free(native);
        }
    }

    private protected override nint ToNativeValue() =>
        NativeError.CheckValue(NativeMethods.slint_dotnet_value_clone(value.DangerousGetHandle()));
}
