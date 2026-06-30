<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Slint demo images for MPU boards (Yocto)

**Task 2** of the demo-binaries roadmap. The firmware pipeline (nightly
`demo_binaries*`) covers **MCU** boards — small no-OS chips you *flash*. This
covers **MPU / embedded-Linux SoCs** (Raspberry Pi, i.MX8, …) where the demo runs
on Linux: here we build a **bootable SD-card image** with Yocto that boots
straight into a Slint demo.

> **Status: foundation / RFC scaffold — not yet built end-to-end.** The metadata
> here is structured and reviewable, but a full Yocto build needs hours + ~50 GB
> and has **not been run**. The one substantive gap is the Rust crate list for the
> demo recipe (see below). Treat this as the starting point, not a turnkey build.

## Why separate from the firmware PR

| | MCU firmware (nightly `demo_binaries`) | MPU images (this) |
|---|---|---|
| Artifact | `.uf2` / `.bin` / `.elf` you flash | bootable `.wic.xz` SD image |
| Runtime | bare metal, no OS | Linux (Weston/Wayland or DRM/KMS) |
| Build | `cargo` / ESP-IDF, minutes | Yocto/bitbake, hours |
| Hosting | www-releases | a **separate images site** (large files) |

These are deliberately decoupled: different build systems, sizes, cadence, and
hosting. This also conceptually supersedes the lightweight
`demo_binaries_linux` track (a cross-compiled `printerdemo` you download and run)
— that stays as a quick "run on any ARM Linux" option; the Yocto images are the
"flash a whole board" story.

## Layout

```
yocto/
  kas/
    base.yml               # shared: poky + meta-openembedded, distro, sstate mirrors
    raspberrypi4-64.yml    # first MPU target (RPi 4); add siblings per board
  meta-slint-demos/        # the Yocto layer
    conf/layer.conf
    recipes-slint/slint-demos/      # builds a Slint demo as a Linux binary
    recipes-core/images/            # slint-demo-image.bb (Weston + the demo)
    recipes-graphics/weston/        # autostart the demo as the kiosk client
.github/workflows/yocto-images.yaml # manual kas build → .wic.xz artifact
```

## Decisions (settled)

1. **kas** for declarative, CI-friendly builds (`kas build yocto/kas/<machine>.yml`).
2. **Yocto Wrynose (6.0 LTS, Apr 2026)** — the release the maintainer named; supported
   until 2030. Scarthgap (5.0 LTS) is the safe fallback if a layer lags.
3. **Raspberry Pi 4** as the first target — the most accessible MPU with mature
   Yocto/Wayland support. More boards = more `kas/<machine>.yml` files (e.g. i.MX8
   via `meta-freescale`).
4. **Shared-state + source mirrors** (`sstate.yoctoproject.org`, the YP source mirror)
   so CI pulls prebuilt artifacts instead of compiling the world — the maintainer's
   "yocto provided sstate cache" point. Wired in `kas/base.yml`.
5. **Weston kiosk**: the image autostarts `/usr/bin/slint-demo`. For a no-compositor
   DRM/KMS kiosk, drop Weston and start the demo with `SLINT_BACKEND=linuxkms`.

## Open items before this builds green

1. **Crate list (the real gap).** Yocto builds offline, so every Rust crate the demo
   pulls must be in the recipe's `SRC_URI`. Generate it once with **`cargo bitbake`**
   and paste the `crate://…` block into `slint-demos_git.bb` (or switch to cargo
   vendoring). Until then the recipe won't fetch dependencies.
2. **Which demo(s).** Currently `printerdemo` (std). Could instead/also ship
   `home-automation` / `usecases` / a gallery.
3. **First real build.** Run `Yocto MPU images` (workflow_dispatch) on a large or
   self-hosted runner; expect to iterate on layer revs, `MACHINE_FEATURES`, and the
   GLES/EGL providers for the Pi's vc4 stack.
4. **Images site.** Decide the "separate site" (e.g. `images.slint.dev` bucket or a
   `www-images` repo) and wire the workflow's `publish` step, mirroring how
   `publish_artifacts` pushes firmware demos to www-releases.
5. **Repo home.** This can live here under `yocto/`, or be extracted to a dedicated
   `meta-slint-demos` repo if the board matrix grows.

## Local build (once the crate list is in)

```sh
pipx install kas
kas build yocto/kas/raspberrypi4-64.yml
# → tmp/deploy/images/raspberrypi4-64/slint-demo-image-*.wic.xz  (flash with bmaptool)
```
