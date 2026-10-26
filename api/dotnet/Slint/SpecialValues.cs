// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using System.Runtime.InteropServices;
using Slint.Interop;

namespace Slint;

/// <summary>A Slint value that lives in the native library, compared by value.</summary>
public abstract class NativeValue : IEquatable<NativeValue>, IDisposable
{
    private protected NativeValue(ValueHandle handle)
    {
        Handle = handle;
    }

    internal ValueHandle Handle { get; }

    /// <inheritdoc/>
    public bool Equals(NativeValue? other) =>
        other is not null && other.GetType() == GetType() && Checked(NativeMethods.slint_dotnet_value_equals(Handle, other.Handle));

    /// <inheritdoc/>
    public override bool Equals(object? obj) => obj is NativeValue other && Equals(other);

    /// <inheritdoc/>
    public override int GetHashCode() => GetType().GetHashCode();

    /// <summary>Releases the native value.</summary>
    public void Dispose() => Handle.Dispose();

    private protected static ValueHandle New(nint value) => new(NativeError.CheckValue(value));

    private protected static T Checked<T>(T value)
    {
        NativeError.ThrowPending();
        if (NativeError.TakeMessage() is { } message)
        {
            throw new SlintException(message);
        }
        return value;
    }
}

/// <summary>A <c>keys</c> value: a keyboard shortcut, such as <c>Control+Z</c>.</summary>
public sealed unsafe class Keys : NativeValue
{
    internal Keys(ValueHandle handle)
        : base(handle)
    {
    }

    /// <summary>Creates a shortcut from modifiers and a key, such as <c>"Control"</c>, <c>"Shift?"</c>, and <c>"Z"</c>.</summary>
    /// <exception cref="SlintException">A part isn't a modifier or a key.</exception>
    public static Keys FromParts(params string[] parts)
    {
        using var pinned = new PinnedUtf8Array(parts);
        fixed (Utf8Str* ptr = pinned.Strs)
        {
            return new Keys(New(NativeMethods.slint_dotnet_value_new_keys(ptr, (nuint)parts.Length)));
        }
    }

    /// <inheritdoc/>
    public override string ToString() => NativeMethods.slint_dotnet_value_describe(Handle).Take() ?? "";
}

/// <summary>A <c>styled-text</c> value: text with formatting, such as from Markdown.</summary>
public sealed class StyledText : NativeValue
{
    /// <summary>Creates empty styled text.</summary>
    public StyledText()
        : this(New(NativeMethods.slint_dotnet_value_new_styled_text(0, default)))
    {
    }

    internal StyledText(ValueHandle handle)
        : base(handle)
    {
    }

    /// <summary>Creates styled text without formatting.</summary>
    public static StyledText FromPlainText(string text) => Create(1, text);

    /// <summary>Parses Markdown.</summary>
    /// <exception cref="SlintException">The Markdown has syntax that styled text doesn't support.</exception>
    public static StyledText FromMarkdown(string markdown) => Create(2, markdown);

    /// <inheritdoc/>
    public override string ToString() => NativeMethods.slint_dotnet_value_describe(Handle).Take() ?? "";

    private static StyledText Create(int kind, string text)
    {
        using var textStr = new PinnedUtf8(text);
        return new StyledText(New(NativeMethods.slint_dotnet_value_new_styled_text(kind, textStr.Str)));
    }
}

/// <summary>A <c>data-transfer</c> value: what drag and drop or the clipboard carries.</summary>
public sealed class DataTransfer : NativeValue
{
    /// <summary>Creates an empty data transfer.</summary>
    public DataTransfer()
        : this(New(NativeMethods.slint_dotnet_value_new_data_transfer()))
    {
    }

    internal DataTransfer(ValueHandle handle)
        : base(handle)
    {
    }

    /// <summary>The plain text, or <c>null</c> if there's none; setting <c>null</c> or <c>""</c> clears it.</summary>
    public string? PlainText
    {
        get => Checked(NativeMethods.slint_dotnet_data_transfer_plain_text(Handle).Take());
        set
        {
            using var text = new PinnedUtf8(value ?? "");
            NativeError.Check(NativeMethods.slint_dotnet_data_transfer_set_plain_text(Handle, text.Str));
        }
    }

    /// <summary>The image, or <c>null</c> if there's none; setting <c>null</c> clears it.</summary>
    public Image? Image
    {
        get
        {
            var image = Checked(NativeMethods.slint_dotnet_data_transfer_image(Handle));
            return image == 0 ? null : new Image(new ValueHandle(image));
        }
        set => NativeError.Check(NativeMethods.slint_dotnet_data_transfer_set_image(
            Handle, value is null ? 0 : value.Handle.DangerousGetHandle()));
    }

    /// <summary>
    /// An object of the application, such as the item that a drag started on.
    /// It stays in this process, so a drop in another application doesn't see it.
    /// </summary>
    /// <exception cref="ArgumentNullException">Setting <c>null</c>; Slint can't clear the user data.</exception>
    public unsafe object? UserData
    {
        get
        {
            var handle = Checked(NativeMethods.slint_dotnet_data_transfer_user_data(Handle));
            return handle == 0 ? null : GCHandle.FromIntPtr(handle).Target;
        }
        set
        {
            ArgumentNullException.ThrowIfNull(value);
            NativeError.Check(NativeMethods.slint_dotnet_data_transfer_set_user_data(
                Handle, GCHandle.ToIntPtr(GCHandle.Alloc(value)), &ActionTrampoline.Release));
        }
    }

    /// <summary>Whether it has plain text.</summary>
    public bool HasPlainText => (State & 1) != 0;

    /// <summary>Whether it has an image.</summary>
    public bool HasImage => (State & 2) != 0;

    /// <summary>Whether it carries nothing.</summary>
    public bool IsEmpty => (State & 4) != 0;

    private int State => Checked(NativeMethods.slint_dotnet_data_transfer_state(Handle));
}
