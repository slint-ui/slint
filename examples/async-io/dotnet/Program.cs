// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using System.Text.Json;
using AsyncIo;
using Slint;

internal static class Program
{
    private static readonly HttpClient Http = new();

    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var window = MainWindow.Create();
        var stocks = new ListModel<Symbol>(
            new[] { "AAPL.US", "MSFT.US", "AMZN.US" }.Select(name => new Symbol { Name = name }));
        window.Stocks = stocks;

        // While the event loop runs, each await continues on the Slint thread, so the handler can update the model.
        window.OnRefresh(async () =>
        {
            try
            {
                await RefreshAsync(stocks);
            }
            catch (Exception e) when (e is HttpRequestException or JsonException or KeyNotFoundException)
            {
                Console.Error.WriteLine($"Couldn't fetch the prices: {e.Message}");
            }
        });

        EventLoop.InvokeFromEventLoop(window.InvokeRefresh);
        window.Run();
    }

    private static async Task RefreshAsync(ListModel<Symbol> stocks)
    {
        var symbols = string.Join("+", stocks.Select(stock => stock.Name));
        using var json = JsonDocument.Parse(await Http.GetStringAsync($"https://stooq.com/q/l/?s={symbols}&f=sd2t2ohlcvn&h&e=json"));
        foreach (var quote in json.RootElement.GetProperty("symbols").EnumerateArray())
        {
            var name = quote.GetProperty("symbol").GetString();
            var row = stocks.ToList().FindIndex(stock => stock.Name == name);
            if (row >= 0 && quote.TryGetProperty("close", out var close) && close.ValueKind == JsonValueKind.Number)
            {
                stocks[row] = stocks[row] with { Price = close.GetSingle() };
            }
        }
    }
}
