// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

namespace Slint.Tests;

[TestClass]
public sealed class ModelAdapterTests
{
    private const string Source = """
        export component App {
            in-out property <[string]> names;
            out property <int> name-count: names.length;
            out property <string> first-name: names.length > 0 ? names[0] : "";
        }
        """;

    [TestMethod]
    public void FilterFollowsTheSource() => SlintThread.Run(() =>
    {
        var numbers = new ListModel<int>([1, 2, 3, 4, 5, 6]);
        var even = numbers.Filter(n => n % 2 == 0);
        CollectionAssert.AreEqual(new[] { 2, 4, 6 }, even.ToArray());

        numbers.Add(8);
        numbers[0] = 10;
        CollectionAssert.AreEqual(new[] { 10, 2, 4, 6, 8 }, even.ToArray());

        even.SetRowData(0, 12);
        Assert.AreEqual(12, numbers[0]);
    });

    [TestMethod]
    public void SortIsStableAndFollowsTheSource() => SlintThread.Run(() =>
    {
        var people = new ListModel<(string Name, int Age)>([("Ada", 36), ("Grace", 85), ("Alan", 36)]);
        var byAge = people.Sort((a, b) => a.Age.CompareTo(b.Age));
        CollectionAssert.AreEqual(new[] { "Ada", "Alan", "Grace" }, byAge.Select(p => p.Name).ToArray());

        people.Add(("Linus", 20));
        CollectionAssert.AreEqual(new[] { "Linus", "Ada", "Alan", "Grace" }, byAge.Select(p => p.Name).ToArray());
    });

    [TestMethod]
    public void MapForwardsEachChange() => SlintThread.Run(() =>
    {
        var names = new ListModel<string>(["Ada", "Grace"]);
        var lengths = names.Map(name => name.Length);
        CollectionAssert.AreEqual(new[] { 3, 5 }, lengths.ToArray());
        names.Insert(1, "Linus");
        names.RemoveAt(0);
        CollectionAssert.AreEqual(new[] { 5, 5 }, lengths.ToArray());
    });

    [TestMethod]
    public void ReverseMapsIndicesBothWays() => SlintThread.Run(() =>
    {
        var numbers = new ListModel<int>([1, 2, 3]);
        var reversed = numbers.Reverse();
        CollectionAssert.AreEqual(new[] { 3, 2, 1 }, reversed.ToArray());

        reversed.SetRowData(0, 30);
        Assert.AreEqual(30, numbers[2]);
        numbers.RemoveAt(0);
        numbers.Add(4);
        CollectionAssert.AreEqual(new[] { 4, 30, 2 }, reversed.ToArray());
    });

    [TestMethod]
    public void SlintSeesTheChangesOfAdapters() => SlintThread.Run(() =>
    {
        using var app = ModelTests.Create(Source);
        var names = new ListModel<string>(["Ada", "Bob", "Grace"]);
        app.SetProperty("names", names.Filter(name => name.Length > 3).Map(name => name.ToUpperInvariant()));
        Assert.AreEqual(1.0, app.GetProperty("name-count"));
        Assert.AreEqual("GRACE", app.GetProperty("first-name"));

        names.Insert(0, "Linus");
        Assert.AreEqual(2.0, app.GetProperty("name-count"));
        Assert.AreEqual("LINUS", app.GetProperty("first-name"));

        app.SetProperty("names", names.Reverse());
        names.Add("Zoe");
        Assert.AreEqual("Zoe", app.GetProperty("first-name"));
    });
}
