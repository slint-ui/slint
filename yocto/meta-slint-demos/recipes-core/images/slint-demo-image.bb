# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT
#
# A minimal Wayland/Weston image that boots straight into a Slint demo — the
# "flash an SD card and it just runs" counterpart to the per-board firmware in
# the nightly demo-binaries pipeline, but for MPU-class (embedded Linux) SoCs.

SUMMARY = "Slint demo image (Weston + a Slint demo, autostarted)"
LICENSE = "MIT"

inherit core-image

# Weston gives the demo a Wayland compositor; the demo autostarts via the
# weston-init bbappend in this layer. For a no-compositor DRM/KMS kiosk, drop
# weston here and start the demo with SLINT_BACKEND=linuxkms instead.
CORE_IMAGE_EXTRA_INSTALL += " \
    slint-demos \
    weston weston-init \
    "

# Keep the image small; this is a single-purpose appliance image.
IMAGE_FEATURES += "splash"
IMAGE_INSTALL:append = " kernel-modules"

# Build a flashable SD-card image (.wic.xz) in addition to the rootfs tarball.
IMAGE_FSTYPES += "wic.xz wic.bmap"
