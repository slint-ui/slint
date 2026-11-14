// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

namespace Slint.Tests;

[TestClass]
public sealed class TimerTests
{
    [TestMethod]
    public void SingleShotTimerFiresOnce() => SlintThread.Run(() =>
    {
        using var timer = new Timer();
        Assert.IsFalse(timer.IsRunning);
        var fired = 0;
        timer.Start(TimerMode.SingleShot, TimeSpan.FromMilliseconds(5), () => fired++);
        Assert.IsTrue(timer.IsRunning);
        Assert.AreEqual(TimeSpan.FromMilliseconds(5), timer.Interval);

        Timer.SingleShot(TimeSpan.FromMilliseconds(50), EventLoop.Quit);
        EventLoop.Run();
        Assert.AreEqual(1, fired);
        Assert.IsFalse(timer.IsRunning);
    });

    [TestMethod]
    public void RepeatedTimerFiresUntilStopped() => SlintThread.Run(() =>
    {
        using var timer = new Timer();
        var fired = 0;
        timer.Start(TimerMode.Repeated, TimeSpan.FromMilliseconds(2), () =>
        {
            if (++fired == 3)
            {
                timer.Stop();
                EventLoop.Quit();
            }
        });
        EventLoop.Run();
        Assert.AreEqual(3, fired);
        Assert.IsFalse(timer.IsRunning);

        timer.Restart();
        Assert.IsTrue(timer.IsRunning);
    });

    [TestMethod]
    public void TimerExceptionsStopTheLoop() => SlintThread.Run(() =>
    {
        Timer.SingleShot(TimeSpan.Zero, () => throw new InvalidOperationException("timer"));
        var e = Assert.ThrowsExactly<InvalidOperationException>(EventLoop.Run);
        Assert.AreEqual("timer", e.Message);
    });
}
