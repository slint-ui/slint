---
title: Development Cycle
description: Tools, processes, and infrastructure used for Slint SC development.
---

This section describes the tools we use and the processes we follow to develop Slint SC, aiming to fulfill the supporting process requirements of ISO 26262 Part 8 (Clauses 7, 8, 9, 11, and 12).

## Tool Qualification (ISO 26262-8 Clause 11)

To ensure that software tools used in the development of Slint SC do not introduce or fail to detect errors, tools are assessed based on their Tool Impact (TI) and Tool Error Detection (TD) to determine their Tool Confidence Level (TCL).

### Language and Compiler

Slint SC is written in Rust and requires `rustc` + `cargo` version 1.88 or newer to build. We use Ferrocene (Version?) to build Slint SC.

* **Tool Impact (TI2):** The compiler can introduce errors into the executable.
* **Tool Error Detection (TD2/TD3):** While Rust's strong type system and borrow checker catch many errors, compiler bugs might still bypass detection.
* **Tool Confidence Level (TCL):** A combination of TI2 and TD2/TD3 results in a Tool Confidence Level of TCL2 or TCL3.
* **Qualification Strategy:** As a high-impact tool, Ferrocene is already ASIL D qualified for safety-critical deployment.

## CI System and Infrastructure

Continuous Integration (CI) is driven by GitHub Actions. CI triggers automated testing for any Pull Request (PR) or direct push to the `master` branch.
* **Tool Impact (TI1):** The CI system schedules and runs tests but does not generate the final executable code itself. Failures during CI block the PR.
* **Qualification Strategy:** Verification of the CI pipeline is achieved through increased confidence from use.

## Configuration Management (ISO 26262-8 Clause 7)

Slint SC utilizes a structured configuration management process to ensure artifacts are reproducible and traceable.

* **Version Control:** Slint is hosted as an open-source project on GitHub (<https://github.com/slint-ui/slint>).
* **Baselining:** Releases should be tagged using Semantic Versioning (SemVer). A safety-related release baseline could consist  of the specific git commit hash, the pinned compiler version, and the exact state of all verification artifacts at that point in time.
* **Nightly Builds:** Automated nightly builds generate documentation, examples, and release artifacts to provide continuous visibility into the master branch's stability.

## Change Management (ISO 26262-8 Clause 8)

Modifications to the Slint SC codebase are managed to preserve the safety and integrity of the system.

* **Reporting and Tracking:** Issues, bugs, and feature requests are tracked via GitHub Issues (<https://github.com/slint-ui/slint/issues>).
* **Impact Analysis:** Before implementing a change for a safety-related concern, an impact analysis is conducted to ensure the modification will not compromise existing safety mechanisms or introduce new hazards.
* **Code Reviews:** While administrators have push access, all safety-impacting changes must go through a formal Pull Request (PR) process. PRs enforce peer review, providing an opportunity to challenge the impact analysis and verify the implementation before merging.
* **Traceability:** Every functional change merged into the codebase must be traceable back to an established issue or requirement, ensuring comprehensive oversight.

## Software Component Qualification (ISO 26262-8 Clause 12)

### Dependencies

Because Slint SC is intended for embedded systems, there are very few internal dependencies, and exactly **zero external dependencies** for the core runtime portion of Slint SC.

For build-time tooling and optional features (e.g., image decoding), external libraries may be utilized.
* **Qualification Strategy:** Any necessary external Software of Unknown Kinematics (SOUP) must undergo rigorous evaluation, including static analysis, functional testing, and security auditing, to justify its suitability for reuse in a safety-related context before integration.

## Verification (ISO 26262-8 Clause 9)

A comprehensive verification strategy is employed to confirm that Slint SC meets its safety requirements. The CI pipeline enforces automated test execution, including:
* **Interpreter and API Tests:** Ensuring the core APIs behave as expected.
* **Syntax and Compiler Tests:** Verifying that the Slint compiler correctly parses and rejects invalid `.slint` markup.
* **Screenshot Tests:** Validating UI rendering fidelity against established references to catch visual regressions.

*Note: Specific test coverage metrics (e.g., MC/DC, statement coverage) and static analysis results are required for formal safety certification and are collected during the release baselining process.*

## Hardware Qualification (ISO 26262-8 Clause 13)

*Note: Clause 13 is Out of Scope.* Slint SC is a purely software-based UI toolkit. Qualification of the underlying hardware elements (e.g., MCU/MPU, memory, display controllers) operating the safety-critical UI is the responsibility of the system integrator.
