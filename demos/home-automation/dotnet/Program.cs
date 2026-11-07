// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using HomeAutomation;

internal static class Program
{
    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var window = AppWindow.Create();

        // The UI shows the date and time that it gets from here, every second.
        using var clock = new Slint.Timer();
        clock.Start(Slint.TimerMode.Repeated, TimeSpan.FromSeconds(1), () =>
        {
            var now = DateTime.Now;
            window.Api.CurrentDate = new Date { Year = now.Year, Month = now.Month, Day = now.Day };
            window.Api.CurrentTime = new Time { Hour = now.Hour, Minute = now.Minute, Second = now.Second };
        });

        window.Run();
    }
}
