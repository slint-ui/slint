// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using Lols;

internal static class Program
{
    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static int Main(string[] args)
    {
        var mode = args.Length > 0 ? args[0] : LolsBenchmark.Modes[0];
        if (!LolsBenchmark.Modes.Contains(mode))
        {
            Console.Error.WriteLine($"Usage: Lols [{string.Join('|', LolsBenchmark.Modes)}]");
            return 2;
        }

        using var window = MainWindow.Create();
        using var benchmark = LolsBenchmark.Start(window, mode, Console.WriteLine);
        window.Run();
        return 0;
    }
}
