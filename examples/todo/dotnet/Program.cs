// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using Slint;
using Todo;

internal static class Program
{
    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var window = MainWindow.Create();
        var todos = TodoLogic.Connect(window);

        // Closing with unfinished items asks first.
        window.OnPopupConfirmed(window.Hide);
        window.Window.OnCloseRequested(() =>
        {
            if (todos.Any(todo => !todo.Checked))
            {
                window.InvokeShowConfirmPopup();
                return CloseRequestResponse.KeepWindowShown;
            }
            return CloseRequestResponse.HideWindow;
        });

        window.ShowHeader = true;
        window.Run();
    }
}
