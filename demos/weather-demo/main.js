#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell:ignore dummyweather

import * as slint from "slint-ui";
import { readFileSync } from "node:fs";

const ui = slint.loadFile(new URL("ui/main.slint", import.meta.url));
const appWindow = new ui.AppWindow();

// --- Dummy weather data (provides initial city list) ---

const dummyData = JSON.parse(
    readFileSync(
        new URL("src/weather/dummyweather.json", import.meta.url),
        "utf-8",
    ),
);

const dayFmt = new Intl.DateTimeFormat("en-US", { weekday: "long" });

function dayNameForIndex(index) {
    if (index === 0) return "Today";
    const d = new Date();
    d.setDate(d.getDate() + index);
    return dayFmt.format(d);
}

// --- Icon type (matches the Slint IconType enum) ---

const validIconTypes = new Set([
    "Sunny", "PartiallyCloudy", "MostlyCloudy", "Cloudy",
    "SunnyRainy", "Rainy", "Stormy", "Snowy", "Foggy",
]);

function iconTypeFromCondition(condition) {
    return validIconTypes.has(condition) ? condition : "Unknown";
}

// --- Data conversion ---

function weatherInfoFromData(data) {
    return {
        description: data.description,
        icon_type: iconTypeFromCondition(data.condition),
        current_temp: data.current_temperature,
        detailed_temp: {
            min: data.detailed_temperature.min,
            max: data.detailed_temperature.max,
            morning: data.detailed_temperature.morning,
            day: data.detailed_temperature.day,
            evening: data.detailed_temperature.evening,
            night: data.detailed_temperature.night,
        },
        uv: Math.round(data.uv_index),
        precipitation_prob: data.precipitation.probability,
        rain: data.precipitation.rain_volume,
        snow: data.precipitation.snow_volume,
    };
}

function cityWeatherInfoFromData(cityData) {
    return {
        city_name: cityData.city_data.city_name,
        current_weather: weatherInfoFromData(cityData.weather_data.current_data),
        forecast_weather: cityData.weather_data.forecast_data.map(
            (fd, index) => ({
                day_name: dayNameForIndex(index),
                weather_info: weatherInfoFromData(fd.weather_data),
            }),
        ),
    };
}

// --- SVG graph command for temperature curves ---

function forecastGraphCommand(model, daysCount, width, height) {
    if (daysCount === 0 || width === 0 || height === 0) return "";

    const temperatures = [];
    for (let i = 0; i < daysCount; i++) {
        const row = model.rowData(i);
        if (row) temperatures.push(row.weather_info.detailed_temp.day);
    }

    const MIN_MAX_MARGIN = 5.0;
    const minTemp = Math.min(...temperatures) - MIN_MAX_MARGIN;
    const maxTemp = Math.max(...temperatures) + MIN_MAX_MARGIN;
    const maxTempValue = maxTemp - minTemp;
    const tempRatio = height / maxTempValue;

    const dayWidth = width / daysCount;
    const maxDayShift = daysCount * dayWidth;

    let cmd =
        `M 0 0 M ${maxDayShift} 0 ` +
        `M ${maxDayShift} ${maxTempValue * tempRatio} M 0 ${maxTempValue * tempRatio} `;

    const dayShift = (i) => i * dayWidth + 0.5 * dayWidth;
    const dayTemp = (t) => (maxTemp - t) * tempRatio;

    for (let i = 0; i < temperatures.length; i++) {
        const t = temperatures[i];
        if (i === 0) {
            cmd += `M ${dayShift(i)} ${dayTemp(t)} `;
        }

        if (i + 1 < temperatures.length) {
            const tNext = temperatures[i + 1];
            const d1 = dayShift(i);
            const d2 = dayShift(i + 1);
            const t1 = dayTemp(t);
            const t2 = dayTemp(tNext);
            const dMid = (d1 + d2) / 2;
            const tMid = (t1 + t2) / 2;
            const cpD1 = (dMid + d1) / 2;
            const cpD2 = (dMid + d2) / 2;
            cmd += `Q ${cpD1} ${t1} ${dMid} ${tMid} Q ${cpD2} ${t2} ${d2} ${t2} `;
        }
    }

    return cmd;
}

