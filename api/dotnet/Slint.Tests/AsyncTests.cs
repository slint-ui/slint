// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

namespace Slint.Tests;

[TestClass]
public sealed class AsyncTests
{
    [TestMethod]
    public void AwaitContinuesOnTheSlintThread() => SlintThread.Run(() =>
    {
        var slintThread = Environment.CurrentManagedThreadId;
        var threads = new List<int>();
        EventLoop.Run(async () =>
        {
            Assert.IsInstanceOfType<SlintSynchronizationContext>(SynchronizationContext.Current);
            threads.Add(Environment.CurrentManagedThreadId);
            await Task.Delay(5);
            threads.Add(Environment.CurrentManagedThreadId);
            await Task.Run(() => Thread.Sleep(5));
            threads.Add(Environment.CurrentManagedThreadId);
        });
        CollectionAssert.AreEqual(new[] { slintThread, slintThread, slintThread }, threads);
        Assert.IsNotInstanceOfType<SlintSynchronizationContext>(SynchronizationContext.Current);
    });

    [TestMethod]
    public void RunRethrowsTheExceptionOfMain() => SlintThread.Run(() =>
    {
        var e = Assert.ThrowsExactly<InvalidOperationException>(() => EventLoop.Run(async () =>
        {
            await Task.Yield();
            throw new InvalidOperationException("async");
        }));
        Assert.AreEqual("async", e.Message);
    });

    [TestMethod]
    public void AsyncCallbacksUpdateProperties() => SlintThread.Run(() =>
    {
        using var app = ModelTests.Create("""
            export component App {
                in-out property <string> status: "idle";
                callback load();
                public function start() { root.load(); }
            }
            """);
        var loaded = new TaskCompletionSource();
        app.SetCallback("load", args =>
        {
            async Task LoadAsync()
            {
                app.SetProperty("status", "loading");
                var data = await Task.Run(() => "done");
                app.SetProperty("status", data);
                loaded.SetResult();
            }
            _ = LoadAsync();
        });

        EventLoop.Run(async () =>
        {
            app.Invoke("start");
            Assert.AreEqual("loading", app.GetProperty("status"));
            await loaded.Task;
        });
        Assert.AreEqual("done", app.GetProperty("status"));
    });

    [TestMethod]
    public void SendRunsSynchronouslyFromOtherThreads() => SlintThread.Run(() =>
    {
        var ranOn = 0;
        EventLoop.Run(async () =>
        {
            var context = SynchronizationContext.Current!;
            await Task.Run(() => context.Send(_ => ranOn = Environment.CurrentManagedThreadId, null));
        });
        Assert.AreEqual(Environment.CurrentManagedThreadId, ranOn);
    });
}
