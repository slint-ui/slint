<!-- cSpell: ignore lavapipe -->

# Slint Flight Tracker

Follow a flight across a globe of the earth, like on the moving map of an airplane's entertainment system.
wgpu renders the globe, and Slint draws the route, the aircraft, the airports, the city names, and the panels with the flight's details on top.
The globe shows the day and night sides as they are right now, with the lights of the cities on the night side.

The demo starts with the planner beside the globe, which floats over a pattern that Slint draws.
Pick the origin and the destination, and the globe turns to show the route.
Take off, and the globe grows to fill the window as the flight boards.
After landing, the globe shrinks back beside the planner, and the next flight starts where this one landed.
On its own, the demo picks the flights like a kiosk, and during a flight it switches between following the aircraft, showing the whole route, and showing the globe.
The flights are sped up, but the speed, altitude, temperature, and the time to go are those of a real flight.

Search for the airports by city, name, country, or code, and during a flight, Plan a flight returns to the planner.
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
  The ground is NASA's Blue Marble for July, 4096 pixels wide.
  `compress.rs` compresses it at startup to BC1 or ETC2, whichever the GPU supports, to an eighth of its size in GPU memory.
- `map.rs` computes a map with mip levels from the outlines in `data/` at startup.
  It holds the distances to the coasts and the borders, so that the shader draws them sharp at any zoom, and the lights of the cities.
- Beside the planner, the frame is transparent but for the globe and its atmosphere, so Slint's background shows through.
  The globe's place there shows the middle of a frame rendered for the whole window, so it grows to the window seamlessly, as space and the stars fade in.
- `adaptive.rs`, from the racer demo, keeps the frame rate near 60 fps: it lowers the globe's render scale first, then the quality.
  High quality blurs the globe behind the panels, medium and low show flat panels, and low also renders the globe at one pixel per logical pixel.
  The settings pick a quality, or Auto.
- The route, the aircraft, the airports, and the city names are ordinary Slint elements.
  Each frame, `main.rs` projects them to the screen with the camera's matrices, and passes their positions to Slint.

The map data comes from [Natural Earth](https://www.naturalearthdata.com/), the airports from [OurAirports](https://ourairports.com/), and the imagery from [NASA's Visible Earth](https://visibleearth.nasa.gov/), all in the public domain.
`data/convert.py` converts them into the files in `data/`.

Without a GPU, set `SLINT_WGPU_CPU=1` to render on the CPU, for example with Mesa's lavapipe.
