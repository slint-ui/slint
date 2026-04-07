---
title: ISO 26262 Requirements
description: Overview of ISO 26262 safety requirements for Slint SC.
---

## Specification and Management of Safety Requirements (ISO 26262-6 6.x)

The ISO 26262 standard tells us what properties a safety-critical system must have (traceability, freedom from interference, determinism, etc.), but it doesn't tell us how to write those requirements for a GUI toolkit. The following sections contain specific, actionable engineering requirements that should be considered for Slint SC.

## ASIL B Capable

ASIL (Automotive Safety Integrity Level) describes the risk level of something. ASIL D=highest, C=high, B=medium, A=low risk, QM = not safety critical.

Since a compiler and a toolkit don't have a specific vehicle function, they don't have an intrinsic ASIL derived from a HARA (Hazard Analysis and Risk Assessment).

Slint SC is a "Safety Element out of Context" (SEooC). Slint SC is meant to be used for
mission-critical digital instrument clusters.

For non-critical (QM) interactive applications such as infotainment systems, Slint can be used. In that case, these ASIL requirements do not apply.

Each Requirement under this Section has a descriptive ID that begins with SR_, a description, and ASIL=B.

## Traceability

All ISO26262 references below are valid for the 2018 edition of the standard.

* ISO 26262-4 5.x: See [Development Phases](/development-phases/).
* ISO 26262-4 9.x: See [Validation](/qualification-plan/validation/).
* ISO 26262-6 7.x: See [Architecture Design](/using-slint-sc/#slint-sc-architecture-design-iso-262626-74)
* ISO 26262-8 5.x: See [Distributed Development](/development-process/#distributed-development-iso-26262-8-5x)
* ISO 26262-8 6.4: The safety requirements shall be traceable to the safety goals and to the safety concept. The traceability shall be documented and maintained.
* ISO 26262-8 7.x: See [Configuration Management](/development-process/#configuration-management-iso-26262-8-7x)
* ISO 26262-8 8.x: See [Change Management](/development-process/#change-management-iso-26262-8-8x)
* ISO 26262-8 9.4.x: See [Verification](/development-process/#verification-iso-26262-8-94x)
* ISO 26262-8 11.4.8: See [The Development Process](/development-process/#the-development-process-iso-26262-8-1148)
* ISO 26262-8 12.x: See [Software Component Qualification](/development-process/#software-component-qualification-iso-26262-8-12x)

