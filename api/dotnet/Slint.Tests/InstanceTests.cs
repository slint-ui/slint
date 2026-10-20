// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

namespace Slint.Tests;

[TestClass]
public sealed class InstanceTests
{
    private const string Source = """
        export enum Mode { idle, busy }
        export struct Person { name: string, age: int }
        export global Settings {
            in-out property <string> theme: "dark";
            callback changed(string) -> string;
            public function describe() -> string { return "theme: " + self.theme; }
        }
        export component App inherits Window {
            in-out property <int> count: 42;
            in-out property <float> ratio: 0.5;
            in-out property <string> caption: "hello";
            in-out property <bool> ready: true;
            in-out property <color> tint: #102030;
            in-out property <brush> fill: @linear-gradient(90deg, red 0%, blue 100%);
            in-out property <Person> person: { name: "Ada", age: 36 };
            in-out property <[int]> numbers: [1, 2, 3];
            in-out property <Mode> mode: Mode.busy;
            in-out property <image> picture;
            in-out property <keys> shortcut;
            in-out property <Point> point;
            callback compute(int, string) -> string;
            callback notify();
            public function twice(x: int) -> int { return x * 2; }
            public function call-compute() -> string { return root.compute(7, "x"); }
            public function call-notify() { root.notify(); }
            public function call-global() -> string { return Settings.changed("light"); }
        }
        """;

    private static ComponentInstance CreateApp()
    {
        using var compiler = new Compiler();
        using var result = compiler.BuildFromSource(Source, "app.slint");
        Assert.IsFalse(result.HasErrors, string.Join("\n", result.Diagnostics));
        using var definition = result.Component("App")!;
        return definition.Create();
    }

    [TestMethod]
    public void ReadsPropertyDefaults() => SlintThread.Run(() =>
    {
        using var app = CreateApp();
        Assert.AreEqual(42.0, app.GetProperty("count"));
        Assert.AreEqual(0.5, (double)app.GetProperty("ratio")!, 1e-6);
        Assert.AreEqual("hello", app.GetProperty("caption"));
        Assert.IsTrue((bool)app.GetProperty("ready")!);
        Assert.AreEqual(new Color(0x10, 0x20, 0x30), app.GetProperty("tint"));
        Assert.AreEqual(new EnumValue("Mode", "busy"), app.GetProperty("mode"));
        CollectionAssert.AreEqual(new object[] { 1.0, 2.0, 3.0 }, ((Model<object?>)app.GetProperty("numbers")!).ToArray());

        var person = (Struct)app.GetProperty("person")!;
        Assert.AreEqual("Ada", person["name"]);
        Assert.AreEqual(36.0, person["age"]);

        using var fill = (Brush)app.GetProperty("fill")!;
        Assert.AreEqual(new Color(0xFF, 0, 0), fill.Color);
        using var picture = (Image)app.GetProperty("picture")!;
        Assert.IsNull(picture.Path);
    });

    [TestMethod]
    public void WritesProperties() => SlintThread.Run(() =>
    {
        using var app = CreateApp();
        app.SetProperty("count", 7);
        app.SetProperty("ratio", 1.25f);
        app.SetProperty("caption", "wörld ✓");
        app.SetProperty("ready", false);
        app.SetProperty("tint", Color.FromArgb(0x80FF0000));
        app.SetProperty("mode", new EnumValue("Mode", "idle"));
        app.SetProperty("numbers", new[] { 4, 5 });
        app.SetProperty("person", new Struct { ["name"] = "Grace", ["age"] = 85 });

        Assert.AreEqual(7.0, app.GetProperty("count"));
        Assert.AreEqual(1.25, app.GetProperty("ratio"));
        Assert.AreEqual("wörld ✓", app.GetProperty("caption"));
        Assert.IsFalse((bool)app.GetProperty("ready")!);
        Assert.AreEqual(new Color(0xFF, 0, 0, 0x80), app.GetProperty("tint"));
        Assert.AreEqual(new EnumValue("Mode", "idle"), app.GetProperty("mode"));
        CollectionAssert.AreEqual(new object[] { 4.0, 5.0 }, ((Model<object?>)app.GetProperty("numbers")!).ToArray());
        Assert.AreEqual("Grace", ((Struct)app.GetProperty("person")!)["name"]);
    });

