// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using System.Globalization;
using System.Text;
using Slint;

namespace WeatherDemo.Weather;

public static class ForecastGraph
{
    // The graph leaves this many degrees above the warmest and below the coldest day.
    private const float Margin = 5;

    /// <summary>
    /// Returns the commands of an SVG path through the day temperatures of the forecast,
    /// for a <c>Path</c> element of <paramref name="width"/> by <paramref name="height"/>.
    /// </summary>
    public static string Command(Model<WeatherForecastInfo> forecast, int days, float width, float height)
    {
        days = Math.Min(days, forecast.RowCount);
        if (days == 0 || width == 0 || height == 0)
        {
            return "";
        }
        var temperatures = Enumerable.Range(0, days).Select(i => forecast[i].WeatherInfo.DetailedTemp.Day).ToList();
        var min = temperatures.Min() - Margin;
        var max = temperatures.Max() + Margin;
        var ratio = height / (max - min);
        var dayWidth = width / days;
        float X(int day) => day * dayWidth + 0.5f * dayWidth;
        float Y(float temperature) => (max - temperature) * ratio;

        var command = new StringBuilder();
        void Append(FormattableString part) => command.Append(part.ToString(CultureInfo.InvariantCulture));
        // The invisible moves span the whole area, so that the Path scales the curve to it.
        Append($"M 0 0 M {days * dayWidth} 0 M {days * dayWidth} {(max - min) * ratio} M 0 {(max - min) * ratio} ");
        Append($"M {X(0)} {Y(temperatures[0])} ");
        for (var i = 0; i + 1 < temperatures.Count; i++)
        {
            var (x1, y1, x2, y2) = (X(i), Y(temperatures[i]), X(i + 1), Y(temperatures[i + 1]));
            var (midX, midY) = ((x1 + x2) / 2, (y1 + y2) / 2);
            Append($"Q {(midX + x1) / 2} {y1} {midX} {midY} Q {(midX + x2) / 2} {y2} {x2} {y2} ");
        }
        return command.ToString();
    }
}
