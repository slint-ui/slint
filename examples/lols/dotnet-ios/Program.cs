// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using Lols;

internal static class Program
{
    // .NET for iOS passes no arguments to Main, so pick the mode with an environment variable, such as:
    // SIMCTL_CHILD_LOLS_MODE=elements xcrun simctl launch booted dev.slint.dotnet.lols
    private static void Main()
    {
        var mode = Environment.GetEnvironmentVariable("LOLS_MODE") is { Length: > 0 } value ? value : LolsBenchmark.Modes[0];
        if (!LolsBenchmark.Modes.Contains(mode))
        {
            throw new ArgumentException($"Unknown mode {mode}, expected one of {string.Join(", ", LolsBenchmark.Modes)}");
        }

        using var window = MainWindow.Create();
        using var benchmark = LolsBenchmark.Start(window, mode, Console.WriteLine);
        // Slint starts UIKit, and Run doesn't return.
        window.Run();
    }
}
