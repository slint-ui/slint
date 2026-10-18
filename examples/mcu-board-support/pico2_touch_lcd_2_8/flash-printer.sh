#!/bin/bash
set -euo pipefail

if (( $# != 0 )) && { (( $# != 2 )) || [[ "$1" != --uf2 && "$1" != --previous ]]; }; then
    echo "Usage: $0 [--uf2 output.uf2 | --previous previous.elf]" >&2
    exit 2
fi

if [[ "${1:-}" == --previous && "${PICO_PROFILE:-0}" != 1 && "${PICO_RENDERER_PROFILE:-0}" != 1 ]]; then
    echo 'Guarded updates require PICO_PROFILE=1 or PICO_RENDERER_PROFILE=1 for flash verification.' >&2
    exit 2
fi

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$repo_root"
export CARGO_TARGET_THUMBV8M_MAIN_NONE_EABIHF_RUSTFLAGS="${CARGO_TARGET_THUMBV8M_MAIN_NONE_EABIHF_RUSTFLAGS:-} -C target-cpu=cortex-m33"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repo_root/target}"

features=pico2
if [[ "${PICO_CLOCK_240:-0}" == 1 ]]; then
    features+=,mcu-board-support/pico2-clock-240
else
    export CARGO_PROFILE_RELEASE_LTO="${CARGO_PROFILE_RELEASE_LTO:-thin}"
    export CARGO_PROFILE_RELEASE_CODEGEN_UNITS="${CARGO_PROFILE_RELEASE_CODEGEN_UNITS:-1}"
fi
if [[ "${PICO_PROFILE:-0}" == 1 ]]; then
    features+=,pico2-queue-profile
fi
if [[ "${PICO_RENDERER_PROFILE:-0}" == 1 ]]; then
    features+=,pico2-queue-profile,mcu-board-support/pico2-renderer-profile
fi

cargo build --locked --manifest-path demos/printerdemo_mcu/Cargo.toml \
    --no-default-features --features "$features" \
    --target thumbv8m.main-none-eabihf --release

elf="$CARGO_TARGET_DIR/thumbv8m.main-none-eabihf/release/printerdemo_mcu"
if [[ "${1:-}" == --uf2 ]]; then
    picotool uf2 convert "$elf" -t elf "$2" --platform rp2350 --family rp2350-arm-s --abs-block
    exit
fi

selection=()
if [[ -n "${PICO_SERIAL:-}" ]]; then
    selection=(--ser "$PICO_SERIAL")
fi
if [[ "${1:-}" == --previous ]]; then
    : "${PICO_SERIAL:?Set PICO_SERIAL for a guarded update}"
    llvm_bin="${PICO_LLVM_BIN:-$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/^host: //p')/bin}"
    "${PYTHON:-python3}" examples/mcu-board-support/pico2_touch_lcd_2_8/flash-delta.py \
        --previous "$2" --elf "$elf" --output "${elf}.delta.uf2" \
        --objdump "$llvm_bin/llvm-objdump" --objcopy "$llvm_bin/llvm-objcopy" \
        --serial "$PICO_SERIAL" --apply
    exit
fi
picotool load "$elf" \
    -t elf -u -v -x -f "${selection[@]}"
