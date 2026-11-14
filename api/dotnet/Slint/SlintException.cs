// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using System.Runtime.ExceptionServices;
using Slint.Interop;

namespace Slint;

/// <summary>The exception Slint throws when a native call fails.</summary>
public class SlintException(string message) : Exception(message);

/// <summary>Turns native failures and exceptions thrown by .NET callbacks into exceptions.</summary>
internal static class NativeError
{
    [ThreadStatic]
    private static ExceptionDispatchInfo? s_pendingCallbackException;

    /// <summary>
    /// Keeps an exception thrown by a .NET callback until the native call returns,
    /// since exceptions can't cross the native frames.
    /// </summary>
    public static void SetPending(Exception exception)
    {
        s_pendingCallbackException ??= ExceptionDispatchInfo.Capture(exception);
        if (EventLoop.IsRunning)
        {
            NativeMethods.slint_dotnet_quit_event_loop();
        }
    }

    public static void ThrowPending()
    {
        var pending = s_pendingCallbackException;
        s_pendingCallbackException = null;
        pending?.Throw();
    }

    public static string? TakeMessage() => NativeMethods.slint_dotnet_last_error().Take();

    public static void Check(bool ok)
    {
        ThrowPending();
        if (!ok)
        {
            throw new SlintException(TakeMessage() ?? "unknown Slint error");
        }
    }

    /// <summary>Throws if <paramref name="handle"/> is invalid because of an error, and returns whether it's valid.</summary>
    public static bool Check(NativeHandle handle)
    {
        ThrowPending();
        if (!handle.IsInvalid)
        {
            return true;
        }
        handle.Dispose();
        if (TakeMessage() is { } message)
        {
            throw new SlintException(message);
        }
        return false;
    }

    /// <summary>Throws if <paramref name="value"/> is null, and returns it.</summary>
    public static nint CheckValue(nint value)
    {
        ThrowPending();
        if (value == 0)
        {
            throw new SlintException(TakeMessage() ?? "unknown Slint error");
        }
        return value;
    }
}
