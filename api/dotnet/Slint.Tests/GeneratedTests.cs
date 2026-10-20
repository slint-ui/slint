// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

namespace Slint.Tests;

/// <summary>Tests the code that <c>slint-compiler -f csharp</c> generates from <c>ui/generated.slint</c>.</summary>
[TestClass]
public sealed class GeneratedTests
{
    [TestMethod]
    public void ReadsTypedProperties() => SlintThread.Run(() =>
    {
        using var app = Generated.Create();
        Assert.AreEqual(42, app.Count);
        Assert.IsTrue(app.Ready);
        Assert.AreEqual(0.5f, app.Ratio);
        Assert.AreEqual(TimeSpan.FromMilliseconds(250), app.Delay);
        Assert.AreEqual(10f, app.Size);
        Assert.AreEqual(new Color(0x10, 0x20, 0x30), app.Tint);
        Assert.AreEqual(Mode.BusyNow, app.Mode);
        Assert.AreEqual("Ada", app.Person.Name);
        Assert.AreEqual(36, app.Person.Age);
        CollectionAssert.AreEqual(new[] { "math" }, app.Person.Tags.ToArray());
        CollectionAssert.AreEqual(new[] { 1, 2, 3 }, app.Numbers.ToArray());
    });

    [TestMethod]
    public void WritesTypedProperties() => SlintThread.Run(() =>
    {
        using var app = Generated.Create();
        app.Count = 7;
        app.Delay = TimeSpan.FromSeconds(1);
        app.Mode = Mode.Idle;
        app.Person = new Person { Name = "Grace", Age = 85, Tags = new ListModel<string>(["navy"]) };

        Assert.AreEqual(7, app.Count);
        Assert.AreEqual(TimeSpan.FromSeconds(1), app.Delay);
        Assert.AreEqual(Mode.Idle, app.Mode);
        Assert.AreEqual("Grace", app.Person.Name);
        Assert.AreEqual("navy", app.Person.Tags[0]);
    });

    [TestMethod]
    public void ModelsOfStructs() => SlintThread.Run(() =>
    {
        using var app = Generated.Create();
        var people = new ListModel<Person>([new Person { Name = "Ada", Age = 36 }]);
        app.People = people;
        Assert.AreSame(people, app.People);
        Assert.AreEqual(1, app.PeopleCount);
        people.Insert(0, new Person { Name = "Grace", Age = 85 });
        Assert.AreEqual(2, app.PeopleCount);
        Assert.AreEqual("Grace", app.Oldest());

        app.Numbers.SetRowData(0, 0);
        Assert.AreEqual(0, app.Numbers[0]);
    });

    [TestMethod]
    public void TypedCallbacksAndFunctions() => SlintThread.Run(() =>
    {
        using var app = Generated.Create();
        app.OnClicked((n, s) => $"{s}{n}");
        var notified = 0;
        app.OnNotify(() => notified++);

        Assert.AreEqual("x7", app.CallClicked());
        Assert.AreEqual("y1", app.InvokeClicked(1, "y"));
        app.CallNotify();
        Assert.AreEqual(1, notified);
        Assert.AreEqual(42, app.Twice(21));
    });

    [TestMethod]
    public void TypedGlobals() => SlintThread.Run(() =>
    {
        using var app = Generated.Create();
        Assert.AreEqual("dark", app.Settings.Theme);
        app.Settings.Theme = "light";
        Assert.AreEqual("theme: light", app.Settings.Describe());
        app.Settings.OnChanged(theme => $"now {theme}");
        Assert.AreEqual("now light", app.CallChanged());
    });

    [TestMethod]
    public void ReportsOutOfDateGeneratedCode() => SlintThread.Run(() =>
    {
        using var compiler = new Compiler();
        using var result = compiler.BuildFromSource(
            "export global G { in property <int> x; }\nexport component App { in property <string> name; callback done(); }", "app.slint");
        var expected = new global::Slint.Generated.ExpectedApi(
            new Dictionary<string, ValueType> { ["name"] = ValueType.Number, ["missing"] = ValueType.Bool },
            ["done", "gone"],
            ["helper"]);
        var globals = new Dictionary<string, global::Slint.Generated.ExpectedApi>
        {
            ["G"] = new(new Dictionary<string, ValueType> { ["x"] = ValueType.Number }, [], []),
            ["Removed"] = new(new Dictionary<string, ValueType>(), [], []),
        };

        var e = Assert.ThrowsExactly<SlintException>(() => global::Slint.Generated.GeneratedApi.Create(result, "App.g.cs", "App", expected, globals));
        StringAssert.Contains(e.Message, "App.g.cs is out of date");
        StringAssert.Contains(e.Message, "property 'name' of 'App' is String, not Number");
        StringAssert.Contains(e.Message, "'App' has no property 'missing'");
        StringAssert.Contains(e.Message, "'App' has no callback 'gone'");
        StringAssert.Contains(e.Message, "'App' has no function 'helper'");
        StringAssert.Contains(e.Message, "there's no global 'Removed'");

        Assert.ThrowsExactly<SlintException>(() => global::Slint.Generated.GeneratedApi.Create(result, "App.g.cs", "Other", expected, globals));
    });

    [TestMethod]
    public void ImportsThroughIncludeAndLibraryPaths() => SlintThread.Run(() =>
    {
        using var app = ImportPaths.Create();
        Assert.AreEqual("badge/tag", app.Text);
    });
}
