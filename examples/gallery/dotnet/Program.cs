// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using System.Globalization;
using Gallery;
using Slint;

internal static class Program
{
    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var app = App.Create();

        // The strings of the .slint files are English; German, French, and Japanese are bundled.
        var language = CultureInfo.CurrentUICulture.TwoLetterISOLanguageName;
        if (language is "de" or "fr" or "ja")
        {
            Translations.SelectBundledTranslation(language);
        }

        var rows = new ListModel<Model<Struct>>(Enumerable.Range(1, 100).Select(row =>
            (Model<Struct>)new ListModel<Struct>(Enumerable.Range(1, 4).Select(column => Item($"Item {row}.{column}")))));
        var table = app.TableViewPageAdapter;
        table.RowData = rows;
        table.OnFilterSortModel(FilterAndSort);

        app.Run();
    }

    private static Struct Item(string text) => new() { ["text"] = text };

    private static string Text(Model<Struct> row, int column) => (string)row[column]["text"]!;

    /// <summary>Shows the rows whose first column contains <paramref name="filter"/>, sorted by a column.</summary>
    private static Model<Model<Struct>> FilterAndSort(Model<Model<Struct>> rows, string filter, int sortColumn, bool ascending)
    {
        var shown = rows;
        if (filter.Length > 0)
        {
            shown = shown.Filter(row => Text(row, 0).Contains(filter, StringComparison.CurrentCultureIgnoreCase));
        }
        if (sortColumn >= 0)
        {
            var direction = ascending ? 1 : -1;
            shown = shown.Sort((a, b) => direction * string.CompareOrdinal(Text(a, sortColumn), Text(b, sortColumn)));
        }
        return shown;
    }
}
