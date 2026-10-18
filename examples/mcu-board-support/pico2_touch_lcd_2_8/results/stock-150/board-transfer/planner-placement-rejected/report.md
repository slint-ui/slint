# Rejected SRAM Planner Placement

Moving the uniform-tile planner to SRAM reduces planning from 0.480 to 0.426 ms for Queue, 0.681 to 0.618 ms for Settings, and 0.293 to 0.247 ms for Ink.
However, boosted tilted Ink composition increases from 7.573–7.600 ms to 7.843–7.873 ms.
All timing windows and pixel checks pass, but the local planning gain does not compensate for the Ink regression.
The trial also consumes approximately 2.3 KiB of SRAM.
The qualified image keeps the planner in flash.
