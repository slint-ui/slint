// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

namespace Slint.Tests;

[TestClass]
public sealed class WindowTests
{
    private static ComponentInstance CreateApp() =>
        ModelTests.Create("export component App inherits Window { preferred-width: 100px; preferred-height: 50px; }");

    /// <summary>Hiding the last window queues a quit; run the loop once to take it.</summary>
    private static void TakeQueuedQuit()
    {
        using var safety = new Timer();
        safety.Start(TimerMode.SingleShot, TimeSpan.FromSeconds(5), () => Assert.Fail("hiding the last window didn't quit"));
        EventLoop.Run();
    }

    [TestMethod]
    public void ShowAndHideChangeVisibility() => SlintThread.Run(() =>
    {
        using var app = CreateApp();
        Assert.IsFalse(app.Window.IsVisible);
        app.Window.Show();
        Assert.IsTrue(app.Window.IsVisible);
        app.Window.Hide();
        Assert.IsFalse(app.Window.IsVisible);
        TakeQueuedQuit();
    });

    [TestMethod]
    public void SizesRoundTrip() => SlintThread.Run(() =>
    {
        using var app = CreateApp();
        var window = app.Window;
        Assert.AreSame(window, app.Window);
        Assert.IsGreaterThan(0f, window.ScaleFactor);

        window.LogicalSize = new LogicalSize(300, 200);
        Assert.AreEqual(new PhysicalSize(300, 200), window.Size);
        window.Size = new PhysicalSize(400, 250);
        Assert.AreEqual(new LogicalSize(400, 250), window.LogicalSize);

        window.Position = new PhysicalPosition(10, 20);
        window.LogicalPosition = new LogicalPosition(30, 40);
        _ = window.Position;
        window.RequestRedraw();
    });

    [TestMethod]
    public void StatesRoundTrip() => SlintThread.Run(() =>
    {
        using var app = CreateApp();
        var window = app.Window;
        window.IsFullscreen = true;
        Assert.IsTrue(window.IsFullscreen);
        window.IsFullscreen = false;
        window.IsMaximized = true;
        Assert.IsTrue(window.IsMaximized);
        window.IsMinimized = true;
        Assert.IsTrue(window.IsMinimized);
    });

    [TestMethod]
    public void CloseRequestsCanKeepTheWindow() => SlintThread.Run(() =>
    {
        using var app = CreateApp();
        var requests = 0;
        var response = CloseRequestResponse.KeepWindowShown;
        app.Window.OnCloseRequested(() =>
        {
            requests++;
            return response;
        });
        app.Window.Show();

        app.Window.DispatchCloseRequested();
        Assert.AreEqual(1, requests);
        Assert.IsTrue(app.Window.IsVisible);

        response = CloseRequestResponse.HideWindow;
        app.Window.DispatchCloseRequested();
        Assert.AreEqual(2, requests);
        Assert.IsFalse(app.Window.IsVisible);
        TakeQueuedQuit();
    });

    [TestMethod]
    public void CloseHandlerExceptionsPropagate() => SlintThread.Run(() =>
    {
        using var app = CreateApp();
        app.Window.OnCloseRequested(() => throw new InvalidOperationException("close"));
        var e = Assert.ThrowsExactly<InvalidOperationException>(app.Window.DispatchCloseRequested);
        Assert.AreEqual("close", e.Message);
    });

    [TestMethod]
    public void WindowFromAnotherThreadFails()
    {
        var window = SlintThread.Run(() => CreateApp().Window);
        Assert.ThrowsExactly<SlintException>(() => window.IsVisible);
    }

    [TestMethod]
    public void GeneratedComponentsHaveTheirWindow() => SlintThread.Run(() =>
    {
        using var app = Generated.Create();
        Assert.AreSame(app.Instance.Window, app.Window);
        Assert.IsGreaterThan(0f, app.Window.ScaleFactor);
    });

    [TestMethod]
    public void DispatchesKeyEventsToTheFocus() => SlintThread.Run(() =>
    {
        using var app = ModelTests.Create("""
            export component App inherits Window {
                forward-focus: input;
                in-out property <string> text <=> input.text;
                input := TextInput { }
            }
            """);
        var window = app.Window;
        Assert.AreEqual(WindowEventDispatchResult.Accepted, window.DispatchEvent(new WindowEvent.KeyPressed("a")));
        window.DispatchEvent(new WindowEvent.KeyReleased("a"));
        Assert.AreEqual("a", app.GetProperty("text"));
    });

    [TestMethod]
    public void DispatchesPointerAndResizeEvents() => SlintThread.Run(() =>
    {
        using var app = ModelTests.Create("""
            export component App inherits Window {
                width: 100px;
                height: 100px;
                in-out property <int> clicks;
                TouchArea { clicked => { clicks += 1; } }
            }
            """);
        var window = app.Window;
        window.DispatchEvent(new WindowEvent.PointerPressed(new LogicalPosition(50, 50), PointerEventButton.Left));
        window.DispatchEvent(new WindowEvent.PointerReleased(new LogicalPosition(50, 50), PointerEventButton.Left));
        Assert.AreEqual(1.0, app.GetProperty("clicks"));

        Assert.AreEqual(
            WindowEventDispatchResult.Rejected,
            window.DispatchEvent(new WindowEvent.KeyPressed("a")),
            "no element has the focus");
    });
}
