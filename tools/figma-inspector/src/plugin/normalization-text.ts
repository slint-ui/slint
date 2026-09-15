// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import {
    color,
    isMixed,
    number,
    problem,
    type MaterializedNode,
    type NormalizedGeometry,
    type NormalizationContext,
    type NodeResult,
} from "./normalization-context";
import { visiblePaints } from "./normalization-visual";
import type { ImageResolver } from "./normalize";
import type {
    Diagnostic,
    SnapshotTextAutoResize,
    SnapshotTextNode,
    SnapshotTextRun,
} from "./snapshot";
type MixedValue = unknown;

function normalizeTextAutoResize(
    value: unknown,
    mixedValue: MixedValue,
    node: MaterializedNode,
    warnings: Diagnostic[],
): SnapshotTextAutoResize {
    if (value === "NONE") return "none";
    if (value === "WIDTH_AND_HEIGHT") return "width-and-height";
    if (value === "HEIGHT") return "height";
    if (value === "TRUNCATE") return "truncate";
    warnings.push({
        ...problem(
            "MIXED_TEXT_STYLE_APPROXIMATED",
            node,
            isMixed(value, mixedValue)
                ? "Mixed text auto-resize uses the deterministic none fallback"
                : "Unavailable text auto-resize uses the deterministic none fallback",
            "textAutoResize",
        ),
        severity: "warning",
    });
    return "none";
}

type EffectiveFont = {
    readonly family: string;
    readonly style: string;
};

type IconTextInfo = {
    readonly fontName: EffectiveFont;
};

export function classifyNonInterText(
    node: MaterializedNode,
    mixedValue: MixedValue,
): boolean | { readonly error: Diagnostic } {
    if (node.characters.length === 0) return false;
    const isInter = (font: EffectiveFont) =>
        font.family.trim().toLowerCase() === "inter";
    const font = effectiveFont(node.fontName);
    if (font !== undefined && !isInter(font)) return true;
    const result = readStyledTextSegments(node);
    let coveredText = "";
    let complete = result !== undefined && "segments" in result;
    if (result !== undefined && "segments" in result) {
        for (const segment of result.segments) {
            if (!record(segment) || typeof segment.characters !== "string") {
                complete = false;
                continue;
            }
            if (segment.characters.length === 0) continue;
            const segmentFont = effectiveFont(segment.fontName ?? font);
            if (segmentFont !== undefined && !isInter(segmentFont)) return true;
            if (segmentFont === undefined) complete = false;
            coveredText += segment.characters;
        }
    }
    if (
        (isMixed(node.fontName, mixedValue) || font === undefined) &&
        (!complete || coveredText !== node.characters)
    )
        return {
            error: problem(
                "TEXT_FONT_UNRESOLVED",
                node,
                "Cannot determine all text fonts for conversion to an image",
                "fontName",
            ),
        };
    return false;
}

function record(value: unknown): value is Record<string, unknown> {
    return typeof value === "object" && value !== null;
}

function effectiveFont(value: unknown): EffectiveFont | undefined {
    if (
        !record(value) ||
        typeof value.family !== "string" ||
        typeof value.style !== "string"
    )
        return undefined;
    return { family: value.family, style: value.style };
}

function normalizedFontFamily(value: string): string {
    return value
        .normalize("NFKC")
        .toLocaleLowerCase("en-US")
        .replace(/[^\p{L}\p{N}]/gu, "");
}

function isLigatureFont(value: string): boolean {
    const family = normalizedFontFamily(value);
    return (
        [
            "materialicons",
            "materialsymbols",
            "fontawesome",
            "icomoon",
            "glyphicons",
            "phosphor",
            "remixicon",
        ].some((name) => family.includes(name)) ||
        family.includes("icon") ||
        family.includes("symbol")
    );
}

function isPrivateUseCodePoint(codePoint: number): boolean {
    return (
        (codePoint >= 0xe000 && codePoint <= 0xf8ff) ||
        (codePoint >= 0xf0000 && codePoint <= 0xffffd) ||
        (codePoint >= 0x100000 && codePoint <= 0x10fffd)
    );
}

