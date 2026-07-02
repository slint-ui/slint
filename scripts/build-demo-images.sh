#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
#
# Build the flashable Slint demo images for the website board catalog and emit a
# demos.json index. Covers `printerdemo_mcu` for every board in
# examples/mcu-board-support, plus the `home-automation` demo for the ESP32-P4
# Function EV board (ESP-IDF).
#
# Output (in $OUT, default ./demo-images/):
#   printerdemo-<feature>.{uf2,elf}  |  printerdemo-<feature>.bin (ESP)
#   home-automation-esp32-p4.bin
#   demos.json
#
# The images + demos.json are meant to be hosted in the Slint website. demos.json
# is what `slint.dev/flash` reads: it downloads
# `base + boards[<feature>].file`, verifies `.sha256`, and flashes with the tool for
# `.method` (uf2→picotool / esp→espflash / probe-rs).
#
# Prerequisites (the script checks and reports what's missing):
#   - rustup + targets: thumbv6m-none-eabi thumbv7em-none-eabihf thumbv8m.main-none-eabihf
#   - the `esp` Rust channel + `cargo +esp` (espup) for the ESP32-S3 boards
#   - elf2uf2-rs >= 2.2.0, picotool, espflash    (MCU conversions)
#   - a full ESP-IDF 5.4 env (idf.py) for the ESP32-P4 home-automation demo
#
# Usage: scripts/build-demo-images.sh [--only mcu|p4] [--skip-p4]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
OUT="${OUT:-$ROOT/demo-images}"
# URL prefix the flasher prepends to each file; override to match where the
# website serves the committed images.
BASE="${BASE:-https://slint.dev/demos}"

ONLY=""
for a in "$@"; do
  case "$a" in
    --only) shift; ONLY="${1:-}";;
    --only=*) ONLY="${a#*=}";;
    --skip-p4) ONLY="mcu";;
  esac
done

# Match the README flash build: opt-level=s, no SLINT_FONT_SIZES, no slint_int_coord.
export CARGO_PROFILE_RELEASE_OPT_LEVEL=s CARGO_PROFILE_DEV_DEBUG=0
unset SLINT_FONT_SIZES RUSTFLAGS 2>/dev/null || true

rm -rf "$OUT"; mkdir -p "$OUT"
: > "$OUT/.index.tsv"   # feature \t name \t method \t file \t chip \t offset \t demo

# cSpell: ignore espcfg esptool flashable fname hashlib hexdigest rstrip utcnow
log()  { printf '==> %s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }
have() { command -v "$1" >/dev/null 2>&1; }

# feature | target | kind(rp2040|rp2350|stm|esp) | method(uf2|esp|probe-rs) | chip | esp cargo-config | name
BOARDS='
pico-st7789|thumbv6m-none-eabi|rp2040|uf2|||Raspberry Pi Pico
pico2-st7789|thumbv8m.main-none-eabihf|rp2350|uf2|||Raspberry Pi Pico 2
pico2-touch-lcd-2-8|thumbv8m.main-none-eabihf|rp2350|uf2|||Pico 2 Touch LCD 2.8
stm32h735g|thumbv7em-none-eabihf|stm|probe-rs|STM32H735IGKx||STM32H735G-DK
stm32u5g9j-dk2|thumbv8m.main-none-eabihf|stm|probe-rs|STM32U5G9ZJTxQ||STM32U5G9J-DK2
esp32-s3-box-3|xtensa-esp32s3-none-elf|esp|esp|esp32s3|examples/mcu-board-support/esp32_s3_box_3/cargo-config.toml|ESP32-S3-BOX-3
esp32-s3-lcd-ev-board|xtensa-esp32s3-none-elf|esp|esp|esp32s3|examples/mcu-board-support/esp32_s3_lcd_ev_board/cargo-config.toml|ESP32-S3-LCD-EV-Board
esope-sld-c-w-s3|xtensa-esp32s3-none-elf|esp|esp|esp32s3|examples/mcu-board-support/esope_sld_c_w_s3/cargo-config.toml|ESoPe SLD-C-W-S3
waveshare-esp32-s3-touch-amoled-1-8|xtensa-esp32s3-none-elf|esp|esp|esp32s3|examples/mcu-board-support/waveshare_esp32_s3_touch_amoled_1_8/cargo-config.toml|Waveshare ESP32-S3 Touch AMOLED 1.8
m5stack-cores3|xtensa-esp32s3-none-elf|esp|esp|esp32s3|examples/mcu-board-support/m5stack_cores3/cargo-config.toml|M5Stack CoreS3
'

record() { # feature name method file chip offset demo
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "$4" "$5" "$6" "$7" >> "$OUT/.index.tsv"
}

build_mcu() {
  have elf2uf2-rs || warn "elf2uf2-rs not found (needed for RP2040 .uf2)"
  have picotool   || warn "picotool not found (needed for RP2350 .uf2)"
  have espflash   || warn "espflash not found (needed for ESP .bin)"
  while IFS='|' read -r feature target kind method chip espcfg name; do
    [ -z "$feature" ] && continue
    log "printerdemo_mcu: $feature ($kind / $target)"
    if [ "$kind" = "esp" ]; then
      cargo +esp build -p printerdemo_mcu --target "$target" --no-default-features \
        --features="mcu-board-support/$feature" --config "$espcfg" --release
    elif [ "$kind" = "stm" ]; then
      # STM keeps Slint's textures in external OSPI flash (.slint_assets); otherwise
      # rodata overflows the internal flash. Only the stm32* boards define it.
      SLINT_ASSET_SECTION=.slint_assets cargo build -p printerdemo_mcu --target "$target" \
        --no-default-features --features="mcu-board-support/$feature" --release
    else
      cargo build -p printerdemo_mcu --target "$target" --no-default-features \
        --features="mcu-board-support/$feature" --release
    fi
    elf="target/$target/release/printerdemo_mcu"
    case "$kind" in
      rp2040) f="printerdemo-$feature.uf2"; elf2uf2-rs "$elf" "$OUT/$f" ;;
      rp2350) f="printerdemo-$feature.uf2"; picotool uf2 convert -t elf "$elf" "$OUT/$f" ;;
      stm)    f="printerdemo-$feature.elf"; cp "$elf" "$OUT/$f" ;;
      esp)    f="printerdemo-$feature.bin"
              espflash save-image --merge --skip-padding --chip "$chip" --flash-size 16mb "$elf" "$OUT/$f" ;;
    esac
    [ "$method" = "esp" ] && offset="0x0" || offset=""
    record "$feature" "$name" "$method" "$f" "$chip" "$offset" "printerdemo_mcu"
  done <<< "$BOARDS"
}

