
# Slint SC Safety Manual

Slint SC is the “ISO 26262 compliant subset” of Slint.
It does not yet exist. This document is a Work In Progress.

## Purpose of Document

This document contains a Safety Manual and a Qualification Plan.

The Safety Manual lists the requirements for ISO 26262, and
describes slint-compiler, its library components, and how to use them safely.

The Qualification Plan contains lists of possible failures, known issues, and descriptions of related test cases. This document needs to be updated after each version release.

### Audience

Currently, this is an internal document for Slint SC developers, but when there is a Slint SC product ready, this document will be for people who want to use Slint SC to build automotive software and certify it for ISO 26262.

## Policy and strategy for achieving functional safety

The safety and quality of Slint SC are the primary objectives of all management, development, maintenance and support activities. In order to achieve these goals, all employees must be involved in the quality assurance measures. This requires

* Adequate qualification of employees
* A culture of safe working
* Compliance with the safety life cycle to avoid systematic errors
* Precise, complete documentation
* Compliance with procedures and measures
* Independent verification of all work results
* Processes are work products that are constantly being improved.
* Technical support, use of tools, automation where possible.

## External assessment of functional safety

The functional safety of Slint SC will be assessed independently by TÜV Nord in Germany.

## ISO 26262 Requirements for Slint SC

The ISO 26262 standard tells us what properties a safety-critical system must have (traceability, freedom from interference, determinism, etc.), but it doesn't tell us how to write those requirements for a GUI toolkit. The following sections contain some specific, actionable engineering requirements that should be considered for Slint SC.

### ASIL B Capable

ASIL (Automotive Safety Integrity Level) describes the risk level of something. ASIL D=highest, C=high, B=medium, A=low risk, QM = not safety critical.

Since a compiler and a toolkit don’t have a specific vehicle function, they don’t have an intrinsic ASIL derived from a HARA (Hazard Analysis and Risk Assessment).

Slint SC is a “Safety Element out of Context” (SEooC). The GUI components of Slint SC may be used for mission-critical digital instrument clusters, or they may be used for non-critical applications such as infotainment systems.

Each Requirement below has a descriptive ID that begins with SR_, a description, and ASIL=B.

### SR_SAFE_RUST_CODING_STANDARDS

Slint SC Rust code must adhere to the **Ferrocene Language Specification** (the ISO 26262 ASIL D qualified Rust toolchain) and the emerging **AUTOSAR Rust guidelines**. Slint SC must also avoid, or properly encapsulate,
document, justify, and test the use of any unsafe features of Rust.

* https://doc.rust-lang.org/stable/reference/unsafety.html

### SR_STATIC_MEMORY_ALLOCATION

According to the standard, Slint SC should not perform dynamic memory allocation during the continuous rendering loop. All memory pools, vertex buffers, and command buffers should be pre-allocated.

Slint SC is written in `no_std` rust, so it does not use the standard library. However, it currently does make use of a global allocator. This is a **known issue** (bugID?) that we plan to address in the future.

**(Reference: ISO 26262-6 Annex D.2.2 "Memory management", which identifies "unbounded memory consumption" and "memory leaks" as interference faults.)**

### SR_BOUNDED_EXECUTION_TIME

Slint SC shall guarantee a strictly bounded maximum execution time for rendering a single frame, ensuring that the critical rendering loop never blocks the main execution thread beyond the hardware display refresh interval (e.g., 16.6ms for 60Hz).

**(Reference: ISO 26262-6 Annex D.2.2 "Timing and execution", which identifies "incorrect allocation of execution time" and "blocking of execution" as interference faults.)**

### SR_STATE_MACHINE_DETERMINISM

Slint SC's internal state machine for UI component lifecycle, event propagation, and rendering state must be fully deterministic and reproducible given a specific sequence of inputs.

### SR_RESOURCE_FALLBACK

If an external graphical asset (e.g., image, font glyph, 3D mesh) is corrupted, missing, or fails to decode, the toolkit shall not crash or halt rendering. Missing resources should be detected at compile-time in Slint SC.

### SR_CODE_GENERATION

Slint-compiler’s Rust output must be verifiable/qualified according to **ISO 26262-8 Clause 11 (Confidence in the use of software tools)**. This means
writing test cases that include each language feature of Slint SC, and verifying that the generated Rust code is correct.

### SR_TEST_COVERAGE

Slint SC must support headless or offline automated testing frameworks capable of running on CI/CD pipelines to verify layout engines, event propagation, and state transitions.

Code coverage is highly measurable in Rust using LLVM source-based coverage tools such as `cargo-tarpaulin` or `grcov`. The goal for ASIL D is typically strict structural coverage, requiring >90% statement/branch coverage and often Modified Condition/Decision Coverage (MC/DC).

### SR_SEPARATION_OF_CONCERNS

