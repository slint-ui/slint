#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

"""Sorts the traces the app saved into the case folders under `cases/`.

Per capture, writes into its case folder:
`<name>.positions.csv`, `<name>.uikit-scroll.csv`, `<name>.events.csv`,
`<name>.touches.csv` for captures with touches, and a row in `captures.csv`.
Then regenerates `cases/from-rest.csv` and `cases/from-rest-uikit-scroll.csv`,
the single scroll-to captures from rest of all cases, normalized for fitting.
Times are seconds from the first scroll-to call; offsets are in points.
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
FROM_REST_CASES = (1, 2, 3)
SETTLED_PT = 0.5
UIKIT_COLOR, SLINT_COLOR = "#d62a2a", "#2a78d6"

POSITION_COLUMNS = [
    "seconds_from_command",
    "target_seconds_from_command",
    "slint_tick_seconds_from_command",
    "uikit_offset_pt",
    "uikit_presentation_offset_pt",
    "slint_offset_pt",
    "uikit_progress",
    "slint_progress",
]
UIKIT_SCROLL_COLUMNS = [
    "seconds_from_command",
    "uikit_offset_pt",
    "uikit_presentation_offset_pt",
    "uikit_progress",
]
EVENT_COLUMNS = [
    "event",
    "seconds_from_command",
    "slint_tick_seconds_from_command",
    "command_index",
    "target_offset_pt",
    "uikit_offset_pt",
    "slint_offset_pt",
    "uikit_velocity",
    "slint_velocity",
]
TOUCH_COLUMNS = [
    "event_type",
    "callback_seconds_from_command",
    "touch_seconds_from_release",
    "touch_index",
    "touch_phase",
    "coalesced_index",
    "x",
    "y",
    "pan_velocity_y",
    "uikit_content_y",
    "slint_content_y",
]
SUMMARY_COLUMNS = [
    "trial",
    "engine_commit",
    "device",
    "viewport_pt",
    "content_pt",
    "maximum_offset_pt",
    "commands",
    "start_offset_pt",
    "uikit_offset_at_command_pt",
    "slint_offset_at_command_pt",
    "uikit_velocity_at_command",
    "slint_velocity_at_command",
    "target_offset_pt",
    "uikit_t50_s",
    "uikit_t90_s",
    "uikit_t99_s",
    "slint_t50_s",
    "slint_t90_s",
    "slint_t99_s",
    "uikit_animation_end_s",
    "uikit_settle_s",
    "slint_settle_s",
    "uikit_final_offset_pt",
    "slint_final_offset_pt",
    "max_offset_difference_pt",
    "max_sample_gap_ms",
    "issues",
]
FROM_REST_COLUMNS = [
    "case",
    "file",
    "trial",
    "start_offset_pt",
    "target_offset_pt",
    "distance_pt",
    "seconds_from_command",
    "uikit_offset_pt",
    "slint_offset_pt",
    "uikit_progress",
    "slint_progress",
]


def read_csv(path):
    with path.open(newline="") as source:
        return list(csv.DictReader(source))


def write_csv(path, columns, rows):
    with path.open("w", newline="") as target:
        writer = csv.DictWriter(target, fieldnames=columns, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def number(text):
    try:
        return float(text)
    except (TypeError, ValueError):
        return math.nan


def fmt(value, digits=3):
    return "" if value is None or not math.isfinite(value) else f"{value:.{digits}f}"


def case_folders():
    return {
        int(folder.name[:2]): folder
        for folder in CASES.iterdir()
        if folder.is_dir() and folder.name[:2].isdigit()
    }


def progress(offset, start, target):
    return (offset - start) / (target - start) if target != start else math.nan


def first_time_reaching(times, values, level):
    return next((t for t, v in zip(times, values) if t >= 0 and v >= level), math.nan)


def settle_time(times, offsets, since):
    """The first time after `since` from which the offset stays within `SETTLED_PT` of its final value."""
    final = offsets[-1]
    outside = [t for t, o in zip(times, offsets) if t >= since and abs(o - final) > SETTLED_PT]
    if not outside:
        return since
    later = [t for t in times if t > outside[-1]]
    return later[0] if later else math.nan


def collect(scroll_path, folder, engine_commit, device):
    raw = scroll_path.parent
    scenario = scroll_path.stem.removeprefix("scroll-")
    name = SCENARIO.match(scenario)["name"]
    frames = read_csv(scroll_path)
    events = read_csv(raw / f"events-{scenario}.csv")
    geometry = json.loads((raw / f"geometry-{scenario}.json").read_text())
    input_path = raw / f"input-{scenario}.csv"
    inputs = read_csv(input_path) if input_path.exists() else []
    if not frames:
        raise ValueError(f"{scenario}: empty trace")

    issues = []
    commands = [e for e in events if e["event"] == "command"]
    if not commands:
        issues.append("no command")
    origin_event = (commands or [e for e in events if e["event"] == "release"] or [None])[0]
    origin = number(origin_event["callback_time"]) if origin_event else number(frames[0]["callback_time"])

    def seconds(text):
        return number(text) - origin

    def slint_tick(callback_time, lag_ms):
        return number(callback_time) - number(lag_ms) / 1000 - origin

    single = len(commands) == 1
    first = commands[0] if commands else None
    target = number(commands[-1]["target_offset"]) if commands else math.nan
    uikit_start = number(first["uikit_offset"]) if first else math.nan
    slint_start = number(first["slint_offset"]) if first else math.nan

    times = [seconds(r["callback_time"]) for r in frames]
    uikit = [number(r["uikit_offset"]) for r in frames]
    slint = [number(r["slint_offset"]) for r in frames]
    uikit_progress = [progress(o, uikit_start, target) if single else math.nan for o in uikit]
    slint_progress = [progress(o, slint_start, target) if single else math.nan for o in slint]
    write_csv(
        folder / f"{name}.positions.csv",
        POSITION_COLUMNS,
        [
            dict(
                seconds_from_command=fmt(t, 6),
                target_seconds_from_command=fmt(seconds(r["display_target_time"]), 6),
                slint_tick_seconds_from_command=fmt(
                    slint_tick(r["callback_time"], r["slint_clock_lag_ms"]), 6
                ),
                uikit_offset_pt=fmt(u),
                uikit_presentation_offset_pt=fmt(number(r["uikit_presentation_offset"])),
                slint_offset_pt=fmt(s),
                uikit_progress=fmt(up, 6),
                slint_progress=fmt(sp, 6),
            )
            for t, r, u, s, up, sp in zip(times, frames, uikit, slint, uikit_progress, slint_progress)
        ],
    )

    write_csv(
        folder / f"{name}.uikit-scroll.csv",
        UIKIT_SCROLL_COLUMNS,
        [
            dict(
                seconds_from_command=fmt(seconds(e["callback_time"]), 6),
                uikit_offset_pt=fmt(number(e["uikit_offset"])),
                uikit_presentation_offset_pt=fmt(number(e["uikit_presentation_offset"])),
                uikit_progress=fmt(
                    progress(number(e["uikit_offset"]), uikit_start, target) if single else math.nan, 6
                ),
            )
            for e in events
            if e["event"] == "uikit_did_scroll"
        ],
    )

    write_csv(
        folder / f"{name}.events.csv",
        EVENT_COLUMNS,
        [
            dict(
                event=e["event"],
                seconds_from_command=fmt(seconds(e["callback_time"]), 6),
                slint_tick_seconds_from_command=fmt(
                    slint_tick(e["callback_time"], e["slint_clock_lag_ms"]), 6
                ),
                command_index=e["command_index"] if e["event"] == "command" else "",
                target_offset_pt=fmt(number(e["target_offset"])),
                uikit_offset_pt=fmt(number(e["uikit_offset"])),
                slint_offset_pt=fmt(number(e["slint_offset"])),
                uikit_velocity=fmt(number(e["uikit_velocity"])),
                slint_velocity=fmt(number(e["slint_velocity"])),
            )
            for e in events
            if e["event"] != "uikit_did_scroll"
        ],
    )

    if inputs:
        releases = [r for r in inputs if r["event_type"] == "touch_callback" and r["touch_phase"] == "3"]
        touch_origin = number(releases[0]["touch_timestamp"]) if releases else math.nan
        touch_indices = {}
        write_csv(
            folder / f"{name}.touches.csv",
            TOUCH_COLUMNS,
            [
                dict(
                    event_type=r["event_type"],
                    callback_seconds_from_command=fmt(seconds(r["callback_time"]), 6),
                    touch_seconds_from_release=fmt(number(r["touch_timestamp"]) - touch_origin, 6),
                    touch_index=touch_indices.setdefault(r["touch_id"], len(touch_indices)),
                    touch_phase=r["touch_phase"],
                    coalesced_index=r["coalesced_index"],
                    x=r["x"],
                    y=r["y"],
                    pan_velocity_y=r["pan_velocity_y"],
                    uikit_content_y=r["uikit_content_y"],
                    slint_content_y=r["slint_content_y"],
                )
                for r in inputs
            ],
        )

    last_command = seconds(commands[-1]["callback_time"]) if commands else 0.0
    after = [i for i, t in enumerate(times) if t >= 0]
    gaps = [times[b] - times[a] for a, b in zip(after, after[1:])]
    animation_ends = [seconds(e["callback_time"]) for e in events if e["event"] == "uikit_animation_end"]
    if geometry["uikit_viewport"] != geometry["slint_viewport"]:
        issues.append("geometry mismatch")
    if not gaps or max(gaps) > 0.030:
        issues.append("sampling gap")
    tail = [i for i, t in enumerate(times) if t >= times[-1] - 0.2]
    for label, offsets in (("uikit", uikit), ("slint", slint)):
        values = [offsets[i] for i in tail]
        if max(values) - min(values) > 0.05:
            issues.append(f"{label} not at rest")
        if commands and abs(offsets[-1] - target) > SETTLED_PT:
            issues.append(f"{label} misses target")

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
        content_pt=geometry["uikit_content"][1],
        maximum_offset_pt=fmt(geometry["maximum_offset"]),
        commands=geometry["commands"],
        start_offset_pt=fmt(geometry["start_offset"]),
        uikit_offset_at_command_pt=fmt(uikit_start),
        slint_offset_at_command_pt=fmt(slint_start),
        uikit_velocity_at_command=fmt(number(first["uikit_velocity"]) if first else math.nan),
        slint_velocity_at_command=fmt(number(first["slint_velocity"]) if first else math.nan),
        target_offset_pt=fmt(target),
        **{
            f"{label}_t{level}_s": fmt(first_time_reaching(times, values, level / 100))
            for label, values in (("uikit", uikit_progress), ("slint", slint_progress))
            for level in (50, 90, 99)
        },
        uikit_animation_end_s=fmt(animation_ends[-1] if animation_ends else math.nan),
        uikit_settle_s=fmt(settle_time(times, uikit, last_command)),
        slint_settle_s=fmt(settle_time(times, slint, last_command)),
        uikit_final_offset_pt=fmt(uikit[-1]),
        slint_final_offset_pt=fmt(slint[-1]),
        max_offset_difference_pt=fmt(max(abs(s - u) for s, u in zip(slint, uikit))),
        max_sample_gap_ms=fmt(max(gaps) * 1000 if gaps else math.nan),
        issues=";".join(issues),
    )
    return summary, list(parameters)


def update_summary(folder, summaries, parameter_keys):
    path = folder / "captures.csv"
    rows = {r["file"]: r for r in read_csv(path)} if path.exists() else {}
    rows.update({s["file"]: s for s in summaries})
    columns = ["file"] + parameter_keys + SUMMARY_COLUMNS
    write_csv(path, columns, [rows[name] for name in sorted(rows)])


def write_from_rest(folders):
    """Concatenates the normalized single scroll-to captures from rest of all case folders."""
    rows, uikit_rows = [], []
    for number_, folder in sorted(folders.items()):
        summary = folder / "captures.csv"
        if number_ not in FROM_REST_CASES or not summary.exists():
            continue
        for capture in read_csv(summary):
            positions = folder / f"{capture['file']}.positions.csv"
            if not positions.exists() or "no command" in capture["issues"]:
                continue
            start = number(capture["uikit_offset_at_command_pt"])
            target = number(capture["target_offset_pt"])
            common = dict(
                case=folder.name,
                file=capture["file"],
                trial=capture["trial"],
                start_offset_pt=capture["uikit_offset_at_command_pt"],
                target_offset_pt=capture["target_offset_pt"],
                distance_pt=fmt(target - start),
            )
            for p in read_csv(positions):
                rows.append(
                    dict(
                        common,
                        seconds_from_command=p["seconds_from_command"],
                        uikit_offset_pt=p["uikit_offset_pt"],
                        slint_offset_pt=p["slint_offset_pt"],
                        uikit_progress=p["uikit_progress"],
                        slint_progress=p["slint_progress"],
                    )
                )
            scroll = folder / f"{capture['file']}.uikit-scroll.csv"
            for p in read_csv(scroll) if scroll.exists() else []:
                uikit_rows.append(
                    dict(
                        common,
                        seconds_from_command=p["seconds_from_command"],
                        uikit_offset_pt=p["uikit_offset_pt"],
                        uikit_progress=p["uikit_progress"],
                    )
                )
    write_csv(CASES / "from-rest.csv", FROM_REST_COLUMNS, rows)
    uikit_columns = [c for c in FROM_REST_COLUMNS if not c.startswith("slint")]
    write_csv(CASES / "from-rest-uikit-scroll.csv", uikit_columns, uikit_rows)
    return len(rows)


def plot(folder, names):
    try:
        import matplotlib
    except ImportError:
        print("matplotlib isn't installed, skipping the plots")
        return
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    plots = folder / "plots"
    plots.mkdir(exist_ok=True)

    def draw(ax, name):
        rows = read_csv(folder / f"{name}.positions.csv")
        t = [float(r["seconds_from_command"]) for r in rows]
        ax.plot(t, [float(r["uikit_offset_pt"]) for r in rows], color=UIKIT_COLOR, label="UIKit")
        ax.plot(t, [float(r["slint_offset_pt"]) for r in rows], color=SLINT_COLOR, label="Slint")
        replay = folder / f"{name}.positions.slint.csv"
        if replay.exists():
            rows = read_csv(replay)
            ax.plot(
                [float(r["seconds_from_command"]) for r in rows],
                [float(r["slint_offset_pt"]) for r in rows],
                color=SLINT_COLOR,
                linestyle="--",
                label="Slint replay",
            )
        for event in read_csv(folder / f"{name}.events.csv"):
            if event["event"] in ("command", "release"):
                ax.axvline(float(event["seconds_from_command"]), color="#888888", linewidth=0.7)
        ax.set_title(name, fontsize=9)
        ax.grid(alpha=0.2)

    for name in names:
        fig, ax = plt.subplots(figsize=(7, 4))
        draw(ax, name)
        ax.set_xlabel("Seconds from the first scroll-to")
        ax.set_ylabel("Content offset (pt)")
        ax.legend()
        fig.tight_layout()
        fig.savefig(plots / f"{name}.png", dpi=120)
        plt.close(fig)

    first_trials = sorted(
        p.name.removesuffix(".positions.csv") for p in folder.glob("*-trial1.positions.csv")
    )
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
    parser.add_argument("raw_dir", type=Path, nargs="?", help="copy of the app's Documents folder")
    parser.add_argument("--engine-commit", default="", help="the engine commit tested")
    parser.add_argument("--device", default="", help="device model and iOS build")
    parser.add_argument("--no-plots", action="store_true")
    parser.add_argument(
        "--plots-only", action="store_true", help="redraw the plots of all captures, e.g. after a replay"
    )
    args = parser.parse_args()

    folders = case_folders()
    if args.plots_only:
        for folder in folders.values():
            plot(folder, [p.name.removesuffix(".positions.csv") for p in folder.glob("*.positions.csv")])
        return
    if args.raw_dir is None:
        parser.error("raw_dir is required")

    collected = {}
    failures = 0
    for scroll_path in sorted(args.raw_dir.glob("scroll-case*.csv")):
        match = SCENARIO.match(scroll_path.stem.removeprefix("scroll-"))
        folder = folders.get(int(match["number"])) if match else None
        if folder is None:
            print(f"skipping {scroll_path.name}: no case folder")
            continue
        try:
            summary, keys = collect(scroll_path, folder, args.engine_commit, args.device)
        except (ValueError, KeyError, FileNotFoundError) as error:
            failures += 1
            print(f"skipping {scroll_path.name}: {error!r}")
            continue
        collected.setdefault(folder, (keys, []))[1].append(summary)
        print(f"{folder.name}/{summary['file']}: {summary['issues'] or 'ok'}")
    for folder, (keys, summaries) in collected.items():
        update_summary(folder, summaries, keys)
        if not args.no_plots:
            plot(folder, [s["file"] for s in summaries])
    if not collected:
        raise SystemExit(f"No case captures in {args.raw_dir}.")
    samples = write_from_rest(folders)
    print(f"{CASES / 'from-rest.csv'}: {samples} samples")
    if failures:
        raise SystemExit(f"{failures} captures couldn't be collected.")


if __name__ == "__main__":
    main()
