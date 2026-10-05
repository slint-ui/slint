#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

"""Sorts the traces the app saved into the case folders under `cases/`.

Per capture, writes `<name>.positions.csv` and `<name>.touches.csv` into the case folder,
updates the case's `captures.csv`, and draws plots into its `plots/` folder.
Times are seconds from the first release; touch identifiers and device clocks stay out.
"""

# cspell:ignore axvline fontsize xlabel ylabel suptitle figsize squeeze

import argparse
import csv
import json
import math
import re
from pathlib import Path

CASES = Path(__file__).resolve().parents[1] / "cases"
SCENARIO = re.compile(r"case(?P<number>\d{2})-(?P<name>.+)$")
PARAMETER = re.compile(r"(?P<key>[a-z]+)(?P<value>\d+)$")
TOUCH_EVENTS = ("touch_callback", "coalesced_sample", "pan", "content_offset")
TOUCH_PHASE_ENDED = "3"
SETTLED_PT = 0.5
UIKIT_COLOR, SLINT_COLOR = "#2a78d6", "#eb6834"

POSITION_COLUMNS = [
    "seconds_from_release",
    "target_seconds_from_release",
    "uikit_offset_pt",
    "slint_offset_pt",
]
TOUCH_COLUMNS = [
    "event_type",
    "callback_seconds_from_release",
    "touch_seconds_from_release",
    "touch_index",
    "touch_phase",
    "coalesced_index",
    "x",
    "y",
    "pan_state",
    "pan_velocity_y",
    "uikit_content_y",
    "slint_content_y",
]
SUMMARY_COLUMNS = [
    "trial",
    "engine_commit",
    "device",
    "viewport_pt",
    "releases",
    "uikit_release_offset_pt",
    "slint_release_offset_pt",
    "release_pan_velocity_y",
    "finger_speed_last_25_ms",
    "uikit_final_offset_pt",
    "slint_final_offset_pt",
    "uikit_settle_s",
    "slint_settle_s",
    "max_sample_gap_ms",
    "issues",
]


def read_csv(path):
    with path.open(newline="") as source:
        return list(csv.DictReader(source))