function readStyledTextSegments(
    node: MaterializedNode,
): MaterializedNode["sourceSegments"] {
    return node.sourceSegments;
}

export function classifyIconText(
    node: MaterializedNode,
    mixedValue: MixedValue,
): IconTextInfo | undefined {
    const characters = node.characters;
    if (
        typeof characters !== "string" ||
        /[\r\n\u2028\u2029]/u.test(characters)
    )
        return undefined;
    const nodeFontMixed = isMixed(node.fontName, mixedValue);
    const nodeFontName = effectiveFont(node.fontName);
    if (nodeFontName === undefined && !nodeFontMixed) return undefined;

    const meaningful = Array.from(characters).filter(
        (character) => !/^\s$/u.test(character),
    );
    if (meaningful.length === 0) return undefined;
    const privateUse = meaningful.every((character) => {
        const codePoint = character.codePointAt(0);
        return codePoint !== undefined && isPrivateUseCodePoint(codePoint);
    });
    const nodeIconFont =
        nodeFontName !== undefined && isLigatureFont(nodeFontName.family);
    if (!privateUse && !nodeIconFont && !nodeFontMixed) return undefined;

    let representativeFont = nodeFontName;
    const styledTextResult = readStyledTextSegments(node);
    if (styledTextResult !== undefined) {
        if ("error" in styledTextResult) {
            return nodeFontName === undefined
                ? undefined
                : { fontName: nodeFontName };
        }
        const segments = styledTextResult.segments;
        let completeCoverage = segments.length > 0;
        let expectedEnd = 0;
        for (const segment of segments) {
            if (!record(segment) || typeof segment.characters !== "string")
                return undefined;
            const segmentCharacters = segment.characters;
            if (/[\r\n\u2028\u2029]/u.test(segmentCharacters)) return undefined;
            const hasStart = "start" in segment;
            const hasEnd = "end" in segment;
            if (hasStart !== hasEnd) return undefined;
            if (hasStart) {
                const start = segment.start;
                const end = segment.end;
                if (
                    !number(start) ||
                    !Number.isInteger(start) ||
                    !number(end) ||
                    !Number.isInteger(end) ||
                    start !== expectedEnd ||
                    end < start ||
                    end > characters.length ||
                    end - start !== segmentCharacters.length
                )
                    return undefined;
                expectedEnd = end;
            } else completeCoverage = false;
            if (segmentCharacters.length === 0) continue;
            const segmentFont = effectiveFont(segment.fontName ?? nodeFontName);
            if (segmentFont === undefined) return undefined;
            if (nodeFontMixed) {
                if (!isLigatureFont(segmentFont.family)) return undefined;
                representativeFont ??= segmentFont;
            } else if (
                nodeIconFont
                    ? segmentFont.family !== nodeFontName?.family
                    : segmentFont.family !== nodeFontName?.family ||
                      segmentFont.style !== nodeFontName?.style
            )
                return undefined;
        }
        if (completeCoverage && expectedEnd !== characters.length)
            return undefined;
    }
    if (representativeFont === undefined) return undefined;
    return { fontName: representativeFont };
}

