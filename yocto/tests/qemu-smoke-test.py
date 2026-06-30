#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT
#
# Boot the built slint-demo-image in QEMU and confirm it actually starts the Slint
# demo — the "is this the right image with the demo running?" test, no hardware needed.
#
# Usage (after `kas build yocto/kas/qemuarm64.yml`):
#   KAS_BUILD_DIR=<build> python3 yocto/tests/qemu-smoke-test.py
#
# Drives `runqemu qemuarm64 nographic` over the serial console: waits for boot,
# then asserts Weston is up and the slint-demo process is running. Rendering itself
# is verified separately (a framebuffer screenshot / on real hardware); this is the
# automated, headless smoke gate.

import os
import sys
import pexpect

BUILD_DIR = os.environ.get("KAS_BUILD_DIR", "build")
IMAGE = os.environ.get("IMAGE", "slint-demo-image")
MACHINE = os.environ.get("MACHINE", "qemuarm64")
TIMEOUT = int(os.environ.get("BOOT_TIMEOUT", "300"))


def main() -> int:
    # `runqemu` lives in the build env; the workflow sources oe-init-build-env / kas shell first.
    cmd = f"runqemu {MACHINE} {IMAGE} nographic slirp"
    print(f"+ {cmd}", flush=True)
    child = pexpect.spawn(cmd, timeout=TIMEOUT, encoding="utf-8", logfile=sys.stdout)

    # 1. Reach a usable shell (serial-autologin-root gets us straight to a prompt).
    child.expect([r"login:", r"# "], timeout=TIMEOUT)
    if child.match.group(0).strip() == "login:":
        child.sendline("root")
        child.expect(r"# ", timeout=60)

    def run(line, expect_ok=True):
        child.sendline(f"{line}; echo RC=$?")
        child.expect(r"RC=(\d+)", timeout=60)
        rc = int(child.match.group(1))
        if expect_ok and rc != 0:
            raise SystemExit(f"FAIL: `{line}` exited {rc}")
        child.expect(r"# ", timeout=30)
        return rc

    # 2. The Slint demo binary is installed.
    run("test -x /usr/bin/slint-demo")
    # 3. Weston (the compositor the demo renders on) is active.
    run("systemctl is-active weston || (sleep 5; systemctl is-active weston)")
    # 4. The demo process is actually running (autostarted by weston-init).
    run(
        "for i in $(seq 1 20); do pgrep -x slint-demo && break; sleep 1; done; pgrep -x slint-demo"
    )

    print(
        "\nSMOKE TEST PASSED: slint-demo is running on Weston in the image.", flush=True
    )
    child.sendline("poweroff")
    child.expect(pexpect.EOF, timeout=120)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (pexpect.TIMEOUT, pexpect.EOF) as e:
        print(
            f"\nSMOKE TEST FAILED: {type(e).__name__} — the image did not reach a running demo.",
            flush=True,
        )
        sys.exit(1)
