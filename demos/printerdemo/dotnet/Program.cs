// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using PrinterDemo;
using Slint;

internal static class Program
{
    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var window = MainWindow.Create();

        window.PrinterState.InkLevels = new ListModel<InkLevel>(
        [
            new() { Color = new Color(0, 255, 255), Level = 0.4f },
            new() { Color = new Color(255, 0, 255), Level = 0.2f },
            new() { Color = new Color(255, 255, 0), Level = 0.5f },
            new() { Color = new Color(0, 0, 0), Level = 0.8f },
        ]);

        var queue = new ListModel<PrinterQueueItem>(window.PrinterQueue.PrinterQueue_);
        window.PrinterQueue.PrinterQueue_ = queue;

        window.OnQuit(window.Hide);
        window.PrinterQueue.OnStartJob(title => queue.Add(new PrinterQueueItem
        {
            Status = "waiting",
            Title = title,
            Owner = Environment.UserName,
            Pages = 1,
            Size = "100kB",
            SubmissionDate = DateTime.Now.ToString("g"),
        }));
        window.PrinterQueue.OnCancelJob(queue.RemoveAt);

        // Advances the job at the top of the queue by one percent every second.
        using var progress = new Slint.Timer();
        progress.Start(TimerMode.Repeated, TimeSpan.FromSeconds(1), () =>
        {
            if (queue.Count == 0)
            {
                return;
            }
            var top = queue[0] with { Progress = queue[0].Progress + 1, Status = "printing" };
            if (top.Progress > 100)
            {
                queue.RemoveAt(0);
                return;
            }
            queue[0] = top;
        });

        // The strings in the .slint files are English, so English selects no translation.
        window.PrinterSettings.OnChangeLanguage(language =>
        {
            switch (language)
            {
                case 0:
                    Translations.SelectBundledTranslation("");
                    break;
                case 1:
                    Translations.SelectBundledTranslation("fr");
                    break;
            }
        });

        window.Run();
    }
}