export async function captureTableCellText(
    text: Record<string, unknown>,
    id: string,
    name: string,
    width: number,
    height: number,
    mixedValue: MixedValue,
    imageResolver: ImageResolver,
    warnings: Diagnostic[],
): Promise<SnapshotTextNode> {
    const fontName =
        !isMixed(text.fontName, mixedValue) &&
        typeof text.fontName === "object" &&
        text.fontName !== null &&
        typeof (text.fontName as Record<string, unknown>).family === "string" &&
        typeof (text.fontName as Record<string, unknown>).style === "string"
            ? (text.fontName as FontName)
            : { family: "Inter", style: "Regular" };
    const fontSize =
        !isMixed(text.fontSize, mixedValue) &&
        number(text.fontSize) &&
        text.fontSize > 0
            ? text.fontSize
            : 16;
    const fontWeight =
        !isMixed(text.fontWeight, mixedValue) && number(text.fontWeight)
            ? text.fontWeight
            : 400;
    const rawFills = Array.isArray(text.fills) ? text.fills : [];
    const fills = await visiblePaints(
        rawFills,
        { id, name } as unknown as MaterializedNode,
        mixedValue,
        "fills",
        imageResolver,
    );
    warnings.push(...fills.warnings);
    const fill = fills.paints.find((paint) => paint.kind === "solid");
    const characters = String(text.characters);
    return {
        kind: "text",
        id: `${id}:text`,
        name: `${name} text`,
        x: 8,
        y: 8,
        width: Math.max(0, width - 16),
        height: Math.max(0, height - 16),
        opacity: 1,
        visible: true,
        rotation: 0,
        layoutPositioning: "absolute",
        layoutSizingHorizontal: null,
        layoutSizingVertical: null,
        characters,
        fills: fills.paints,
        fontFamily: fontName.family,
        fontStyle: fontName.style,
        fontSize,
        fontWeight,
        horizontalAlign: "LEFT",
        verticalAlign: "CENTER",
        italic: fontName.style.toLowerCase().includes("italic"),
        letterSpacing: 0,
        lineHeightFactor: null,
        wrap: true,
        overflow: "clip",
        maxLines: null,
        textAutoResize: "none",
        runs: [
            {
                range: [0, characters.length],
                text: characters,
                color: fill?.kind === "solid" ? fill.color : null,
                bold: fontWeight >= 600,
                italic: fontName.style.toLowerCase().includes("italic"),
                underline: false,
                strike: false,
            },
        ],
    };
}

