#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Generate flatpak-builder sources for building Skia from source with rust-skia.

Resolves the Skia revision pinned by the workspace's skia-bindings crate and
converts Skia's DEPS file into commit-pinned `type: git` flatpak sources. Add
the output file to your application module's `sources` and point the
SKIA_SOURCE_DIR environment variable at the checkout
(/run/build/<module-name>/deps/skia-src).

Run this from the repository root, alongside flatpak-cargo-generator.py, and
re-run it whenever Cargo.lock changes.
"""
import argparse
import json
import re
import subprocess
import sys
import urllib.request

parser = argparse.ArgumentParser()
parser.add_argument("-o", "--output", default="skia-sources.json")
parser.add_argument("--dest", default="deps/skia-src", help="checkout path relative to the module build directory")
args = parser.parse_args()

metadata = json.loads(
    subprocess.check_output(["cargo", "metadata", "--format-version", "1", "--locked"])
)
package = next(
    (p for p in metadata["packages"] if p["name"] == "skia-bindings"), None
) or sys.exit("skia-bindings is not a dependency; is the renderer-skia feature enabled?")
tag = package["metadata"]["skia"]
print(f"skia-bindings {package['version']}, skia fork tag {tag}", file=sys.stderr)

deps_url = f"https://raw.githubusercontent.com/rust-skia/skia/{tag}/DEPS"
ns = {}
ns["Var"] = lambda name: ns["vars"][name]
exec(urllib.request.urlopen(deps_url).read().decode(), ns)

sources = [
    {"type": "git", "url": "https://github.com/rust-skia/skia.git", "tag": tag, "dest": args.dest}
]
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
            "dest": f"{args.dest}/{path}",
            "disable-submodules": True,
        }
    )

with open(args.output, "w") as f:
    json.dump(sources, f, indent=4)
    f.write("\n")
print(f"wrote {args.output} ({len(sources)} sources)", file=sys.stderr)

# Skia records the gn revision its own CI uses; surface it so the gn module
# in the manifest can be kept in sync when the Skia pin moves.
fetch_gn_url = f"https://raw.githubusercontent.com/rust-skia/skia/{tag}/bin/fetch-gn"
gn_rev = re.search(r"rev = '([0-9a-f]{40})'", urllib.request.urlopen(fetch_gn_url).read().decode())
if gn_rev:
    print(f"skia pins gn revision {gn_rev.group(1)} -- use it for the gn module in your manifest", file=sys.stderr)
