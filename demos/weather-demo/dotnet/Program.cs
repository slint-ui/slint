// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using Slint;
using WeatherDemo;
using WeatherDemo.Weather;

internal static class Program
{
    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var window = AppWindow.Create();
        var cities = new List<City>();
        var shown = new ListModel<CityWeatherInfo>();
        var weather = window.CityWeather;
        weather.CityWeather_ = shown;
        weather.CanAddCity = true;
        weather.OnGetForecastGraphCommand(ForecastGraph.Command);

        void Show(List<City> updated)
        {
            cities = updated;
            shown.Clear();
            shown.AddRange(updated.Select(city => city.Info));
        }

        // Refreshes every city, and keeps the old weather of a city whose download fails.
        CancellationTokenSource? refresh = null;
        async Task RefreshAsync(List<City> current)
        {
            refresh?.Cancel();
            var cancellation = (refresh = new CancellationTokenSource()).Token;
            try
            {
                var refreshed = await Task.WhenAll(current.Select(async city =>
                {
                    try
                    {
                        return await OpenMeteo.FetchAsync(city, cancellation);
                    }
                    catch (Exception e) when (e is HttpRequestException or System.Text.Json.JsonException or KeyNotFoundException or InvalidOperationException)
                    {
                        Console.Error.WriteLine($"Failed to refresh {city.Name}: {e.Message}");
                        return city;
                    }
                }));
                if (!cancellation.IsCancellationRequested)
                {
                    Show([.. refreshed]);
                }
            }
            catch (OperationCanceledException)
            {
            }
            finally
            {
                window.BusyLayerController.UnsetBusy();
            }
        }

        weather.OnRefreshAll(async () => await RefreshAsync(cities));
        weather.OnDelete(index =>
        {
            cities.RemoveAt(index);
            shown.RemoveAt(index);
        });
        weather.OnReorder((from, to) =>
        {
            (cities[from], cities[to]) = (cities[to], cities[from]);
            (shown[from], shown[to]) = (shown[to], shown[from]);
        });

        CancellationTokenSource? search = null;
        window.GeoLocation.OnSearchLocation(async query =>
        {
            if (query.Length == 0)
            {
                return;
            }
            search?.Cancel();
            var cancellation = (search = new CancellationTokenSource()).Token;
            try
            {
                var results = await OpenMeteo.SearchAsync(query, cancellation);
                window.GeoLocation.ResultList = new ListModel<GeoLocationEntry>(results);
            }
            catch (Exception e) when (e is OperationCanceledException or HttpRequestException or System.Text.Json.JsonException)
            {
            }
        });
        window.GeoLocation.OnAddLocation(async location =>
        {
            try
            {
                if (!cities.Any(city => city.Lat == location.Lat && city.Lon == location.Lon && city.Name == location.Name))
                {
                    var city = await OpenMeteo.FetchAsync(new City(location.Lat, location.Lon, location.Name, new CityWeatherInfo { CityName = location.Name }));
                    cities.Add(city);
                    shown.Add(city.Info);
                }
            }
            catch (Exception e) when (e is HttpRequestException or System.Text.Json.JsonException or KeyNotFoundException or InvalidOperationException)
            {
                Console.Error.WriteLine($"Failed to add {location.Name}: {e.Message}");
            }
            finally
            {
                window.BusyLayerController.UnsetBusy();
            }
        });

        // Shows the saved weather right away, and the current one once it's downloaded.
        Show(OpenMeteo.LoadSaved(Path.Combine(AppContext.BaseDirectory, "dummyweather.json")));
        EventLoop.InvokeFromEventLoop(async () =>
        {
            window.BusyLayerController.SetBusy();
            await RefreshAsync(cities);
        });
        window.Run();
    }
}
