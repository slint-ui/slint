---
title: ISO 26262 Requirements
description: Overview of ISO 26262 requirements for Slint SC.
---

The ISO 26262 standard tells us what properties a safety-critical system must have (traceability, freedom from interference, determinism, etc.), but it doesn't tell us how to write those requirements for a GUI toolkit. The following sections contain specific, actionable engineering requirements that should be considered for Slint SC.

## ASIL B Capable

ASIL (Automotive Safety Integrity Level) describes the risk level of something. ASIL D=highest, C=high, B=medium, A=low risk, QM = not safety critical.

Since a compiler and a toolkit don't have a specific vehicle function, they don't have an intrinsic ASIL derived from a HARA (Hazard Analysis and Risk Assessment).

Slint SC is a "Safety Element out of Context" (SEooC). Slint SC is meant to be used for
mission-critical digital instrument clusters.

For non-critical (QM) interactive applications such as infotainment systems, Slint can be used. In that case, these ASIL requirements do not apply.

Each Requirement below has a descriptive ID that begins with SR_, a description, and ASIL=B.
