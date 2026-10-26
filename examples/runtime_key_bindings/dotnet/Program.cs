// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using System.Text;
using RuntimeKeyBindings;
using Slint;

internal static class Program
{
    private const string UserAction = "user";
    private static readonly string ConfigPath = Path.Combine(AppContext.BaseDirectory, "user_shortcut.conf");

    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var window = MainWindow.Create();
        if (LoadShortcut(UserAction) is { } loaded)
        {
            Console.WriteLine($"Loaded shortcut from {ConfigPath}: {loaded}");
            window.UserShortcut = loaded;
        }

        window.OnShortcutActivated(action =>
        {
            switch (action)
            {
                case "save":
                    Console.WriteLine("Save");
                    break;
                case "undo":
                    Console.WriteLine("Undo");
                    break;
                case "user":
                    Console.WriteLine($"User shortcut ({window.UserShortcut})");
                    break;
                case "reassign-ctrl-p":
                    var keys = Keys.FromParts("Control", "P");
                    Console.WriteLine($"Reassigned to {keys}");
                    window.UserShortcut = keys;
                    SaveShortcut(UserAction, keys);
                    break;
            }
        });

        // The captured key event becomes the user shortcut.
        window.OnKeyEvent(keyEvent =>
        {
            var modifiers = (Struct)keyEvent["modifiers"]!;
            var parts = new[] { "control", "alt", "shift", "meta" }
                .Where(modifier => modifiers[modifier] is true)
                .Select(modifier => char.ToUpperInvariant(modifier[0]) + modifier[1..])
                .Append((string)keyEvent["text"]!)
                .ToArray();
            try
            {
                var keys = Keys.FromParts(parts);
                Console.WriteLine($"Captured shortcut: {keys}");
                window.UserShortcut = keys;
                SaveShortcut(UserAction, keys);
            }
            catch (SlintException e)
            {
                Console.Error.WriteLine($"Invalid shortcut: {e.Message}");
            }
        });

        Console.WriteLine("Press Ctrl+S, Ctrl+Z, or Ctrl+E (default user shortcut)");
        Console.WriteLine("Click 'Capture shortcut' then press a key combo to reassign");
        Console.WriteLine($"Reassigned shortcut is saved to {ConfigPath} and restored on next launch");
        window.Run();
    }

    /// <summary>Returns the shortcut of <paramref name="action"/> from the config file, if there's a valid one.</summary>
    private static Keys? LoadShortcut(string action)
    {
        if (!File.Exists(ConfigPath))
        {
            return null;
        }
        foreach (var line in File.ReadLines(ConfigPath))
        {
            if (ParseLine(line) is (var name, var parts) && name == action)
            {
                var decoded = parts.Select(Decode).ToArray();
                if (decoded.Any(part => part is null))
                {
                    return null;
                }
                try
                {
                    return Keys.FromParts(decoded!);
                }
                catch (SlintException)
                {
                    return null;
                }
            }
        }
        return null;
    }

    /// <summary>Replaces the line of <paramref name="action"/> in the config file, or adds it.</summary>
    private static void SaveShortcut(string action, Keys keys)
    {
        var newLine = $"{action} {string.Join(' ', keys.ToParts().Select(Encode))}";
        var lines = File.Exists(ConfigPath)
            ? File.ReadAllLines(ConfigPath).ToList()
            :
            [
                "// runtime_key_bindings - user shortcuts.",
                "// One per line: <action> <part> <part> ...",
                "// Parts come from `Keys.ToParts`; non-printable ones are percent-encoded.",
            ];
        var index = lines.FindIndex(line => ParseLine(line)?.Action == action);
        if (index >= 0)
        {
            lines[index] = newLine;
        }
        else
        {
            lines.Add(newLine);
        }
        try
        {
            File.WriteAllLines(ConfigPath, lines);
            Console.WriteLine($"Saved shortcut to {ConfigPath}");
        }
        catch (IOException e)
        {
            Console.Error.WriteLine($"Failed to save shortcut to {ConfigPath}: {e.Message}");
        }
    }

    private static (string Action, string[] Parts)? ParseLine(string line)
    {
        line = line.Trim();
        if (line.Length == 0 || line.StartsWith("//", StringComparison.Ordinal))
        {
            return null;
        }
        var tokens = line.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries);
        return (tokens[0], tokens[1..]);
    }

    /// <summary>Percent-encodes the bytes of a part that aren't printable ASCII, and '%' itself.</summary>
    private static string Encode(string part)
    {
        var encoded = new StringBuilder();
        Span<byte> bytes = stackalloc byte[4];
        foreach (var rune in part.EnumerateRunes())
        {
            if (rune.IsAscii && rune.Value > ' ' && rune.Value < 127 && rune.Value != '%')
            {
                encoded.Append((char)rune.Value);
            }
            else
            {
                foreach (var b in bytes[..rune.EncodeToUtf8(bytes)])
                {
                    encoded.Append($"%{b:X2}");
                }
            }
        }
        return encoded.ToString();
    }

    private static string? Decode(string part)
    {
        var input = Encoding.UTF8.GetBytes(part);
        var bytes = new List<byte>();
        for (var i = 0; i < input.Length; i++)
        {
            if (input[i] != '%')
            {
                bytes.Add(input[i]);
            }
            else if (i + 2 < input.Length && byte.TryParse(part.AsSpan(i + 1, 2), System.Globalization.NumberStyles.HexNumber, null, out var b))
            {
                bytes.Add(b);
                i += 2;
            }
            else
            {
                return null;
            }
        }
        try
        {
            return new UTF8Encoding(false, throwOnInvalidBytes: true).GetString(bytes.ToArray());
        }
        catch (DecoderFallbackException)
        {
            return null;
        }
    }
}
