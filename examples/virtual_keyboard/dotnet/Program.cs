// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using Slint;
using VirtualKeyboard;

internal static class Program
{
    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var window = MainWindow.Create();

        // A key of the on-screen keyboard acts like the same key on a real one, for the focused TextInput.
        window.VirtualKeyboardHandler.OnKeyPressed(key =>
        {
            window.Window.DispatchEvent(new WindowEvent.KeyPressed(key));
            window.Window.DispatchEvent(new WindowEvent.KeyReleased(key));
        });

        window.Run();
    }
}
