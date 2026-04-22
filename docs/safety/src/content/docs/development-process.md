---
title: Development Process
description: Tools, processes, and infrastructure used for Slint SC development.
---


# The Development Process (ISO 26262-8 11.4.8)

This section describes the tools we use and the processes we follow to develop Slint SC, aiming to fulfill the supporting process requirements of ISO 26262 Part 8 (Sections 6-12)

## Organization members

The administrators of the slint-ui Github organization are the founders of SixtyFPS GmbH.
All employees of SixtyFPS GmbH are members of the organization.
External contributors may also be invited to become members of the Github organization.

## Language and Compiler

The `slint-compiler` translates `.slint` files into Rust code. Slint SC is itself written in Rust and is built using Ferrocene version 26.02.0.

* **Tool Impact (TI2):** The `slint-compiler` can introduce errors into the executable.
* **Tool Error Detection (TD2/TD3):** While Rust's strong type system and borrow checker catch many errors, compiler bugs might still bypass detection.
* **Tool Confidence Level (TCL):** The combination of TI2 and TD2/TD3 results in a Tool Confidence Level of TCL2 or TCL3.
* **Qualification Strategy:** As a high-impact tool, Ferrocene is already ASIL D qualified for safety-critical deployment. The `slint-compiler` is not yet qualified and must be qualified before being used in a safety-critical project.

## Configuration Management (ISO 26262-8 7.x)

Slint SC utilizes a structured configuration management process to ensure artifacts are reproducible and traceable.

* **Version Control:** Slint is hosted on GitHub (<https://github.com/slint-ui/slint>).
* **Baselining:** Releases are tagged in git as `release/MajorVersion.MinorVersion`.
* **Nightly Builds:** Automated nightly builds generate documentation, examples, and release artifacts to provide continuous visibility into the master branch's stability.

## Change Management (ISO 26262-8 8.x)

Modifications to the Slint SC codebase are managed to preserve the safety and integrity of the system.

* **Reporting and Tracking:** Issues, bugs, and feature requests are tracked via GitHub Issues (<https://github.com/slint-ui/slint/issues>).
* **Impact Analysis:** Before implementing a change for a safety-related concern, an impact analysis is conducted to ensure the modification will not compromise existing safety mechanisms or introduce new hazards.
* **Code Reviews:** While administrators have push access, all safety-impacting changes must go through a formal Pull Request (PR) process. PRs enforce peer review, providing an opportunity to challenge the impact analysis and verify the implementation before merging.
* **Traceability:** Every functional change merged into the codebase must be traceable back to an established issue or requirement, ensuring comprehensive oversight.

## CI System and Infrastructure

Continuous Integration (CI) is achieved through GitHub Actions, a platform provided by GitHub that is used to automate building, testing, and deployment of software. Here are some definitions of terms:

* **An Action** is a custom application that performs a complex but frequent repetitive task.
* **A job** is a set of steps that are either expressed as shell scripts or as actions. Steps are executed based on their ordering and dependencies, where data can be shared between steps.
* **A Workflow** is a configurable automated process that runs one or more jobs. Workflows are triggered by events, and executed by runners.
* **An event** is a specific activity associated with a GitHub repository, such as opening an issue or creating a PR.
* **A runner** is a server that executes triggered workflows.

For Slint, [Actions](https://github.com/slint-ui/slint/actions) are triggered for any Pull Request (PR) or direct push to the `master` branch.

(TODO: Show the Actions for Slint SC when we have some)

* **Tool Impact (TI1):** The CI system schedules and runs tests but does not generate the final executable code itself. Failures during CI block merging the PR.
* **Qualification Strategy:** Verification of the CI pipeline is achieved through increased confidence from use.

## Software Component Qualification (ISO 26262-8 12.x)

To ensure that software tools used in the development of Slint SC do not introduce or fail to detect errors, tools are assessed based on their Tool Impact (TI) and Tool Error Detection (TD) to determine their Tool Confidence Level (TCL).

### Dependencies

Because Slint SC is intended for embedded systems, there are very few internal dependencies, and exactly **zero external dependencies** for the core runtime portion of Slint SC.

For build-time tooling and optional features (e.g., image decoding), external libraries may be utilized.

(TODO: List specific dependencies here)

* **Qualification Strategy:** Any necessary external Software of Unknown Provenance (SOUP) must undergo rigorous evaluation, including static analysis, functional testing, and security auditing, to justify its suitability for reuse in a safety-related context before integration.

## Distributed Development (ISO 26262-8 5.x)

Employees of SixtyFPS GmbH are located in different countries, and some work remotely. This is not a problem, as we have all the necessary infrastructure in place to support this.

(TODO: Add more details about how distributed development is supported)

## Release Schedule

(TODO: add release schedule)

## Verification (ISO 26262-8 9.4.x)

A comprehensive verification strategy is employed to confirm that Slint SC meets its safety requirements. The CI pipeline enforces automated test execution, including:
* **Interpreter and API Tests:** Ensuring the core APIs behave as expected.
* **Syntax and Compiler Tests:** Verifying that the Slint compiler correctly parses and rejects invalid `.slint` markup.
* **Screenshot Tests:** Validating UI rendering fidelity against established references to catch visual regressions.

*Note: Specific test coverage metrics (e.g., MC/DC, statement coverage) and static analysis results are required for formal safety certification and are collected during the release baselining process.*

## Hardware Qualification (ISO 26262-8 13.x)

*Note: Section 13 is Out of Scope.* Slint SC is a purely software-based UI toolkit. Qualification of the underlying hardware elements (e.g., MCU/MPU, memory, display controllers) operating the safety-critical UI is the responsibility of the system integrator.
