<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: MIT -->

# Material Capture Profile

Measured in Figma Desktop on October 15, 2026, before the deletion audit, with the development plugin and release Wasm interpreter from source commit `a95c820c68`.
The fixture is [Material 3 Design Kit (Community)](https://www.figma.com/design/uC4fm6ledLDz4Vl7dDYSXp/Material-3-Design-Kit--Community-).
Buttons is section `57994:696`; Icon Button is section `57994:8953`.
Each cold run starts a fresh plugin session, with the developer console closed.
Each repeat switches to the other section, waits for conversion and export validation, then returns within the same session.
Repository checks finish before profiling; no builds compete with these final measurements.
These are individual observations, not statistical benchmarks.

## Pre-PR Base and Final Build

The actual base of PR #14082 is `0f6b1e60ad`, the tip of stacked PR #14049.
We profiled that base in Figma Desktop on October 15, after collecting the final-build measurements above.
The comparison below spans the whole PR, not just its last optimization pass.

| Selection           |              Base Capture | Final Capture | Base Preview and Export Ready  | Final Preview and Export Ready |
| ------------------- | ------------------------: | ------------: | ------------------------------ | -----------------------------: |
| Icon Button, cold   | 55.215 s, raster fallback |      11.901 s | Failed at 56.551 s; no preview |                       19.039 s |
| Icon Button, repeat | 52.801 s, raster fallback |       0.007 s | Failed at 53.520 s; no preview |                        1.798 s |
| Buttons, cold       |                  11.981 s |       5.141 s | 18.802 s                       |                        9.010 s |
| Buttons, repeat     |                  11.135 s |       0.007 s | 13.291 s                       |                        1.451 s |

Ready means both the preview and native export validation have completed.
The base records its completed trace after export validation, so final ready totals use export-validation completion minus selection start.
The final previews appear earlier: 18.495 s and 1.797 s for Icon Button; 8.821 s and 1.449 s for Buttons.
No successful preview duration or percentage speedup is assigned to either failed base Icon Button run.

Both base Icon Button runs exhaust the full and simplified capture deadlines, then return a single raster node with no component families or variants.
The worker result then triggers `RangeError: Maximum call stack size exceeded` in the export-package base64 validator.
The UI retains its busy state and displays no preview; Export ZIP stays disabled.
Failure durations end at that uncaught error, not when we later stop the plugin.
The nominal capture deadlines are 20 seconds each; host work can delay timer callbacks and extend total capture time.

Both base Buttons runs retain 4,241 nodes, 15 component families, and 657 variants, with no automatic fallback.
Their capture work records 837 PNG and 644 SVG exports per selection.
The base repeat outcome is `unchanged`: it recaptures the selection and reuses the surviving Buttons preview after Icon Button fails.
All four final runs retain their complete captured trees and variants, use zero automatic fallback, and pass native export validation.

Each baseline cold run starts a fresh plugin session in the same already-open Material file, at device pixel ratio 2.
Icon Button repeat follows a completed Buttons conversion in the same session.
Buttons repeat follows the terminal Icon Button failure; the base cannot complete that intervening conversion.
The final repeats follow completed conversions of the other section.
The console stays closed through the measured completion or error and opens afterward to retrieve evidence.
These are individual observations, not paired statistical benchmarks or cold-file loading measurements.

The baseline uses its original plugin sources, dependencies, and unchanged release Wasm compiler/runtime.
A temporary read-only UI observer retains incoming timing metadata and uncaught errors before validation discards the result.
It changes no capture work, deadlines, fallback, conversion, or error handling, and is not included in the PR.
An initial run without the observer also reproduced fallback followed by the validator crash.
The local evidence files are `figma-base-icon-cold.trace.json`, `figma-base-icon-repeat.trace.json`,
`figma-base-buttons-cold.trace.json`, and `figma-base-buttons-repeat.trace.json` in `/private/tmp`.
The final plugin bundle was restored after profiling.

## Cold Capture Breakdown

Active time measures the union of intervals with at least one operation in that category running.
Summed time adds each operation's elapsed duration, including concurrent operations.
Synchronous overlap measures the union of instrumented synchronous work inside those intervals.
Nested synchronous scopes contribute once to that overlap.
Node-read and encoding counts measure batches, not individual getters or unique nodes.

Metadata contains reads and encoding; component assembly contains planning and reads.
Bulk JSON includes response dispatch and indexing within its interval.
Async durations include host waits, uninstrumented work, and delays before JavaScript resumes.
They don't measure Figma's internal renderer CPU time.
Don't add category active times to estimate total capture time.

### Icon Button

| Category            | Operations |  Active |   Summed | Synchronous Overlap |
| ------------------- | ---------: | ------: | -------: | ------------------: |
| Node reads          |     57,606 | 3.732 s |  3.732 s |             3.732 s |
| Encoding            |     25,153 | 0.674 s |  0.674 s |             0.674 s |
| Metadata            |     25,102 | 3.896 s |  3.896 s |             3.896 s |
| Component lookup    |         64 | 6.644 s | 14.916 s |             4.006 s |
| Component assembly  |          1 | 0.159 s |  0.159 s |             0.159 s |
| Component planning  |          1 | 0.032 s |  0.032 s |             0.032 s |
| Export queue        |         12 | 0.098 s |  0.098 s |             0.061 s |
| Bulk index          |          9 | 0.011 s |  0.011 s |             0.011 s |
| Bulk dispatch       |          9 | 0.593 s |  0.593 s |             0.593 s |
| Component discovery |      5,335 | 8.151 s | 62.884 s |             5.106 s |
| Bulk JSON           |          9 | 1.721 s |  1.734 s |             0.604 s |
| PNG exports         |          6 | 8.989 s |  9.111 s |             5.123 s |
| SVG exports         |          6 | 8.963 s | 26.772 s |             5.116 s |
| Image reads         |          0 | 0.000 s |  0.000 s |             0.000 s |
| Variable reads      |         14 | 0.003 s |  0.003 s |             0.000 s |

The collector spans 11.247 s with a tracked union of 10.891 s.
Uninstrumented time inside the collector is 0.356 s.
Capture includes another 0.654 s of result assembly, dependency handling, and cache bookkeeping outside the collector.
Worker bulk indexing takes 3.3 ms and overlaps capture.
Instrumented metadata batches record 137,444 live property reads, including 5,118 variable-mode reads and 0 x-position reads.
Identity, visibility, and traversal getters are outside that property counter.

### Buttons

| Category            | Operations |  Active |   Summed | Synchronous Overlap |
| ------------------- | ---------: | ------: | -------: | ------------------: |
| Node reads          |     19,600 | 1.880 s |  1.880 s |             1.880 s |
| Encoding            |      9,159 | 0.364 s |  0.364 s |             0.364 s |
| Metadata            |      8,482 | 2.003 s |  2.003 s |             2.003 s |
| Component lookup    |          7 | 1.457 s |  1.923 s |             1.060 s |
| Component assembly  |          1 | 0.056 s |  0.056 s |             0.056 s |
| Component planning  |          1 | 0.010 s |  0.010 s |             0.010 s |
| Export queue        |         12 | 0.021 s |  0.021 s |             0.002 s |
| Bulk index          |          7 | 0.004 s |  0.004 s |             0.004 s |
| Bulk dispatch       |          7 | 0.240 s |  0.240 s |             0.240 s |
| Component discovery |      1,451 | 3.242 s | 28.172 s |             2.433 s |
| Bulk JSON           |          7 | 0.810 s |  0.825 s |             0.244 s |
| PNG exports         |          6 | 3.769 s |  3.817 s |             2.414 s |
| SVG exports         |          6 | 3.731 s | 11.149 s |             2.392 s |
| Image reads         |          0 | 0.000 s |  0.000 s |             0.000 s |
| Variable reads      |         15 | 0.001 s |  0.001 s |             0.000 s |

The collector spans 4.822 s with a tracked union of 4.690 s.
Uninstrumented time inside the collector is 0.132 s.
Capture includes another 0.319 s of result assembly, dependency handling, and cache bookkeeping outside the collector.
Worker bulk indexing takes 1.4 ms and overlaps capture.
Instrumented metadata batches record 54,963 live property reads, including 2,606 variable-mode reads and 0 x-position reads.
Identity, visibility, and traversal getters are outside that property counter.

## Preview Costs After Capture

| Phase                          | Icon Button Cold | Icon Button Repeat | Buttons Cold | Buttons Repeat |
| ------------------------------ | ---------------: | -----------------: | -----------: | -------------: |
| Capture serialization          |         803.0 ms |             0.0 ms |     254.0 ms |         0.0 ms |
| Capture parse                  |          96.6 ms |             0.0 ms |      43.5 ms |         0.0 ms |
| Bulk hydration                 |         212.6 ms |             0.0 ms |      94.0 ms |         0.0 ms |
| Normalization                  |         229.8 ms |             0.0 ms |     166.9 ms |         0.0 ms |
| Worker queue/transport         |          14.2 ms |             0.0 ms |      10.0 ms |         0.0 ms |
| Slint source generation        |         389.5 ms |             0.0 ms |     191.0 ms |         0.0 ms |
| Asset packing                  |           0.5 ms |             0.0 ms |       0.4 ms |         0.0 ms |
| Plugin message dispatch        |        1338.0 ms |             0.0 ms |     656.0 ms |         1.0 ms |
| Message transport              |         269.0 ms |            34.0 ms |     108.0 ms |        35.0 ms |
| Asset unpacking                |          12.9 ms |             1.2 ms |      14.9 ms |         0.7 ms |
| Preview queue                  |           0.1 ms |             0.1 ms |       0.3 ms |         0.0 ms |
| Wasm initialization wait       |           0.0 ms |                  — |       0.0 ms |              — |
| Slint compilation              |         685.4 ms |             0.0 ms |     493.8 ms |         0.0 ms |
| Component creation/replacement |         530.8 ms |           157.7 ms |     217.1 ms |       160.1 ms |
| Show                           |        1605.9 ms |            69.2 ms |    1239.9 ms |       115.2 ms |
| Presentation                   |          29.4 ms |          1506.3 ms |      51.2 ms |      1120.9 ms |
| Other overhead                 |         376.3 ms |            21.5 ms |     139.0 ms |         9.1 ms |

Wasm and fonts initialize during capture, contributing zero critical-path initialization wait in both cold runs.
Repeat conversion and compilation are cached; component creation and presentation still run.

## Browser Presentation

| Selection           | First Browser Frame | Second Browser Frame | Total Presentation |
| ------------------- | ------------------: | -------------------: | -----------------: |
| Icon Button, cold   |              9.5 ms |              19.9 ms |            29.4 ms |
| Icon Button, repeat |           1497.2 ms |               9.1 ms |          1506.3 ms |
| Buttons, cold       |             22.8 ms |              28.2 ms |            51.2 ms |
| Buttons, repeat     |           1103.2 ms |              17.7 ms |          1120.9 ms |

Most repeat presentation time is spent reaching the first frame, rather than waiting for the second.
The split identifies the delayed frame but doesn't attribute that interval to a specific renderer or scheduling operation.
Further work should measure first-frame rendering and scheduling, or evaluate bounded reuse of live rendered instances.
Changing the completion timestamp alone wouldn't establish a faster displayed preview.

## Export Validation Breakdown

| Phase                               | Icon Button Cold | Icon Button Repeat | Buttons Cold | Buttons Repeat |
| ----------------------------------- | ---------------: | -----------------: | -----------: | -------------: |
| Prepare source                      |           2.9 ms |             0.0 ms |       6.1 ms |         0.0 ms |
| Compiler queue wait                 |           1.4 ms |             1.0 ms |       1.7 ms |         1.1 ms |
| Compilation                         |         538.7 ms |             0.0 ms |     179.2 ms |         0.0 ms |
| Total including completion overhead |         543.5 ms |             1.3 ms |     188.2 ms |         2.0 ms |

Validation uses asset blob URLs, avoiding base64 expansion solely for compilation.
Preview and export compilation share a serialized interpreter queue.
Runtime failures invalidate the interpreter and cached definitions.

## Validation

After the deletion audit, `pnpm verify` passes 444 unit tests and 82 browser/raster tests.
Obsolete timeout, flattened-capture, raster-capture, and dialog tests were removed with their implementation.
Repository autofix, lint, license-header, and diff checks pass.
Regressions cover visible presentation with stale-selection guards and frame-breakdown finalization.
They also cover exact geometry reuse, rotated/scaled/missing bounds, inherited variable modes, source encoding, and live property counters.
Template regressions retain 200 distinct occurrences and compare all rendered pixels against expanded output.
Both complete sections pass cold and repeat Figma verification with zero automatic capture fallback and valid native export.

After the deletion audit, Figma verification repeated Buttons, Icon Button, Buttons, then Icon Button in one fresh plugin session.
All four rendered and passed native export validation; both repeats reused their captures and validated exports.
Icon Button retained 12,551 nodes and 1,809 variants; Buttons retained 4,241 nodes and 657 variants.
The retired whole-selection fallback modes and their dialog are absent from the audited build.
