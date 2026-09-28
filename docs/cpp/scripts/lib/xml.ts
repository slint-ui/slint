// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell:ignore apos hellip ndash

// A tiny, dependency-free XML parser, just enough for the well-formed XML that
// Doxygen emits. It is deliberately small: Doxygen output is machine generated,
// so we don't need to handle every pathological case a general XML parser must.

export interface XmlElement {
    type: "element";
    name: string;
    attrs: Record<string, string>;
    children: XmlNode[];
}

export interface XmlText {
    type: "text";
    value: string;
}

export type XmlNode = XmlElement | XmlText;

const NAMED_ENTITIES: Record<string, string> = {
    amp: "&",
    lt: "<",
    gt: ">",
    quot: '"',
    apos: "'",
    nbsp: " ",
    mdash: "—",
    ndash: "–",
    hellip: "…",
    copy: "©",
    reg: "®",
    trade: "™",
    deg: "°",
};

function decodeEntities(text: string): string {
    return text.replace(
        /&(#x?[0-9a-fA-F]+|[a-zA-Z][a-zA-Z0-9]*);/g,
        (whole, body: string) => {
            if (body[0] === "#") {
                const codePoint =
                    body[1] === "x" || body[1] === "X"
                        ? Number.parseInt(body.slice(2), 16)
                        : Number.parseInt(body.slice(1), 10);
                return Number.isNaN(codePoint)
                    ? whole
                    : String.fromCodePoint(codePoint);
            }
            const mapped = NAMED_ENTITIES[body];
            return mapped ?? whole;
        },
    );
}

/** Parse an XML document into a tree. Throws on clearly malformed input. */
export function parseXml(source: string): XmlElement {
    let i = 0;
    const len = source.length;

    const root: XmlElement = {
        type: "element",
        name: "#document",
        attrs: {},
        children: [],
    };
    const stack: XmlElement[] = [root];

    const top = (): XmlElement => stack[stack.length - 1] as XmlElement;

    const skipUntil = (marker: string): void => {
        const at = source.indexOf(marker, i);
        i = at === -1 ? len : at + marker.length;
    };

    while (i < len) {
        if (source[i] === "<") {
            if (source.startsWith("<!--", i)) {
                i += 4;
                skipUntil("-->");
                continue;
            }
            if (source.startsWith("<![CDATA[", i)) {
                const end = source.indexOf("]]>", i);
                const cdata = source.slice(i + 9, end === -1 ? len : end);
                top().children.push({ type: "text", value: cdata });
                i = end === -1 ? len : end + 3;
                continue;
            }
            if (source.startsWith("<?", i)) {
                i += 2;
                skipUntil("?>");
                continue;
            }
            if (source.startsWith("<!", i)) {
                // DOCTYPE or similar; skip to the next '>'.
                skipUntil(">");
                continue;
            }
            if (source[i + 1] === "/") {
                // Closing tag.
                const end = source.indexOf(">", i);
                const name = source.slice(i + 2, end).trim();
                if (top().name !== name) {
                    throw new Error(
                        `Mismatched closing tag </${name}> (expected </${top().name}>)`,
                    );
                }
                stack.pop();
                i = end + 1;
                continue;
            }
            // Opening tag.
            const end = source.indexOf(">", i);
            if (end === -1) {
                throw new Error("Unterminated tag");
            }
            let raw = source.slice(i + 1, end);
            i = end + 1;
            const selfClosing = raw.endsWith("/");
            if (selfClosing) {
                raw = raw.slice(0, -1);
            }

            const { name, attrs } = parseTag(raw);
            const element: XmlElement = {
                type: "element",
                name,
                attrs,
                children: [],
            };
            top().children.push(element);
            if (!selfClosing) {
                stack.push(element);
            }
            continue;
        }

        // Text run up to the next '<'.
        const next = source.indexOf("<", i);
        const chunk = source.slice(i, next === -1 ? len : next);
        if (chunk.length > 0) {
            top().children.push({ type: "text", value: decodeEntities(chunk) });
        }
        i = next === -1 ? len : next;
    }

    const documentElement = root.children.find(
        (child): child is XmlElement => child.type === "element",
    );
    if (!documentElement) {
        throw new Error("XML document has no root element");
    }
    return documentElement;
}

function parseTag(raw: string): {
    name: string;
    attrs: Record<string, string>;
} {
    const nameMatch = /^([^\s]+)/.exec(raw.trim());
    const name = nameMatch ? nameMatch[1] : raw.trim();
    const attrs: Record<string, string> = {};
    const attrRe = /([\w:.-]+)\s*=\s*"([^"]*)"|([\w:.-]+)\s*=\s*'([^']*)'/g;
    let m: RegExpExecArray | null = attrRe.exec(raw);
    while (m !== null) {
        const key = m[1] ?? m[3];
        const value = m[2] ?? m[4] ?? "";
        attrs[key] = decodeEntities(value);
        m = attrRe.exec(raw);
    }
    return { name, attrs };
}

// --- small tree helpers -----------------------------------------------------

export function isElement(node: XmlNode): node is XmlElement {
    return node.type === "element";
}

export function children(node: XmlElement, name: string): XmlElement[] {
    return node.children.filter(
        (c): c is XmlElement => isElement(c) && c.name === name,
    );
}

export function child(node: XmlElement, name: string): XmlElement | undefined {
    return children(node, name)[0];
}

/** Concatenated text content of an element, recursively. */
export function textContent(node: XmlNode): string {
    if (node.type === "text") {
        return node.value;
    }
    return node.children.map(textContent).join("");
}
