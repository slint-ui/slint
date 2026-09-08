// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { binding as property, type SlintLine } from "./slint-ir";
import {
    color,
    escaped,
    number,
    paint,
    type RenderContext,
} from "./converter-context";
import type { SnapshotTextNode } from "../plugin/snapshot";

function markdownEscaped(value: string): string {
    return (
        value
            .replaceAll("\\", "\\\\")
            .replaceAll("&", "&amp;")
            .replaceAll("<", "&lt;")
            .replaceAll(">", "&gt;")
            // Escape the remaining CommonMark punctuation so design copy cannot
            // accidentally become a heading, list, rule, link, table, or other
            // parser construct. HTML entities above deliberately run first so
            // their ampersands and semicolons remain valid entities.
            .replaceAll("!", "\\!")
            .replaceAll("#", "\\#")
            .replaceAll("*", "\\*")
            .replaceAll("+", "\\+")
            .replaceAll("-", "\\-")
            .replaceAll(".", "\\.")
            .replaceAll("?", "\\?")
            .replaceAll("@", "\\@")
            .replaceAll("_", "\\_")
            .replaceAll("~", "\\~")
            .replaceAll("[", "\\[")
            .replaceAll("]", "\\]")
            .replaceAll("{", "\\{")
            .replaceAll("}", "\\}")
            .replaceAll("(", "\\(")
            .replaceAll(")", "\\)")
            .replaceAll("|", "\\|")
            .replaceAll("^", "\\^")
            .replaceAll("`", "\\`")
            .replaceAll("\r", "")
            .replaceAll("\n", "\n")
    );
}

function markdownRun(run: SnapshotTextNode["runs"][number]): string {
    let text = markdownEscaped(run.text);
    if (run.bold) text = `**${text}**`;
    if (run.italic) text = `*${text}*`;
    if (run.strike) text = `~~${text}~~`;
    if (run.underline) text = `<u>${text}</u>`;
    if (run.color !== null)
        text = `<font color="${color(run.color)}">${text}</font>`;
    return text;
}

function styledTextSource(node: SnapshotTextNode, depth: number): SlintLine[] {
    const markdown = node.runs.map(markdownRun).join("");
    return [property("text", `@markdown("${escaped(markdown)}")`, depth)];
}

export function textRunNeedsStyled(text: SnapshotTextNode): boolean {
    if (text.runs.length === 0) return false;
    const baseFill = text.fills[0];
    const baseColor =
        baseFill !== undefined && baseFill.kind === "solid"
            ? baseFill.color
            : null;
    const sameColor = (
        left: typeof baseColor,
        right: typeof baseColor,
    ): boolean =>
        left === null
            ? right === null
            : right !== null &&
              left.r === right.r &&
              left.g === right.g &&
              left.b === right.b &&
              left.a === right.a;
    return text.runs.some(
        (run) =>
            run.bold !== text.fontWeight >= 600 ||
            run.italic !== text.italic ||
            run.underline ||
            run.strike ||
            !sameColor(run.color, baseColor),
    );
}
export function textSource(
    node: SnapshotTextNode,
    styledText: boolean,
    context: RenderContext,
    depth: number,
): SlintLine[] {
    const lines: SlintLine[] = [];
    const fill = node.fills[0];
    if (
        styledText &&
        (node.letterSpacing !== 0 ||
            node.lineHeightFactor !== null ||
            (node.overflow === "elide" && node.maxLines === null))
    )
        context.warnings.push({
            severity: "warning",
            code: "MIXED_TEXT_STYLE_APPROXIMATED",
            nodeId: node.id,
            nodeName: node.name,
            propertyPath: "textStyle",
            message:
                "StyledText preserves supported markup; letter spacing, line height, and some elision settings are approximated",
        });
    if (fill !== undefined && !styledText) {
        if (fill.kind === "image")
            context.warnings.push({
                severity: "warning",
                code: "IMAGE_TEXT_FILL_IGNORED",
                nodeId: node.id,
                nodeName: node.name,
                propertyPath: "fills",
                message:
                    "Image text fills are not supported by Slint Text and were ignored",
            });
        else lines.push(property("color", paint(fill), depth));
    }
    if (styledText) lines.push(...styledTextSource(node, depth));
    else lines.push(property("text", `"${escaped(node.characters)}"`, depth));
    if (styledText && fill !== undefined && fill.kind !== "image")
        lines.push(property("default-color", paint(fill), depth));
    lines.push(
        property(
            styledText ? "default-font-family" : "font-family",
            `"${escaped(node.fontFamily)}"`,
            depth,
        ),
        property(
            styledText ? "default-font-size" : "font-size",
            `${number(node.fontSize)}px`,
            depth,
        ),
    );
    if (!styledText)
        lines.push(property("font-weight", node.fontWeight, depth));
    lines.push(
        property(
            "horizontal-alignment",
            node.horizontalAlign.toLowerCase(),
            depth,
        ),
    );
    if (node.verticalAlign !== "TOP")
        lines.push(
            property(
                "vertical-alignment",
                node.verticalAlign.toLowerCase(),
                depth,
            ),
        );
    if (!styledText && node.italic)
        lines.push(property("font-italic", true, depth));
    if (
        !styledText &&
        typeof node.letterSpacing === "number" &&
        node.letterSpacing !== 0
    )
        lines.push(
            property(
                "letter-spacing",
                `${number(node.letterSpacing)}px`,
                depth,
            ),
        );
    if (
        !styledText &&
        typeof node.lineHeightFactor === "number" &&
        node.lineHeightFactor !== 1
    )
        lines.push(
            property("line-height-factor", node.lineHeightFactor, depth),
        );
    if (!styledText && node.wrap)
        lines.push(property("wrap", "word-wrap", depth));
    if (!styledText && node.overflow === "elide")
        lines.push(property("overflow", "elide", depth));
    if (typeof node.maxLines === "number")
        lines.push(property("max-lines", node.maxLines, depth));
    return lines;
}
