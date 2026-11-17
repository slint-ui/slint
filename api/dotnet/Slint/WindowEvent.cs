// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

namespace Slint;

/// <summary>The button of a pointer event.</summary>
public enum PointerEventButton
{
    /// <summary>A button that is none of the others, such as the task button of a mouse with many buttons.</summary>
    Other,
    /// <summary>The left button.</summary>
    Left,
    /// <summary>The right button.</summary>
    Right,
    /// <summary>The center button.</summary>
    Middle,
    /// <summary>The back button.</summary>
    Back,
    /// <summary>The forward button.</summary>
    Forward,
}

/// <summary>Whether the scene handled an event that <see cref="Window.DispatchEvent"/> sent.</summary>
public enum WindowEventDispatchResult
{
    /// <summary>The scene handled the event, such as a key handler that consumed a key press.</summary>
    Accepted,
    /// <summary>No element handled the event, or a handler refused it.</summary>
    Rejected,
}

/// <summary>
/// User input or a change of the windowing system, to send to a window with <see cref="Window.DispatchEvent"/>,
/// such as for a virtual keyboard or for tests.
/// </summary>
/// <remarks>Positions and sizes are in logical pixels, relative to the top left corner of the window.</remarks>
public abstract record WindowEvent
{
    private protected WindowEvent()
    {
    }

    /// <summary>A pointer button was pressed.</summary>
    public sealed record PointerPressed(LogicalPosition Position, PointerEventButton Button) : WindowEvent;

    /// <summary>A pointer button was released.</summary>
    public sealed record PointerReleased(LogicalPosition Position, PointerEventButton Button) : WindowEvent;

    /// <summary>The pointer moved.</summary>
    public sealed record PointerMoved(LogicalPosition Position) : WindowEvent;

    /// <summary>The wheel was turned, or the user scrolled on a touchpad, by a delta in logical pixels.</summary>
    public sealed record PointerScrolled(LogicalPosition Position, float DeltaX, float DeltaY) : WindowEvent;

    /// <summary>The pointer left the window.</summary>
    public sealed record PointerExited : WindowEvent;

    /// <summary>A key was pressed. <paramref name="Text"/> is the key's text, or a code such as <c>"\u0011"</c> for Control.</summary>
    public sealed record KeyPressed(string Text) : WindowEvent;

    /// <summary>A key is held down and repeats.</summary>
    public sealed record KeyPressRepeated(string Text) : WindowEvent;

    /// <summary>A key was released.</summary>
    public sealed record KeyReleased(string Text) : WindowEvent;

    /// <summary>The number of physical pixels per logical pixel changed, such as on another screen.</summary>
    public sealed record ScaleFactorChanged(float ScaleFactor) : WindowEvent;

    /// <summary>The window was resized, which updates the size of the root element.</summary>
    public sealed record Resized(LogicalSize Size) : WindowEvent;

    /// <summary>The user asked to close the window, which runs the handler of <see cref="Window.OnCloseRequested"/>.</summary>
    public sealed record CloseRequested : WindowEvent;

    /// <summary>The window gained or lost the focus.</summary>
    public sealed record WindowActiveChanged(bool Active) : WindowEvent;
}
