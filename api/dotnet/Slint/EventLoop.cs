// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using System.Runtime.InteropServices;
using Slint.Interop;

namespace Slint;

/// <summary>
/// The Slint event loop.
/// On macOS, run it on the main thread, which is the thread that runs <c>Main</c>.
/// </summary>
public static unsafe class EventLoop
{
    [ThreadStatic]
    private static bool t_isRunning;

    internal static bool IsRunning
    {
        get => t_isRunning;
        set => t_isRunning = value;
    }

    /// <summary>
    /// Runs the event loop until <see cref="Quit"/> or until the last window closes.
    /// Rethrows the first exception that a callback threw, after stopping the loop.
    /// </summary>
    public static void Run()
    {
        IsRunning = true;
        try
        {
            NativeError.Check(NativeMethods.slint_dotnet_run_event_loop());
        }
        finally
        {
            IsRunning = false;
        }
    }

    /// <summary>Stops the event loop; callable from any thread.</summary>
    public static void Quit() => NativeError.Check(NativeMethods.slint_dotnet_quit_event_loop());

    /// <summary>Runs <paramref name="action"/> on the Slint thread while the event loop runs; callable from any thread.</summary>
    public static void InvokeFromEventLoop(Action action)
    {
        var userData = GCHandle.ToIntPtr(GCHandle.Alloc(action));
        if (!NativeMethods.slint_dotnet_invoke_from_event_loop(&InvokeTrampoline, userData))
        {
            GCHandle.FromIntPtr(userData).Free();
            throw new SlintException(NativeError.TakeMessage() ?? "invoke_from_event_loop failed");
        }
    }

    /// <summary>The optional capabilities compiled into the native library, such as <c>backend-testing</c>.</summary>
    public static IReadOnlyList<string> BuildFeatures =>
        (NativeMethods.slint_dotnet_build_features().Take() ?? "")
            .Split(',', StringSplitOptions.RemoveEmptyEntries);

    [UnmanagedCallersOnly]
    private static void InvokeTrampoline(nint userData)
    {
        var handle = GCHandle.FromIntPtr(userData);
        try
        {
            ((Action)handle.Target!)();
        }
        catch (Exception e)
        {
            NativeError.SetPending(e);
        }
        finally
        {
            handle.Free();
        }
    }
}
