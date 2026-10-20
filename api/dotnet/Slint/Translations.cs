// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using System.Runtime.InteropServices;
using Slint.Interop;

namespace Slint;

/// <summary>Translates the <c>@tr()</c> strings of <c>.slint</c> files, such as with a gettext library.</summary>
/// <remarks>Slint may call it from any thread, so make it thread safe.</remarks>
public interface ITranslator
{
    /// <summary>Returns the translation of <paramref name="text"/>, or <c>null</c> to keep it.</summary>
    string? Translate(string text, string? context);

    /// <summary>Returns the translation of <paramref name="singular"/> or <paramref name="plural"/> for <paramref name="n"/>, or <c>null</c> to keep it.</summary>
    string? Translate(string singular, string plural, ulong n, string? context);
}

/// <summary>Selects the language of the <c>@tr()</c> strings in <c>.slint</c> files.</summary>
public static unsafe class Translations
{
    /// <summary>
    /// Switches to the bundled translations of <paramref name="language"/>,
    /// the name of a directory in <see cref="Compiler.BundledTranslationsPath"/>.
    /// An empty string or <c>en</c> selects the strings as written.
    /// Call it after creating a component.
    /// </summary>
    /// <exception cref="SlintException">There are no bundled translations for <paramref name="language"/>.</exception>
    public static void SelectBundledTranslation(string language)
    {
        using var languageStr = new PinnedUtf8(language);
        NativeError.Check(NativeMethods.slint_dotnet_select_bundled_translation(languageStr.Str));
    }

    /// <summary>
    /// Translates the <c>@tr()</c> strings of components compiled without bundled translations
    /// with <paramref name="translator"/>; <c>null</c> removes it.
    /// </summary>
    public static void SetTranslator(ITranslator? translator)
    {
        if (translator is null)
        {
            NativeError.Check(NativeMethods.slint_dotnet_set_translator(null, 0, &ActionTrampoline.Release));
            return;
        }
        // The native side owns this handle from here.
        var userData = GCHandle.ToIntPtr(GCHandle.Alloc(translator));
        NativeError.Check(NativeMethods.slint_dotnet_set_translator(&Translate, userData, &ActionTrampoline.Release));
    }

    [UnmanagedCallersOnly]
    private static OwnedString Translate(nint userData, Utf8Str text, Utf8Str plural, ulong n, byte hasPlural, Utf8Str context, byte hasContext)
    {
        try
        {
            var translator = (ITranslator)GCHandle.FromIntPtr(userData).Target!;
            var contextStr = hasContext != 0 ? context.ToString() : null;
            var translated = hasPlural != 0
                ? translator.Translate(text.ToString(), plural.ToString(), n, contextStr)
                : translator.Translate(text.ToString(), contextStr);
            if (translated is null)
            {
                return default;
            }
            using var result = new PinnedUtf8(translated);
            return NativeMethods.slint_dotnet_string_new(result.Str);
        }
        catch (Exception e)
        {
            // Translation runs inside property bindings, where an exception can't stop the work; report it, keep the text.
            Console.Error.WriteLine($"Slint: the translator threw: {e}");
            return default;
        }
    }
}