export async function normalizeText(
    node: MaterializedNode,
    base: NormalizedGeometry,
    context: NormalizationContext,
    captureWarnings: Diagnostic[],
): Promise<NodeResult> {
    const { mixedValue, imageResolver } = context;
    const textNode = node;
    type Segment = {
        characters: string;
        start: number;
        end: number;
        fontName?: FontName;
        fontSize?: number;
        fontWeight?: number;
        fontStyle?: string;
        textDecoration?: string;
        textCase?: string;
        fills?: ReadonlyArray<Paint>;
        letterSpacing?: LetterSpacing;
        lineHeight?: LineHeight;
    };
    let segments: Segment[] = [];
    const styledTextResult = readStyledTextSegments(textNode);
    if (styledTextResult !== undefined) {
        if ("error" in styledTextResult) {
            captureWarnings.push({
                ...problem(
                    "TEXT_SEGMENTS_APPROXIMATED",
                    node,
                    styledTextResult.error instanceof Error
                        ? `Styled text segments were unavailable: ${styledTextResult.error.message}`
                        : `Styled text segments were unavailable: ${String(styledTextResult.error)}`,
                    "getStyledTextSegments",
                ),
                severity: "warning",
            });
            segments = [];
        } else segments = styledTextResult.segments as unknown as Segment[];
    }
    const baseSegment =
        segments.find((segment) => segment.characters.length > 0) ??
        segments[0];
    const textCaseValue =
        "textCase" in textNode ? textNode.textCase : undefined;
    const mixedTextStyle = [
        textNode.fontName,
        textNode.fontSize,
        textNode.fontWeight,
        textCaseValue,
    ].some((value) => isMixed(value, mixedValue));
    const fontName =
        !isMixed(textNode.fontName, mixedValue) &&
        typeof textNode.fontName === "object"
            ? (textNode.fontName as FontName)
            : (baseSegment?.fontName ?? {
                  family: "Inter",
                  style: "Regular",
              });
    const fontSize =
        !isMixed(textNode.fontSize, mixedValue) && number(textNode.fontSize)
            ? textNode.fontSize
            : (baseSegment?.fontSize ?? 16);
    const fontWeight =
        !isMixed(textNode.fontWeight, mixedValue) && number(textNode.fontWeight)
            ? textNode.fontWeight
            : (baseSegment?.fontWeight ?? 400);
    if (!number(fontSize) || fontSize <= 0 || !number(fontWeight))
        return {
            error: problem(
                "INVALID_TEXT_STYLE",
                node,
                "Text font size and weight must be finite",
                "textStyle",
            ),
        };
    if (mixedTextStyle)
        captureWarnings.push({
            ...problem(
                "MIXED_TEXT_STYLE_APPROXIMATED",
                node,
                "Mixed text typography uses the first non-empty text style while preserving supported run styling",
                "textStyle",
            ),
            severity: "warning",
        });
    const baseLetterSpacing = segments.find(
        (segment) => segment.letterSpacing !== undefined,
    )?.letterSpacing;
    const baseLineHeight = segments.find(
        (segment) => segment.lineHeight !== undefined,
    )?.lineHeight;
    const sameJsonValue = (left: unknown, right: unknown): boolean =>
        JSON.stringify(left) === JSON.stringify(right);
    const unsupportedRunVariation = segments.some(
        (segment) =>
            (segment.fontName !== undefined &&
                (segment.fontName.family !== fontName.family ||
                    segment.fontName.style !== fontName.style)) ||
            (segment.fontSize !== undefined && segment.fontSize !== fontSize) ||
            (segment.fontWeight !== undefined &&
                segment.fontWeight !== fontWeight) ||
            (segment.letterSpacing !== undefined &&
                !sameJsonValue(segment.letterSpacing, baseLetterSpacing)) ||
            (segment.lineHeight !== undefined &&
                !sameJsonValue(segment.lineHeight, baseLineHeight)),
    );
    if (unsupportedRunVariation && !mixedTextStyle)
        captureWarnings.push({
            ...problem(
                "MIXED_TEXT_STYLE_APPROXIMATED",
                node,
                "Per-run font family, size, or weight variations use the first non-empty base style",
                "textStyle",
            ),
            severity: "warning",
        });
    const fills = isMixed(textNode.fills, mixedValue)
        ? await visiblePaints(
              baseSegment?.fills ?? [],
              node,
              mixedValue,
              "fills",
              imageResolver,
          )
        : await visiblePaints(
              textNode.fills,
              node,
              mixedValue,
              "fills",
              imageResolver,
          );
    captureWarnings.push(...fills.warnings);
    const spacingValue =
        "letterSpacing" in textNode
            ? (textNode.letterSpacing as unknown)
            : undefined;
    const letterSpacingObject =
        spacingValue !== undefined &&
        !isMixed(spacingValue, mixedValue) &&
        typeof spacingValue === "object" &&
        spacingValue !== null
            ? (spacingValue as LetterSpacing)
            : baseSegment?.letterSpacing;
    const letterSpacing =
        letterSpacingObject !== undefined && number(letterSpacingObject.value)
            ? letterSpacingObject.value *
              (letterSpacingObject.unit === "PERCENT" ? fontSize / 100 : 1)
            : 0;
    const lineHeightValue =
        "lineHeight" in textNode ? (textNode.lineHeight as unknown) : undefined;
    if (
        (spacingValue !== undefined && isMixed(spacingValue, mixedValue)) ||
        (lineHeightValue !== undefined && isMixed(lineHeightValue, mixedValue))
    )
        captureWarnings.push({
            ...problem(
                "MIXED_TEXT_STYLE_APPROXIMATED",
                node,
                "Mixed text spacing uses the first non-empty text style",
                "textStyle",
            ),
            severity: "warning",
        });
    const effectiveLineHeight =
        lineHeightValue !== undefined && !isMixed(lineHeightValue, mixedValue)
            ? lineHeightValue
            : baseSegment?.lineHeight;
    const lineHeight =
        typeof effectiveLineHeight === "object" &&
        effectiveLineHeight !== null &&
        ((effectiveLineHeight as LineHeight).unit === "PERCENT" ||
            (effectiveLineHeight as LineHeight).unit === "PIXELS") &&
        number((effectiveLineHeight as { value: number }).value)
            ? (effectiveLineHeight as LineHeight).unit === "PERCENT"
                ? (effectiveLineHeight as { value: number }).value / 100
                : (effectiveLineHeight as { value: number }).value / fontSize
            : null;
    const textAutoResize = normalizeTextAutoResize(
        "textAutoResize" in textNode
            ? (textNode.textAutoResize as unknown)
            : undefined,
        mixedValue,
        node,
        captureWarnings,
    );
    const textTruncation =
        "textTruncation" in textNode ? textNode.textTruncation : "DISABLED";
    const maxLines = "maxLines" in textNode ? textNode.maxLines : null;
    if (
        ("textTruncation" in textNode && isMixed(textTruncation, mixedValue)) ||
        ("maxLines" in textNode && isMixed(maxLines, mixedValue))
    )
        captureWarnings.push({
            ...problem(
                "MIXED_TEXT_STYLE_APPROXIMATED",
                node,
                "Mixed text sizing uses deterministic node-level fallbacks",
                "textStyle",
            ),
            severity: "warning",
        });
    const textCase = (value: string | undefined, text: string): string => {
        if (value === "UPPER") return text.toUpperCase();
        if (value === "LOWER") return text.toLowerCase();
        if (value === "TITLE")
            return text.replace(
                /(^|\s)(\S)/g,
                (_, prefix: string, character: string) =>
                    `${prefix}${character.toUpperCase()}`,
            );
        return text;
    };
    const nodeTextCase =
        !isMixed(textCaseValue, mixedValue) && typeof textCaseValue === "string"
            ? textCaseValue
            : undefined;
    const displayedSegments =
        segments.length > 0
            ? segments
            : [
                  {
                      characters: textNode.characters,
                      start: 0,
                      end: textNode.characters.length,
                      fontName,
                      fontSize,
                      fontWeight,
                      fontStyle: fontName.style,
                      textDecoration: undefined,
                      textCase: nodeTextCase,
                      fills: Array.isArray(textNode.fills)
                          ? textNode.fills
                          : undefined,
                  },
              ];
    const displayedCharacters = displayedSegments
        .map((segment) =>
            textCase(segment.textCase ?? nodeTextCase, segment.characters),
        )
        .join("");
    let displayedOffset = 0;
    const runs: SnapshotTextRun[] = displayedSegments.map((segment) => {
        const displayedText = textCase(
            segment.textCase ?? nodeTextCase,
            segment.characters,
        );
        const segmentFill = segment.fills?.find(
            (item) => item.visible !== false && item.type === "SOLID",
        ) as SolidPaint | undefined;
        const range: [number, number] = [
            displayedOffset,
            displayedOffset + displayedText.length,
        ];
        displayedOffset = range[1];
        return {
            range,
            text: displayedText,
            color:
                segmentFill === undefined
                    ? null
                    : color(
                          {
                              ...segmentFill.color,
                              a: segmentFill.opacity ?? 1,
                          },
                          node,
                      ),
            bold: number(segment.fontWeight)
                ? segment.fontWeight >= 600
                : false,
            italic:
                segment.fontStyle?.toLowerCase().includes("italic") ?? false,
            underline: segment.textDecoration === "UNDERLINE",
            strike: segment.textDecoration === "STRIKETHROUGH",
        };
    });
    return {
        node: {
            ...base,
            kind: "text",
            characters: displayedCharacters,
            fills: fills.paints,
            fontFamily: fontName.family,
            fontStyle: fontName.style,
            fontSize,
            fontWeight,
            textAutoResize,
            horizontalAlign: textNode.textAlignHorizontal,
            verticalAlign: textNode.textAlignVertical,
            italic: fontName.style.toLowerCase().includes("italic"),
            letterSpacing,
            lineHeightFactor: lineHeight,
            wrap: textAutoResize === "none" || textAutoResize === "height",
            overflow:
                textTruncation === "ENDING" || textAutoResize === "truncate"
                    ? "elide"
                    : "clip",
            maxLines,
            runs,
        },
        warnings: [...captureWarnings],
    };
}
