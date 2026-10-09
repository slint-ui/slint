#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT
# cspell:ignore devicectl xcodegen xcresult

# Builds the comparison app, runs the capture tests on the connected iPhone, copies the traces
# from the phone, and writes the CSV files into `cases/`. See README.md.

set -euo pipefail

usage() {
    cat <<USAGE
Usage: scripts/run.sh --team TEAM_ID [options]

  --team ID          Apple development team ID (or set DEVELOPMENT_TEAM)
  --device UDID      iPhone to use, if several are connected (or set DEVICE_UDID)
  --only TEST        Run only this test method, e.g. testCase01ScrollDownFromRest; repeatable
  --bundle-prefix P  Bundle identifier prefix, default dev.slint (or set BUNDLE_ID_PREFIX)
  --collect-only DIR Skip building and testing; collect the traces in DIR/Documents again
  --no-replay        Don't replay the captures through Slint on this Mac
  --no-fit           Don't fit the models
  --no-plots         Don't draw plots
USAGE
}

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEAM="${DEVELOPMENT_TEAM:-}"
DEVICE="${DEVICE_UDID:-}"
BUNDLE_ID_PREFIX="${BUNDLE_ID_PREFIX:-dev.slint}"
ONLY=()
COLLECT_ONLY=""
REPLAY=1
FIT=1
PLOTS=()

while [ $# -gt 0 ]; do
    case "$1" in
        --team) TEAM="$2"; shift 2 ;;
        --device) DEVICE="$2"; shift 2 ;;
        --only) ONLY+=("-only-testing:ScrollToComparisonUITests/ScrollToCaseTests/$2"); shift 2 ;;
        --bundle-prefix) BUNDLE_ID_PREFIX="$2"; shift 2 ;;
        --collect-only) COLLECT_ONLY="$2"; shift 2 ;;
        --no-replay) REPLAY=0; shift ;;
        --no-fit) FIT=0; shift ;;
        --no-plots) PLOTS=(--no-plots); shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Unknown option $1" >&2; usage >&2; exit 2 ;;
    esac
done

cd "$HERE"
ENGINE_COMMIT="$(git rev-parse HEAD)$(git diff --quiet HEAD -- ../../../internal ../../../api || echo '-dirty')"

if [ -n "$COLLECT_ONLY" ]; then
    RUN_DIR="$COLLECT_ONLY"
    DEVICE_NAME="$(cat "$RUN_DIR/device.txt" 2>/dev/null || echo unknown)"
    ENGINE_COMMIT="$(cat "$RUN_DIR/engine-commit.txt" 2>/dev/null || echo "$ENGINE_COMMIT")"
else
    if [ -z "$TEAM" ]; then
        echo "Pass the Apple development team ID with --team or DEVELOPMENT_TEAM." >&2
        exit 2
    fi
    for tool in xcodegen xcodebuild xcrun cargo rustup python3; do
        command -v "$tool" >/dev/null || { echo "$tool is missing, see README.md." >&2; exit 2; }
    done
    rustup target list --installed | grep -qx aarch64-apple-ios \
        || { echo "Run: rustup target add aarch64-apple-ios" >&2; exit 2; }

    DEVICE_LINE="$(python3 scripts/device.py $DEVICE)"
    DEVICE="${DEVICE_LINE%%$'\t'*}"
    DEVICE_NAME="${DEVICE_LINE#*$'\t'}"
    BUNDLE_ID="$BUNDLE_ID_PREFIX.scroll-to-comparison"
    RUN_DIR="raw/$(date +%Y-%m-%d-%H%M%S)"
    mkdir -p "$RUN_DIR"
    echo "$DEVICE_NAME" > "$RUN_DIR/device.txt"
    echo "$ENGINE_COMMIT" > "$RUN_DIR/engine-commit.txt"
    echo "Device: $DEVICE_NAME ($DEVICE)"
    echo "Engine: $ENGINE_COMMIT"
    echo "Output: $HERE/$RUN_DIR"

    xcodegen generate

    # Old traces in the app's container would mix into this run.
    xcrun devicectl device uninstall app --device "$DEVICE" "$BUNDLE_ID" >/dev/null 2>&1 || true

    set +e
    xcodebuild test \
        -project ScrollToComparison.xcodeproj -scheme ScrollToComparison \
        -configuration Release -destination "platform=iOS,id=$DEVICE" \
        -allowProvisioningUpdates -parallel-testing-enabled NO \
        -resultBundlePath "$RUN_DIR/Tests.xcresult" \
        DEVELOPMENT_TEAM="$TEAM" BUNDLE_ID_PREFIX="$BUNDLE_ID_PREFIX" \
        "${ONLY[@]:--only-testing:ScrollToComparisonUITests/ScrollToCaseTests}" \
        2>&1 | tee "$RUN_DIR/xcodebuild.log" | grep --line-buffered -E "^(Test Case|Testing|\*\*|error:)"
    TEST_STATUS="${PIPESTATUS[0]}"
    set -e

    mkdir -p "$RUN_DIR/Documents"

    xcrun devicectl device copy from --device "$DEVICE" \
        --domain-type appDataContainer --domain-identifier "$BUNDLE_ID" \
        --source Documents --destination "$RUN_DIR/Documents"
fi

python3 scripts/collect.py "$RUN_DIR/Documents" --engine-commit "$ENGINE_COMMIT" \
    --device "$DEVICE_NAME" ${PLOTS[@]+"${PLOTS[@]}"}

if [ "$REPLAY" = 1 ]; then
    (cd replay && cargo test --release -- --nocapture)
    if [ ${#PLOTS[@]} -eq 0 ]; then
        python3 scripts/collect.py --plots-only
    fi
fi

if [ "$FIT" = 1 ]; then
    python3 scripts/fit.py || echo "Fitting failed; the CSV files in cases/ are complete." >&2
fi

if [ "${TEST_STATUS:-0}" != 0 ]; then
    echo "xcodebuild test failed ($TEST_STATUS), see $RUN_DIR/xcodebuild.log. Collected what the phone saved." >&2
    exit "$TEST_STATUS"
fi
