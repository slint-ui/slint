// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using System.ComponentModel;
using System.Windows.Input;

namespace Slint;

/// <summary>
/// Generates a <c>Bind</c> method for the generated component <paramref name="component"/> and the view model with this attribute,
/// which binds the members of the same name: <c>window.Bind(viewModel)</c>.
/// </summary>
/// <remarks>
/// A property of the view model binds to the component's property of the same C# name;
/// both ways when the view model's property has a setter.
/// A collection binds to an array property with the same item type.
/// A command <c>SaveCommand</c>, also one from <c>[RelayCommand]</c> of CommunityToolkit.Mvvm, binds to the callback <c>save</c>,
/// and its <c>CanExecute</c> to the boolean property <c>save-enabled</c>, if the component has it.
/// The result binds more members with the methods of <see cref="ViewModelBinding{TViewModel}"/>.
/// </remarks>
[AttributeUsage(AttributeTargets.Class, AllowMultiple = true)]
public sealed class SlintViewModelAttribute(Type component) : Attribute
{
    /// <summary>The generated class of the component.</summary>
    public Type Component { get; } = component;
}

/// <summary>Binds a view model to the properties and callbacks of a component.</summary>
public static class ViewModelBindingExtensions
{
    /// <summary>
    /// Starts binding <paramref name="viewModel"/> to <paramref name="instance"/>; add the bindings with the methods of the result.
    /// Call it on the Slint thread, and dispose the result to remove the bindings.
    /// </summary>
    public static ViewModelBinding<TViewModel> Bind<TViewModel>(this ComponentInstance instance, TViewModel viewModel)
        where TViewModel : class => new(instance, viewModel);
}

