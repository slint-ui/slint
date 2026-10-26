// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using Crud;
using Slint;

internal static class Program
{
    private sealed record Name(string First, string Last);

    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var window = MainWindow.Create();
        var names = new ListModel<Name>([new("Hans", "Emil"), new("Max", "Mustermann"), new("Roman", "Tisch")]);

        // The list shows "Last, First" items that start with the prefix the user typed.
        var prefix = "";
        var shown = names
            .Map(name => new Struct { ["text"] = $"{name.Last}, {name.First}" })
            .Filter(item => ((string)item["text"]!).StartsWith(prefix, StringComparison.Ordinal));
        window.NamesList = shown;

        window.OnPrefixEdited(() =>
        {
            prefix = window.Prefix;
            shown.Reset();
        });
        window.OnCreateClicked(() => names.Add(new Name(window.Name, window.Surname)));
        window.OnUpdateClicked(() =>
        {
            if (window.CurrentItem >= 0 && window.CurrentItem < shown.RowCount)
            {
                names[shown.SourceRow(window.CurrentItem)] = new Name(window.Name, window.Surname);
            }
        });
        window.OnDeleteClicked(() =>
        {
            if (window.CurrentItem >= 0 && window.CurrentItem < shown.RowCount)
            {
                names.RemoveAt(shown.SourceRow(window.CurrentItem));
            }
        });

        window.Run();
    }
}
