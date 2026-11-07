// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using System.Globalization;

internal static class Program
{
    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var booker = Booker.Booker.Create();
        booker.OnValidateDate(date => TryParse(date, out _));
        booker.OnCompareDate((first, second) => TryParse(first, out var a) && TryParse(second, out var b) && a <= b);
        booker.Run();
    }

    private static bool TryParse(string date, out DateOnly result) =>
        DateOnly.TryParseExact(date, "d.M.yyyy", CultureInfo.InvariantCulture, DateTimeStyles.None, out result);
}
