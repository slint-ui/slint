// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using System.Collections.Specialized;
using System.ComponentModel;

namespace Slint;

/// <summary>
/// A model over a list that reports its changes through <see cref="INotifyCollectionChanged"/>,
/// such as an <see cref="System.Collections.ObjectModel.ObservableCollection{T}"/> of a view model, so that Slint follows the list.
/// Rows whose items implement <see cref="INotifyPropertyChanged"/> also update when a property of the item changes.
/// Rows that <c>.slint</c> code sets, inserts, or removes change the list.
/// </summary>
/// <remarks>
/// Change the list from the Slint thread only, as a view model does from the UI thread of other frameworks.
/// Dispose the model to stop following the list and its items.
/// Use <see cref="ModelAdapters.Map{T, TResult}"/> to turn items into the structs of the <c>.slint</c> file.
/// </remarks>
public sealed class ObservableCollectionModel<T> : Model<T>, IDisposable
{
    private readonly IList<T> _source;
    private readonly List<INotifyPropertyChanged> _subscribed = [];

    /// <summary>Follows <paramref name="source"/>, which must implement <see cref="INotifyCollectionChanged"/>.</summary>
    /// <exception cref="ArgumentException"><paramref name="source"/> doesn't report its changes.</exception>
    public ObservableCollectionModel(IList<T> source)
    {
        ArgumentNullException.ThrowIfNull(source);
        if (source is not INotifyCollectionChanged notifying)
        {
            throw new ArgumentException("The list must implement INotifyCollectionChanged, as ObservableCollection<T> does.", nameof(source));
        }
        _source = source;
        notifying.CollectionChanged += OnCollectionChanged;
        Subscribe(_source);
    }

    /// <inheritdoc/>
    public override int RowCount => _source.Count;

    /// <inheritdoc/>
    public override T RowData(int row) => _source[row];

    /// <inheritdoc/>
    public override void SetRowData(int row, T value) => _source[row] = value;

    /// <inheritdoc/>
    public override bool InsertRow(int row, T value)
    {
        if (_source.IsReadOnly)
        {
            return false;
        }
        _source.Insert(row, value);
        return true;
    }

    /// <inheritdoc/>
    public override bool RemoveRow(int row)
    {
        if (_source.IsReadOnly)
        {
            return false;
        }
        _source.RemoveAt(row);
        return true;
    }

    /// <summary>Stops following the list and its items.</summary>
    public void Dispose()
    {
        ((INotifyCollectionChanged)_source).CollectionChanged -= OnCollectionChanged;
        foreach (var item in _subscribed)
        {
            item.PropertyChanged -= OnItemChanged;
        }
        _subscribed.Clear();
    }

    private void OnCollectionChanged(object? sender, NotifyCollectionChangedEventArgs e)
    {
        switch (e.Action)
        {
            case NotifyCollectionChangedAction.Add when e.NewStartingIndex >= 0:
                Subscribe(e.NewItems!);
                NotifyRowAdded(e.NewStartingIndex, e.NewItems!.Count);
                break;
            case NotifyCollectionChangedAction.Remove when e.OldStartingIndex >= 0:
                Unsubscribe(e.OldItems!);
                NotifyRowRemoved(e.OldStartingIndex, e.OldItems!.Count);
                break;
            case NotifyCollectionChangedAction.Replace when e.NewStartingIndex >= 0:
                Unsubscribe(e.OldItems!);
                Subscribe(e.NewItems!);
                for (var i = 0; i < e.NewItems!.Count; i++)
                {
                    NotifyRowChanged(e.NewStartingIndex + i);
                }
                break;
            // Slint's models have no notification for a move.
            case NotifyCollectionChangedAction.Move when e.OldStartingIndex >= 0 && e.NewStartingIndex >= 0:
                NotifyRowRemoved(e.OldStartingIndex, e.OldItems!.Count);
                NotifyRowAdded(e.NewStartingIndex, e.NewItems!.Count);
                break;
            default:
                Unsubscribe(_subscribed.ToArray());
                Subscribe(_source);
                NotifyReset();
                break;
        }
    }

    private void Subscribe(System.Collections.IEnumerable items)
    {
        foreach (var item in items)
        {
            if (item is INotifyPropertyChanged notifying)
            {
                notifying.PropertyChanged += OnItemChanged;
                _subscribed.Add(notifying);
            }
        }
    }

    private void Unsubscribe(System.Collections.IEnumerable items)
    {
        foreach (var item in items)
        {
            if (item is INotifyPropertyChanged notifying && _subscribed.Remove(notifying))
            {
                notifying.PropertyChanged -= OnItemChanged;
            }
        }
    }

    private void OnItemChanged(object? sender, PropertyChangedEventArgs e)
    {
        for (var row = 0; row < _source.Count; row++)
        {
            if (ReferenceEquals(_source[row], sender))
            {
                NotifyRowChanged(row);
            }
        }
    }
}
