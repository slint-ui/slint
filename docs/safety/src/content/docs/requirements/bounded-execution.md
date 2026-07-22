---
title: "SR_BOUNDED_EXECUTION_TIME"
description: Bounded execution time requirement for Slint SC.
---

Slint SC shall guarantee a strictly bounded maximum execution time for rendering a single frame, ensuring that the critical rendering loop never blocks the main execution thread beyond the hardware display refresh interval (e.g., 16.6ms for 60Hz).

**(Reference: ISO 26262-6 Annex D.2.2 "Timing and execution", which identifies "incorrect allocation of execution time" and "blocking of execution" as interference faults.)**
