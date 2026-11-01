<!-- cSpell: ignore lavapipe -->

# Slint Flight Tracker

Follow a flight across a globe of the earth, like on the moving map of an airplane's entertainment system.
wgpu renders the globe, and Slint draws the route, the aircraft, the airports, the city names, and the panels with the flight's details on top.
The globe shows the day and night sides as they are right now, with the lights of the cities on the night side.

On its own, the demo flies between random airports near large cities, like a kiosk: it boards, flies, lands, and then takes off for a new destination from where it landed.
Meanwhile it switches between following the aircraft, showing the whole route, and showing the globe.
The flights are sped up, but the speed, altitude, temperature, and the time to go are those of a real flight.

Plan your own flight with Plan a flight: search for the airports by city, name, country, or code.
Drag to move the map, and scroll, pinch, or double-click to zoom.
After a minute without input, the random flights take over again.

## Running the Demo

```sh
cargo run --release -p flight-tracker --manifest-path demos/Cargo.toml
```

## Running in a Browser

The web build uses Slint's FemtoVG renderer on WebGPU.
Uncomment the `#wasm#` lines in `Cargo.toml`, then build and serve this directory:

```sh
wasm-pack build --release --target web
python3 -m http.server
```

Open <http://localhost:8000> in a browser with WebGPU.

## How It Works

Slint renders with wgpu, and the demo renders the globe on Slint's own device:

- `main.rs` requires wgpu with `BackendSelector::require_wgpu_30()`, and gets the device in the rendering notifier.
  Before Slint renders each frame, the demo renders the globe into a texture, and shows it in an `Image` underneath the UI.
- `globe.wgsl` ray traces the globe, its atmosphere, and the stars in a single pass over the screen, so the globe needs no mesh.
- `map.rs` computes a map with mip levels from the outlines in `data/` at startup.
  It holds the distances to the coasts and the borders, so that the shader draws them sharp at any zoom, and the lights of the cities.
- The route, the aircraft, the airports, and the city names are ordinary Slint elements.
  Each frame, `main.rs` projects them to the screen with the camera's matrices, and passes their positions to Slint.

The map data comes from [Natural Earth](https://www.naturalearthdata.com/), and the airports from [OurAirports](https://ourairports.com/), both in the public domain.
`data/convert.py` converts them into the compact text files in `data/`.

Without a GPU, set `SLINT_WGPU_CPU=1` to render on the CPU, for example with Mesa's lavapipe.
