// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

namespace Slint;

/// <summary>
/// Views of a model that follow its changes, like the model adapters of the Rust API.
/// The source model keeps a view alive, and a view of a model that Slint created doesn't see changes made in Slint.
/// </summary>
public static class ModelAdapters
{
    /// <summary>Returns a view with the rows of <paramref name="source"/> that match <paramref name="predicate"/>.</summary>
    public static FilterModel<T> Filter<T>(this Model<T> source, Func<T, bool> predicate) =>
        new FilterModel<T>(source, predicate);

    /// <summary>Returns a view with the rows of <paramref name="source"/> in the order of <paramref name="comparison"/>; equal rows keep their order.</summary>
    public static SortModel<T> Sort<T>(this Model<T> source, Comparison<T> comparison) =>
        new SortModel<T>(source, comparison);

    /// <summary>Returns a view with each row of <paramref name="source"/> converted by <paramref name="map"/>.</summary>
    public static Model<TResult> Map<T, TResult>(this Model<T> source, Func<T, TResult> map) =>
        new MapModel<T, TResult>(source, map);

    /// <summary>Returns a view with the rows of <paramref name="source"/> in reverse order.</summary>
    public static Model<T> Reverse<T>(this Model<T> source) => new ReverseModel<T>(source);
}

internal sealed class MapModel<T, TResult> : Model<TResult>
{
    private readonly Model<T> _source;
    private readonly Func<T, TResult> _map;

    public MapModel(Model<T> source, Func<T, TResult> map)
    {
        _source = source;
        _map = map;
        source.Changed += Forward;
    }

    public override int RowCount => _source.RowCount;

    public override TResult RowData(int row) => _map(_source.RowData(row));

    private void Forward(ModelChange change)
    {
        switch (change.Kind)
        {
            case ModelChangeKind.Changed:
                NotifyRowChanged(change.Index);
                break;
            case ModelChangeKind.Added:
                NotifyRowAdded(change.Index, change.Count);
                break;
            case ModelChangeKind.Removed:
                NotifyRowRemoved(change.Index, change.Count);
                break;
            default:
                NotifyReset();
                break;
        }
    }
}

internal sealed class ReverseModel<T> : Model<T>
{
    private readonly Model<T> _source;

    public ReverseModel(Model<T> source)
    {
        _source = source;
        source.Changed += Forward;
    }

    public override int RowCount => _source.RowCount;

    public override T RowData(int row) => _source.RowData(RowCount - 1 - row);

    public override void SetRowData(int row, T value) => _source.SetRowData(RowCount - 1 - row, value);

    private void Forward(ModelChange change)
    {
        // RowCount is the count after the change.
        switch (change.Kind)
        {
            case ModelChangeKind.Changed:
                NotifyRowChanged(RowCount - 1 - change.Index);
                break;
            case ModelChangeKind.Added:
                NotifyRowAdded(RowCount - change.Index - change.Count, change.Count);
                break;
            case ModelChangeKind.Removed:
                NotifyRowRemoved(RowCount - change.Index, change.Count);
                break;
            default:
                NotifyReset();
                break;
        }
    }
}

/// <summary>A view through a list of source rows, which it computes again on every change of the source.</summary>
public abstract class IndexedModel<T> : Model<T>
{
    private int[]? _rows;

    private protected IndexedModel(Model<T> source)
    {
        Source = source;
        source.Changed += _ =>
        {
            _rows = null;
            NotifyReset();
        };
    }

    /// <summary>The model that this view shows.</summary>
    public Model<T> Source { get; }

    private int[] Rows => _rows ??= ComputeRows();

    /// <inheritdoc/>
    public override int RowCount => Rows.Length;

    /// <inheritdoc/>
    public override T RowData(int row) => Source.RowData(Rows[row]);

    /// <inheritdoc/>
    public override void SetRowData(int row, T value) => Source.SetRowData(Rows[row], value);

    /// <summary>Returns the row of <see cref="Source"/> that <paramref name="row"/> of this view shows.</summary>
    public int SourceRow(int row) => Rows[row];

    /// <summary>Computes the rows again, such as after the condition of a filter changed.</summary>
    public void Reset()
    {
        _rows = null;
        NotifyReset();
    }

    private protected abstract int[] ComputeRows();
}

/// <summary>The rows of a model that match a condition; see <see cref="ModelAdapters.Filter"/>.</summary>
public sealed class FilterModel<T> : IndexedModel<T>
{
    private readonly Func<T, bool> _predicate;

    internal FilterModel(Model<T> source, Func<T, bool> predicate)
        : base(source)
    {
        _predicate = predicate;
    }

    private protected override int[] ComputeRows() =>
        Enumerable.Range(0, Source.RowCount).Where(i => _predicate(Source.RowData(i))).ToArray();
}

/// <summary>The rows of a model in a sorted order; see <see cref="ModelAdapters.Sort"/>.</summary>
public sealed class SortModel<T> : IndexedModel<T>
{
    private readonly Comparison<T> _comparison;

    internal SortModel(Model<T> source, Comparison<T> comparison)
        : base(source)
    {
        _comparison = comparison;
    }

    private protected override int[] ComputeRows()
    {
        var rows = Enumerable.Range(0, Source.RowCount).Select(i => (Index: i, Row: Source.RowData(i))).ToArray();
        // Array.Sort isn't stable; the index breaks ties.
        Array.Sort(rows, (a, b) => _comparison(a.Row, b.Row) is var order && order != 0 ? order : a.Index.CompareTo(b.Index));
        return rows.Select(r => r.Index).ToArray();
    }
}
