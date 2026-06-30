# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT
#
# Builds a small standalone Slint app (DRM/KMS + software renderer — no GL) and
# installs it as /usr/bin/slint-demo. The app depends on the published `slint`
# crate (crates.io), so it builds with the Yocto cargo class; the crate list lives
# in slint-demo-app-crates.inc (generated from the app's Cargo.lock — regenerate
# with the script in that file's header after changing dependencies).

SUMMARY = "Slint demo application for embedded Linux"
HOMEPAGE = "https://slint.dev"
LICENSE = "MIT"
LIC_FILES_CHKSUM = "file://${COMMON_LICENSE_DIR}/MIT;md5=0835ade6a2d2b4831dec71e9f5d3c2e2"

inherit cargo

SRC_URI = "file://slint-demo-app"
require slint-demo-app-crates.inc

S = "${WORKDIR}/slint-demo-app"

# slint-build runs the Slint compiler (a build-time codegen step) and the app uses
# fontconfig + a default font at runtime.
DEPENDS += "fontconfig"
RDEPENDS:${PN} += "fontconfig ttf-dejavu-sans"

do_install() {
    install -d ${D}${bindir}
    install -m 0755 ${B}/target/${CARGO_TARGET_SUBDIR}/slint-demo-app ${D}${bindir}/slint-demo
}

FILES:${PN} = "${bindir}/slint-demo"
