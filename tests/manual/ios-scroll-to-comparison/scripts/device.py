#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

"""Prints `<UDID>\\t<model>, iOS <version> (<build>)` of the connected iPhone.

Uses `xcrun devicectl list devices`. Pass a UDID to select one of several devices.
"""

# cspell:ignore devicectl

import json
import subprocess
import sys
import tempfile
from pathlib import Path


def main():
    wanted = sys.argv[1] if len(sys.argv) > 1 else ""
    with tempfile.TemporaryDirectory() as directory:
        output = Path(directory) / "devices.json"
        subprocess.run(
            ["xcrun", "devicectl", "list", "devices", "--json-output", str(output)],
            check=True,
            stdout=subprocess.DEVNULL,
        )
        devices = json.loads(output.read_text())["result"]["devices"]

    phones = []
    for device in devices:
        hardware = device.get("hardwareProperties", {})
        properties = device.get("deviceProperties", {})
        connection = device.get("connectionProperties", {})
        if hardware.get("platform") != "iOS" or hardware.get("reality", "physical") != "physical":
            continue
        udid = hardware.get("udid", device.get("identifier", ""))
        if wanted and wanted not in (udid, device.get("identifier")):
            continue
        name = (
            f"{hardware.get('marketingName', hardware.get('productType', 'iPhone'))}, "
            f"iOS {properties.get('osVersionNumber', '?')} ({properties.get('osBuildUpdate', '?')})"
        )
        phones.append((connection.get("tunnelState") == "connected", udid, name))

    if not phones:
        raise SystemExit("No iOS device found. Connect and unlock the iPhone, then trust this Mac.")
    connected = [p for p in phones if p[0]] or phones
    if len(connected) > 1 and not wanted:
        listing = "\n".join(f"  {udid}  {name}" for _, udid, name in connected)
        raise SystemExit(f"Several devices found, pass one with --device:\n{listing}")
    _, udid, name = connected[0]
    print(f"{udid}\t{name}")


if __name__ == "__main__":
    main()