// --- City list state ---

let cities = [];
const cityModel = new slint.ArrayModel([]);
appWindow.CityWeather.city_weather = cityModel;

function updateCities(data) {
    cities = data;
    const items = data.map(cityWeatherInfoFromData);
    const oldCount = cityModel.rowCount();
    const shared = Math.min(oldCount, items.length);
    for (let i = 0; i < shared; i++) cityModel.setRowData(i, items[i]);
    for (let i = oldCount - 1; i >= shared; i--) cityModel.remove(i, 1);
    for (let i = shared; i < items.length; i++) cityModel.push(items[i]);
}

// --- Open-Meteo API (no API key required) ---

function conditionFromWmo(code) {
    if (code === 0 || code === 1) return "Sunny";
    if (code === 2) return "PartiallyCloudy";
    if (code === 3) return "Cloudy";
    if (code === 45 || code === 48) return "Foggy";
    if (code >= 51 && code <= 55) return "SunnyRainy";
    if (code >= 56 && code <= 67) return "Rainy";
    if (code >= 71 && code <= 77) return "Snowy";
    if (code >= 80 && code <= 82) return "Rainy";
    if (code >= 85 && code <= 86) return "Snowy";
    if (code >= 95) return "Stormy";
    return "Unknown";
}

const wmoDescriptions = {
    0: "Clear sky", 1: "Mainly clear", 2: "Partly cloudy", 3: "Overcast",
    45: "Fog", 48: "Depositing rime fog",
    51: "Light drizzle", 53: "Moderate drizzle", 55: "Dense drizzle",
    56: "Light freezing drizzle", 57: "Dense freezing drizzle",
    61: "Slight rain", 63: "Moderate rain", 65: "Heavy rain",
    66: "Light freezing rain", 67: "Heavy freezing rain",
    71: "Slight snowfall", 73: "Moderate snowfall", 75: "Heavy snowfall",
    77: "Snow grains",
    80: "Slight rain showers", 81: "Moderate rain showers", 82: "Violent rain showers",
    85: "Slight snow showers", 86: "Heavy snow showers",
    95: "Thunderstorm", 96: "Thunderstorm with slight hail",
    99: "Thunderstorm with heavy hail",
};

function descriptionFromWmo(code) {
    return wmoDescriptions[code] ?? "Unknown";
}

async function fetchCityWeather(city, signal) {
    const url =
        `https://api.open-meteo.com/v1/forecast` +
        `?latitude=${city.city_data.lat}&longitude=${city.city_data.lon}` +
        `&current=temperature_2m,weather_code,uv_index` +
        `&daily=temperature_2m_max,temperature_2m_min,weather_code,` +
        `precipitation_probability_max,rain_sum,snowfall_sum,uv_index_max` +
        `&hourly=temperature_2m` +
        `&timezone=auto&forecast_days=8`;

    const resp = await fetch(url, { signal });
    if (!resp.ok) throw new Error(`Open-Meteo API: ${resp.status}`);
    const data = await resp.json();

    const current = data.current;
    const daily = data.daily;
    const hourly = data.hourly.temperature_2m;
    const fallback = current.temperature_2m ?? 0;

    // Extract morning/day/evening/night temps from hourly data (hours 6, 12, 18, 0).
    function hourlyTemp(dayIndex, hour) {
        const i = dayIndex * 24 + hour;
        return i < hourly.length ? hourly[i] : fallback;
    }

    const currentData = {
        condition: conditionFromWmo(current.weather_code ?? 0),
        description: descriptionFromWmo(current.weather_code ?? 0),
        current_temperature: fallback,
        detailed_temperature: {
            min: daily.temperature_2m_min[0] ?? fallback,
            max: daily.temperature_2m_max[0] ?? fallback,
            morning: hourlyTemp(0, 6),
            day: hourlyTemp(0, 12),
            evening: hourlyTemp(0, 18),
            night: hourlyTemp(0, 0),
        },
        precipitation: { probability: 0, rain_volume: 0, snow_volume: 0 },
        uv_index: current.uv_index ?? 0,
    };

    const forecastData = daily.time.map((_, i) => ({
        weather_data: {
            condition: conditionFromWmo(daily.weather_code[i] ?? 0),
            description: descriptionFromWmo(daily.weather_code[i] ?? 0),
            current_temperature: hourlyTemp(i, 12),
            detailed_temperature: {
                min: daily.temperature_2m_min[i] ?? 0,
                max: daily.temperature_2m_max[i] ?? 0,
                morning: hourlyTemp(i, 6),
                day: hourlyTemp(i, 12),
                evening: hourlyTemp(i, 18),
                night: hourlyTemp(i, 0),
            },
            precipitation: {
                probability: (daily.precipitation_probability_max[i] ?? 0) / 100,
                rain_volume: daily.rain_sum[i] ?? 0,
                snow_volume: daily.snowfall_sum[i] ?? 0,
            },
            uv_index: daily.uv_index_max[i] ?? 0,
        },
    }));

    return {
        city_data: city.city_data,
        weather_data: { current_data: currentData, forecast_data: forecastData },
    };
}

