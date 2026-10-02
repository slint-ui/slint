#!/usr/bin/env python3
"""Compare fixed spring coefficients and paired, release-aligned position traces."""

# cspell:ignore interp axvline xlim ylim fontsize xlabel ylabel suptitle
import argparse
import csv
import json
import re
import subprocess
from collections import defaultdict
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

COMMIT = "af6e8e7c357f7eec5e894cfaac8390c04fd104e3"
CASES = [
    (774, 50, 400, 400),
    (774, 200, 400, 400),
    (774, 600, 400, 400),
    (387, 500, 400, 400),
    (774, 100, 400, 0),
    (774, 100, 1200, 0),
]
RED, BLUE = "#c53b32", "#1976b4"


def read_csv(path):
    with path.open() as handle:
        return list(csv.DictReader(handle))


def coefficients(source):
    text = source.read_text()

    def number(name):
        return float(
            re.search(rf"const {name}: f32 = ([0-9_.]+);", text)[1].replace("_", "")
        )

    delay = re.search(
        r"RETURN_DELAY: Duration = Duration::from_millis\(([0-9_]+)\)", text
    )
    return dict(
        frequency=number("RETURN_FREQUENCY"),
        rate_min=number("RETURN_RATE_MIN"),
        rate_rise=number("RETURN_RATE_RISE"),
        half_distance=number("RETURN_RATE_HALF_DISTANCE"),
        delay_s=int(delay[1].replace("_", "")) / 1e3,
    )


def predict(t, exposure, params):
    rate = params["rate_min"] + params["rate_rise"] * exposure**2 / (
        params["half_distance"] ** 2 + exposure**2
    )
    s = np.maximum(0, t - params["delay_s"])
    return (
        exposure
        * (1 + (params["frequency"] - rate) * s)
        * np.exp(-params["frequency"] * s)
    )


def error_metrics(error):
    return dict(
        rms_pt=float(np.sqrt(np.mean(error**2))),
        max_gap_pt=float(np.max(abs(error))),
        within_0_5_pt=bool(np.max(abs(error)) <= 0.5),
    )


def replay(native_csv, params):
    groups = defaultdict(list)
    for row in read_csv(native_csv):
        groups[row["scenario"]].append(row)
    results = []
    for name, rows in sorted(groups.items()):
        callback = np.array([float(r["time"]) for r in rows])
        target = np.array([float(r["target_time"]) for r in rows])
        y = np.array([float(r["uikit_exposure"]) for r in rows])
        x0 = float(np.interp(0, callback, y))
        mask = callback >= 0
        hold, distance = int(rows[0]["hold_ms"]), int(rows[0]["distance"])
        split = (
            (
                "held_fit_distances"
                if distance in [50, 100, 200, 300, 600, 700]
                else "held_validation_distances"
            )
            if hold
            else "moving_releases"
        )
        clocks = {}
        for clock, times in [("callback", callback), ("display_target", target)]:
            clocks[clock] = error_metrics(predict(times[mask], x0, params) - y[mask])
        if not hold:
            continue
        results.append(
            dict(
                scenario=name,
                split=split,
                starting_exposure_pt=x0,
                median_target_lead_ms=float(np.median(target - callback) * 1000),
                **clocks,
            )
        )
    return results, groups


def settle_time(t, y):
    outside = np.where((t >= 0) & (abs(y) > 0.5))[0]
    if not len(outside):
        return 0.0
    idx = outside[-1] + 1
    return float(t[idx]) if idx < len(t) else None


def first_inward_time(t, exposure, starting_exposure):
    indices = np.where((t >= 0) & (exposure < starting_exposure - 0.5))[0]
    return float(t[indices[0]]) if len(indices) else None


