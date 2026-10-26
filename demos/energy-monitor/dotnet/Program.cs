// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using System.Globalization;
using System.Text.Json;
using EnergyMonitor;
using Slint;

internal static class Program
{
    private static readonly HttpClient Http = new();
    private static readonly CultureInfo English = CultureInfo.GetCultureInfo("en-US");

    private static readonly Dictionary<int, string> WeatherDescriptions = new()
    {
        [0] = "Clear sky", [1] = "Mainly clear", [2] = "Partly cloudy", [3] = "Overcast",
        [45] = "Fog", [48] = "Depositing rime fog",
        [51] = "Light drizzle", [53] = "Moderate drizzle", [55] = "Dense drizzle",
        [61] = "Slight rain", [63] = "Moderate rain", [65] = "Heavy rain",
        [71] = "Slight snowfall", [73] = "Moderate snowfall", [75] = "Heavy snowfall",
        [80] = "Slight rain showers", [81] = "Moderate rain showers",
        [95] = "Thunderstorm",
    };

    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var window = MainWindow.Create();

        UpdateHeader(window);
        using var clock = new Slint.Timer();
        clock.Start(TimerMode.Repeated, TimeSpan.FromMilliseconds(300), () => UpdateHeader(window));

        // In kiosk mode, the overview moves to the next page every four seconds.
        using var kiosk = new Slint.Timer();
        kiosk.Start(TimerMode.Repeated, TimeSpan.FromSeconds(4), () =>
        {
            var menu = window.MenuOverviewAdapter;
            if (window.SettingsAdapter.KioskModeChecked)
            {
                menu.CurrentPage = menu.CurrentPage >= menu.Count - 1 ? 0 : menu.CurrentPage + 1;
            }
        });

        EventLoop.InvokeFromEventLoop(async () => await FetchWeatherAsync(window));
        window.Run();
    }

    private static void UpdateHeader(MainWindow window)
    {
        var now = DateTime.Now;
        var header = window.HeaderAdapter;
        // Setting only what changed keeps the header from redrawing on every tick.
        var date = now.ToString("dddd, MMMM d, yyyy", English);
        var time = now.ToString("h:mm", English);
        var suffix = now.ToString("tt", English);
        if (header.Date != date)
        {
            header.Date = date;
        }
        if (header.Time != time)
        {
            header.Time = time;
        }
        if (header.TimeSuffix != suffix)
        {
            header.TimeSuffix = suffix;
        }
    }

    /// <summary>Shows the weather of WEATHER_LAT and WEATHER_LONG, or of Berlin, from Open-Meteo.</summary>
    private static async Task FetchWeatherAsync(MainWindow window)
    {
        var latitude = Coordinate("WEATHER_LAT", 52.520008);
        var longitude = Coordinate("WEATHER_LONG", 13.404954);
        try
        {
            using var json = JsonDocument.Parse(await Http.GetStringAsync(FormattableString.Invariant(
                $"https://api.open-meteo.com/v1/forecast?latitude={latitude}&longitude={longitude}") +
                "&current=temperature_2m,weather_code&daily=temperature_2m_max,temperature_2m_min,weather_code" +
                "&timezone=auto&forecast_days=3"));
            var current = json.RootElement.GetProperty("current");
            var code = current.GetProperty("weather_code").GetInt32();
            var now = DateTime.Now;
            var weather = window.WeatherAdapter;
            weather.CurrentTemperature = $"{Math.Round(current.GetProperty("temperature_2m").GetDouble())}°";
            weather.CurrentDay = now.ToString("d MMMM yyyy", English);
            weather.CurrentWeatherDescription = WeatherDescriptions.GetValueOrDefault(code, "Unknown");
            weather.CurrentTemperatureIcon = Icon(window, code);

            var daily = json.RootElement.GetProperty("daily");
            var days = daily.GetProperty("time").EnumerateArray().Select(day => DateTime.Parse(day.GetString()!, CultureInfo.InvariantCulture)).ToList();
            var maxima = daily.GetProperty("temperature_2m_max").EnumerateArray().Select(t => (int)Math.Round(t.GetDouble())).ToList();
            var minima = daily.GetProperty("temperature_2m_min").EnumerateArray().Select(t => (int)Math.Round(t.GetDouble())).ToList();
            var codes = daily.GetProperty("weather_code").EnumerateArray().Select(c => c.GetInt32()).ToList();
            weather.WeekModel = new ListModel<BarTileModel>(days.Select((day, i) => new BarTileModel
            {
                Title = day.ToString("ddd", English),
                Icon = Icon(window, codes[i]),
                Max = maxima[i],
                Min = minima[i],
                AbsoluteMax = maxima.Max(),
                AbsoluteMin = minima.Min(),
                Unit = "°",
            }));
        }
        catch (Exception e) when (e is HttpRequestException or JsonException or KeyNotFoundException or InvalidOperationException)
        {
            Console.Error.WriteLine($"Failed to fetch the weather: {e.Message}");
        }
    }

    private static double Coordinate(string variable, double fallback) =>
        double.TryParse(Environment.GetEnvironmentVariable(variable), NumberStyles.Float, CultureInfo.InvariantCulture, out var value)
            ? value
            : fallback;

    /// <summary>The icon of a WMO weather code.</summary>
    private static Image Icon(MainWindow window, int code) => code switch
    {
        2 => window.Images.Cloudy,
        >= 3 => window.Images.Cloud,
        _ => window.Images.Sunny,
    };
}
