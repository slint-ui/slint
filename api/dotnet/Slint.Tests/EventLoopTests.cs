// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

namespace Slint.Tests;

[TestClass]
public sealed class EventLoopTests
{
    [TestMethod]
    public void NativeLibraryHasTheTestingBackend() =>
        CollectionAssert.Contains(SlintThread.Run(() => EventLoop.BuildFeatures.ToArray()), "backend-testing");

    [TestMethod]
    public void RunsInvokedActionsUntilQuit() => SlintThread.Run(() =>
    {
        var order = new List<int>();
        EventLoop.InvokeFromEventLoop(() => order.Add(1));
        EventLoop.InvokeFromEventLoop(() =>
        {
            order.Add(2);
            EventLoop.Quit();
        });
        EventLoop.Run();
        CollectionAssert.AreEqual(new[] { 1, 2 }, order);
    });

    [TestMethod]
    public void InvokeFromEventLoopWorksFromOtherThreads()
    {
        var ranOn = SlintThread.Run(() =>
        {
            int? threadId = null;
            var worker = new Thread(() => EventLoop.InvokeFromEventLoop(() =>
            {
                threadId = Environment.CurrentManagedThreadId;
                EventLoop.Quit();
            }));
            worker.Start();
            worker.Join();
            EventLoop.Run();
            return (threadId, Environment.CurrentManagedThreadId);
        });
        Assert.AreEqual(ranOn.CurrentManagedThreadId, ranOn.threadId);
    }

    [TestMethod]
    public void RunRethrowsCallbackExceptions() => SlintThread.Run(() =>
    {
        EventLoop.InvokeFromEventLoop(() => throw new InvalidOperationException("from the loop"));
        var e = Assert.ThrowsExactly<InvalidOperationException>(EventLoop.Run);
        Assert.AreEqual("from the loop", e.Message);
    });

    [TestMethod]
    public void ShowAndHideWithTheTestingBackend() => SlintThread.Run(() =>
    {
        using var compiler = new Compiler();
        using var result = compiler.BuildFromSource("export component App inherits Window { }", "app.slint");
        using var definition = result.Component("App")!;
        using var app = definition.Create();
        app.Show();
        app.Hide();
    });
}
