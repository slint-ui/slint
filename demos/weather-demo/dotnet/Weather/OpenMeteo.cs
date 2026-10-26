// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using System.Globalization;
using System.Text.Json;
using Slint;

namespace WeatherDemo.Weather;

/// <summary>A city of the list: where it is, and its weather as the UI shows it.</summary>
public sealed record City(float Lat, float Lon, string Name, CityWeatherInfo Info);

/// <summary>The weather and the locations of <see href="https://open-meteo.com/">Open-Meteo</see>.</summary>
public static class OpenMeteo
{
    private static readonly HttpClient Http = new();
    private static readonly CultureInfo English = CultureInfo.GetCultureInfo("en-US");

    private static readonly Dictionary<int, string> Descriptions = new()
    {
        [0] = "Clear sky",
        [1] = "Mainly clear",
        [2] = "Partly cloudy",
        [3] = "Overcast",
        [45] = "Fog",
        [48] = "Depositing rime fog",
        [51] = "Light drizzle",
        [53] = "Moderate drizzle",
        [55] = "Dense drizzle",
        [56] = "Light freezing drizzle",
        [57] = "Dense freezing drizzle",
        [61] = "Slight rain",
        [63] = "Moderate rain",
        [65] = "Heavy rain",
        [66] = "Light freezing rain",
        [67] = "Heavy freezing rain",
        [71] = "Slight snowfall",
        [73] = "Moderate snowfall",
        [75] = "Heavy snowfall",
        [77] = "Snow grains",
        [80] = "Slight rain showers",
        [81] = "Moderate rain showers",
        [82] = "Violent rain showers",
        [85] = "Slight snow showers",
        [86] = "Heavy snow showers",
        [95] = "Thunderstorm",
        [96] = "Thunderstorm with slight hail",
        [99] = "Thunderstorm with heavy hail",
    };

    /// <summary>Returns <paramref name="city"/> with the current weather and the forecast of the next 8 days.</summary>
    public static async Task<City> FetchAsync(City city, CancellationToken cancellation = default)
    {
        var url = FormattableString.Invariant(
            $"https://api.open-meteo.com/v1/forecast?latitude={city.Lat}&longitude={city.Lon}") +
            "&current=temperature_2m,weather_code,uv_index" +
            "&daily=temperature_2m_max,temperature_2m_min,weather_code,precipitation_probability_max,rain_sum,snowfall_sum,uv_index_max" +
            "&hourly=temperature_2m&timezone=auto&forecast_days=8";
        using var json = JsonDocument.Parse(await Http.GetStringAsync(url, cancellation));
        var root = json.RootElement;
        var current = root.GetProperty("current");
        var daily = root.GetProperty("daily");
        var hourly = root.GetProperty("hourly").GetProperty("temperature_2m");
        var fallback = Number(current, "temperature_2m");
        float Hourly(int day, int hour) => day * 24 + hour < hourly.GetArrayLength() ? Number(hourly[day * 24 + hour], fallback) : fallback;
        float Daily(string name, int day, float defaultValue = 0) => Number(daily.GetProperty(name)[day], defaultValue);

        var code = (int)Number(current, "weather_code");
        var now = new WeatherInfo
        {
            IconType = Icon(code),
            Description = Descriptions.GetValueOrDefault(code, "Unknown"),
            CurrentTemp = fallback,
            DetailedTemp = Temperatures(Daily("temperature_2m_min", 0, fallback), Daily("temperature_2m_max", 0, fallback), day => Hourly(0, day)),
            Uv = (int)MathF.Round(Number(current, "uv_index")),
        };
        var forecast = Enumerable.Range(0, daily.GetProperty("time").GetArrayLength()).Select(day =>
        {
            var dayCode = (int)Daily("weather_code", day);
            return new WeatherForecastInfo
            {
                DayName = DayName(day),
                WeatherInfo = new WeatherInfo
                {
                    IconType = Icon(dayCode),
                    Description = Descriptions.GetValueOrDefault(dayCode, "Unknown"),
                    CurrentTemp = Hourly(day, 12),
                    DetailedTemp = Temperatures(Daily("temperature_2m_min", day), Daily("temperature_2m_max", day), hour => Hourly(day, hour)),
                    Uv = (int)MathF.Round(Daily("uv_index_max", day)),
                    PrecipitationProb = Daily("precipitation_probability_max", day) / 100,
                    Rain = Daily("rain_sum", day),
                    Snow = Daily("snowfall_sum", day),
                },
            };
        });
        return city with
        {
            Info = new CityWeatherInfo { CityName = city.Name, CurrentWeather = now, ForecastWeather = new ListModel<WeatherForecastInfo>(forecast) },
        };
    }

