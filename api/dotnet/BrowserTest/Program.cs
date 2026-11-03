// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using BrowserTest;
using Slint;

using var app = AppWindow.Create();
app.OnRequestIncreaseValue(() =>
{
    app.Counter++;
    Console.WriteLine($"counter: {app.Counter}");
});
app.OnNameAccepted(async name =>
{
    await Task.Yield();
    app.Name = name;
    Console.WriteLine($"name: {name}");
});
app.OnQuit(() =>
{
    if (app.Name == "throw")
    {
        throw new InvalidOperationException("thrown by a callback");
    }
    EventLoop.Quit();
});
using var timer = new Slint.Timer();
timer.Start(TimerMode.Repeated, TimeSpan.FromMilliseconds(100), () => app.Ticks++);

Console.WriteLine("ready");
try
{
    await app.RunAsync();
    Console.WriteLine($"quit after {app.Ticks} ticks");
}
catch (InvalidOperationException e)
{
    Console.WriteLine($"stopped: {e.Message}");
}
