// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { Diagnostic } from "../plugin/snapshot";

export function exportFailure(
    error: unknown,
    code = "EXPORT_UNAVAILABLE",
): Diagnostic {
    const message = error instanceof Error ? error.message : String(error);
    return {
        severity: "warning",
        category: "export",
        code,
        message: `Export unavailable: ${message}; Export ZIP is disabled`,
    };
}
