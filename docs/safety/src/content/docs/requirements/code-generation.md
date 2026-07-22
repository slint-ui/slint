---
title: "SR_CODE_GENERATION"
description: Code generation qualification requirement for Slint SC.
---

Slint-compiler's Rust output must be verifiable/qualified according to **ISO 26262-8 Clause 11 (Confidence in the use of software tools)**. This means
writing test cases that include each language feature of Slint SC, and verifying that the generated Rust code is correct.
