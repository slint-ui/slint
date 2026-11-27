// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using System.Runtime.CompilerServices;

namespace Slint.Tests;

[TestClass]
public sealed class LifetimeTests
{
    private const string Source = """
        export component App {
            in-out property <[string]> names;
            callback clicked();
        }
        """;

    private static void Collect()
    {
        GC.Collect();
        GC.WaitForPendingFinalizers();
        GC.Collect();
    }

    /// <summary>Runs the event loop until the work queued so far, such as deferred releases, is done.</summary>
    private static void Drain()
    {
        EventLoop.InvokeFromEventLoop(EventLoop.Quit);
        EventLoop.Run();
    }

    [MethodImpl(MethodImplOptions.NoInlining)]
    private static (ComponentInstance, WeakReference) CreateWithCallback()
    {
        var app = ModelTests.Create(Source);
        var calls = new List<int>();
        Action<object?[]> handler = _ => calls.Add(1);
        app.SetCallback("clicked", handler);
        return (app, new WeakReference(handler));
    }

    [TestMethod]
    public void DisposeReleasesCallbacks() => SlintThread.Run(() =>
    {
        var (app, handler) = CreateWithCallback();
        Collect();
        Assert.IsTrue(handler.IsAlive);

        app.Dispose();
        Collect();
        Assert.IsFalse(handler.IsAlive);
    });

    [MethodImpl(MethodImplOptions.NoInlining)]
    private static WeakReference CreateAndForget() => CreateWithCallback().Item2;

    [TestMethod]
    public void FinalizerReleasesInstancesOnTheSlintThread() => SlintThread.Run(() =>
    {
        var handler = CreateAndForget();
        Collect();
        // The finalizer thread queued the release for the Slint thread.
        Drain();
        Collect();
        Assert.IsFalse(handler.IsAlive);
    });

    [MethodImpl(MethodImplOptions.NoInlining)]
    private static WeakReference SetModel(ComponentInstance app)
    {
        var names = new ListModel<string>(["kept"]);
        app.SetProperty("names", names);
        return new WeakReference(names);
    }

    // Keeps the model returned by GetProperty out of the caller's frame, where a debug build would keep it alive.
    [MethodImpl(MethodImplOptions.NoInlining)]
    private static string FirstName(ComponentInstance app) => ((Model<string>)app.GetProperty("names")!)[0];

    [TestMethod]
    public void SlintKeepsModelsAliveWhileItUsesThem() => SlintThread.Run(() =>
    {
        using var app = ModelTests.Create(Source);
        var names = SetModel(app);
        Collect();
        Assert.IsTrue(names.IsAlive);
        Assert.AreEqual("kept", FirstName(app));

        app.SetProperty("names", Array.Empty<string>());
        Collect();
        Assert.IsFalse(names.IsAlive);
    });

    [TestMethod]
    public void ModelsCanBeSetAgainAfterSlintDroppedThem() => SlintThread.Run(() =>
    {
        using var app = ModelTests.Create(Source);
        var names = new ListModel<string>(["a"]);
        app.SetProperty("names", names);
        app.SetProperty("names", Array.Empty<string>());
        names.Add("b");
        app.SetProperty("names", names);
        Assert.AreSame(names, app.GetProperty("names"));
        Assert.AreEqual(2, ((Model<string>)app.GetProperty("names")!).Count);
    });
}
