# Guarded Printer Updates

A controlled one-byte USB product-name change leaves the UI, rendering code and particle state unchanged.
The original ELF SHA-256 is `81bf85a53ba6ebabcb44a916981e6498878f6bb13b5d395ef53b4184175ea6e5`.
The temporary diagnostic ELF is not retained in source.

| Path | Verified update and reboot |
| --- | ---: |
| Full ELF with picotool update mode | 8.462 s |
| Guarded changed-sector payload, including preceding-image checks | 1.908 s |

The guarded path is 4.43 times faster for this change.
It sends 8,448 bytes rather than 1,637,376 bytes.
The preceding-image check takes 0.248 seconds.
The update and rollback both pass loader verification and automatic reboot.
Three host tests verify current pages, unchanged neighbors, RP2350-E10 preservation and malformed-image rejection.

The short profiler set now includes boosted tilted Ink instead of upright boosted Ink.
Compiler settings remain Cortex-M33, optimization level 3, ThinLTO and one code-generation unit.
Subsequent changes use immutable ELF snapshots and warmed ten-second windows at fixed orientation.
