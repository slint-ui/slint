// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using Todo;

internal static class Program
{
    private static void Main()
    {
        using var window = MainWindow.Create();
        TodoLogic.Connect(window);
        window.ShowHeader = true;
        // Slint starts UIKit, and Run doesn't return.
        window.Run();
    }
}