build_p4() {
  local dir="demos/home-automation/esp-idf"
  have idf.py || { warn "idf.py not found — skipping ESP32-P4 home-automation (needs an ESP-IDF 5.4 env)"; return; }
  log "home-automation: ESP32-P4 (ESP-IDF)"
  ( cd "$dir"
    idf.py set-target esp32p4
    idf.py build
    # Merge the built artifacts directly (idf.py merge-bin re-runs the build and trips
    # the esp Rust toolchain; build/flash_args has the offsets).
    ( cd build && python -m esptool --chip esp32p4 merge_bin -o "$OUT/home-automation-esp32-p4.bin" -f raw @flash_args )
  )
  record "esp32-p4-function-ev-board" "ESP32-P4 Function EV" "esp" \
    "home-automation-esp32-p4.bin" "esp32p4" "0x0" "home-automation"
}

emit_demos_json() {
  log "writing $OUT/demos.json"
  OUT="$OUT" BASE="$BASE" python3 - <<'PY'
import hashlib, json, os
out = os.environ["OUT"]; base = os.environ["BASE"].rstrip("/")
boards = {}
demo = "printerdemo_mcu"
with open(os.path.join(out, ".index.tsv")) as fh:
    for line in fh:
        line = line.rstrip("\n")
        if not line:
            continue
        feature, name, method, fname, chip, offset, d = (line.split("\t") + [""] * 7)[:7]
        path = os.path.join(out, fname)
        with open(path, "rb") as f:
            sha = hashlib.sha256(f.read()).hexdigest()
        entry = {"name": name, "method": method, "file": fname, "sha256": sha}
        if d and d != demo:            # per-board demo only when it differs from the default
            entry["demo"] = d
        if chip and method == "probe-rs":  # chip is read only by probe-rs; espflash auto-detects
            entry["chip"] = chip
        if offset:                     # esp: flash offset for the merged bin
            entry["offset"] = offset
        boards[feature] = entry
doc = {"demo": demo, "base": base, "generated": __import__("datetime").datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ"), "boards": boards}
with open(os.path.join(out, "demos.json"), "w") as f:
    json.dump(doc, f, indent=2)
    f.write("\n")
print(json.dumps(doc, indent=2))
PY
  rm -f "$OUT/.index.tsv"
}

[ "$ONLY" = "p4" ] || build_mcu
[ "$ONLY" = "mcu" ] || build_p4
emit_demos_json
log "done — images + demos.json in $OUT"