    /// <summary>Returns up to 5 places whose name matches <paramref name="query"/>.</summary>
    public static async Task<List<GeoLocationEntry>> SearchAsync(string query, CancellationToken cancellation)
    {
        var url = $"https://geocoding-api.open-meteo.com/v1/search?name={Uri.EscapeDataString(query)}&count=5&language=en";
        using var json = JsonDocument.Parse(await Http.GetStringAsync(url, cancellation));
        if (!json.RootElement.TryGetProperty("results", out var results))
        {
            return [];
        }
        return results.EnumerateArray().Select(place => new GeoLocationEntry
        {
            Name = Text(place, "name"),
            State = Text(place, "admin1"),
            Country = Text(place, "country"),
            Lat = Number(place, "latitude"),
            Lon = Number(place, "longitude"),
        }).ToList();
    }

    /// <summary>Reads the cities and the saved weather of <c>dummyweather.json</c>, to show before the first download.</summary>
    public static List<City> LoadSaved(string path)
    {
        using var json = JsonDocument.Parse(File.ReadAllText(path));
        return json.RootElement.EnumerateArray().Select(entry =>
        {
            var city = entry.GetProperty("city_data");
            var weather = entry.GetProperty("weather_data");
            var name = Text(city, "city_name");
            return new City(Number(city, "lat"), Number(city, "lon"), name, new CityWeatherInfo
            {
                CityName = name,
                CurrentWeather = SavedWeather(weather.GetProperty("current_data")),
                ForecastWeather = new ListModel<WeatherForecastInfo>(weather.GetProperty("forecast_data").EnumerateArray().Select((day, index) =>
                    new WeatherForecastInfo { DayName = DayName(index), WeatherInfo = SavedWeather(day.GetProperty("weather_data")) })),
            });
        }).ToList();
    }

    private static WeatherInfo SavedWeather(JsonElement data)
    {
        var temperature = data.GetProperty("detailed_temperature");
        var precipitation = data.GetProperty("precipitation");
        return new WeatherInfo
        {
            Description = Text(data, "description"),
            IconType = Enum.TryParse<IconType>(Text(data, "condition"), out var icon) ? icon : IconType.Unknown,
            CurrentTemp = Number(data, "current_temperature"),
            DetailedTemp = new TemperatureInfo
            {
                Min = Number(temperature, "min"),
                Max = Number(temperature, "max"),
                Morning = Number(temperature, "morning"),
                Day = Number(temperature, "day"),
                Evening = Number(temperature, "evening"),
                Night = Number(temperature, "night"),
            },
            Uv = (int)MathF.Round(Number(data, "uv_index")),
            PrecipitationProb = Number(precipitation, "probability"),
            Rain = Number(precipitation, "rain_volume"),
            Snow = Number(precipitation, "snow_volume"),
        };
    }

    private static TemperatureInfo Temperatures(float min, float max, Func<int, float> atHour) => new()
    {
        Min = min,
        Max = max,
        Morning = atHour(6),
        Day = atHour(12),
        Evening = atHour(18),
        Night = atHour(0),
    };

    private static string DayName(int day) => day == 0 ? "Today" : DateTime.Today.AddDays(day).ToString("dddd", English);

    /// <summary>The icon of a WMO weather code.</summary>
    private static IconType Icon(int code) => code switch
    {
        0 or 1 => IconType.Sunny,
        2 => IconType.PartiallyCloudy,
        3 => IconType.Cloudy,
        45 or 48 => IconType.Foggy,
        >= 51 and <= 55 => IconType.SunnyRainy,
        >= 56 and <= 67 or >= 80 and <= 82 => IconType.Rainy,
        >= 71 and <= 77 or >= 85 and <= 86 => IconType.Snowy,
        >= 95 => IconType.Stormy,
        _ => IconType.Unknown,
    };

    private static float Number(JsonElement value, float fallback = 0) =>
        value.ValueKind == JsonValueKind.Number ? value.GetSingle() : fallback;

    private static float Number(JsonElement parent, string name) =>
        parent.TryGetProperty(name, out var value) ? Number(value) : 0;

    private static string Text(JsonElement parent, string name) =>
        parent.TryGetProperty(name, out var value) && value.ValueKind == JsonValueKind.String ? value.GetString()! : "";
}
