# Physical touch report capture

The application received about 38 touchscreen reports per second during both held drags.
Settings also dispatched one false release while the user confirmed their finger stayed down.
The firmware and UI were unchanged during this capture.
Clocks were 150 MHz CPU and 75 MHz flash.

## Timing

The user panned Queue, tapped Settings in the sidebar, then panned Settings.
They confirmed one continuous contact on each page.
The 120-second capture contains 301 new reports, with no missing acquisition sequence numbers.
Earlier reports retained in the device ring are excluded.

| Measurement | Queue | Settings |
| --- | ---: | ---: |
| Drag sample duration, through final release | 2.951 s | 4.727 s |
| Reports, including release packets | 113 | 180 |
| Received report rate | 37.95 Hz | 37.86 Hz |
| Minimum report gap | 25.878 ms | 25.517 ms |
| Median report gap | 26.276 ms | 26.318 ms |
| Mean report gap | 26.349 ms | 26.410 ms |
| 95th percentile report gap | 26.648 ms | 26.760 ms |
| 99th percentile report gap | 26.730 ms | 33.484 ms |
| Maximum report gap | 33.680 ms | 33.916 ms |
| Report intervals over 30 ms | 1 / 112 | 3 / 179 |
| Maximum gap between valid contact coordinates | 33.680 ms | 52.715 ms |
| False releases during the confirmed held drag | 0 observed | 1 observed |

These are acquisition timestamps after the CST328 register read, not its internal scan timestamps.
The result measures the physical reports received by this firmware, rather than the controller's maximum possible scan rate.
Repeated coordinates are included because they are valid contact reports.
Reports with changing coordinates averaged 31.17 per second in Queue and 34.27 in Settings for these particular gestures.
Those rates also depend on finger movement speed and coordinate quantization.

Most acquisition intervals cluster closely around 26.3 ms.
Four intervals were around 34 ms; none exceeded 34 ms between raw reports.
The 52.715 ms Settings gap occurs between valid coordinates across a release packet, despite raw reports continuing normally.
This sample does not establish that false releases never occur on Queue.

## False release in Settings

| Sequence | Time | Raw bytes | Interpretation by current driver |
| --- | ---: | --- | --- |
| 156 | 278076073 us | `060b0bad4701` | Contact ID 0 active |
| 157 | 278102409 us | `000b0bad4702` | First contact lifted; contact-count field is 2; sends PointerReleased and PointerExited |
| 158 | 278128788 us | `16080b464501` | Contact ID 1 active; sends a new PointerPressed |

The user confirmed their finger remained down during this gesture.
The new press arrived 26.379 ms after the false release.
The valid-coordinate interval across the interruption was 52.715 ms.

The Pico adapter reads only the first six bytes of the touch report.
Its parser returns no point whenever the first contact status is not 6.
The adapter then releases its single pointer even when the count field indicates more contact records.
The next report's active contact uses a different ID.
This is consistent with a controller contact handover that our single-contact adapter handles as a complete release.
The six-byte trace cannot show the second contact record, so the controller's complete state remains unobserved.
The trace does not prove why the controller changed contact IDs while one physical finger was held.

This is evidence of a false release originating in the Pico touch adapter, rather than Slint inventing one internally.
A false release can start flick inertia and reset drag capture when the next press arrives.
No input handling changes were made in this measurement commit.

## Reproduction

Run `profile-touch.py --serial BFC13272EF67B96D --seconds 120 --output /tmp/pico-touch` against firmware commit `eae4346267`.
Pan each page with one held contact and record intentional lifts.
`reports.csv` contains every newly acquired raw report, including the sidebar navigation tap.
`analysis.json` contains full interval statistics and the selected sequence ranges.
`capture.json` contains the baseline, final dispatch counters, and the user's contact confirmation.
