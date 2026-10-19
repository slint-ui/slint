// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using System.Collections.Concurrent;
using System.Runtime.ExceptionServices;
using System.Runtime.InteropServices;

[assembly: DoNotParallelize]

namespace Slint.Tests;

/// <summary>
/// Runs test bodies on one thread, since Slint objects belong to the thread that first used Slint.
/// </summary>
public static partial class SlintThread
{
    private static readonly BlockingCollection<Action> s_queue = [];

    static SlintThread()
    {
        // The native library reads the process environment, which Environment.SetEnvironmentVariable doesn't change on Unix.
        if (!OperatingSystem.IsWindows() && Environment.GetEnvironmentVariable("SLINT_BACKEND") is null)
        {
            setenv("SLINT_BACKEND", "testing", 1);
        }
        var thread = new Thread(() =>
        {
            foreach (var action in s_queue.GetConsumingEnumerable())
            {
                action();
            }
        })
        {
            IsBackground = true,
            Name = "Slint",
        };
        thread.Start();
    }

    public static void Run(Action body) =>
        Run<object?>(() =>
        {
            body();
            return null;
        });

    public static T Run<T>(Func<T> body)
    {
        T result = default!;
        ExceptionDispatchInfo? error = null;
        using var done = new ManualResetEventSlim();
        s_queue.Add(() =>
        {
            try
            {
                result = body();
            }
            catch (Exception e)
            {
                error = ExceptionDispatchInfo.Capture(e);
            }
            finally
            {
                done.Set();
            }
        });
        done.Wait();
        error?.Throw();
        return result;
    }

    [LibraryImport("libc", StringMarshalling = StringMarshalling.Utf8)]
    private static partial int setenv(string name, string value, int overwrite);
}
