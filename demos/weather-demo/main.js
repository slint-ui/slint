#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import * as slint from "slint-ui";
import { readFileSync } from "node:fs";

const ui = slint.loadFile(new URL("ui/main.slint", import.meta.url));
const appWindow = new ui.AppWindow();

// --- Dummy weather data (used when no API key is provided) ---

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

// --- OpenWeather API (One Call 3.0) ---

const API_KEY = process.env.OPEN_WEATHER_API_KEY;

function conditionFromId(id) {
    if (id >= 200 && id < 300) return "Stormy";
    if (id >= 300 && id < 400) return "SunnyRainy";
    if (id >= 500 && id < 600) return "Rainy";
    if (id >= 600 && id < 700) return "Snowy";
    if (id >= 700 && id < 800) return "Foggy";
    if (id === 800) return "Sunny";
    if (id === 801) return "PartiallyCloudy";
    if (id === 802) return "MostlyCloudy";
    return "Cloudy";
}

async function fetchCityWeather(city, signal) {
    const url =
        `https://api.openweathermap.org/data/3.0/onecall` +
        `?lat=${city.city_data.lat}&lon=${city.city_data.lon}` +
        `&exclude=minutely,hourly,alerts` +
        `&units=metric&appid=${API_KEY}`;

    const resp = await fetch(url, { signal });
    if (!resp.ok) throw new Error(`OpenWeather API: ${resp.status}`);
    const data = await resp.json();

    const current = data.current;
    const daily = data.daily || [];
    const todayTemp = daily[0]?.temp;
    const fallback = current.temp ?? 0;

    const currentData = {
        condition: conditionFromId(current.weather?.[0]?.id ?? 0),
        description: current.weather?.[0]?.description || "",
        current_temperature: todayTemp?.day ?? fallback,
        detailed_temperature: {
            min: todayTemp?.min ?? fallback,
            max: todayTemp?.max ?? fallback,
            morning: todayTemp?.morn ?? fallback,
            day: todayTemp?.day ?? fallback,
            evening: todayTemp?.eve ?? fallback,
            night: todayTemp?.night ?? fallback,
        },
        precipitation: { probability: 0, rain_volume: 0, snow_volume: 0 },
        uv_index: current.uvi ?? 0,
    };

    const forecastData = daily.map((d) => ({
        weather_data: {
            condition: conditionFromId(d.weather?.[0]?.id ?? 0),
            description: d.weather?.[0]?.description || "",
            current_temperature: d.temp?.day ?? 0,
            detailed_temperature: {
                min: d.temp?.min ?? 0,
                max: d.temp?.max ?? 0,
                morning: d.temp?.morn ?? 0,
                day: d.temp?.day ?? 0,
                evening: d.temp?.eve ?? 0,
                night: d.temp?.night ?? 0,
            },
            precipitation: {
                probability: d.pop ?? 0,
                rain_volume: d.rain ?? 0,
                snow_volume: d.snow ?? 0,
            },
            uv_index: d.uvi ?? 0,
        },
    }));

    return {
        city_data: city.city_data,
        weather_data: { current_data: currentData, forecast_data: forecastData },
    };
}

async function searchLocationApi(query, signal) {
    const url =
        `https://api.openweathermap.org/geo/1.0/direct` +
        `?q=${encodeURIComponent(query)}&limit=5&appid=${API_KEY}`;
    const resp = await fetch(url, { signal });
    if (!resp.ok) return [];
    const results = await resp.json();

    const unique = [];
    for (const loc of results) {
        const name = loc.name;
        const country = loc.country;
        const state = loc.state || "";
        if (!unique.some((u) => u.name === name && u.country === country && u.state === state)) {
            unique.push({ name, state, country, lat: loc.lat, lon: loc.lon });
        }
    }
    return unique;
}

// --- Wire up callbacks ---

appWindow.CityWeather.can_add_city = !!API_KEY;

appWindow.CityWeather.get_forecast_graph_command = forecastGraphCommand;

let refreshController = null;

appWindow.CityWeather.refresh_all = async function () {
    if (refreshController) refreshController.abort();
    refreshController = new AbortController();
    const { signal } = refreshController;
    try {
        if (API_KEY) {
            const refreshed = await Promise.all(
                cities.map((c) => fetchCityWeather(c, signal).catch(() => c)),
            );
            if (!signal.aborted) updateCities(refreshed);
        }
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
    if (!API_KEY || !query) return;
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

        if (API_KEY) {
            const fetched = await fetchCityWeather(newCity);
            cities.push(fetched);
            cityModel.push(cityWeatherInfoFromData(fetched));
        }
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
        if (API_KEY) {
            const refreshed = await Promise.all(
                dummyData.map((c) => fetchCityWeather(c).catch(() => c)),
            );
            updateCities(refreshed);
        } else {
            updateCities(dummyData);
        }
    } finally {
        appWindow.BusyLayerController.unset_busy();
    }
}

load();
await appWindow.run();
process.exit(0);