async function searchLocationApi(query, signal) {
    const url =
        `https://geocoding-api.open-meteo.com/v1/search` +
        `?name=${encodeURIComponent(query)}&count=5&language=en`;
    const resp = await fetch(url, { signal });
    if (!resp.ok) return [];
    const data = await resp.json();
    const results = data.results || [];

    return results.map((loc) => ({
        name: loc.name,
        state: loc.admin1 || "",
        country: loc.country || "",
        lat: loc.latitude,
        lon: loc.longitude,
    }));
}

// --- Wire up callbacks ---

appWindow.CityWeather.can_add_city = true;

appWindow.CityWeather.get_forecast_graph_command = forecastGraphCommand;

let refreshController = null;

appWindow.CityWeather.refresh_all = async function () {
    if (refreshController) refreshController.abort();
    refreshController = new AbortController();
    const { signal } = refreshController;
    try {
        const refreshed = await Promise.all(
            cities.map((c) => fetchCityWeather(c, signal).catch(() => c)),
        );
        if (!signal.aborted) updateCities(refreshed);
    } finally {
        appWindow.BusyLayerController.unset_busy();
    }
};

appWindow.CityWeather.delete = function (index) {
    cities.splice(index, 1);
    cityModel.remove(index, 1);
};

appWindow.CityWeather.reorder = function (from, to) {
    [cities[from], cities[to]] = [cities[to], cities[from]];

    const fromData = cityModel.rowData(from);
    const toData = cityModel.rowData(to);
    cityModel.setRowData(from, toData);
    cityModel.setRowData(to, fromData);
};

let searchController = null;

appWindow.GeoLocation.search_location = async function (query) {
    if (!query) return;
    if (searchController) searchController.abort();
    searchController = new AbortController();
    const { signal } = searchController;
    const results = await searchLocationApi(query, signal);
    if (!signal.aborted) appWindow.GeoLocation.result_list = results;
};

appWindow.GeoLocation.add_location = async function (location) {
    const alreadyExists = cities.some(
        (c) =>
            c.city_data.lat === location.lat &&
            c.city_data.lon === location.lon &&
            c.city_data.city_name === location.name,
    );
    if (alreadyExists) return;

    try {
        const newCity = {
            city_data: {
                lat: location.lat,
                lon: location.lon,
                city_name: location.name,
            },
            weather_data: null,
        };

        const fetched = await fetchCityWeather(newCity);
        cities.push(fetched);
        cityModel.push(cityWeatherInfoFromData(fetched));
    } catch (err) {
        console.error("Failed to add city:", err.message);
    } finally {
        appWindow.BusyLayerController.unset_busy();
    }
};

// --- Load initial data ---

async function load() {
    appWindow.BusyLayerController.set_busy();
    try {
        const refreshed = await Promise.all(
            dummyData.map((c) => fetchCityWeather(c).catch(() => c)),
        );
        updateCities(refreshed);
    } finally {
        appWindow.BusyLayerController.unset_busy();
    }
}

load();
await appWindow.run();
process.exit(0);