def paired(raw, evidence):
    metrics, traces, public_rows = [], {}, []
    for vp, distance, speed, hold in CASES:
        for trial in (1, 2):
            name = f"latest-af6e8e-vp{vp}-d{distance}-v{speed}-hold{hold}-trial{trial}"
            events = [
                r
                for r in read_csv(raw / f"input-{name}.csv")
                if r["event_type"] == "touch_callback"
            ]
            frames = read_csv(raw / f"scroll-{name}.csv")
            geometry = json.loads((raw / f"geometry-{name}.json").read_text())
            presses = [r for r in events if r["touch_phase"] == "0"]
            releases = [r for r in events if r["touch_phase"] == "3"]
            if len(presses) != 1 or len(releases) != 1:
                raise ValueError(f"{name}: incomplete touch sequence")
            press, release = presses[0], releases[0]
            origin = float(release["callback_time"])
            samples = sorted(
                {float(r["touch_timestamp"]): float(r["y"]) for r in events}.items()
            )
            changed = [
                samples[i][0]
                for i in range(1, len(samples))
                if abs(samples[i][1] - samples[i - 1][1]) > 0.01
            ]
            actual_hold = (
                (float(release["touch_timestamp"]) - changed[-1]) * 1000
                if changed
                else None
            )
            t = np.array([float(r["callback_time"]) - origin for r in frames])
            target = np.array(
                [float(r["display_target_time"]) - origin for r in frames]
            )
            native = -np.array([float(r["uikit_offset"]) for r in frames])
            slint = -np.array([float(r["slint_offset"]) for r in frames])
            post = (t >= 0) & (t <= 1.5)
            early = (t >= 0) & (t <= 0.8)
            gaps = np.diff(t[post])
            issues = []
            if abs(float(release["press_dy"]) - distance) > 0.5:
                issues.append("distance delivery")
            if (
                actual_hold is None
                or (hold and actual_hold < 350)
                or (not hold and actual_hold > 30)
            ):
                issues.append("hold delivery")
            if (
                geometry["uikit_viewport"] != geometry["slint_viewport"]
                or geometry["uikit_content"] != geometry["slint_content"]
            ):
                issues.append("geometry mismatch")
            if (
                max(
                    abs(float(press["uikit_content_y"])),
                    abs(float(press["slint_content_y"])),
                )
                > 0.5
            ):
                issues.append("initial offset")
            if not len(gaps) or max(gaps) > 0.030:
                issues.append("sampling gap")
            if max(abs(native[-1]), abs(slint[-1])) > 0.5:
                issues.append("did not settle")
            n0, s0 = (
                -float(release["uikit_content_y"]),
                -float(release["slint_content_y"]),
            )
            metrics.append(
                dict(
                    scenario=name,
                    hold_ms=hold,
                    requested_viewport=vp,
                    actual_viewport=geometry["uikit_viewport"][3],
                    requested_distance_pt=distance,
                    requested_speed_setting=speed,
                    trial=trial,
                    delivered_distance_pt=float(release["press_dy"]),
                    contact_ms=(
                        float(release["touch_timestamp"])
                        - float(press["touch_timestamp"])
                    )
                    * 1000,
                    delivered_hold_ms=actual_hold,
                    release_pan_velocity_y=float(release["pan_velocity_y"]),
                    uikit_release_exposure_pt=n0,
                    slint_release_exposure_pt=s0,
                    release_gap_pt=s0 - n0,
                    uikit_first_inward_0_5_pt_s=first_inward_time(t, native, n0)
                    if hold
                    else None,
                    slint_first_inward_0_5_pt_s=first_inward_time(t, slint, s0)
                    if hold
                    else None,
                    uikit_extra_exposure_pt=float(max(native[post]) - n0),
                    slint_extra_exposure_pt=float(max(slint[post]) - s0),
                    uikit_settle_s=settle_time(t, native),
                    slint_settle_s=settle_time(t, slint),
                    median_sampling_ms=float(np.median(gaps) * 1000),
                    max_sample_gap_ms=float(max(gaps) * 1000),
                    median_target_lead_ms=float(
                        np.median(target[post] - t[post]) * 1000
                    ),
                    post_release=error_metrics(slint[post] - native[post]),
                    first_0_8_s=error_metrics(slint[early] - native[early]),
                    issues=issues,
                )
            )
            traces[name] = (t, native, slint)
            for time, future, n, s in zip(t, target, native, slint):
                public_rows.append(
                    dict(
                        scenario=name,
                        callback_seconds_from_release=f"{time:.9f}",
                        target_seconds_from_release=f"{future:.9f}",
                        uikit_offset_pt=f"{-n:.3f}",
                        slint_offset_pt=f"{-s:.3f}",
                    )
                )
    with (evidence / "paired-positions.csv").open("w") as handle:
        writer = csv.DictWriter(
            handle, fieldnames=list(public_rows[0]), lineterminator="\n"
        )
        writer.writeheader()
        writer.writerows(public_rows)
    return metrics, traces


