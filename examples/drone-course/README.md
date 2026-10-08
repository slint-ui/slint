<!-- cSpell: ignore wasd -->

# Slint Drone Race

Race a drone against four rivals over three laps through the gates of an indoor racing hall, rendered with wgpu, with a touch-friendly Slint UI.
Every start generates a new course, and the fastest races by average speed make the leaderboard.
The demo starts with the autopilot racing the field; tap Fly to take over, and after a run it goes back to racing on its own.

Steer with the on-screen stick or WASD, and hold boost or Space to go faster.
Boost lasts three seconds on a full charge, which refills slowly and with every gate you pass.
With the Assist pilot, the default, the drone keeps to the course's height and is pulled gently along the course, and a missed gate costs two seconds instead of a turn back.
Press P for the autopilot, C to switch cameras, N for a new course, and Q to switch between high and low quality.

A small renderer draws the hall on Slint's wgpu device, and Slint shows each frame as an image.
It keeps to what small embedded GPUs handle well: one render pass per frame, and in low quality, lighting per vertex.

## Running the Demo

```sh
cargo run --release -p drone-course --manifest-path examples/Cargo.toml
```

Start in low quality with `--low`.

## Running in a Browser

The web build uses Slint's FemtoVG renderer on WebGPU.
Uncomment the `#wasm#` lines in `Cargo.toml`, then build and serve this directory:

```sh
wasm-pack build --release --target web
python3 -m http.server
```

Open <http://localhost:8000> in a browser with WebGPU.