Slint SC architecture enforces the separation of business logic (backend/state) from presentation logic (frontend/pixels).

### SR_CONCURRENCY_CONTROL

To avoid race conditions that could yield incorrect displays, the core UI update, layout, and rendering commands must execute sequentially on a single managed thread or explicitly defined thread pool with static concurrency constraints. This makes it possible to show that the core runtime, especially the property binding evaluation and Z-ordering layout mechanisms, are fully deterministic, bounded, and provably testable.

## Components of the System

This is what is included in Slint SC:

* A Slint Compiler, from the internal `i-slint-compiler` crate
* The `slint_build` crate which provides a Rust API for the compiler.
* Individual slint language features
* Features offered by specific crates that are part of the Slint Rust library

Each of these things can have a **Usage** and a **Constraints** section.

Each feature of the language or a library can map to a Requirement ID, and have 1 or more code test-examples.

<Insert diagram that shows how the components/crates relate to each other?>

### Installation Procedures

Slint SC does not exist yet, so you can't install it yet.

### Compiling Slint into Rust

Rust developers using Slint SC can instantiate and configure a `CompilerConfiguration` from the [`slint_build`](https://docs.slint.dev/latest/docs/rust/slint_build/) crate to compile `.slint` files into Rust.

This structure is typically created and used from a [Rust build script](https://doc.rust-lang.org/cargo/reference/build-scripts.html), `build.rs`, located in the root directory of the package. After it has the correct
values, it can be passed to `slint_build::compile_with_config`.
Here is a simple example:

```
fn main() {
    let mut config = slint_build::CompilerConfiguration::new();
    // [ ... ] set some values on config here
    slint_build::compile_with_config("mainFile.slint", config).unwrap();
}
```

### Constraints

The standard essentially views a **Requirement** as what the system *must do* (or a property it must have), whereas a **Constraint** is a boundary condition that *limits the solution space*.

For APIs, the Constraints might explain that some functions are experimental and can not be used safely yet. Or, that certain values passed as parameters into functions are not supported in Slint SC. In other words, certain features can only be used a certain way to be safe.

Individual Constraints can have a section each here, with a descriptive ID that begins with CON_, and a Rationale, Impact, and Mitigation.

## Slint SC Development Cycle

This section describes what tools we use and processes we follow, to develop slint. (ISO 26262-8 sections 11, 12 and 13)

### Language / Compiler

Slint SC is written in Rust and requires rustc+cargo version 1.88 or newer to build it.

### Dependencies

Because Slint SC is intended for embedded systems, there are very few internal
dependencies, and no external dependencies for the runtime portion of Slint SC.

At build-time, there may be dependencies on certain external libraries, for the purposes
of image decoding.

### Procedure for Version Control/Configuration Management

Slint is an open-source project hosted on GitHub. <https://github.com/slint-ui/slint>

### Code Reviews

Admins can push directly to the master branch, but all other
changes must go through a code review process, involving Pull Requests (PRs).
PRs enable developers to easily review each other's work, add specific comments,
ask questions, and allow/deny changes.

### CI system used

Continuous Integration (CI) is driven by Github Actions. The actions are  triggered for any PR (including drafts) to the master branch, or to a branch that starts with feature/. Actions are also triggered for every push to the master branch. A new commit on a branch will cancel the previous action on that branch if it is still running.

### Nighly Builds

There is a nightly build that runs every night. It will build release artifacts and upload them to the nightly Github release. It will also generate the docs and examples, and publish them on snapshots.slint.dev/master

It will also run a few extra tests (like running cargo update to check for broken dependencies).

### Reporting Bugs

Issues are tracked in github: <https://github.com/slint-ui/slint/issues>

# Qualification Plan

The ISO 26262 standard requires us to track and report known safety-critical issues and possible failures, documenting for each how it arises, what issue# addresses it, what test case tests it, and what version it is fixed in.

## Failure Scenarios

Specific possible failure scenarios (especially in the context of software running on a car) each get their own section here. Discovery of these scenarios is done as part of the Hazard Analysis and Risk Assessment (HARA) phase.

## Known Issues

This section will describe the safety critical issues (referenced by github issue ID) that we have faced, Which ones are fixed, fixed in which version, and which testcases test them.

This list/table could be auto-generated based on what is in the github issue database, if the relevant issues were each properly tagged with “safety-critical”.

### Global Allocator

Slint SC makes use of a global allocator. This is a known issue (bugID?) that we plan to address in the future.

## Validation - Running the Tests

* Validation activities (see ISO 26262-4 section 9)

Validation involves running a set of safety-critical tests. We describe how to run them here, and how to understand the results.  Each test should be described in a section here, tagged with appropriate requirement IDs.

We should probably write a Validator GUI that runs the safety-critical tests and shows the results in a nice way, and then the instructions for running Validator will go here.

