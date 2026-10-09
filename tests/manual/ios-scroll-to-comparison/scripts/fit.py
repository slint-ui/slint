#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

"""Fits animation models to the normalized scroll-to curves in `cases/from-rest.csv`.

Writes `cases/fit-from-rest.csv` with one row per series, scope, and model.
The scope is a single capture, or `all` for all captures together.
Models, with `tau = max(0, t - t0)` and progress going from 0 to 1:

- `critical`: critically damped spring, `1 - (1 + omega tau) exp(-omega tau)`.
  `stiffness` is `omega^2`, for a mass of 1.
- `spring`: damped spring at rest at the start, with `omega` and `zeta`.
- `bezier`: `cubic-bezier(x1, y1, x2, y2)` over `duration_s`.

Requires NumPy and SciPy.
"""

# cspell:ignore scipy lstsq

import argparse
import csv
import math
from collections import defaultdict
from pathlib import Path

CASES = Path(__file__).resolve().parents[1] / "cases"
FIT_WINDOW_S = (-0.05, 2.0)
COLUMNS = [
    "series",
    "scope",
    "distance_pt",
    "model",
    "t0_s",
    "omega",
    "zeta",
    "stiffness",
    "duration_s",
    "x1",
    "y1",
    "x2",
    "y2",
    "rmse_progress",
    "rmse_pt",
    "samples",
]


def spring(np, tau, omega, zeta):
    """Progress of a spring released at rest from -1 towards 0, as 0..1."""
    if abs(zeta - 1) < 1e-4:
        return 1 - (1 + omega * tau) * np.exp(-omega * tau)
    if zeta < 1:
        wd = omega * math.sqrt(1 - zeta * zeta)
        return 1 - np.exp(-zeta * omega * tau) * (np.cos(wd * tau) + zeta * omega / wd * np.sin(wd * tau))
    root = omega * math.sqrt(zeta * zeta - 1)
    r1, r2 = -zeta * omega + root, -zeta * omega - root
    return 1 - (r2 * np.exp(r1 * tau) - r1 * np.exp(r2 * tau)) / (r2 - r1)


def bezier(np, u, x1, y1, x2, y2):
    """CSS `cubic-bezier` easing at time fraction `u`, solved by bisection."""
    u = np.clip(u, 0, 1)
    lo, hi = np.zeros_like(u), np.ones_like(u)
    for _ in range(40):
        s = (lo + hi) / 2
        x = 3 * (1 - s) ** 2 * s * x1 + 3 * (1 - s) * s * s * x2 + s**3
        lo, hi = np.where(x < u, s, lo), np.where(x < u, hi, s)
    s = (lo + hi) / 2
    return 3 * (1 - s) ** 2 * s * y1 + 3 * (1 - s) * s * s * y2 + s**3


MODELS = {
    "critical": dict(
        names=["t0_s", "omega"],
        initial=[0.0, 20.0],
        bounds=([-0.05, 0.1], [0.1, 500.0]),
        curve=lambda np, t, t0, omega: spring(np, np.maximum(t - t0, 0), omega, 1.0),
    ),
    "spring": dict(
        names=["t0_s", "omega", "zeta"],
        initial=[0.0, 20.0, 0.9],
        bounds=([-0.05, 0.1, 0.05], [0.1, 500.0, 5.0]),
        curve=lambda np, t, t0, omega, zeta: spring(np, np.maximum(t - t0, 0), omega, zeta),
    ),
    "bezier": dict(
        names=["t0_s", "duration_s", "x1", "y1", "x2", "y2"],
        initial=[0.0, 0.4, 0.25, 0.1, 0.25, 1.0],
        bounds=([-0.05, 0.02, 0.0, -1.0, 0.0, 0.0], [0.1, 5.0, 1.0, 2.0, 1.0, 2.0]),
        curve=lambda np, t, t0, duration, x1, y1, x2, y2: bezier(
            np, np.maximum(t - t0, 0) / duration, x1, y1, x2, y2
        ),
    ),
}


def fit(np, least_squares, model, t, p):
    spec = MODELS[model]

    def residuals(params):
        return spec["curve"](np, t, *params) - p

    best = None
    for scale in (0.5, 1.0, 2.0):
        initial = list(spec["initial"])
        initial[1] *= scale
        result = least_squares(residuals, initial, bounds=spec["bounds"])
        if best is None or result.cost < best.cost:
            best = result
    return dict(zip(spec["names"], best.x)), math.sqrt(np.mean(best.fun**2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, default=CASES / "from-rest.csv")
    parser.add_argument("--output", type=Path, default=CASES / "fit-from-rest.csv")
    parser.add_argument("--series", nargs="+", default=["uikit", "slint"], choices=["uikit", "slint"])
    args = parser.parse_args()
    try:
        import numpy as np
        from scipy.optimize import least_squares
    except ImportError:
        raise SystemExit("fit.py needs NumPy and SciPy: python3 -m pip install numpy scipy")

    with args.input.open(newline="") as source:
        rows = list(csv.DictReader(source))
    rows = [r for r in rows if FIT_WINDOW_S[0] <= float(r["seconds_from_command"]) <= FIT_WINDOW_S[1]]
    if not rows:
        raise SystemExit(f"No samples in {args.input}")

    output = []
    for series in args.series:
        captures = defaultdict(list)
        distances = {}
        for r in rows:
            value = r.get(f"{series}_progress", "")
            if value:
                scope = f"{r['case']}/{r['file']}"
                captures[scope].append((float(r["seconds_from_command"]), float(value)))
                distances[scope] = float(r["distance_pt"])
        scopes = {name: samples for name, samples in sorted(captures.items())}
        scopes["all"] = [s for samples in captures.values() for s in samples]
        for scope, samples in scopes.items():
            t = np.array([s[0] for s in samples])
            p = np.array([s[1] for s in samples])
            distance = distances.get(scope, math.nan)
            for model in MODELS:
                params, rmse = fit(np, least_squares, model, t, p)
                if model == "critical":
                    params["zeta"] = 1.0
                omega = params.get("omega", math.nan)
                output.append(
                    dict(
                        series=series,
                        scope=scope,
                        distance_pt="" if math.isnan(distance) else f"{distance:.3f}",
                        model=model,
                        **{k: f"{v:.6f}" for k, v in params.items()},
                        stiffness="" if math.isnan(omega) else f"{omega * omega:.3f}",
                        rmse_progress=f"{rmse:.6f}",
                        rmse_pt="" if math.isnan(distance) else f"{rmse * abs(distance):.3f}",
                        samples=len(samples),
                    )
                )
            best = min((o for o in output if o["series"] == series and o["scope"] == scope),
                       key=lambda o: float(o["rmse_progress"]))
            print(f"{series:5} {scope:40} best {best['model']:8} rmse {float(best['rmse_progress']):.4f}")

    with args.output.open("w", newline="") as target:
        writer = csv.DictWriter(target, fieldnames=COLUMNS, lineterminator="\n")
        writer.writeheader()
        writer.writerows(output)
    print(f"Wrote {args.output}")


if __name__ == "__main__":
    main()
