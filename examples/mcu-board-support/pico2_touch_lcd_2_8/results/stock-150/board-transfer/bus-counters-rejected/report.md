# Deferred Bus-Counter Integration

An optional profiling command samples the RP2350 crossbar counters for bounded 50 ms intervals.
Its disabled-counter image passes Queue, Settings and boosted tilted Ink timing windows with zero misses, late strips and scan errors.
The command returns 36 unsaturated samples and disables counters afterward.
However, a subsequent scene-switch and capture sequence triggers watchdog recovery.
The Ink samples stop showing normal memory activity, so they are not valid rendering evidence.
The root cause is not established by this trial.

The counter integration is removed from the qualified firmware.
The native graphics checkout retains the existing standalone counter tool for further investigation.
No bus priorities, display settings or flash timings are changed.
