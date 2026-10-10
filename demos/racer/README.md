<!-- cSpell: ignore wasd -->

# Slint Racer

Race four rivals over three laps of a track at dusk, rendered with wgpu underneath a Slint UI.
Every new track is generated: an oval or a figure eight whose upper stretch crosses the lower one on a bridge, with tunnels through hills, and boost pads on the road that speed up whoever drives over them.
The demo starts with the autopilot racing; press Race to take over.

Drive with W A S D or the arrow keys.
On a touch screen, steer with the slider, or in manual, with the stick, which also works the gas and the brake.
With Assist, the default, the car keeps its speed for the turns and steering pulls gently along the track; press H to drive on your own.
Press C for the bumper camera, P for the autopilot, N for a new track, R to restart, and Q to switch between low, medium, and high quality.
Esc opens the settings, which pause a race you drive yourself.

## Running the Demo

```sh
cargo run --release -p racer --manifest-path demos/Cargo.toml
```

Start in low or medium quality with `--low` or `--medium`, and on a fixed track with `RACER_SEED=4242`.
The web build starts in medium quality.

## Running in a Browser

The web build uses Slint's FemtoVG renderer on WebGPU.
Uncomment the `#wasm#` lines in `Cargo.toml`, then build and serve this directory:

```sh
wasm-pack build --release --target web
python3 -m http.server
```

Open <http://localhost:8000> in a browser with WebGPU.