def comparison_plot(metrics, traces, held, destination):
    selected = [r for r in metrics if bool(r["hold_ms"]) == held]
    fig, axes = plt.subplots(
        len(selected) // 2, 2, figsize=(12, 3 * (len(selected) // 2)), squeeze=False
    )
    for ax, result in zip(axes.flat, selected):
        t, native, slint = traces[result["scenario"]]
        ax.plot(t, native, color=RED, label="UIKit measured")
        ax.plot(t, slint, color=BLUE, label="Slint measured (af6e8e7c35)")
        ax.axvline(0, color="#777777", linewidth=0.7)
        ax.set_xlim(-0.05, 0.85)
        ax.set_ylim(bottom=-2)
        ax.set_title(
            f"{result['requested_distance_pt']} pt pull · viewport {result['actual_viewport']} pt · trial {result['trial']}\n"
            f"speed setting {result['requested_speed_setting']} · max gap {result['post_release']['max_gap_pt']:.2f} pt",
            fontsize=10,
        )
        ax.set_xlabel("Seconds from delivered release callback")
        ax.set_ylabel("Top exposure (pt) = −content offset")
        ax.grid(alpha=0.2)
        ax.legend(fontsize=8)
    fig.suptitle(
        "Held release (400 ms stop)" if held else "Release while moving", fontsize=16
    )
    fig.tight_layout(rect=(0, 0, 1, 0.975))
    fig.savefig(destination, dpi=160)
    plt.close(fig)


def clock_plot(groups, params, destination):
    name = "return-vp774-d200-speed400-hold400-trial-1"
    rows = groups[name]
    y = np.array([float(r["uikit_exposure"]) for r in rows])
    callback = np.array([float(r["time"]) for r in rows])
    x0 = float(np.interp(0, callback, y))
    fig, axes = plt.subplots(1, 2, figsize=(12, 4))
    for ax, key, label in zip(
        axes,
        ["time", "target_time"],
        ["Position-read callback clock", "Recorded future display-target clock"],
    ):
        t = np.array([float(r[key]) for r in rows])
        ax.plot(t, y, color=RED, label="UIKit recorded position")
        ax.plot(
            t,
            predict(t, x0, params),
            color=BLUE,
            label="Offline source-formula prediction",
        )
        ax.set_xlim(0, 0.35)
        ax.set_title(label)
        ax.set_xlabel("Seconds from delivered release callback")
        ax.set_ylabel("Top exposure (pt)")
        ax.grid(alpha=0.2)
        ax.legend(fontsize=8)
    fig.suptitle(
        "Fixed af6e8e source coefficients · same native trace · no fitted time shift"
    )
    fig.tight_layout()
    fig.savefig(destination, dpi=160)
    plt.close(fig)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--raw-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    evidence, figures = args.output_dir / "evidence", args.output_dir / "figures"
    evidence.mkdir(parents=True, exist_ok=True)
    figures.mkdir(parents=True, exist_ok=True)
    params = coefficients(
        args.source_root / "internal/core/animations/simulations/scroll_spring.rs"
    )
    source_path = "internal/core/animations/simulations/scroll_spring.rs"
    archived = subprocess.check_output(
        ["git", "show", f"{COMMIT}:{source_path}"], cwd=args.source_root
    )
    if archived != (args.source_root / source_path).read_bytes():
        raise SystemExit("The spring source differs from the measured engine commit.")
    curves, groups = replay(evidence / "source-return-curves.csv", params)
    metrics, traces = paired(args.raw_dir, evidence)
    replay_summary = {}
    for split in sorted({r["split"] for r in curves}):
        cell = [r for r in curves if r["split"] == split]
        replay_summary[split] = {
            clock: dict(
                count=len(cell),
                mean_curve_rms_pt=float(np.mean([r[clock]["rms_pt"] for r in cell])),
                worst_gap_pt=max(r[clock]["max_gap_pt"] for r in cell),
                within_0_5_pt=sum(r[clock]["within_0_5_pt"] for r in cell),
            )
            for clock in ["callback", "display_target"]
        }
    summary = dict(
        tested_engine_commit=COMMIT,
        replay_assumption="Held native pulls only, zero release velocity",
        coefficients=params,
        error_window="paired: 0–1.5 seconds; replay: every recorded post-release sample",
        replay_summary=replay_summary,
        replay_curves=curves,
        paired_traces=metrics,
    )
    (evidence / "latest-validation.json").write_text(
        json.dumps(summary, indent=2) + "\n"
    )
    comparison_plot(metrics, traces, True, figures / "latest-held-returns.png")
    comparison_plot(metrics, traces, False, figures / "latest-moving-returns.png")
    clock_plot(groups, params, figures / "source-clock-comparison.png")
    for r in metrics:
        print(
            r["scenario"],
            "issues",
            r["issues"],
            "gap",
            round(r["post_release"]["max_gap_pt"], 3),
            "release",
            round(r["release_gap_pt"], 3),
            "settle",
            r["uikit_settle_s"],
            r["slint_settle_s"],
        )
    print(json.dumps(replay_summary, indent=2))
    if any(r["issues"] for r in metrics):
        raise SystemExit(
            "Delivery qualification failed; retain and inspect the affected traces."
        )


if __name__ == "__main__":
    main()
