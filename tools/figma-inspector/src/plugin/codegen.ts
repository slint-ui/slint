// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { captureSource, captureCodegenVariables } from "./capture";
import { normalizeSource } from "./normalize";
import { convertSnapshot } from "../preview/converter";
import type { Diagnostic } from "./snapshot";

function diagnostics(items: readonly Diagnostic[]): CodegenResult[] {
    return items.length
        ? [
              {
                  title: "Diagnostics",
                  language: "PLAINTEXT",
                  code: items
                      .map((item) => `${item.code}: ${item.message}`)
                      .join("\n"),
              },
          ]
        : [];
}

export async function generateCodegen(
    node: SceneNode,
    mixed: unknown,
    useVariables = false,
): Promise<CodegenResult[]> {
    try {
        const captured = await captureSource(
            node,
            mixed,
            undefined,
            undefined,
            undefined,
            1,
            false,
            undefined,
            undefined,
            4,
            false,
            undefined,
            undefined,
            false,
            "root-only",
        );
        const normalized = await normalizeSource(captured.source, "export");
        if (!normalized.ok) return diagnostics(normalized.diagnostics);
        if (normalized.empty) return [];
        const converted = convertSnapshot(normalized.snapshot, {
            scope: "root-only",
            codegenVariables: useVariables
                ? await captureCodegenVariables(captured.source.root)
                : [],
        });
        if (!converted.ok) return diagnostics(converted.diagnostics);
        return [
            {
                title: `Slint Code: ${captured.source.root.name}`,
                language: "CSS",
                code: converted.source,
            },
            ...diagnostics([...normalized.warnings, ...converted.warnings]),
        ];
    } catch (error) {
        return diagnostics([
            {
                severity: "error",
                code: "CODEGEN_ERROR",
                message: error instanceof Error ? error.message : String(error),
            },
        ]);
    }
}
