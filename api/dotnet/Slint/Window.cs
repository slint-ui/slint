// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using System.Runtime.InteropServices;
using Slint.Interop;

namespace Slint;

/// <summary>A position in logical pixels, which the scale factor maps to physical pixels.</summary>
public readonly record struct LogicalPosition(float X, float Y);

/// <summary>A position in physical pixels on the screen.</summary>
public readonly record struct PhysicalPosition(int X, int Y);

/// <summary>A size in logical pixels, which the scale factor maps to physical pixels.</summary>
public readonly record struct LogicalSize(float Width, float Height);

/// <summary>A size in physical pixels on the screen.</summary>
public readonly record struct PhysicalSize(uint Width, uint Height);

/// <summary>What happens to a window after a handler of <see cref="Window.OnCloseRequested"/> runs.</summary>
public enum CloseRequestResponse
{
    /// <summary>The window hides, the default.</summary>
    HideWindow = 0,
    /// <summary>The window stays shown.</summary>
    KeepWindowShown = 1,
}

/// <summary>The window of a <see cref="ComponentInstance"/>; use it from the Slint thread only.</summary>
public sealed unsafe class Window
{
    private readonly ComponentInstanceHandle _instance;

    internal Window(ComponentInstanceHandle instance)
    {
        _instance = instance;
    }

    /// <summary>Shows the window.</summary>
    public void Show() => NativeError.Check(NativeMethods.slint_dotnet_component_instance_show(_instance));

    /// <summary>Hides the window.</summary>
    public void Hide() => NativeError.Check(NativeMethods.slint_dotnet_component_instance_hide(_instance));

    /// <summary>Whether the window is shown.</summary>
    public bool IsVisible => Checked(NativeMethods.slint_dotnet_window_is_visible(_instance));

    /// <summary>The number of physical pixels per logical pixel.</summary>
    public float ScaleFactor => Checked(NativeMethods.slint_dotnet_window_scale_factor(_instance));

    /// <summary>Asks the window to render again, such as after a change that Slint doesn't track.</summary>
    public void RequestRedraw() => Checked(NativeMethods.slint_dotnet_window_request_redraw);

    /// <summary>The position of the window on the screen, in physical pixels.</summary>
    public PhysicalPosition Position
    {
        get
        {
            int x = 0, y = 0;
            NativeMethods.slint_dotnet_window_position(_instance, &x, &y);
            return Checked(new PhysicalPosition(x, y));
        }
        set => Checked(h => NativeMethods.slint_dotnet_window_set_physical_position(h, value.X, value.Y));
    }

    /// <summary>The position of the window on the screen, in logical pixels.</summary>
    public LogicalPosition LogicalPosition
    {
        get
        {
            var position = Position;
            var scale = ScaleFactor;
            return new LogicalPosition(position.X / scale, position.Y / scale);
        }
        set => Checked(h => NativeMethods.slint_dotnet_window_set_logical_position(h, value.X, value.Y));
    }

    /// <summary>The size of the window, without its frame, in physical pixels.</summary>
    public PhysicalSize Size
    {
        get
        {
            uint width = 0, height = 0;
            NativeMethods.slint_dotnet_window_size(_instance, &width, &height);
            return Checked(new PhysicalSize(width, height));
        }
        set => Checked(h => NativeMethods.slint_dotnet_window_set_physical_size(h, value.Width, value.Height));
    }

    /// <summary>The size of the window, without its frame, in logical pixels.</summary>
    public LogicalSize LogicalSize
    {
        get
        {
            var size = Size;
            var scale = ScaleFactor;
            return new LogicalSize(size.Width / scale, size.Height / scale);
        }
        set => Checked(h => NativeMethods.slint_dotnet_window_set_logical_size(h, value.Width, value.Height));
    }

    /// <summary>Whether the window covers the whole screen.</summary>
    public bool IsFullscreen
    {
        get => Checked(NativeMethods.slint_dotnet_window_is_fullscreen(_instance));
        set => Checked(h => NativeMethods.slint_dotnet_window_set_fullscreen(h, value));
    }

    /// <summary>Whether the window is maximized.</summary>
    public bool IsMaximized
    {
        get => Checked(NativeMethods.slint_dotnet_window_is_maximized(_instance));
        set => Checked(h => NativeMethods.slint_dotnet_window_set_maximized(h, value));
    }

