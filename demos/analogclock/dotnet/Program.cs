// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using AnalogClock;

internal static class Program
{
    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var window = MainWindow.Create();

        // The Rust version runs on microcontrollers without a clock, so it counts from a fixed time; this one shows the time of day.
        void ShowTime()
        {
            var now = DateTime.Now;
            window.Hours = now.Hour % 12;
            window.Minutes = now.Minute;
            window.Seconds = now.Second;
        }
        ShowTime();
        using var timer = new Slint.Timer();
        timer.Start(Slint.TimerMode.Repeated, TimeSpan.FromSeconds(1), ShowTime);

        window.Run();
    }
}
