#!/usr/bin/env -S uv run --script
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
# /// script
# requires-python = ">=3.9"
# dependencies = []
# ///

"""Build the flashable Slint demo images for the website board catalog.

Covers `printerdemo_mcu` for every board in examples/mcu-board-support, plus the
`home-automation` demo for the ESP32-P4 Function EV board (ESP-IDF).

Output (in $OUT, default ./demo-images/):
  printerdemo-<feature>.{uf2,elf}  |  printerdemo-<feature>.bin (ESP)
  home-automation-esp32-p4.bin
  demos.json

The images + demos.json are meant to be hosted in the Slint website. demos.json is
what `slint.dev/flash` reads: it downloads `base + boards[<feature>].file`, verifies
`.sha256`, and flashes with the tool for `.method` (uf2 -> picotool / esp -> espflash
/ probe-rs).

Prerequisites (checked up front, before any build):
  - rustup + targets: thumbv6m-none-eabi thumbv7em-none-eabihf thumbv8m.main-none-eabihf
  - the `esp` Rust channel + `cargo +esp` (espup) for the ESP32-S3 boards
  - elf2uf2-rs >= 2.2.0, picotool, espflash    (MCU conversions)
  - a full ESP-IDF >= 6.0 env (idf.py) for the ESP32-P4 home-automation demo

Usage: scripts/build-demo-images.py [--only mcu|p4]
"""

# cSpell: ignore espcfg esptool flashable hexdigest levelname picotool uf2

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_DEMO = "printerdemo_mcu"

# The conversion tool each board kind needs to turn the ELF into a flashable image.
CONVERTERS = {
    "rp2040": "elf2uf2-rs",
    "rp2350": "picotool",
    "esp": "espflash",
    "stm": None,
}


@dataclass(frozen=True)
class Board:
    feature: str
    target: str
    kind: str  # rp2040 | rp2350 | stm | esp
    method: str  # uf2 | esp | probe-rs, the flashing tool slint.dev/flash uses
    name: str
    chip: str = ""
    esp_config: str = ""  # cargo --config for the ESP boards

    @property
    def image_name(self) -> str:
        suffix = {"rp2040": "uf2", "rp2350": "uf2", "stm": "elf", "esp": "bin"}[
            self.kind
        ]
        return f"printerdemo-{self.feature}.{suffix}"


BOARDS = [
    Board("pico-st7789", "thumbv6m-none-eabi", "rp2040", "uf2", "Raspberry Pi Pico"),
    Board(
        "pico2-st7789",
        "thumbv8m.main-none-eabihf",
        "rp2350",
        "uf2",
        "Raspberry Pi Pico 2",
    ),
    Board(
        "pico2-touch-lcd-2-8",
        "thumbv8m.main-none-eabihf",
        "rp2350",
        "uf2",
        "Pico 2 Touch LCD 2.8",
    ),
    Board(
        "stm32h735g",
        "thumbv7em-none-eabihf",
        "stm",
        "probe-rs",
        "STM32H735G-DK",
        chip="STM32H735IGKx",
    ),
    Board(
        "stm32u5g9j-dk2",
        "thumbv8m.main-none-eabihf",
        "stm",
        "probe-rs",
        "STM32U5G9J-DK2",
        chip="STM32U5G9ZJTxQ",
    ),
    Board(
        "esp32-s3-box-3",
        "xtensa-esp32s3-none-elf",
        "esp",
        "esp",
        "ESP32-S3-BOX-3",
        chip="esp32s3",
        esp_config="examples/mcu-board-support/esp32_s3_box_3/cargo-config.toml",
    ),
    Board(
        "esp32-s3-lcd-ev-board",
        "xtensa-esp32s3-none-elf",
        "esp",
        "esp",
        "ESP32-S3-LCD-EV-Board",
        chip="esp32s3",
        esp_config="examples/mcu-board-support/esp32_s3_lcd_ev_board/cargo-config.toml",
    ),
    Board(
        "esope-sld-c-w-s3",
        "xtensa-esp32s3-none-elf",
        "esp",
        "esp",
        "ESoPe SLD-C-W-S3",
        chip="esp32s3",
        esp_config="examples/mcu-board-support/esope_sld_c_w_s3/cargo-config.toml",
    ),
    Board(
        "waveshare-esp32-s3-touch-amoled-1-8",
        "xtensa-esp32s3-none-elf",
        "esp",
        "esp",
        "Waveshare ESP32-S3 Touch AMOLED 1.8",
        chip="esp32s3",
        esp_config="examples/mcu-board-support/waveshare_esp32_s3_touch_amoled_1_8/cargo-config.toml",
    ),
    Board(
        "m5stack-cores3",
        "xtensa-esp32s3-none-elf",
        "esp",
        "esp",
        "M5Stack CoreS3",
        chip="esp32s3",
        esp_config="examples/mcu-board-support/m5stack_cores3/cargo-config.toml",
    ),
]


