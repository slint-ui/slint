// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using Slint;
using SystemTray;

internal static class Program
{
    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var tray = ExampleTray.Create();
        using var window = MainWindow.Create();

        tray.TrayTitle = window.TrayTitle;
        tray.TrayTooltip = window.TrayTooltip;
        tray.TrayVisible = window.TrayVisible;
        tray.MenuEnabled = window.TrayMenuEnabled;

        tray.OnToggleWindow(() =>
        {
            var show = !window.Window.IsVisible;
            if (show)
            {
                window.Show();
            }
            else
            {
                window.Hide();
            }
            window.ActivationLog = $"Tray activated → window {(show ? "shown" : "hidden")}";
        });
        tray.OnQuit(EventLoop.Quit);

        // The window edits the tray's settings.
        window.OnTrayTitleChanged(title => tray.TrayTitle = title);
        window.OnTrayTooltipChanged(tooltip => tray.TrayTooltip = tooltip);
        window.OnTrayVisibleChanged(visible => tray.TrayVisible = visible);
        window.OnTrayMenuEnabledChanged(enabled => tray.MenuEnabled = enabled);
        window.OnHideToTray(window.Hide);
        window.OnQuit(EventLoop.Quit);

        window.Show();
        EventLoop.Run();
    }
}
