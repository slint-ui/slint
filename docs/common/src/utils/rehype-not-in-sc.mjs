// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// `<NotInSC>` … `</NotInSC>` marks documentation of a feature that isn't part
// of the safety-certified surface. The safety manual drops what the tags
// enclose, every other site keeps it, and neither renders the tags: they are
// authoring syntax.
//
// The tags work in hand-written pages and in the reference generated from the
// doc comments of internal/compiler/builtins.slint alike, which is why this
// lives here rather than in the generator: only the specification chapters'
// own source can mark passages of the specification.
//
// Markdown and MDX deliver the tags in three different shapes:
//   - MDX parses them as one `mdxJsxFlowElement` named `NotInSC` whose
//     children are the enclosed content;
//   - Markdown with no blank line inside makes the whole region a single raw
//     HTML node, tags and text together;
//   - Markdown with paragraphs inside makes the tags two raw nodes of their
//     own, with the enclosed nodes between them.
//
// An unbalanced tag would omit the wrong content, so it fails the build.

const OPEN = "<NotInSC>";
const CLOSE = "</NotInSC>";
const TAG_NAME = "NotInSC";

/** The tag a raw markdown node consists of, if it is nothing else. */
function rawTag(node) {
    if (node?.type !== "raw" && node?.type !== "html") return undefined;
    const value = node.value.trim();
    return value === OPEN || value === CLOSE ? value : undefined;
}

/** Whether a raw markdown node holds a whole region, tags and content. */
function isSelfContainedRegion(node) {
    if (node?.type !== "raw" && node?.type !== "html") return false;
    const value = node.value.trim();
    return value.startsWith(OPEN) && value.endsWith(CLOSE);
}

/** The region's content, with the tags removed. */
function regionContent(node) {
    return node.value.trim().slice(OPEN.length, -CLOSE.length).trim();
}

/**
 * Mark content of a region that this site keeps, so that the rest of the
 * pipeline can tell it apart: it documents a feature outside the certified
 * subset, and states no requirement of the specification.
 */
function markNotInSc(node) {
    node.data = { ...node.data, notInSc: true };
    for (const child of node.children ?? []) markNotInSc(child);
    return node;
}

/**
 * @param {{ omit?: boolean }} options `omit` drops what the tags enclose,
 * for the site that documents the certified subset only.
 */
export default function rehypeNotInSc({ omit = false } = {}) {
    return (tree, file) => {
        const sourcePath = file?.path ?? "";
        visit(tree);

        function visit(parent) {
            if (!parent.children) return;
            const children = [];
            // Nodes of an open region, kept only when this site includes them.
            let enclosed;

            for (const node of parent.children) {
                if (
                    node.type === "mdxJsxFlowElement" &&
                    node.name === TAG_NAME
                ) {
                    if (!omit) children.push(...node.children.map(markNotInSc));
                    continue;
                }
                if (isSelfContainedRegion(node)) {
                    if (!omit)
                        children.push(
                            markNotInSc({
                                ...node,
                                value: regionContent(node),
                            }),
                        );
                    continue;
                }
                const tag = rawTag(node);
                if (tag === OPEN) {
                    if (enclosed) {
                        throw new Error(unbalanced(sourcePath, OPEN));
                    }
                    enclosed = [];
                    continue;
                }
                if (tag === CLOSE) {
                    if (!enclosed) {
                        throw new Error(unbalanced(sourcePath, CLOSE));
                    }
                    if (!omit) children.push(...enclosed.map(markNotInSc));
                    enclosed = undefined;
                    continue;
                }
                (enclosed ?? children).push(node);
                visit(node);
            }

            if (enclosed) {
                throw new Error(unbalanced(sourcePath, OPEN));
            }
            parent.children = children;
        }
    };
}

function unbalanced(sourcePath, tag) {
    return (
        `rehype-not-in-sc: unbalanced \`${tag}\` in ${sourcePath}\n` +
        "Each `<NotInSC>` shall be closed by a `</NotInSC>` of its own."
    );
}
