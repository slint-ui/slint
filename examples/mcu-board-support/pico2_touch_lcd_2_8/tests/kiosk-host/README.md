# Kiosk UI Check

Run the generated printer UI with the software renderer and a deterministic host clock:

```sh
cargo run --manifest-path examples/mcu-board-support/pico2_touch_lcd_2_8/tests/kiosk-host/Cargo.toml --release
```

The check clicks the real checkbox and exercises its Rust controller and Slint touch overlay.
It verifies the ten-second page sequence, whole-pixel motion, immediate exit on press, stopped timers, and no click-through.
It renders at native 320 × 240 without a desktop window.
It does not qualify MCU throughput, physical touch latency, or optical scan coherence.

Set `KIOSK_CAPTURE_DIR` to write optional PPM snapshots outside the repository.
The local Cargo lockfile and build outputs are ignored.
