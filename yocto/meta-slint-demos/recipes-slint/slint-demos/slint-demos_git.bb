# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT
#
# Builds a Slint demo as a native Linux executable for the image. On an MPU/SBC
# the demo runs on Wayland (winit backend) under Weston, or fullscreen on DRM/KMS
# (set SLINT_BACKEND=linuxkms in the autostart unit for a no-compositor kiosk).
#
# STATUS: skeleton. The one real gap is the Rust crate dependency list — Yocto
# builds offline, so every crate must appear in SRC_URI. Generate it once with
# `cargo bitbake` (meta-rust-bin / cargo-bitbake) and paste the `SRC_URI += "crate://..."`
# block below, OR switch to cargo vendoring (CARGO_VENDORING_DIRECTORY). Until then
# this recipe will not fetch its dependencies.

SUMMARY = "Slint demo application (printerdemo) for embedded Linux"
HOMEPAGE = "https://slint.dev"
LICENSE = "MIT"
# TODO: set to the real md5 of LICENSES/MIT.txt in the fetched source
# (`md5sum` it after the first do_fetch); the value below is a placeholder.
LIC_FILES_CHKSUM = "file://LICENSES/MIT.txt;md5=0000000000000000000000000000000000"

inherit cargo cargo-update-recipe-crates pkgconfig

# Pin to the Slint release that matches the rest of the demo pipeline.
SLINT_VERSION ?= "1.17.1"
SRCREV ?= "v${SLINT_VERSION}"
SRC_URI = "git://github.com/slint-ui/slint.git;protocol=https;branch=master"

# Build just the printerdemo package with the embedded-Linux backends.
CARGO_SRC_DIR = ""
CARGO_BUILD_FLAGS += "-p printerdemo --no-default-features \
    --features slint/backend-linuxkms-noseat,slint/backend-winit,slint/renderer-femtovg,slint/renderer-software"

DEPENDS += "virtual/libgles2 virtual/egl wayland libxkbcommon fontconfig freetype"
RDEPENDS:${PN} += "fontconfig-utils ttf-dejavu-sans"

S = "${WORKDIR}/git"

# TODO(cargo-bitbake): generated crate list goes here, e.g.
# require slint-demos-crates.inc
# SRC_URI += "crate://crates.io/anyhow/1.0.0 ..."

do_install() {
    install -d ${D}${bindir}
    install -m 0755 ${B}/target/${CARGO_TARGET_SUBDIR}/printerdemo ${D}${bindir}/slint-demo
}

FILES:${PN} = "${bindir}/slint-demo"
