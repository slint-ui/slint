# Ink Heap Fragmentation

All device checks use 150 MHz CPU, 75 MHz flash and the existing 60 MHz PIO display transport.
The UI, vial dimensions, bubble populations and animation calculations are unchanged.

## Reproduction

The counted diagnostic fails while constructing the black vial's second 48-by-140 RGB565 buffer.
Seven buffers are live when allocation fails.
The request needs 13,452 contiguous bytes, including the shared-vector header; the heap reports 26,276 free bytes.
This establishes fragmentation rather than exhaustion of the total free heap.
The panic reports 6,720 "bytes", but that value is the vector's pixel capacity.
The failure occurs during construction, rather than copy-on-write or display DMA.

The local embedded prepared-text cache retains glyph allocations after a component is destroyed.
Those allocations survive page transitions and obstruct reuse of the freed image-buffer space.
This cache is a local optimization, not the stock Slint text path.

## Fix

Release the embedded prepared-text cache with component graphics resources.
The next page regenerates text as needed.
The system-font path is unchanged.
The general heap remains 310 KiB; no larger heap or pixel pool hides the failure.

The same counted diagnostic completes 24 page entries, including eight Ink entries, over 96 seconds after the fix.
It records no panic, late strip or scan error.
Ink's observed free heap is 12,336–13,616 bytes.
Temporary panic handlers and allocation counters are excluded from the release source.

The cache regression test verifies that clearing releases retained glyph storage and regenerates identical glyphs.
All 42 printer host tests pass.
The RGB565 framebuffer hook matches the default renderer across 2,054 complete deterministic frames.
The release binary preserves all 24 artwork symbols and hardware floating point.
Its 27,652-byte main-stack reservation exceeds the unchanged 24 KiB guard.

The clean release repeats another 24 page entries, including eight boosted tilted Ink entries, without stalls, late strips or scan errors.
This second run contains no diagnostic allocator or panic-handler changes.
See `release-reentry.json` for every entry's frame progress and free heap.

Four release measurement windows match the panel at 58.5–58.6 FPS with zero missed phases, late strips and scan errors.
Queue and Settings raster average 11.268 ms and 10.785 ms; repeated boosted tilted Ink windows average 5.396 ms.
Both Ink orientations and Queue/Settings panel checks report zero mismatches across all 76,800 pixels.
See `measurements.json` and `verification.json` for the release evidence.