def log(msg: str) -> None:
    print(f"==> {msg}", flush=True)


def warn(msg: str) -> None:
    print(f"warning: {msg}", file=sys.stderr, flush=True)


def run(
    cmd: list[str], cwd: Path | None = None, env: dict[str, str] | None = None
) -> None:
    subprocess.run(cmd, cwd=cwd or ROOT, env=env, check=True)


def build_env(**extra: str) -> dict[str, str]:
    """Match the README flash build: opt-level=s, no SLINT_FONT_SIZES, no slint_int_coord."""
    env = os.environ.copy()
    env["CARGO_PROFILE_RELEASE_OPT_LEVEL"] = "s"
    env["CARGO_PROFILE_DEV_DEBUG"] = "0"
    env.pop("SLINT_FONT_SIZES", None)
    env.pop("RUSTFLAGS", None)
    env.update(extra)
    return env


def idf_env() -> dict[str, str]:
    """Environment for the ESP-IDF tools, with uv's virtualenv taken back out of PATH.

    This script's shebang runs it under `uv run`, which prepends its own ephemeral
    venv. idf.py and esptool are `#!/usr/bin/env python3` scripts that need ESP-IDF's
    virtualenv, and would otherwise fail with "No module named 'click'".
    """
    env = os.environ.copy()
    venv = env.pop("VIRTUAL_ENV", None)
    if venv:
        uv_bin = str(Path(venv) / "bin")
        env["PATH"] = os.pathsep.join(
            entry for entry in env["PATH"].split(os.pathsep) if entry != uv_bin
        )
    return env


def check_tools(boards: list[Board]) -> None:
    """Abort before any build if a conversion tool the selected boards need is missing.

    The shell version warned here and then died mid-run, after minutes of cargo builds.
    """
    needed: dict[str, list[str]] = {}
    for board in boards:
        tool = CONVERTERS[board.kind]
        if tool and not shutil.which(tool):
            needed.setdefault(tool, []).append(board.feature)
    if needed:
        for tool, features in needed.items():
            warn(f"{tool} not found, needed by: {', '.join(features)}")
        sys.exit("aborting: install the missing tools, or narrow the run with --only")


def build_board(board: Board, out: Path) -> None:
    log(f"printerdemo_mcu: {board.feature} ({board.kind} / {board.target})")
    cargo = ["cargo"]
    env = build_env()
    if board.kind == "esp":
        cargo += ["+esp"]
    elif board.kind == "stm":
        # STM keeps Slint's textures in external OSPI flash (.slint_assets); otherwise
        # rodata overflows the internal flash. Only the stm32* boards define it.
        env = build_env(SLINT_ASSET_SECTION=".slint_assets")
    cargo += [
        "build",
        "--manifest-path",
        "demos/Cargo.toml",
        "-p",
        "printerdemo_mcu",
        "--target",
        board.target,
        "--no-default-features",
        f"--features=mcu-board-support/{board.feature}",
        "--release",
    ]
    if board.kind == "esp":
        cargo += ["--config", board.esp_config]
    run(cargo, env=env)

    # All the repo's workspaces share <repo>/target via .cargo/config.toml.
    elf = ROOT / "target" / board.target / "release" / "printerdemo_mcu"
    image = out / board.image_name
    if board.kind == "rp2040":
        run(["elf2uf2-rs", str(elf), str(image)])
    elif board.kind == "rp2350":
        run(["picotool", "uf2", "convert", "-t", "elf", str(elf), str(image)])
    elif board.kind == "stm":
        shutil.copyfile(elf, image)
    elif board.kind == "esp":
        run(
            [
                "espflash",
                "save-image",
                "--merge",
                "--skip-padding",
                "--chip",
                board.chip,
                "--flash-size",
                "16mb",
                str(elf),
                str(image),
            ]
        )


