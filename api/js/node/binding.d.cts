// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// The default and the "dev" binary share the exact same napi surface, so the
// generated type definitions of the default binary describe both. See
// binding.cjs for the runtime loading logic.

export * from "./rust-module.d.cts";
