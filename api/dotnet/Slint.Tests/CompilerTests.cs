// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

namespace Slint.Tests;

[TestClass]
public sealed class CompilerTests
{
    private const string Source = """
        export enum Mode { idle, busy }
        export struct Person { name: string, age: int }
        export global Settings {
            in-out property <string> theme: "dark";
            callback changed(string);
            public function describe() -> string { return "theme: " + self.theme; }
        }
        export component App inherits Window {
            in property <int> count: 42;
            out property <bool> ready: true;
            in-out property <Person> person;
            in-out property <[string]> names;
            callback clicked(int) -> int;
            public function twice(x: int) -> int { return x * 2; }
        }
        """;

    [TestMethod]
    public void BuildFromSourceReportsComponentsAndMembers() => SlintThread.Run(() =>
    {
        using var compiler = new Compiler();
        using var result = compiler.BuildFromSource(Source, "app.slint");

        Assert.IsFalse(result.HasErrors, string.Join("\n", result.Diagnostics));
        CollectionAssert.AreEqual(new[] { "App" }, result.ComponentNames.ToArray());
        Assert.IsNull(result.Component("Missing"));

        using var app = result.Component("App")!;
        Assert.AreEqual("App", app.Name);
        Assert.AreEqual(ValueType.Number, app.Properties["count"]);
        Assert.AreEqual(ValueType.Bool, app.Properties["ready"]);
        Assert.AreEqual(ValueType.Struct, app.Properties["person"]);
        Assert.AreEqual(ValueType.Model, app.Properties["names"]);
        CollectionAssert.AreEqual(new[] { "clicked" }, app.Callbacks.ToArray());
        CollectionAssert.AreEqual(new[] { "twice" }, app.Functions.ToArray());
        CollectionAssert.AreEqual(new[] { "Settings" }, app.Globals.ToArray());
        Assert.AreEqual(ValueType.String, app.GlobalProperties("Settings")!["theme"]);
        CollectionAssert.AreEqual(new[] { "changed" }, app.GlobalCallbacks("Settings")!.ToArray());
        CollectionAssert.AreEqual(new[] { "describe" }, app.GlobalFunctions("Settings")!.ToArray());
        Assert.IsNull(app.GlobalProperties("Missing"));
    });

    [TestMethod]
    public void DiagnosticsHaveLocations() => SlintThread.Run(() =>
    {
        using var compiler = new Compiler();
        using var result = compiler.BuildFromSource(
            "export component App {\n    property <int> x: \"text\";\n}\n", "broken.slint");

        Assert.IsTrue(result.HasErrors);
        var error = result.Diagnostics.First(d => d.Level == DiagnosticLevel.Error);
        Assert.AreEqual("broken.slint", Path.GetFileName(error.SourceFile));
        Assert.AreEqual(2, error.Line);
        Assert.IsGreaterThan(0, error.Column);
        Assert.Contains("broken.slint:2:", error.ToString());
    });

    [TestMethod]
    public void BuildFromPathResolvesIncludePaths() => SlintThread.Run(() =>
    {
        var dir = Directory.CreateTempSubdirectory("slint-dotnet");
        try
        {
            var lib = Directory.CreateDirectory(Path.Combine(dir.FullName, "lib"));
            File.WriteAllText(Path.Combine(lib.FullName, "label.slint"), "export component Label { in property <string> text; }");
            var main = Path.Combine(dir.FullName, "main.slint");
            File.WriteAllText(main, "import { Label } from \"label.slint\";\nexport component Main { Label { text: \"hi\"; } }");

            using var compiler = new Compiler();
            using (var failed = compiler.BuildFromPath(main))
            {
                Assert.IsTrue(failed.HasErrors);
            }

            compiler.IncludePaths = [lib.FullName];
            CollectionAssert.AreEqual(new[] { lib.FullName }, compiler.IncludePaths.ToArray());
            using var result = compiler.BuildFromPath(main);
            Assert.IsFalse(result.HasErrors, string.Join("\n", result.Diagnostics));
            CollectionAssert.AreEqual(new[] { "Main" }, result.ComponentNames.ToArray());
        }
        finally
        {
            dir.Delete(recursive: true);
        }
    });

    [TestMethod]
    public void StyleIsRemembered() => SlintThread.Run(() =>
    {
        using var compiler = new Compiler();
        Assert.IsNull(compiler.Style);
        compiler.Style = "fluent";
        Assert.AreEqual("fluent", compiler.Style);
        using var result = compiler.BuildFromSource(
            "import { Button } from \"std-widgets.slint\";\nexport component App { Button { } }", "app.slint");
        Assert.IsFalse(result.HasErrors, string.Join("\n", result.Diagnostics));
    });

    [TestMethod]
    public void UsingSlintFromAnotherThreadFails()
    {
        SlintThread.Run(() => new Compiler().Dispose());
        Assert.ThrowsExactly<SlintException>(() => new Compiler());
    }
}