def write_csv(path, columns, rows):
    with path.open("w", newline="") as target:
        writer = csv.DictWriter(target, fieldnames=columns, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def case_folders():
    return {
        int(folder.name[:2]): folder
        for folder in CASES.iterdir()
        if folder.is_dir() and folder.name[:2].isdigit()
    }


def finger_speed(samples, release_time, window=0.025):
    """The touch's speed over the last `window` seconds before `release_time`."""
    moving = [(t, y) for t, y in samples if t <= release_time]
    if len(moving) < 2:
        return 0.0
    end_t, end_y = moving[-1]
    start_t, start_y = ([s for s in moving if s[0] <= end_t - window] or moving[:1])[-1]
    return (end_y - start_y) / (end_t - start_t) if end_t > start_t else 0.0


def settle_time(times, offsets):
    """The first time after which the offset stays within `SETTLED_PT` of its final value."""
    final = offsets[-1]
    outside = [i for i, offset in enumerate(offsets) if abs(offset - final) > SETTLED_PT]
    if not outside:
        return 0.0
    index = outside[-1] + 1
    return times[index] if index < len(times) else math.nan


def collect(scroll_path, folder, engine_commit, device):
    raw = scroll_path.parent
    scenario = scroll_path.stem.removeprefix("scroll-")
    name = SCENARIO.match(scenario)["name"]
    inputs = read_csv(raw / f"input-{scenario}.csv")
    geometry = json.loads((raw / f"geometry-{scenario}.json").read_text())
    releases = [
        r
        for r in inputs
        if r["event_type"] == "touch_callback" and r["touch_phase"] == TOUCH_PHASE_ENDED
    ]
    if not releases:
        raise ValueError(f"{scenario}: no release in the input trace")
    release = releases[0]
    callback_origin = float(release["callback_time"])
    touch_origin = float(release["touch_timestamp"])

    frames = read_csv(scroll_path)
    times = [float(r["callback_time"]) - callback_origin for r in frames]
    uikit = [float(r["uikit_offset"]) for r in frames]
    slint = [float(r["slint_offset"]) for r in frames]
    write_csv(
        folder / f"{name}.positions.csv",
        POSITION_COLUMNS,
        [
            dict(
                seconds_from_release=f"{t:.6f}",
                target_seconds_from_release=(
                    f"{float(r['display_target_time']) - callback_origin:.6f}"
                ),
                uikit_offset_pt=f"{u:.3f}",
                slint_offset_pt=f"{s:.3f}",
            )
            for t, r, u, s in zip(times, frames, uikit, slint)
        ],
    )

    touch_indices = {}
    touches = []
    for row in inputs:
        if row["event_type"] not in TOUCH_EVENTS:
            continue
        touch_time = float(row["touch_timestamp"])
        is_touch = row["event_type"] in ("touch_callback", "coalesced_sample")
        index = touch_indices.setdefault(row["touch_id"], len(touch_indices)) if is_touch else ""
        touches.append(
            dict(
                event_type=row["event_type"],
                callback_seconds_from_release=(
                    f"{float(row['callback_time']) - callback_origin:.6f}"
                ),
                touch_seconds_from_release=(
                    f"{touch_time - touch_origin:.6f}" if math.isfinite(touch_time) else ""
                ),
                touch_index=index,
                **{k: row[k] for k in TOUCH_COLUMNS[4:]},
            )
        )
    write_csv(folder / f"{name}.touches.csv", TOUCH_COLUMNS, touches)

    released_touch = touch_indices.get(release["touch_id"])
    samples = sorted(
        (float(t["touch_seconds_from_release"]), float(t["y"]))
        for t in touches
        if t["event_type"] == "coalesced_sample" and t["touch_index"] == released_touch
    )
    after = [i for i, t in enumerate(times) if t >= 0]
    gaps = [times[b] - times[a] for a, b in zip(after, after[1:])]
    issues = []
    if geometry["uikit_viewport"] != geometry["slint_viewport"]:
        issues.append("geometry mismatch")
    if not gaps or max(gaps) > 0.030:
        issues.append("sampling gap")
    tail_uikit = [o for t, o in zip(times, uikit) if t >= times[-1] - 0.2]
    tail_slint = [o for t, o in zip(times, slint) if t >= times[-1] - 0.2]
    if max(tail_uikit) - min(tail_uikit) > 0.05 or max(tail_slint) - min(tail_slint) > 0.05:
        issues.append("not at rest")

    parameters = {}
    trial = ""
    for token in name.split("-"):
        match = PARAMETER.match(token)
        if match and match["key"] == "trial":
            trial = int(match["value"])
        elif match:
            parameters[match["key"]] = int(match["value"])
    summary = dict(
        file=name,
        **parameters,
        trial=trial,
        engine_commit=engine_commit,
        device=device,
        viewport_pt=geometry["uikit_viewport"][3],
        releases=len(releases),
        uikit_release_offset_pt=release["uikit_content_y"],
        slint_release_offset_pt=release["slint_content_y"],
        release_pan_velocity_y=release["pan_velocity_y"],
        finger_speed_last_25_ms=f"{finger_speed(samples, 0.0):.3f}",
        uikit_final_offset_pt=f"{uikit[-1]:.3f}",
        slint_final_offset_pt=f"{slint[-1]:.3f}",
        uikit_settle_s=f"{settle_time(times, uikit):.3f}",
        slint_settle_s=f"{settle_time(times, slint):.3f}",
        max_sample_gap_ms=f"{max(gaps) * 1000 if gaps else math.nan:.3f}",
        issues=";".join(issues),
    )
    return summary, list(parameters)


def update_summary(folder, summaries, parameter_keys):
    path = folder / "captures.csv"
    rows = {r["file"]: r for r in read_csv(path)} if path.exists() else {}
    rows.update({s["file"]: s for s in summaries})
    columns = ["file"] + parameter_keys + SUMMARY_COLUMNS
    write_csv(path, columns, [rows[name] for name in sorted(rows)])


def plot(folder, names):
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    plots = folder / "plots"
    plots.mkdir(exist_ok=True)

    def draw(ax, name):
        rows = read_csv(folder / f"{name}.positions.csv")
        t = [float(r["seconds_from_release"]) for r in rows]
        ax.plot(t, [float(r["uikit_offset_pt"]) for r in rows], color=UIKIT_COLOR, label="UIKit")
        ax.plot(t, [float(r["slint_offset_pt"]) for r in rows], color=SLINT_COLOR, label="Slint")
        ax.axvline(0, color="#888888", linewidth=0.7)
        ax.set_title(name, fontsize=9)
        ax.grid(alpha=0.2)

    for name in names:
        fig, ax = plt.subplots(figsize=(7, 4))
        draw(ax, name)
        ax.set_xlabel("Seconds from the first release")
        ax.set_ylabel("Content offset (pt)")
        ax.legend()
        fig.tight_layout()
        fig.savefig(plots / f"{name}.png", dpi=120)
        plt.close(fig)

    first_trials = sorted(p.name.removesuffix(".positions.csv")
                          for p in folder.glob("*-trial1.positions.csv"))
    if first_trials:
        columns = min(4, len(first_trials))
        rows = math.ceil(len(first_trials) / columns)
        fig, axes = plt.subplots(rows, columns, figsize=(4 * columns, 3 * rows), squeeze=False)
        for ax in axes.flat[len(first_trials):]:
            ax.set_visible(False)
        for ax, name in zip(axes.flat, first_trials):
            draw(ax, name)
        axes.flat[0].legend()
        fig.suptitle(f"{folder.name}, first trials", fontsize=10)
        fig.tight_layout()
        fig.savefig(plots / "overview.png", dpi=110)
        plt.close(fig)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("raw_dir", type=Path, help="copy of the app's Documents folder")
    parser.add_argument("--engine-commit", required=True, help="the engine commit tested")
    parser.add_argument("--device", required=True, help="device model and iOS build")
    parser.add_argument("--no-plots", action="store_true")
    args = parser.parse_args()

    folders = case_folders()
    collected = {}
    for scroll_path in sorted(args.raw_dir.glob("scroll-case*.csv")):
        match = SCENARIO.match(scroll_path.stem.removeprefix("scroll-"))
        folder = folders.get(int(match["number"])) if match else None
        if folder is None:
            print(f"skipping {scroll_path.name}: no case folder")
            continue
        summary, keys = collect(scroll_path, folder, args.engine_commit, args.device)
        collected.setdefault(folder, (keys, []))[1].append(summary)
        print(f"{folder.name}/{summary['file']}: {summary['issues'] or 'ok'}")
    for folder, (keys, summaries) in collected.items():
        update_summary(folder, summaries, keys)
        if not args.no_plots:
            plot(folder, [s["file"] for s in summaries])
    if not collected:
        raise SystemExit(f"No case captures in {args.raw_dir}.")


if __name__ == "__main__":
    main()
