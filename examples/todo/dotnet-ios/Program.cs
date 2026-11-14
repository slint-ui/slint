// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using Todo;
using UIKit;

internal static class Program
{
    private static void Main()
    {
        using var window = MainWindow.Create();
        var todos = TodoLogic.Connect(window);
        window.ShowHeader = true;

        // iOS ends an app in the background without notice, so keep the list whenever the app leaves the screen.
        // UIKit posts the notification on the main thread, which is the Slint thread.
        var file = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments), "todos.json");
        if (File.Exists(file))
        {
            TodoState.Restore(window, todos, File.ReadAllBytes(file));
        }
        using var observer = UIApplication.Notifications.ObserveDidEnterBackground(
            (_, _) => File.WriteAllBytes(file, TodoState.Save(window, todos)));

        // Slint starts UIKit, and Run doesn't return.
        window.Run();
    }
}
