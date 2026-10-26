// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

namespace Slint.Tests;

[TestClass]
public sealed class SpecialValueTests
{
    [TestMethod]
    public void KeysParseAndCompare() => SlintThread.Run(() =>
    {
        using var undo = Keys.FromParts("Control", "Z");
        using var same = Keys.FromParts("Control", "Z");
        using var other = Keys.FromParts("Control", "Y");
        Assert.AreEqual(undo, same);
        Assert.AreNotEqual(undo, other);
        Assert.Contains("Z", undo.ToString());
        Assert.ThrowsExactly<SlintException>(() => Keys.FromParts("NotAKey"));
    });

    [TestMethod]
    public void StyledTextFromPlainTextAndMarkdown() => SlintThread.Run(() =>
    {
        using var plain = StyledText.FromPlainText("hello");
        using var again = StyledText.FromPlainText("hello");
        using var bold = StyledText.FromMarkdown("**hello**");
        using var empty = new StyledText();
        Assert.AreEqual(plain, again);
        Assert.AreNotEqual(plain, bold);
        Assert.AreNotEqual(plain, empty);
    });

    [TestMethod]
    public void DataTransferCarriesTextAndImages() => SlintThread.Run(() =>
    {
        using var data = new DataTransfer();
        Assert.IsTrue(data.IsEmpty);
        Assert.IsNull(data.PlainText);

        data.PlainText = "dragged";
        Assert.IsTrue(data.HasPlainText);
        Assert.IsFalse(data.IsEmpty);
        Assert.AreEqual("dragged", data.PlainText);

        var path = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "../../../../../../examples/memory/icons/at.png"));
        data.Image = Image.Load(path);
        Assert.IsTrue(data.HasImage);
        Assert.AreEqual(path, data.Image!.Path);

        data.PlainText = null;
        data.Image = null;
        Assert.IsTrue(data.IsEmpty);
    });

    [TestMethod]
    public void ValuesRoundTripThroughProperties() => SlintThread.Run(() =>
    {
        using var app = Generated.Create();
        using var shortcut = Keys.FromParts("Control", "S");
        app.Shortcut = shortcut;
        Assert.AreEqual(shortcut, app.Shortcut);
    });

    [TestMethod]
    public void AliasesGetTheirOwnClass() => SlintThread.Run(() =>
    {
        using var app = GeneratedAlias.Create();
        Assert.AreEqual(42, app.Count);
    });

    [TestMethod]
    public void DataTransferCarriesUserData() => SlintThread.Run(() =>
    {
        using var app = ModelTests.Create("export component App { in-out property <data-transfer> data; }");
        var payload = new List<string> { "card" };
        using var data = new DataTransfer();
        Assert.IsNull(data.UserData);
        data.UserData = payload;
        Assert.IsFalse(data.IsEmpty);

        app.SetProperty("data", data);
        using var back = (DataTransfer)app.GetProperty("data")!;
        Assert.AreSame(payload, back.UserData);

        Assert.ThrowsExactly<ArgumentNullException>(() => data.UserData = null);
    });

    [TestMethod]
    public void KeysRoundTripThroughTheirParts() => SlintThread.Run(() =>
    {
        using var keys = Keys.FromParts("Control", "Shift?", "Z");
        var parts = keys.ToParts();
        using var again = Keys.FromParts(parts);
        Assert.AreEqual(keys, again);
        CollectionAssert.Contains(parts, "Control");
    });
}
