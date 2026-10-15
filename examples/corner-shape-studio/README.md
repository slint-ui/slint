# FORM Corner Studio

A frameless audio interface for exploring Slint's experimental corner shapes.
The architectural photograph was generated with OpenAI image generation.
The player artwork and waveform are decorative; the play control runs the shape showcase.

## Run

This example requires the `corner-shape` branch and the Skia renderer.
From the repository root, run:

```sh
sh examples/corner-shape-studio/run.sh
```

The launcher always builds and runs in release mode.
Set `CARGO_TARGET_DIR` to reuse an existing build directory.
Pass `--auto-reload` to reload the UI when its source changes.

## Controls

- Choose Round, Squircle, Bevel, Scoop, Notch, or Square.
  The window, buttons, and clipped photograph share the selected shape.
- Corner character explores the values between presets.
  Corner size changes the radius throughout the interface, including the window chrome.
- Make space opens a lower-left inset with a compact player.
  Inset shape changes that corner independently, from Notch through Scoop to Bevel.
  Its setting survives closing and reopening the inset.
- The play control cycles through the presets and the inset transition.
  Manual input stops the showcase.

Sliders update immediately.
Preset changes and opening or closing the inset animate.
A zero corner size disables shaping of the window outline.

Drag the custom title bar to move the window, or double-click it to maximize.
Its controls minimize, maximize, and close the window.
Corner shapes remain editable while maximized.
