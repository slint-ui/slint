// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import {
    codegenVariablePath,
    type CodegenVariable,
} from "../plugin/codegen-variables";
import type { Diagnostic } from "../plugin/snapshot";
import { binding, reference, type SlintLine } from "./slint-ir";

/** Replace only native root bindings; generated helpers retain resolved values. */
export function applyCodegenVariables(
    lines: SlintLine[],
    variables: readonly CodegenVariable[],
    warnings: Diagnostic[],
) {
    const fields: Record<string, string> = {
        x: "x",
        y: "y",
        width: "width",
        height: "height",
        opacity: "opacity",
        cornerRadius: "border-radius",
        topLeftRadius: "border-top-left-radius",
        topRightRadius: "border-top-right-radius",
        bottomLeftRadius: "border-bottom-left-radius",
        bottomRightRadius: "border-bottom-right-radius",
        strokeWeight: "border-width",
        characters: "text",
        fontFamily: "font-family",
        fontSize: "font-size",
        fontWeight: "font-weight",
    };
    const root = lines.find((line) => line.kind === "open" && line.depth === 0);
    for (const variable of variables) {
        let field = fields[variable.field];
        if (variable.field === "fills")
            field =
                root?.kind === "open" && root.type === "Text"
                    ? "color"
                    : root?.kind === "open" && root.type === "Path"
                      ? "fill"
                      : "background";
        if (variable.field === "strokes")
            field =
                root?.kind === "open" && root.type === "Path"
                    ? "stroke"
                    : "border-color";
        if (
            variable.field === "strokeWeight" &&
            root?.kind === "open" &&
            root.type === "Path"
        )
            field = "stroke-width";
        let line = lines.find(
            (line) =>
                line.kind === "binding" &&
                line.depth === 1 &&
                line.name === field,
        );
        // Default-valued native properties can be omitted from the literal IR.
        // Keep bindings on simple native roots without overriding raster helpers.
        if (
            !line &&
            root?.kind === "open" &&
            !lines.some((item) => item.kind === "open" && item.depth > 0) &&
            ((field === "opacity" &&
                ["Rectangle", "Text", "Path"].includes(root.type)) ||
                (root.type === "Rectangle" &&
                    [
                        "border-radius",
                        "border-top-left-radius",
                        "border-top-right-radius",
                        "border-bottom-left-radius",
                        "border-bottom-right-radius",
                        "border-width",
                    ].includes(field)))
        ) {
            line = binding(field, 0, 1);
        }
        const path = codegenVariablePath(variable);
        const type = line?.kind === "binding" ? line.value.type : undefined;
        const compatible =
            variable.type === "COLOR"
                ? type === "brush"
                : variable.type === "STRING"
                  ? type === "string"
                  : variable.type === "FLOAT"
                    ? ["length", "float", "int"].includes(type ?? "")
                    : false;
        if (path && compatible && line?.kind === "binding") {
            line.value = reference(line.value.type, path);
            if (!lines.includes(line)) lines.splice(1, 0, line);
        } else
            warnings.push({
                severity: "warning",
                code: "CODEGEN_VARIABLE_FALLBACK",
                propertyPath: variable.field,
                message: `Kept the resolved value for ${variable.field}: its variable or native binding is unavailable or unsupported`,
            });
    }
}
