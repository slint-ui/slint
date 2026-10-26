// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

namespace Slint.Tests;

[TestClass]
public sealed class ModelTests
{
    private const string Source = """
        export struct Item { label: string, done: bool }
        export component App {
            in-out property <[string]> names;
            in-out property <[int]> numbers;
            in-out property <[Item]> items;
            in-out property <[int]> literal: [10, 20, 30];
            out property <int> name-count: names.length;
            out property <string> first-name: names.length > 0 ? names[0] : "";
            public function rename-first(s: string) { root.names[0] = s; }
            public function bump-first() { root.numbers[0] = root.numbers[0] + 1; }
            public function first-label() -> string { return root.items[0].label; }
        }
        """;

    internal static ComponentInstance Create(string source = Source)
    {
        using var compiler = new Compiler();
        using var result = compiler.BuildFromSource(source, "models.slint");
        Assert.IsFalse(result.HasErrors, string.Join("\n", result.Diagnostics));
        using var definition = result.Component(result.ComponentNames[0])!;
        return definition.Create();
    }

    [TestMethod]
    public void ReturnsTheSameModelObject() => SlintThread.Run(() =>
    {
        using var app = Create();
        var names = new ListModel<string>(["a", "b"]);
        app.SetProperty("names", names);
        Assert.AreSame(names, app.GetProperty("names"));
    });

    [TestMethod]
    public void BindingsFollowListChanges() => SlintThread.Run(() =>
    {
        using var app = Create();
        var names = new ListModel<string>();
        app.SetProperty("names", names);
        Assert.AreEqual(0.0, app.GetProperty("name-count"));

        names.Add("Ada");
        names.AddRange(["Grace", "Linus"]);
        Assert.AreEqual(3.0, app.GetProperty("name-count"));
        Assert.AreEqual("Ada", app.GetProperty("first-name"));

        names[0] = "Barbara";
        Assert.AreEqual("Barbara", app.GetProperty("first-name"));

        names.Insert(0, "Alan");
        names.RemoveAt(1);
        Assert.AreEqual("Alan", app.GetProperty("first-name"));
        Assert.AreEqual(3.0, app.GetProperty("name-count"));

        names.Clear();
        Assert.AreEqual(0.0, app.GetProperty("name-count"));
    });

    [TestMethod]
    public void SlintWritesRowsBack() => SlintThread.Run(() =>
    {
        using var app = Create();
        var names = new ListModel<string>(["a"]);
        var numbers = new ListModel<int>([41]);
        app.SetProperty("names", names);
        app.SetProperty("numbers", numbers);

        app.Invoke("rename-first", "z");
        app.Invoke("bump-first");

        Assert.AreEqual("z", names[0]);
        Assert.AreEqual(42, numbers[0]);
    });

    [TestMethod]
    public void RowsCanBeStructs() => SlintThread.Run(() =>
    {
        using var app = Create();
        app.SetProperty("items", new ListModel<Struct>([new Struct { ["label"] = "write tests", ["done"] = false }]));
        Assert.AreEqual("write tests", app.Invoke("first-label"));
    });

    [TestMethod]
    public void SlintModelsAreReadableAndWritable() => SlintThread.Run(() =>
    {
        using var app = Create();
        var literal = (Model<object?>)app.GetProperty("literal")!;
        Assert.AreEqual(3, literal.Count);
        CollectionAssert.AreEqual(new object[] { 10.0, 20.0, 30.0 }, literal.ToArray());
        Assert.ThrowsExactly<ArgumentOutOfRangeException>(() => literal[3]);

        literal.SetRowData(1, 25);
        Assert.AreEqual(25.0, literal[1]);
    });

    private sealed class Squares(int count) : Model<int>
    {
        private int _count = count;

        public override int RowCount => _count;

        public override int RowData(int row) => row == 2 && _count == 99 ? throw new InvalidOperationException("bad row") : row * row;

        public void Grow()
        {
            _count++;
            NotifyRowAdded(_count - 1, 1);
        }

        public void Break()
        {
            _count = 99;
            NotifyReset();
        }
    }

    [TestMethod]
    public void CustomModelsNotifySlint() => SlintThread.Run(() =>
    {
        using var app = Create("""
            export component App {
                in property <[int]> squares;
                out property <int> last: squares.length > 0 ? squares[squares.length - 1] : -1;
                public function third() -> int { return squares[2]; }
            }
            """);
        var squares = new Squares(2);
        app.SetProperty("squares", squares);
        Assert.AreEqual(1.0, app.GetProperty("last"));
        squares.Grow();
        Assert.AreEqual(4.0, app.GetProperty("last"));

        squares.Break();
        var e = Assert.ThrowsExactly<InvalidOperationException>(() => app.Invoke("third"));
        Assert.AreEqual("bad row", e.Message);
    });

    [TestMethod]
    public void ArraysAndListsConvertToModels() => SlintThread.Run(() =>
    {
        using var app = Create();
        app.SetProperty("numbers", new List<int> { 1, 2 });
        CollectionAssert.AreEqual(new object[] { 1.0, 2.0 }, ((Model<object?>)app.GetProperty("numbers")!).ToArray());
    });

    private sealed class ReadOnlyNumbers : Model<int>
    {
        public override int RowCount => 3;

        public override int RowData(int row) => (row + 1) * 10;
    }

    [TestMethod]
    public void SlintCodeAddsAndRemovesRows() => SlintThread.Run(() =>
    {
        using var app = Create("""
            export component App {
                in-out property <[int]> numbers;
                public function push(value: int) { numbers.push(value); }
                public function insert(row: int, value: int) { numbers.insert(row, value); }
                public function remove(row: int) { numbers.remove(row); }
            }
            """);
        var numbers = new ListModel<int>([1, 2]);
        app.SetProperty("numbers", numbers);
        app.Invoke("push", 3);
        app.Invoke("insert", 0, 0);
        app.Invoke("remove", 2);
        CollectionAssert.AreEqual(new[] { 0, 1, 3 }, numbers.ToArray());

        // Rows out of range, and a model that doesn't support the change, leave the model as it is.
        app.Invoke("remove", 5);
        app.Invoke("insert", 9, 9);
        CollectionAssert.AreEqual(new[] { 0, 1, 3 }, numbers.ToArray());
        var readOnly = new ReadOnlyNumbers();
        app.SetProperty("numbers", readOnly);
        app.Invoke("push", 40);
        app.Invoke("remove", 0);
        CollectionAssert.AreEqual(new[] { 10, 20, 30 }, readOnly.ToArray());
    });
}
