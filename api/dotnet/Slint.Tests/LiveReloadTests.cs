// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

namespace Slint.Tests;

[TestClass]
public sealed class LiveReloadTests
{
    private static string Source(string label) => $$"""
        export component App inherits Window {
            in-out property <int> value: 1;
            out property <string> label: "{{label}}";
            in-out property <[string]> names;
            callback clicked() -> string;
            public function call-clicked() -> string { return root.clicked(); }
        }
        """;

    /// <summary>
    /// A file to change in the test output directory.
    /// The system temporary directory can be behind a symbolic link, such as /var on macOS,
    /// and the file watcher reports the resolved paths.
    /// </summary>
    private static string NewFile(string source)
    {
        var dir = Path.Combine(AppContext.BaseDirectory, "live-reload", Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(dir);
        var path = Path.Combine(dir, "app.slint");
        File.WriteAllText(path, source);
        return path;
    }

    /// <summary>Runs the event loop, calls <paramref name="change"/> in it, and returns whether the component reloaded.</summary>
    private static bool RunUntilReloaded(ComponentInstance app, Action change, TimeSpan timeout)
    {
        var reloaded = false;
        void OnReloaded()
        {
            reloaded = true;
            EventLoop.Quit();
        }
        app.Reloaded += OnReloaded;
        using var timer = new Timer();
        timer.Start(TimerMode.SingleShot, timeout, EventLoop.Quit);
        EventLoop.InvokeFromEventLoop(change);
        EventLoop.Run();
        app.Reloaded -= OnReloaded;
        return reloaded;
    }

    [TestMethod]
    public void NativeLibraryCanReload() =>
        Assert.IsTrue(SlintThread.Run(() => ComponentInstance.LiveReloadAvailable));

    [TestMethod]
    public void ReloadKeepsPropertiesModelsAndCallbacks() => SlintThread.Run(() =>
    {
        var path = NewFile(Source("first"));
        using var app = ComponentInstance.CreateLive(path, "App");
        Assert.IsTrue(app.IsLive);
        app.SetProperty("value", 5);
        var names = new ListModel<string>(["Ada"]);
        app.SetProperty("names", names);
        app.SetCallback("clicked", _ => "handled");
        Assert.AreEqual("first", app.GetProperty("label"));

        Assert.IsTrue(
            RunUntilReloaded(app, () => File.WriteAllText(path, Source("second")), TimeSpan.FromSeconds(20)),
            "the component didn't reload");

        Assert.AreEqual("second", app.GetProperty("label"));
        Assert.AreEqual(5.0, app.GetProperty("value"));
        Assert.AreSame(names, app.GetProperty("names"));
        Assert.AreEqual("handled", app.Invoke("call-clicked"));
    });

    [TestMethod]
    public void BrokenFileKeepsThePreviousVersion() => SlintThread.Run(() =>
    {
        var path = NewFile(Source("first"));
        using var app = ComponentInstance.CreateLive(path, "App");

        Assert.IsFalse(
            RunUntilReloaded(app, () => File.WriteAllText(path, "export component App { syntax error"), TimeSpan.FromSeconds(2)),
            "a file with errors replaced the component");
        Assert.AreEqual("first", app.GetProperty("label"));

        Assert.IsTrue(
            RunUntilReloaded(app, () => File.WriteAllText(path, Source("fixed")), TimeSpan.FromSeconds(20)),
            "the component didn't reload after the fix");
        Assert.AreEqual("fixed", app.GetProperty("label"));
    });

    [TestMethod]
    public void CreateLiveReportsErrors() => SlintThread.Run(() =>
    {
        var path = NewFile(Source("first"));
        Assert.ThrowsExactly<SlintException>(() => ComponentInstance.CreateLive(path, "Missing"));
        var broken = NewFile("export component App { syntax error");
        Assert.ThrowsExactly<SlintException>(() => ComponentInstance.CreateLive(broken, "App"));
    });

    [TestMethod]
    public void StaticInstancesAreNotLive() => SlintThread.Run(() =>
    {
        using var compiler = new Compiler();
        using var result = compiler.BuildFromSource(Source("first"), "app.slint");
        using var definition = result.Component("App")!;
        using var app = definition.Create();
        Assert.IsFalse(app.IsLive);
        app.Reloaded += () => { };
    });

    [TestMethod]
    public void GeneratedCodeCreatesLiveComponents() => SlintThread.Run(() =>
    {
        using var app = LiveGenerated.Create();
        Assert.IsTrue(app.Instance.IsLive);
        Assert.AreEqual(3, app.Count);
        app.Count = 4;
        Assert.AreEqual(4, app.Count);
    });
}
