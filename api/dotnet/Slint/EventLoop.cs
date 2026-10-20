// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using System.Runtime.InteropServices;
using Slint.Interop;

namespace Slint;

/// <summary>
/// The Slint event loop.
/// On macOS, run it on the main thread, which is the thread that runs <c>Main</c>.
/// </summary>
/// <remarks>
/// While the loop runs, <see cref="SynchronizationContext.Current"/> is a <see cref="SlintSynchronizationContext"/>,
/// so <c>await</c> in a callback continues on the Slint thread.
/// </remarks>
public static unsafe class EventLoop
{
    [ThreadStatic]
    private static bool t_isRunning;

    internal static bool IsRunning => t_isRunning;

    /// <summary>
    /// Runs the event loop until <see cref="Quit"/> or until the last window closes.
    /// Rethrows the first exception that a callback threw, after stopping the loop.
    /// </summary>
    /// <exception cref="InvalidOperationException">On Windows, the thread isn't in a single-threaded apartment, which winit needs.</exception>
    public static void Run() => RunNative(NativeMethods.slint_dotnet_run_event_loop);

    /// <summary>
    /// Runs the event loop and <paramref name="main"/> on it, until <paramref name="main"/> completes,
    /// <see cref="Quit"/> is called, or the last window closes.
    /// Rethrows the exception of <paramref name="main"/>.
    /// </summary>
    public static void Run(Func<Task> main)
    {
        Task? task = null;
        InvokeFromEventLoop(() =>
        {
            task = main();
            task.ContinueWith(_ => Quit(), TaskContinuationOptions.ExecuteSynchronously);
        });
        Run();
        if (task is { IsCompleted: true })
        {
            task.GetAwaiter().GetResult();
        }
    }

    internal static void RunNative(Func<bool> run)
    {
        CheckApartment();
        var previous = SynchronizationContext.Current;
        SynchronizationContext.SetSynchronizationContext(new SlintSynchronizationContext(Environment.CurrentManagedThreadId));
        t_isRunning = true;
        try
        {
            NativeError.Check(run());
        }
        finally
        {
            t_isRunning = false;
            SynchronizationContext.SetSynchronizationContext(previous);
        }
    }

    /// <summary>
    /// On Windows, winit sets up drag and drop with <c>OleInitialize</c>,
    /// which fails on a thread in a multithreaded apartment and aborts the process.
    /// </summary>
    internal static void CheckApartment()
    {
        if (OperatingSystem.IsWindows()
            && Thread.CurrentThread.GetApartmentState() != ApartmentState.STA
            && NativeMethods.slint_dotnet_backend_is_winit())
        {
            throw new InvalidOperationException(
                "Slint windows on Windows need a single-threaded apartment. "
                + "Add [STAThread] to Main, which top-level statements and async Main can't have, "
                + "or use Slint from a thread that you start after calling SetApartmentState(ApartmentState.STA).");
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

/// <summary>Runs posted work on the Slint thread through <see cref="EventLoop.InvokeFromEventLoop"/>.</summary>
public sealed class SlintSynchronizationContext : SynchronizationContext
{
    private readonly int _slintThreadId;

    internal SlintSynchronizationContext(int slintThreadId)
    {
        _slintThreadId = slintThreadId;
    }

    /// <inheritdoc/>
    public override void Post(SendOrPostCallback d, object? state) => EventLoop.InvokeFromEventLoop(() => d(state));

    /// <inheritdoc/>
    public override void Send(SendOrPostCallback d, object? state)
    {
        if (Environment.CurrentManagedThreadId == _slintThreadId)
        {
            d(state);
            return;
        }
        System.Runtime.ExceptionServices.ExceptionDispatchInfo? error = null;
        using var done = new ManualResetEventSlim();
        EventLoop.InvokeFromEventLoop(() =>
        {
            try
            {
                d(state);
            }
            catch (Exception e)
            {
                error = System.Runtime.ExceptionServices.ExceptionDispatchInfo.Capture(e);
            }
            finally
            {
                done.Set();
            }
        });
        done.Wait();
        error?.Throw();
    }

    /// <inheritdoc/>
    public override SynchronizationContext CreateCopy() => this;
}