def build_mcu(out: Path) -> tuple[list[dict], list[str]]:
    """Build every board, keeping going past a failure so one bad board doesn't
    discard the images that did build."""
    records, failed = [], []
    for board in BOARDS:
        try:
            build_board(board, out)
        except subprocess.CalledProcessError as e:
            warn(
                f"{board.feature} failed: {' '.join(e.cmd)} exited with {e.returncode}"
            )
            failed.append(board.feature)
            continue
        records.append(
            {
                "feature": board.feature,
                "name": board.name,
                "method": board.method,
                "file": board.image_name,
                "chip": board.chip,
                "offset": "0x0" if board.method == "esp" else "",
                "demo": DEFAULT_DEMO,
            }
        )
    return records, failed


def build_p4(out: Path) -> tuple[list[dict], list[str]]:
    directory = ROOT / "demos/home-automation/esp-idf"
    image = "home-automation-esp32-p4.bin"
    if not shutil.which("idf.py"):
        # ESP-IDF is an optional prerequisite: a plain run still produces the MCU images.
        warn(
            "idf.py not found, skipping ESP32-P4 home-automation (needs an ESP-IDF >= 6.0 env)"
        )
        return [], []
    log("home-automation: ESP32-P4 (ESP-IDF)")
    env = idf_env()
    try:
        run(["idf.py", "set-target", "esp32p4"], cwd=directory, env=env)
        run(["idf.py", "build"], cwd=directory, env=env)
        # Merge the built artifacts directly (idf.py merge-bin re-runs the build and trips
        # the esp Rust toolchain; build/flash_args has the offsets).
        run(
            [
                "python",
                "-m",
                "esptool",
                "--chip",
                "esp32p4",
                "merge_bin",
                "-o",
                str(out / image),
                "-f",
                "raw",
                "@flash_args",
            ],
            cwd=directory / "build",
            env=env,
        )
    except subprocess.CalledProcessError as e:
        warn(
            f"home-automation ESP32-P4 failed: {' '.join(e.cmd)} exited with {e.returncode}"
        )
        return [], ["esp32-p4-function-ev-board"]
    return [
        {
            "feature": "esp32-p4-function-ev-board",
            "name": "ESP32-P4 Function EV",
            "method": "esp",
            "file": image,
            "chip": "esp32p4",
            "offset": "0x0",
            "demo": "home-automation",
        }
    ], []


def emit_demos_json(out: Path, base: str, records: list[dict]) -> None:
    log(f"writing {out / 'demos.json'}")
    boards = {}
    for record in records:
        digest = hashlib.sha256((out / record["file"]).read_bytes()).hexdigest()
        entry = {
            "name": record["name"],
            "method": record["method"],
            "file": record["file"],
            "sha256": digest,
        }
        if (
            record["demo"] != DEFAULT_DEMO
        ):  # per-board demo only when it differs from the default
            entry["demo"] = record["demo"]
        if (
            record["chip"] and record["method"] == "probe-rs"
        ):  # probe-rs only; espflash auto-detects
            entry["chip"] = record["chip"]
        if record["offset"]:  # esp: flash offset for the merged bin
            entry["offset"] = record["offset"]
        boards[record["feature"]] = entry
    doc = {
        "demo": DEFAULT_DEMO,
        "base": base.rstrip("/"),
        "generated": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "boards": boards,
    }
    (out / "demos.json").write_text(json.dumps(doc, indent=2) + "\n")
    print(json.dumps(doc, indent=2))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--only",
        choices=["mcu", "p4"],
        help="build just the MCU boards or just the ESP32-P4 demo",
    )
    args = parser.parse_args()

    out = Path(os.environ.get("OUT", ROOT / "demo-images")).resolve()
    # URL prefix the flasher prepends to each file; override to match where the
    # website serves the committed images.
    base = os.environ.get("BASE", "https://slint.dev/demos")

    with_mcu, with_p4 = args.only != "p4", args.only != "mcu"
    if args.only == "p4" and not shutil.which("idf.py"):
        sys.exit("aborting: --only p4 needs idf.py from a full ESP-IDF >= 6.0 env")
    check_tools(BOARDS if with_mcu else [])

    shutil.rmtree(out, ignore_errors=True)
    out.mkdir(parents=True)

    records: list[dict] = []
    failed: list[str] = []
    for enabled, build in ((with_mcu, build_mcu), (with_p4, build_p4)):
        if enabled:
            built, errors = build(out)
            records += built
            failed += errors

    emit_demos_json(out, base, records)
    if failed:
        warn(
            f"demos.json is incomplete, these boards failed to build: {', '.join(failed)}"
        )
        return 1
    log(f"done, images + demos.json in {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
