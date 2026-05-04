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

// --- Weather data (fetch from weatherapi.com if API key is set) ---

const WEATHER_API_KEY = process.env.WEATHER_API;
const WEATHER_LAT = parseFloat(process.env.WEATHER_LAT) || 52.520008;
const WEATHER_LONG = parseFloat(process.env.WEATHER_LONG) || 13.404954;

function iconFromConditionCode(code) {
    if (code === 1003) return appWindow.Images.cloudy;
    if (code === 1006) return appWindow.Images.cloud;
    return appWindow.Images.sunny;
}

async function fetchWeather() {
    if (!WEATHER_API_KEY) return;

    try {
        const url =
            `https://api.weatherapi.com/v1/forecast.json` +
            `?key=${WEATHER_API_KEY}` +
            `&q=${WEATHER_LAT},${WEATHER_LONG}` +
            `&days=3`;

        const response = await fetch(url);
        if (!response.ok) {
            console.error(`Weather API error: ${response.status}`);
            return;
        }

        const data = await response.json();
        const now = new Date();

        appWindow.WeatherAdapter.current_temperature =
            `${Math.round(data.current.temp_c)}°`;
        appWindow.WeatherAdapter.current_day =
            `${now.getDate()} ${monthFmt.format(now)} ${now.getFullYear()}`;
        appWindow.WeatherAdapter.current_weather_description =
            data.current.condition.text;
        appWindow.WeatherAdapter.current_temperature_icon =
            iconFromConditionCode(data.current.condition.code);

        const forecasts = data.forecast.forecastday;
        const absoluteMax = Math.round(
            Math.max(...forecasts.map((d) => d.day.maxtemp_c)),
        );
        const absoluteMin = Math.round(
            Math.min(...forecasts.map((d) => d.day.mintemp_c)),
        );

        appWindow.WeatherAdapter.week_model = forecasts.map((day) => ({
            title: shortDayFmt.format(new Date(day.date)),
            icon: iconFromConditionCode(day.day.condition.code),
            max: Math.round(day.day.maxtemp_c),
            min: Math.round(day.day.mintemp_c),
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
