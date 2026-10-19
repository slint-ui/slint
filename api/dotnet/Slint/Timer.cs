// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using System.Runtime.InteropServices;
using Slint.Interop;

namespace Slint;

/// <summary>Whether a <see cref="Timer"/> fires once or repeatedly.</summary>
public enum TimerMode
{
    /// <summary>Fires once.</summary>
    SingleShot,
    /// <summary>Fires every interval until stopped or disposed.</summary>
    Repeated,
}

/// <summary>
/// Calls an action on the Slint thread after an interval, while the event loop runs.
/// The timer stops when disposed or collected.
/// </summary>
public sealed unsafe class Timer : IDisposable
{
    private readonly TimerHandle _handle;

    /// <summary>Creates a stopped timer.</summary>
    public Timer()
    {
        _handle = NativeMethods.slint_dotnet_timer_new();
        NativeError.Check(_handle);
    }

    /// <summary>Starts the timer, replacing the action and interval of a running one.</summary>
    public void Start(TimerMode mode, TimeSpan interval, Action action) =>
        NativeError.Check(NativeMethods.slint_dotnet_timer_start(
            _handle,
            mode == TimerMode.Repeated,
            ToMilliseconds(interval),
            &ActionTrampoline.Invoke,
            ActionTrampoline.Alloc(action),
            &ActionTrampoline.Release));

    /// <summary>Calls <paramref name="action"/> once after <paramref name="interval"/>, without a timer object.</summary>
    public static void SingleShot(TimeSpan interval, Action action) =>
        NativeError.Check(NativeMethods.slint_dotnet_timer_single_shot(
            ToMilliseconds(interval),
            &ActionTrampoline.Invoke,
            ActionTrampoline.Alloc(action),
            &ActionTrampoline.Release));

    /// <summary>Stops the timer; <see cref="Restart"/> starts it again with the same action.</summary>
    public void Stop() => NativeError.Check(NativeMethods.slint_dotnet_timer_stop(_handle));

    /// <summary>Starts the timer again from now, with its last action and interval.</summary>
    public void Restart() => NativeError.Check(NativeMethods.slint_dotnet_timer_restart(_handle));

    /// <summary>Whether the timer is started and hasn't fired yet, or repeats.</summary>
    public bool IsRunning
    {
        get
        {
            var running = NativeMethods.slint_dotnet_timer_running(_handle);
            NativeError.ThrowPending();
            return running;
        }
    }

    /// <summary>The interval of a started timer; setting it on a stopped timer has no effect.</summary>
    public TimeSpan Interval
    {
        get => TimeSpan.FromMilliseconds(NativeMethods.slint_dotnet_timer_interval_ms(_handle));
        set => NativeError.Check(NativeMethods.slint_dotnet_timer_set_interval(_handle, ToMilliseconds(value)));
    }

    /// <summary>Stops the timer and releases it.</summary>
    public void Dispose() => _handle.Dispose();

    private static ulong ToMilliseconds(TimeSpan interval) =>
        interval < TimeSpan.Zero ? 0 : (ulong)interval.TotalMilliseconds;
}

/// <summary>Native callbacks to an <see cref="Action"/> that the native side owns.</summary>
internal static class ActionTrampoline
{
    public static nint Alloc(Action action) => GCHandle.ToIntPtr(GCHandle.Alloc(action));

    [UnmanagedCallersOnly]
    public static void Invoke(nint userData)
    {
        try
        {
            ((Action)GCHandle.FromIntPtr(userData).Target!)();
        }
        catch (Exception e)
        {
            NativeError.SetPending(e);
        }
    }

    [UnmanagedCallersOnly]
    public static void Release(nint userData) => GCHandle.FromIntPtr(userData).Free();
}
