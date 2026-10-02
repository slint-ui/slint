# Slint + Bevy Robot Arm

This demo shows a robot cell rendered with [Bevy](https://bevyengine.org), controlled through a touch-friendly Slint UI.

In auto mode, the arm moves a box between two fixtures.
Touch a joint control or a pose to take over in manual mode.
Drag the 3D view to orbit and pinch to zoom.

The demo reuses `slint_bevy_adapter.rs` from the [`slint-hosts-bevy`](../slint-hosts-bevy) example.

## Running the Demo

On a desktop system, run:

```sh
cargo run --release -p bevy-robot-arm --manifest-path examples/Cargo.toml
```

On an embedded Linux device, use Slint's `linuxkms` backend with a Vulkan driver:

```sh
SLINT_BACKEND=linuxkms-skia ./bevy_robot_arm
```
