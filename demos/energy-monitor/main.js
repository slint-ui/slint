#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import * as slint from "slint-ui";

const ui = slint.loadFile(
    new URL("ui/desktop_window.slint", import.meta.url),
);
const appWindow = new ui.MainWindow();

// --- Live clock ---

const dateFmt = new Intl.DateTimeFormat("en-US", {
    weekday: "long", day: "numeric", month: "long", year: "numeric",
});
const monthFmt = new Intl.DateTimeFormat("en-US", { month: "long" });
const shortDayFmt = new Intl.DateTimeFormat("en-US", { weekday: "short" });

let lastDate = "", lastTime = "", lastSuffix = "";

function updateHeader() {
    const now = new Date();

    const date = dateFmt.format(now);
    let hours = now.getHours();
    const suffix = hours >= 12 ? "PM" : "AM";
    hours = hours % 12 || 12;
    const time = `${hours}:${String(now.getMinutes()).padStart(2, "0")}`;

    if (date !== lastDate) { appWindow.HeaderAdapter.date = date; lastDate = date; }
    if (time !== lastTime) { appWindow.HeaderAdapter.time = time; lastTime = time; }
    if (suffix !== lastSuffix) { appWindow.HeaderAdapter.time_suffix = suffix; lastSuffix = suffix; }
}

updateHeader();
const clockTimer = setInterval(updateHeader, 300);

// --- Kiosk mode: rotate pages every 4 seconds ---

const kioskTimer = setInterval(() => {
    if (!appWindow.SettingsAdapter.kiosk_mode_checked) return;

    const current = appWindow.MenuOverviewAdapter.current_page;
    const count = appWindow.MenuOverviewAdapter.count;
    appWindow.MenuOverviewAdapter.current_page =
        current >= count - 1 ? 0 : current + 1;
}, 4000);

// --- Weather data (fetched from Open-Meteo, no API key required) ---

const WEATHER_LAT = parseFloat(process.env.WEATHER_LAT) || 52.520008;
const WEATHER_LONG = parseFloat(process.env.WEATHER_LONG) || 13.404954;

function iconFromWmo(code) {
    if (code === 2) return appWindow.Images.cloudy;
    if (code >= 3) return appWindow.Images.cloud;
    return appWindow.Images.sunny;
}

const wmoDescriptions = {
    0: "Clear sky", 1: "Mainly clear", 2: "Partly cloudy", 3: "Overcast",
    45: "Fog", 48: "Depositing rime fog",
    51: "Light drizzle", 53: "Moderate drizzle", 55: "Dense drizzle",
    61: "Slight rain", 63: "Moderate rain", 65: "Heavy rain",
    71: "Slight snowfall", 73: "Moderate snowfall", 75: "Heavy snowfall",
    80: "Slight rain showers", 81: "Moderate rain showers",
    95: "Thunderstorm",
};

async function fetchWeather() {
    try {
        const url =
            `https://api.open-meteo.com/v1/forecast` +
            `?latitude=${WEATHER_LAT}&longitude=${WEATHER_LONG}` +
            `&current=temperature_2m,weather_code` +
            `&daily=temperature_2m_max,temperature_2m_min,weather_code` +
            `&timezone=auto&forecast_days=3`;

        const response = await fetch(url);
        if (!response.ok) {
            console.error(`Open-Meteo API error: ${response.status}`);
            return;
        }

        const data = await response.json();
        const now = new Date();

        appWindow.WeatherAdapter.current_temperature =
            `${Math.round(data.current.temperature_2m)}°`;
        appWindow.WeatherAdapter.current_day =
            `${now.getDate()} ${monthFmt.format(now)} ${now.getFullYear()}`;
        appWindow.WeatherAdapter.current_weather_description =
            wmoDescriptions[data.current.weather_code] ?? "Unknown";
        appWindow.WeatherAdapter.current_temperature_icon =
            iconFromWmo(data.current.weather_code);

        const daily = data.daily;
        const absoluteMax = Math.round(Math.max(...daily.temperature_2m_max));
        const absoluteMin = Math.round(Math.min(...daily.temperature_2m_min));

        appWindow.WeatherAdapter.week_model = daily.time.map((date, i) => ({
            title: shortDayFmt.format(new Date(date)),
            icon: iconFromWmo(daily.weather_code[i]),
            max: Math.round(daily.temperature_2m_max[i]),
            min: Math.round(daily.temperature_2m_min[i]),
            absolute_max: absoluteMax,
            absolute_min: absoluteMin,
            unit: "°",
        }));
    } catch (err) {
        console.error("Failed to fetch weather:", err.message);
    }
}

fetchWeather();

// --- Run ---

await appWindow.run();
process.exit(0);
