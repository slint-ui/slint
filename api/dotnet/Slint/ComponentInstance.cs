// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using System.Runtime.InteropServices;
using Slint.Interop;

namespace Slint;

/// <summary>
/// A live instance of a component.
/// Values are converted as listed in <see cref="ValueType"/>; numbers accept any .NET numeric type.
/// </summary>
/// <remarks>
/// Rust keeps each callback delegate alive until the instance is dropped,
/// so a delegate that captures its own instance keeps the instance alive until disposed.
/// </remarks>
public sealed unsafe class ComponentInstance : IDisposable
{
    private readonly ComponentInstanceHandle _handle;
    private readonly LiveEvents? _live;

    internal ComponentInstance(ComponentInstanceHandle handle, LiveEvents? live = null)
    {
        _handle = handle;
        _live = live;
    }

    /// <summary>
    /// Compiles the component <paramref name="componentName"/> of the <c>.slint</c> file <paramref name="path"/> and creates it.
    /// While the event loop runs, the component reloads whenever the file or a file it imports changes,
    /// keeping the window and every property and callback you set.
    /// </summary>
    /// <remarks>
    /// Use it during development only.
    /// A file with errors keeps the previous version.
    /// If a property or callback that you set no longer exists after a change, the application terminates.
    /// </remarks>
    /// <exception cref="InvalidOperationException">On Windows, the thread isn't in a single-threaded apartment, which winit needs.</exception>
    public static ComponentInstance CreateLive(
        string path,
        string componentName,
        string? style = null,
        IReadOnlyList<string>? includePaths = null,
        IReadOnlyDictionary<string, string>? libraryPaths = null,
        string? translationDomain = null,
        string? bundledTranslationsPath = null,
        DefaultTranslationContext defaultTranslationContext = DefaultTranslationContext.ComponentName)
    {
        EventLoop.CheckApartment();
        using var pathStr = new PinnedUtf8(Path.GetFullPath(path));
        using var domainStr = new PinnedUtf8(translationDomain ?? "");
        using var bundledStr = new PinnedUtf8(bundledTranslationsPath is null ? "" : Path.GetFullPath(bundledTranslationsPath));
        using var nameStr = new PinnedUtf8(componentName);
        using var styleStr = new PinnedUtf8(style ?? "");
        using var includes = new PinnedUtf8Array(includePaths ?? []);
        var libraries = (libraryPaths ?? new Dictionary<string, string>()).ToArray();
        using var libraryNames = new PinnedUtf8Array(libraries.Select(l => l.Key));
        using var libraryFiles = new PinnedUtf8Array(libraries.Select(l => l.Value));
        ComponentInstanceHandle handle;
        fixed (Utf8Str* includesPtr = includes.Strs)
        fixed (Utf8Str* namesPtr = libraryNames.Strs)
        fixed (Utf8Str* filesPtr = libraryFiles.Strs)
        {
            handle = NativeMethods.slint_dotnet_live_component_new(
                pathStr.Str,
                nameStr.Str,
                styleStr.Str,
                includesPtr,
                (nuint)includes.Strs.Length,
                namesPtr,
                filesPtr,
                (nuint)libraries.Length,
                domainStr.Str,
                bundledStr.Str,
                defaultTranslationContext == DefaultTranslationContext.ComponentName);
        }
        if (!NativeError.Check(handle))
        {
            throw new SlintException($"Failed to create component {componentName} from {path}");
        }
        var live = new LiveEvents();
        NativeError.Check(NativeMethods.slint_dotnet_live_component_set_reload_hook(
            handle, &ActionTrampoline.Invoke, ActionTrampoline.Alloc(live.Raise), &ActionTrampoline.Release));
        return new ComponentInstance(handle, live);
    }

    /// <summary>Whether the native library can create components with <see cref="CreateLive"/>.</summary>
    public static bool LiveReloadAvailable => EventLoop.BuildFeatures.Contains("live-preview");

    /// <summary>The window of the component.</summary>
    public Window Window => field ??= new Window(_handle);

    /// <summary>Whether the instance comes from <see cref="CreateLive"/>.</summary>
    public bool IsLive => _live != null;

    /// <summary>Raised on the Slint thread after a component from <see cref="CreateLive"/> reloaded.</summary>
    public event Action? Reloaded
    {
        add
        {
            if (_live != null)
            {
                _live.Reloaded += value;
            }
        }
        remove
        {
            if (_live != null)
            {
                _live.Reloaded -= value;
            }
        }
    }

    /// <summary>Returns the value of the public property <paramref name="name"/>.</summary>
    public object? GetProperty(string name)
    {
        using var nameStr = new PinnedUtf8(name);
        return ValueConverter.TakeFromNative(
            NativeError.CheckValue(NativeMethods.slint_dotnet_component_instance_get_property(_handle, nameStr.Str)));
    }

    /// <summary>Sets the public property <paramref name="name"/>.</summary>
    public void SetProperty(string name, object? value)
    {
        using var nameStr = new PinnedUtf8(name);
        var native = ValueConverter.ToNative(value);
        try
        {
            NativeError.Check(NativeMethods.slint_dotnet_component_instance_set_property(_handle, nameStr.Str, native));
        }
        finally
        {
            NativeMethods.slint_dotnet_value_free(native);
        }
    }

    /// <summary>Returns the value of the property <paramref name="name"/> of the global <paramref name="global"/>.</summary>
    public object? GetGlobalProperty(string global, string name)
    {
        using var globalStr = new PinnedUtf8(global);
        using var nameStr = new PinnedUtf8(name);
        return ValueConverter.TakeFromNative(NativeError.CheckValue(
            NativeMethods.slint_dotnet_component_instance_get_global_property(_handle, globalStr.Str, nameStr.Str)));
    }