    /// <summary>Whether the window is minimized.</summary>
    public bool IsMinimized
    {
        get => Checked(NativeMethods.slint_dotnet_window_is_minimized(_instance));
        set => Checked(h => NativeMethods.slint_dotnet_window_set_minimized(h, value));
    }

    /// <summary>
    /// Sets the handler that runs when the user asks to close the window, such as with its close button.
    /// Return <see cref="CloseRequestResponse.KeepWindowShown"/> to keep it, such as to ask for unsaved changes first.
    /// </summary>
    public void OnCloseRequested(Func<CloseRequestResponse> handler)
    {
        // The native side owns this handle from here.
        var userData = GCHandle.ToIntPtr(GCHandle.Alloc(handler));
        Checked(h => NativeMethods.slint_dotnet_window_on_close_requested(h, &CloseTrampoline, userData, &ActionTrampoline.Release));
    }

    /// <summary>Sends <paramref name="windowEvent"/> to the window, as if it came from the windowing system.</summary>
    public WindowEventDispatchResult DispatchEvent(WindowEvent windowEvent)
    {
        ArgumentNullException.ThrowIfNull(windowEvent);
        // The arguments of slint_dotnet_window_dispatch_event in api/dotnet/native/window.rs.
        (int Kind, LogicalPosition Xy, float A, float B, PointerEventButton Button, bool Active, string Text) args = windowEvent switch
        {
            WindowEvent.PointerPressed e => (0, e.Position, 0, 0, e.Button, false, ""),
            WindowEvent.PointerReleased e => (1, e.Position, 0, 0, e.Button, false, ""),
            WindowEvent.PointerMoved e => (2, e.Position, 0, 0, default, false, ""),
            WindowEvent.PointerScrolled e => (3, e.Position, e.DeltaX, e.DeltaY, default, false, ""),
            WindowEvent.PointerExited => (4, default, 0, 0, default, false, ""),
            WindowEvent.KeyPressed e => (5, default, 0, 0, default, false, e.Text),
            WindowEvent.KeyPressRepeated e => (6, default, 0, 0, default, false, e.Text),
            WindowEvent.KeyReleased e => (7, default, 0, 0, default, false, e.Text),
            WindowEvent.ScaleFactorChanged e => (8, default, e.ScaleFactor, 0, default, false, ""),
            WindowEvent.Resized e => (9, new LogicalPosition(e.Size.Width, e.Size.Height), 0, 0, default, false, ""),
            WindowEvent.CloseRequested => (10, default, 0, 0, default, false, ""),
            WindowEvent.WindowActiveChanged e => (11, default, 0, 0, default, e.Active, ""),
            _ => throw new ArgumentException($"Unknown window event {windowEvent}", nameof(windowEvent)),
        };
        using var text = new PinnedUtf8(args.Text);
        var result = NativeMethods.slint_dotnet_window_dispatch_event(
            _instance, args.Kind, args.Xy.X, args.Xy.Y, args.A, args.B, (int)args.Button, args.Active, text.Str);
        NativeError.Check(result >= 0);
        return (WindowEventDispatchResult)result;
    }

    /// <summary>Sends the window a close request, like the window manager does; for tests.</summary>
    internal void DispatchCloseRequested() => Checked(NativeMethods.slint_dotnet_window_dispatch_close_requested);

    [UnmanagedCallersOnly]
    private static int CloseTrampoline(nint userData)
    {
        try
        {
            return (int)((Func<CloseRequestResponse>)GCHandle.FromIntPtr(userData).Target!)();
        }
        catch (Exception e)
        {
            NativeError.SetPending(e);
            return (int)CloseRequestResponse.HideWindow;
        }
    }

    /// <summary>Throws for a native error; these exports return a default value then, such as on another thread.</summary>
    private static T Checked<T>(T value)
    {
        NativeError.ThrowPending();
        if (NativeError.TakeMessage() is { } message)
        {
            throw new SlintException(message);
        }
        return value;
    }

    private void Checked(Action<ComponentInstanceHandle> call)
    {
        call(_instance);
        Checked(0);
    }
}
