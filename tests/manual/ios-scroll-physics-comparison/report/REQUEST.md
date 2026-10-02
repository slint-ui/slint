<!-- cspell:ignore xcodegen xcresult Murmele UDID -->
# Next Capture: Release-Speed Sweep

## Why

When the pointer releases a pull past the edge while still moving outward, UIKit weakens the spring's initial pull back.
The faster the pointer, the weaker the pull.
`RETURN_RATE_FADE_START` and `RETURN_RATE_FADE_END` in `internal/core/animations/simulations/scroll_spring.rs` model that as a linear fade over the pointer speed.
They're fitted to only three speed levels: about 240–400, 730, and 1,224 points per second.
This campaign measures enough speeds to fit the fade's shape, and whether it depends on the pull distance.

## What to Run

Build the engine from the commit of this branch that you test, without local engine changes.
Prepare the fixture and generate its project as in [FINDINGS.md](FINDINGS.md), with a new fixture directory.
The fixture contains two test methods; run both on the iPhone 13 Pro Max in Release:

- `testMurmeleLatestSpringValidation`: the same six cases as before, two repetitions each, now prefixed `validation-`.
  It checks that held releases still match.
- `testMurmeleReleaseSpeedSweep`: releases without a hold at viewport 774, prefixed `sweep-`.
  It pulls 100 points at 200–1,200, 300 points at 200–2,000, and 600 points at 200–2,000 points per second, two repetitions each.
  That's 52 captures, about eight minutes.

```sh
SLINT_STYLE=cupertino SLINT_BACKEND=winit-skia xcodebuild test \
  -project NativeSlintScroll.xcodeproj -scheme NativeSlintScroll \
  -configuration Release -destination 'platform=iOS,id=DEVICE_UDID' \
  DEVELOPMENT_TEAM=YOUR_TEAM -parallel-testing-enabled NO \
  -only-testing:NativeSlintScrollUITests/ScrollComparisonTests/testMurmeleLatestSpringValidation \
  -only-testing:NativeSlintScrollUITests/ScrollComparisonTests/testMurmeleReleaseSpeedSweep
```

Copy every scenario's `input-*.csv`, `scroll-*.csv`, and `geometry-*.json` from the app's Documents directory into one raw directory.
Then run the analysis with the commit you tested:

```sh
python3 tests/manual/ios-scroll-physics-comparison/report/scripts/analyze_latest.py \
  --source-root "$PWD" --raw-dir /path/to/raw --engine-commit TESTED_COMMIT \
  --output-dir tests/manual/ios-scroll-physics-comparison/report
```

The analysis finds the cases from the file names and rejects incomplete deliveries.
Keep failed traces and list them instead of rerunning only the passing ones.

## What to Push

Commit these files to this branch and push them:

- `evidence/paired-positions.csv`, `evidence/latest-validation.json`, and `evidence/sweep-summary.csv`.
- `figures/latest-held-returns.png`, `figures/latest-moving-returns.png`, `figures/sweep-moving-returns.png`, and `figures/source-clock-comparison.png`.
- A short update of `FINDINGS.md` with the sweep results.

Put the device, the iOS build, the tested commit, and any failed or missing cases in the commit message.
Overwrite the existing evidence files; Git keeps the earlier runs.
Leave raw touch recordings and `.xcresult` bundles out.
Don't run `compare_runs.py`; it only compares the two earlier runs.
Don't change the engine or refit any constants; the fit happens afterwards from `sweep-summary.csv` and `paired-positions.csv`.
