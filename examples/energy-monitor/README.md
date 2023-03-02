See the [MCU backend Readme](../mcu-board-support) to see how to run the example on a smaller device like the Raspberry Pi Pico.

The example can run with the mcu simulator with the following command

```cargo run -p energy-monitor --no-default-features --features=simulator --release```

## display weather data

To display real weather data in the demo an application key from https://www.weatherapi.com/ is needed. The api key can be injected by settings the
`WEATHER_API` environment variable. With the `WEATHER_LAT` and `WEATHER_LONG` variable position can be set, default is Berlin.