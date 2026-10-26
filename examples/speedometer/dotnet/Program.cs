// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using Speedometer;

internal static class Program
{
    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var window = MainWindow.Create();
        window.Run();
    }
}
