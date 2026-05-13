// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import type { Diagnostic } from "./backend";

/**
 * Represents an errors that can be emitted by the compiler.
 */
export class CompileError extends Error {
    /**
     * List of diagnostic items emitted while compiling .slint code.
     */
    diagnostics: Diagnostic[];

    /**
     * Creates a new CompileError.
     *
     * @param message human-readable description of the error.
     * @param diagnostics represent a list of diagnostic items emitted while compiling .slint code.
     */
    constructor(message: string, diagnostics: Diagnostic[]) {
        const formattedDiagnostics = diagnostics
            .map(
                (d) =>
                    `[${d.fileName ?? ""}:${d.lineNumber}:${d.columnNumber}] ${d.message}`,
            )
            .join("\n");

        let formattedMessage = message;
        if (diagnostics.length > 0) {
            formattedMessage += `\nDiagnostics:\n${formattedDiagnostics}`;
        }

        super(formattedMessage);
        this.diagnostics = diagnostics;
    }
}
