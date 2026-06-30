# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT
#
# Autostart the Slint demo as the Weston "kiosk" client, so the board boots
# straight into the demo with no shell or desktop.

FILESEXTRAPATHS:prepend := "${THISDIR}/files:"

do_install:append() {
    # Launch the demo when Weston comes up.
    install -d ${D}${sysconfdir}/xdg/weston
    cat >> ${D}${sysconfdir}/xdg/weston/weston.ini <<'EOF'

[core]
idle-time=0

[autolaunch]
path=/usr/bin/slint-demo
watch=true
EOF
}
