// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using Slint.Interop;

namespace Slint;

/// <summary>
/// Simulates input and time for the test cases of <c>tests/cases</c>.
/// Needs a native library with the <c>testing</c> feature.
/// </summary>
internal static unsafe class Testing
{
    /// <summary>Uses the testing backend with mocked time; call it before anything else uses Slint.</summary>
    public static void Init() => NativeError.Check(NativeMethods.slint_dotnet_testing_init());

    /// <summary>Advances the mocked time, which fires timers and runs animations.</summary>
    public static void MockElapsedTime(ulong milliseconds) =>
        NativeError.Check(NativeMethods.slint_dotnet_testing_mock_elapsed_time(milliseconds));

    /// <summary>The mocked time in milliseconds.</summary>
    public static ulong MockedTime => NativeMethods.slint_dotnet_testing_get_mocked_time();

    /// <summary>Clicks at the logical position (<paramref name="x"/>, <paramref name="y"/>) of the window.</summary>
    public static void SendMouseClick(ComponentInstance instance, float x, float y) =>
        NativeError.Check(NativeMethods.slint_dotnet_testing_send_mouse_click(instance.Handle, x, y));

    /// <summary>Types <paramref name="sequence"/>, pressing Shift for uppercase letters.</summary>
    public static void SendKeyboardStringSequence(ComponentInstance instance, string sequence)
    {
        using var sequenceStr = new PinnedUtf8(sequence);
        NativeError.Check(NativeMethods.slint_dotnet_testing_send_keyboard_string_sequence(instance.Handle, sequenceStr.Str));
    }

    /// <summary>Presses <paramref name="keys"/> in order, then releases them in reverse order.</summary>
    /// <remarks>A key is its text, such as <c>"\u0011"</c> for Control, see <c>internal/common/key_codes.rs</c>.</remarks>
    public static void SendKeyCombo(ComponentInstance instance, params string[] keys)
    {
        using var pinned = new PinnedUtf8Array(keys);
        fixed (Utf8Str* ptr = pinned.Strs)
        {
            NativeError.Check(NativeMethods.slint_dotnet_testing_send_key_combo(instance.Handle, ptr, (nuint)keys.Length));
        }
    }
}
