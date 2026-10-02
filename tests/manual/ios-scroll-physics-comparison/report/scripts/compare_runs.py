#!/usr/bin/env python3
"""Regenerate the measured release-velocity comparison from published evidence."""

# cspell:ignore af6e8e fontsize axvline xlim ylim xlabel ylabel suptitle
import csv
import json
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

ROOT = Path(__file__).resolve().parents[1]
EVIDENCE = ROOT / "evidence"


def read_json(name):
    return json.loads((EVIDENCE / name).read_text())


def read_positions(name):
    with (EVIDENCE / name).open() as source:
        return list(csv.DictReader(source))


def main():
    before = read_json("previous-validation.json")["paired_traces"]
    after = read_json("latest-validation.json")["paired_traces"]
    lookup = {r["scenario"].replace("latest-af6e8e", "latest-69f715"): r for r in after}
    comparisons = []
    for old in before:
        new = lookup[old["scenario"]]
        comparisons.append(
            dict(
                scenario=new["scenario"],
                distance_pt=new["requested_distance_pt"],
                viewport_pt=new["actual_viewport"],
                hold_ms=new["hold_ms"],
                requested_speed_setting=new["requested_speed_setting"],
                trial=new["trial"],
                old_max_gap_pt=old["post_release"]["max_gap_pt"],
                new_max_gap_pt=new["post_release"]["max_gap_pt"],
                old_rms_pt=old["post_release"]["rms_pt"],
                new_rms_pt=new["post_release"]["rms_pt"],
                old_release_gap_pt=old["release_gap_pt"],
                new_release_gap_pt=new["release_gap_pt"],
                new_uikit_extra_exposure_pt=new["uikit_extra_exposure_pt"],
                new_slint_extra_exposure_pt=new["slint_extra_exposure_pt"],
                issues=new["issues"],
            )
        )
    (EVIDENCE / "release-velocity-comparison.json").write_text(
        json.dumps(comparisons, indent=2) + "\n"
    )
    positions = [
        read_positions("previous-paired-positions.csv"),
        read_positions("paired-positions.csv"),
    ]
    fig, axes = plt.subplots(2, 2, figsize=(12, 8))
    for idx, (distance, hold, speed) in enumerate([(600, 400, 400), (100, 0, 1200)]):
        for col, (prefix, title) in enumerate(
            [
                ("latest-69f715", "Before: zero release velocity"),
                ("latest-af6e8e", "After: release velocity included"),
            ]
        ):
            scenario = f"{prefix}-vp774-d{distance}-v{speed}-hold{hold}-trial1"
            rows = [r for r in positions[col] if r["scenario"] == scenario]
            ax = axes[idx, col]
            times = [float(r["callback_seconds_from_release"]) for r in rows]
            for key, color in [("uikit", "#c53b32"), ("slint", "#1976b4")]:
                ax.plot(
                    times,
                    [-float(r[key + "_offset_pt"]) for r in rows],
                    color=color,
                    label="UIKit measured" if key == "uikit" else "Slint measured",
                )
            ax.axvline(0, color="#888888", linewidth=0.7)
            ax.set_xlim(-0.05, 0.85)
            ax.set_ylim(-2, 235 if idx == 0 else 67)
            ax.set_title(
                f"{title}\n{distance} pt pull · "
                + ("400 ms hold" if hold else "release while moving")
            )
            ax.set_xlabel("Seconds from delivered release callback")
            ax.set_ylabel("Top exposure (pt)")
            ax.grid(alpha=0.2)
            ax.legend()
    fig.suptitle(
        "Paired iPhone captures · actual time and distance · first repetition",
        fontsize=15,
    )
    fig.tight_layout(rect=(0, 0, 1, 0.95))
    fig.savefig(ROOT / "figures/release-velocity-before-after.png", dpi=160)
    plt.close(fig)


if __name__ == "__main__":
    main()
