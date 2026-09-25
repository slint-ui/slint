#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Turn the Skia revision that skia-bindings pins into flatpak-builder sources.

Usage: flatpak-skia-generator.py [-o skia-sources.json] [--dest deps/skia-src]
"""

import json
import re
import subprocess
import sys
import urllib.request

flags = dict(zip(sys.argv[1::2], sys.argv[2::2]))
output = flags.get("-o", "skia-sources.json")
dest = flags.get("--dest", "deps/skia-src")


def fetch(url):
    return urllib.request.urlopen(url).read().decode()


cargo = ["cargo", "metadata", "--format-version", "1", "--locked"]
packages = json.loads(subprocess.check_output(cargo))["packages"]
bindings = [p for p in packages if p["name"] == "skia-bindings"]
if not bindings:
    sys.exit("no skia-bindings; is the renderer-skia feature enabled?")
tag = bindings[0]["metadata"]["skia"]
print(f"skia-bindings {bindings[0]['version']}, skia fork tag {tag}", file=sys.stderr)

raw = f"https://raw.githubusercontent.com/rust-skia/skia/{tag}"

# Skia's DEPS is Python, listing every checkout its own build expects
ns = {"Var": lambda name: ns["vars"][name]}
exec(fetch(f"{raw}/DEPS"), ns)  # noqa: S102 -- evaluating DEPS is the point

fork = "https://github.com/rust-skia/skia.git"
sources = [{"type": "git", "url": fork, "tag": tag, "dest": dest}]
for path, spec in sorted(ns["deps"].items()):
    if not isinstance(spec, str) or "emsdk" in path:
        continue  # cipd packages and the wasm-only emsdk are not needed
    url, _, commit = spec.partition("@")
    assert commit, f"DEPS entry {path} has no pinned commit: {spec}"
    sources.append(
        {
            "type": "git",
            "url": url,
            "commit": commit,
            "dest": f"{dest}/{path}",
            "disable-submodules": True,
        }
    )

with open(output, "w") as f:
    json.dump(sources, f, indent=4)
    f.write("\n")
print(f"wrote {output} ({len(sources)} sources)", file=sys.stderr)

# Skia records the gn revision its own CI builds with; print it so the gn
# module in the manifest can follow along when the Skia pin moves
gn = re.search(r"rev = '(\w{40})'", fetch(f"{raw}/bin/fetch-gn"))
if gn:
    print(f"skia pins gn revision {gn.group(1)}", file=sys.stderr)