    [TestMethod]
    public void LoadsImages() => SlintThread.Run(() =>
    {
        var path = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "../../../../../../examples/memory/icons/at.png"));
        using var app = CreateApp();
        using var image = Image.Load(path);
        Assert.AreEqual(path, image.Path);
        Assert.IsGreaterThan(0, image.Width);
        app.SetProperty("picture", image);
        using var again = (Image)app.GetProperty("picture")!;
        Assert.AreEqual(path, again.Path);
        Assert.ThrowsExactly<SlintException>(() => Image.Load(path + ".missing"));
    });

    [TestMethod]
    public void ReportsInvalidAccess() => SlintThread.Run(() =>
    {
        using var app = CreateApp();
        Assert.ThrowsExactly<SlintException>(() => app.GetProperty("missing"));
        Assert.ThrowsExactly<SlintException>(() => app.SetProperty("count", "not a number"));
        Assert.ThrowsExactly<SlintException>(() => app.Invoke("missing"));
        Assert.ThrowsExactly<SlintException>(() => app.SetCallback("missing", _ => null));
        Assert.ThrowsExactly<ArgumentException>(() => app.SetProperty("count", new object()));
    });

    [TestMethod]
    public void InvokesFunctions() => SlintThread.Run(() =>
    {
        using var app = CreateApp();
        Assert.AreEqual(42.0, app.Invoke("twice", 21));
    });

    [TestMethod]
    public void CallsDotnetCallbacks() => SlintThread.Run(() =>
    {
        using var app = CreateApp();
        object?[]? received = null;
        app.SetCallback("compute", args =>
        {
            received = args;
            return $"{args[1]}{args[0]}";
        });
        var notified = 0;
        app.SetCallback("notify", _ => { notified++; });

        Assert.AreEqual("x7", app.Invoke("call-compute"));
        CollectionAssert.AreEqual(new object[] { 7.0, "x" }, received);
        Assert.IsNull(app.Invoke("call-notify"));
        Assert.AreEqual(1, notified);
    });

    [TestMethod]
    public void ReplacingACallbackReleasesTheOldHandler() => SlintThread.Run(() =>
    {
        using var app = CreateApp();
        var weak = SetCallbackAndReturnWeak(app);
        app.SetCallback("notify", _ => { });
        GC.Collect();
        GC.WaitForPendingFinalizers();
        Assert.IsFalse(weak.IsAlive);
    });

    private static WeakReference SetCallbackAndReturnWeak(ComponentInstance app)
    {
        var calls = new List<int>();
        Action<object?[]> handler = _ => calls.Add(1);
        app.SetCallback("notify", handler);
        return new WeakReference(handler);
    }

    [TestMethod]
    public void PropagatesCallbackExceptions() => SlintThread.Run(() =>
    {
        using var app = CreateApp();
        app.SetCallback("compute", _ => throw new InvalidOperationException("boom"));
        var e = Assert.ThrowsExactly<InvalidOperationException>(() => app.Invoke("call-compute"));
        Assert.AreEqual("boom", e.Message);

        app.SetCallback("compute", _ => "ok");
        Assert.AreEqual("ok", app.Invoke("call-compute"));
    });

    [TestMethod]
    public void AccessesGlobals() => SlintThread.Run(() =>
    {
        using var app = CreateApp();
        Assert.AreEqual("dark", app.GetGlobalProperty("Settings", "theme"));
        app.SetGlobalProperty("Settings", "theme", "light");
        Assert.AreEqual("theme: light", app.InvokeGlobal("Settings", "describe"));

        app.SetGlobalCallback("Settings", "changed", args => $"now {args[0]}");
        Assert.AreEqual("now light", app.Invoke("call-global"));
        Assert.ThrowsExactly<SlintException>(() => app.GetGlobalProperty("Missing", "theme"));
    });

    [TestMethod]
    public void BrushesAndGradients() => SlintThread.Run(() =>
    {
        using var app = CreateApp();
        using var gradient = (Brush)app.GetProperty("fill")!;
        Assert.IsTrue(gradient.IsOpaque);
        Assert.IsFalse(gradient.IsTransparent);
        using var faded = gradient.WithAlpha(0.5f);
        Assert.IsFalse(faded.IsOpaque);
        Assert.AreNotEqual(gradient, faded);

        app.SetProperty("fill", faded);
        using var back = (Brush)app.GetProperty("fill")!;
        Assert.AreEqual(faded, back);

        app.SetProperty("fill", new Brush(new Color(0, 0x80, 0)));
        Assert.AreEqual(new Color(0, 0x80, 0), app.GetProperty("fill"));
    });

    [TestMethod]
    public void ColorHelpersMatchBrushes() => SlintThread.Run(() =>
    {
        var color = new Color(0x40, 0x80, 0xC0);
        using var brush = new Brush(color);
        Assert.AreEqual(color.Brighter(0.5f), brush.Brighter(0.5f).Color);
        Assert.AreEqual(color.Darker(0.5f), brush.Darker(0.5f).Color);
        Assert.AreEqual(color.Transparentize(0.5f), brush.Transparentize(0.5f).Color);
        Assert.AreEqual(color.WithAlpha(0.25f), brush.WithAlpha(0.25f).Color);
        Assert.AreEqual(64, color.WithAlpha(0.25f).Alpha);
        Assert.IsGreaterThan(color.Red, color.Brighter(0.5f).Red);
        Assert.IsLessThan(color.Red, color.Darker(0.5f).Red);
        Assert.AreEqual(color, color.Mix(new Color(0, 0, 0), 1f));
    });

    [TestMethod]
    public void PointsConvertToLogicalPositions() => SlintThread.Run(() =>
    {
        using var app = CreateApp();
        app.SetProperty("point", new LogicalPosition(3, 4));
        var point = (Struct)app.GetProperty("point")!;
        Assert.AreEqual(3.0, point["x"]);
        Assert.AreEqual(new LogicalPosition(3, 4), global::Slint.Generated.Conversion.FromSlint<LogicalPosition>(point));
    });
}