    /// <summary>Sets the property <paramref name="name"/> of the global <paramref name="global"/>.</summary>
    public void SetGlobalProperty(string global, string name, object? value)
    {
        using var globalStr = new PinnedUtf8(global);
        using var nameStr = new PinnedUtf8(name);
        var native = ValueConverter.ToNative(value);
        try
        {
            NativeError.Check(NativeMethods.slint_dotnet_component_instance_set_global_property(
                _handle, globalStr.Str, nameStr.Str, native));
        }
        finally
        {
            NativeMethods.slint_dotnet_value_free(native);
        }
    }

    /// <summary>Calls the callback or function <paramref name="name"/> and returns its result, or <c>null</c> for <c>void</c>.</summary>
    public object? Invoke(string name, params object?[] args)
    {
        var nameBytes = System.Text.Encoding.UTF8.GetBytes(name);
        return ValueConverter.WithNativeArgs(args, native =>
        {
            fixed (byte* namePtr = nameBytes)
            fixed (nint* argsPtr = native)
            {
                return ValueConverter.TakeFromNative(NativeError.CheckValue(
                    NativeMethods.slint_dotnet_component_instance_invoke(
                        _handle, new Utf8Str(namePtr, (nuint)nameBytes.Length), argsPtr, (nuint)native.Length)));
            }
        });
    }

    /// <summary>Calls the callback or function <paramref name="name"/> of the global <paramref name="global"/>.</summary>
    public object? InvokeGlobal(string global, string name, params object?[] args)
    {
        var globalBytes = System.Text.Encoding.UTF8.GetBytes(global);
        var nameBytes = System.Text.Encoding.UTF8.GetBytes(name);
        return ValueConverter.WithNativeArgs(args, native =>
        {
            fixed (byte* globalPtr = globalBytes)
            fixed (byte* namePtr = nameBytes)
            fixed (nint* argsPtr = native)
            {
                return ValueConverter.TakeFromNative(NativeError.CheckValue(
                    NativeMethods.slint_dotnet_component_instance_invoke_global(
                        _handle,
                        new Utf8Str(globalPtr, (nuint)globalBytes.Length),
                        new Utf8Str(namePtr, (nuint)nameBytes.Length),
                        argsPtr,
                        (nuint)native.Length)));
            }
        });
    }

    /// <summary>
    /// Sets the handler of the callback <paramref name="name"/>.
    /// The handler receives the arguments and returns the result, or <c>null</c> for <c>void</c>.
    /// </summary>
    public void SetCallback(string name, Func<object?[], object?> handler)
    {
        using var nameStr = new PinnedUtf8(name);
        // The native side owns the handle from here, also on failure.
        var userData = GCHandle.ToIntPtr(GCHandle.Alloc(handler));
        NativeError.Check(NativeMethods.slint_dotnet_component_instance_set_callback(
            _handle, nameStr.Str, &CallbackTrampoline.Invoke, userData, &CallbackTrampoline.Release));
    }

    /// <summary>Sets the handler of a callback that returns <c>void</c>.</summary>
    public void SetCallback(string name, Action<object?[]> handler) =>
        SetCallback(name, args =>
        {
            handler(args);
            return null;
        });

    /// <summary>Sets the handler of the callback <paramref name="name"/> of the global <paramref name="global"/>.</summary>
    public void SetGlobalCallback(string global, string name, Func<object?[], object?> handler)
    {
        using var globalStr = new PinnedUtf8(global);
        using var nameStr = new PinnedUtf8(name);
        var userData = GCHandle.ToIntPtr(GCHandle.Alloc(handler));
        NativeError.Check(NativeMethods.slint_dotnet_component_instance_set_global_callback(
            _handle, globalStr.Str, nameStr.Str, &CallbackTrampoline.Invoke, userData, &CallbackTrampoline.Release));
    }

    /// <summary>Sets the handler of a global callback that returns <c>void</c>.</summary>
    public void SetGlobalCallback(string global, string name, Action<object?[]> handler) =>
        SetGlobalCallback(global, name, args =>
        {
            handler(args);
            return null;
        });

    /// <summary>Shows the window of the component.</summary>
    public void Show() => NativeError.Check(NativeMethods.slint_dotnet_component_instance_show(_handle));

    /// <summary>Hides the window of the component.</summary>
    public void Hide() => NativeError.Check(NativeMethods.slint_dotnet_component_instance_hide(_handle));

    /// <summary>Shows the window, runs the event loop until the last window closes, then hides the window.</summary>
    public void Run() => EventLoop.RunNative(() => NativeMethods.slint_dotnet_component_instance_run(_handle));

    /// <summary>Releases the instance.</summary>
    public void Dispose() => _handle.Dispose();
}

/// <summary>The receiver of the reload hook; it doesn't reference the instance, so the hook doesn't keep it alive.</summary>
internal sealed class LiveEvents
{
    public event Action? Reloaded;

    public void Raise() => Reloaded?.Invoke();
}

internal static unsafe class CallbackTrampoline
{
    [UnmanagedCallersOnly]
    public static void Invoke(nint userData, nint* args, nuint count, nint* result)
    {
        try
        {
            var handler = (Func<object?[], object?>)GCHandle.FromIntPtr(userData).Target!;
            var managedArgs = new object?[(int)count];
            for (var i = 0; i < managedArgs.Length; i++)
            {
                managedArgs[i] = ValueConverter.FromNative(args[i]);
            }
            var value = handler(managedArgs);
            *result = value is null ? 0 : ValueConverter.ToNative(value);
        }
        catch (Exception e)
        {
            NativeError.SetPending(e);
        }
    }

    [UnmanagedCallersOnly]
    public static void Release(nint userData) => GCHandle.FromIntPtr(userData).Free();
}