/// <summary>
/// The bindings between a view model and a component, from <see cref="ViewModelBindingExtensions.Bind"/>.
/// When the view model raises <see cref="INotifyPropertyChanged.PropertyChanged"/>, from any thread,
/// the bindings read the view model again on the Slint thread, and set the properties whose values changed.
/// </summary>
public sealed class ViewModelBinding<TViewModel> : IDisposable
    where TViewModel : class
{
    private readonly ComponentInstance _instance;
    private readonly TViewModel _viewModel;
    private readonly int _slintThread = Environment.CurrentManagedThreadId;
    private readonly List<Action> _refreshers = [];
    private readonly List<IDisposable> _disposables = [];
    private int _refreshQueued;
    private bool _disposed;

    internal ViewModelBinding(ComponentInstance instance, TViewModel viewModel)
    {
        ArgumentNullException.ThrowIfNull(instance);
        ArgumentNullException.ThrowIfNull(viewModel);
        _instance = instance;
        _viewModel = viewModel;
        if (viewModel is INotifyPropertyChanged notifying)
        {
            notifying.PropertyChanged += OnViewModelChanged;
        }
    }

    /// <summary>
    /// Sets the property <paramref name="name"/> to what <paramref name="get"/> returns, now and after the view model changes.
    /// With <paramref name="set"/>, a change of the property in the UI, such as the user's input, also reaches the view model.
    /// </summary>
    public ViewModelBinding<TViewModel> Property<T>(string name, Func<TViewModel, T> get, Action<TViewModel, T>? set = null)
    {
        ArgumentNullException.ThrowIfNull(get);
        var current = get(_viewModel);
        _instance.SetProperty(name, current);
        _refreshers.Add(() =>
        {
            var value = get(_viewModel);
            if (!EqualityComparer<T>.Default.Equals(value, current))
            {
                current = value;
                _instance.SetProperty(name, value);
            }
        });
        if (set != null)
        {
            _disposables.Add(_instance.WatchProperty(name, () =>
            {
                var value = ValueConverter.ConvertTo<T>(_instance.GetProperty(name));
                if (!EqualityComparer<T>.Default.Equals(value, current))
                {
                    current = value;
                    set(_viewModel, value);
                }
            }));
        }
        return this;
    }

    /// <summary>
    /// Calls <paramref name="set"/> after the property <paramref name="name"/> changes in the UI, such as an <c>out</c> property,
    /// without setting the property from the view model.
    /// </summary>
    public ViewModelBinding<TViewModel> ToViewModel<T>(string name, Action<TViewModel, T> set)
    {
        ArgumentNullException.ThrowIfNull(set);
        set(_viewModel, ValueConverter.ConvertTo<T>(_instance.GetProperty(name)));
        _disposables.Add(_instance.WatchProperty(name, () => set(_viewModel, ValueConverter.ConvertTo<T>(_instance.GetProperty(name)))));
        return this;
    }

    /// <summary>
    /// Sets the array property <paramref name="name"/> to a model of the collection that <paramref name="get"/> returns,
    /// which follows the collection's changes, see <see cref="ObservableCollectionModel{T}"/>.
    /// A collection without change notifications shows as it is.
    /// </summary>
    public ViewModelBinding<TViewModel> Collection<T>(string name, Func<TViewModel, IList<T>?> get) =>
        Collection(name, get, item => item);

    /// <summary>
    /// Like <see cref="Collection{T}(string, Func{TViewModel, IList{T}})"/>,
    /// with <paramref name="map"/> turning each item into a row of the array, such as a struct of the <c>.slint</c> file.
    /// </summary>
    public ViewModelBinding<TViewModel> Collection<T, TRow>(string name, Func<TViewModel, IList<T>?> get, Func<T, TRow> map)
    {
        ArgumentNullException.ThrowIfNull(get);
        ArgumentNullException.ThrowIfNull(map);
        IList<T>? current = null;
        IDisposable? model = null;
        void Apply(IList<T>? collection)
        {
            model?.Dispose();
            model = null;
            current = collection;
            if (collection is System.Collections.Specialized.INotifyCollectionChanged)
            {
                var followed = new ObservableCollectionModel<T>(collection);
                model = followed;
                _instance.SetProperty(name, followed.Map(map));
            }
            else
            {
                _instance.SetProperty(name, new ListModel<TRow>((collection ?? []).Select(map)));
            }
        }
        Apply(get(_viewModel));
        _refreshers.Add(() =>
        {
            var collection = get(_viewModel);
            if (!ReferenceEquals(collection, current))
            {
                Apply(collection);
            }
        });
        _disposables.Add(new Disposer(() => model?.Dispose()));
        return this;
    }

    /// <summary>
    /// Makes the callback <paramref name="callback"/> execute the command that <paramref name="get"/> returns,
    /// with the callback's first argument as the parameter, if it has one.
    /// With <paramref name="enabledProperty"/>, that boolean property follows <see cref="ICommand.CanExecute"/>.
    /// </summary>
    public ViewModelBinding<TViewModel> Command(string callback, Func<TViewModel, ICommand?> get, string? enabledProperty = null)
    {
        ArgumentNullException.ThrowIfNull(get);
        ICommand? command = null;
        void UpdateEnabled()
        {
            if (enabledProperty != null)
            {
                _instance.SetProperty(enabledProperty, command?.CanExecute(null) ?? false);
            }
        }
        void OnCanExecuteChanged(object? sender, EventArgs e) => Post(UpdateEnabled);
        void Apply(ICommand? next)
        {
            if (command != null)
            {
                command.CanExecuteChanged -= OnCanExecuteChanged;
            }
            command = next;
            if (command != null)
            {
                command.CanExecuteChanged += OnCanExecuteChanged;
            }
            UpdateEnabled();
        }
        _instance.SetCallback(callback, args =>
        {
            var parameter = args.Length > 0 ? args[0] : null;
            if (command?.CanExecute(parameter) == true)
            {
                command.Execute(parameter);
            }
        });
        Apply(get(_viewModel));
        _refreshers.Add(() =>
        {
            var next = get(_viewModel);
            if (!ReferenceEquals(next, command))
            {
                Apply(next);
            }
        });
        _disposables.Add(new Disposer(() => Apply(null)));
        return this;
    }

    /// <summary>Removes the bindings; the component keeps the values it has.</summary>
    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }
        _disposed = true;
        if (_viewModel is INotifyPropertyChanged notifying)
        {
            notifying.PropertyChanged -= OnViewModelChanged;
        }
        foreach (var disposable in _disposables)
        {
            disposable.Dispose();
        }
        _disposables.Clear();
        _refreshers.Clear();
    }

    private void OnViewModelChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (Environment.CurrentManagedThreadId == _slintThread)
        {
            Refresh();
        }
        else if (Interlocked.Exchange(ref _refreshQueued, 1) == 0)
        {
            EventLoop.InvokeFromEventLoop(() =>
            {
                Volatile.Write(ref _refreshQueued, 0);
                Refresh();
            });
        }
    }

    private void Refresh()
    {
        if (_disposed)
        {
            return;
        }
        foreach (var refresh in _refreshers)
        {
            refresh();
        }
    }

    private void Post(Action action)
    {
        if (Environment.CurrentManagedThreadId == _slintThread)
        {
            action();
        }
        else
        {
            EventLoop.InvokeFromEventLoop(() =>
            {
                if (!_disposed)
                {
                    action();
                }
            });
        }
    }

    private sealed class Disposer(Action dispose) : IDisposable
    {
        public void Dispose() => dispose();
    }
}
