// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

namespace Slint.Tests;

[TestClass]
public sealed class TranslationTests
{
    /// <summary>The source directory of translations/up/LC_MESSAGES/greeting.po.</summary>
    private static readonly string s_translations =
        Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "../../../translations"));

    private static ComponentInstance CreateApp(string source, bool bundle = true)
    {
        using var compiler = new Compiler { TranslationDomain = "greeting" };
        if (bundle)
        {
            compiler.BundledTranslationsPath = s_translations;
        }
        using var result = compiler.BuildFromSource(source, "app.slint");
        Assert.IsFalse(result.HasErrors, string.Join("\n", result.Diagnostics));
        using var definition = result.Component("App")!;
        return definition.Create();
    }

    /// <summary>Runs <paramref name="body"/>, then restores the global language and translator.</summary>
    private static void WithTranslations(Action body) => SlintThread.Run(() =>
    {
        try
        {
            body();
        }
        finally
        {
            Translations.SetTranslator(null);
            Translations.SelectBundledTranslation("");
        }
    });

    [TestMethod]
    public void SelectsBundledTranslations() => WithTranslations(() =>
    {
        using var app = CreateApp("export component App { out property <string> text: @tr(\"Hello\"); }");
        Assert.AreEqual("Hello", app.GetProperty("text"));

        Translations.SelectBundledTranslation("up");
        Assert.AreEqual("HELLO", app.GetProperty("text"));

        Assert.ThrowsExactly<SlintException>(() => Translations.SelectBundledTranslation("xx"));
        Translations.SelectBundledTranslation("");
        Assert.AreEqual("Hello", app.GetProperty("text"));
    });

    [TestMethod]
    public void GeneratedCodeBundlesTranslations() => WithTranslations(() =>
    {
        using var app = Translated.Create();
        Assert.AreEqual("Hello", app.Text);
        Translations.SelectBundledTranslation("up");
        Assert.AreEqual("HELLO", app.Text);
    });

    private sealed class Translator : ITranslator
    {
        public string? Translate(string text, string? context) =>
            text == "Keep" ? null : $"{text.ToUpperInvariant()} ({context})";

        public string? Translate(string singular, string plural, ulong n, string? context) =>
            n == 1 ? "one file" : "{n} files!";
    }

    [TestMethod]
    public void UsesTheTranslatorAndUpdatesTheStrings() => WithTranslations(() =>
    {
        using var app = CreateApp("""
            export component App {
                in property <int> count: 3;
                out property <string> text: @tr("Hello");
                out property <string> kept: @tr("Keep");
                out property <string> files: @tr("{n} file" | "{n} files" % count);
            }
            """, bundle: false);
        Assert.AreEqual("Hello", app.GetProperty("text"));

        Translations.SetTranslator(new Translator());
        Assert.AreEqual("HELLO (App)", app.GetProperty("text"));
        Assert.AreEqual("Keep", app.GetProperty("kept"));
        Assert.AreEqual("3 files!", app.GetProperty("files"));
        app.SetProperty("count", 1);
        Assert.AreEqual("one file", app.GetProperty("files"));

        Translations.SetTranslator(null);
        Assert.AreEqual("Hello", app.GetProperty("text"));
    });
}
