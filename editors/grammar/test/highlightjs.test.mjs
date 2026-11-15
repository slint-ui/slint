// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { test } from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import hljs from "highlight.js/lib/core";
import { parseGrammarTestCase } from "vscode-tmgrammar-test/dist/unit/parsing.js";
import slint from "../dist/highlightjs-slint.js";

hljs.registerLanguage("slint", slint);

// TextMate scope prefixes and the highlight.js scope they map to, most specific first.
// Assertions on other scopes aren't checked.
const SCOPES = [
    ["keyword.operator", null],
    ["keyword.other.animate.setting", null],
    ["keyword.other.wildcard", null],
    ["keyword", "keyword"],
    ["comment", "comment"],
    ["string", "string"],
    ["constant.numeric", "number"],
    ["constant.language", "literal"],
];

const matches = (scope, prefix) =>
    scope === prefix || scope.startsWith(`${prefix}.`);

function hljsScope(name) {
    return SCOPES.find(([prefix]) => matches(name, prefix))?.[1] ?? null;
}

// Returns the highlight.js scopes of each character in each line of `code`.
function scopesPerCharacter(code) {
    const result = hljs.highlight(code, { language: "slint" });
    assert.equal(result.errorRaised, undefined);
    const scopes = [];
    const visit = (node, stack) => {
        if (typeof node === "string") {
            scopes.push(...Array(node.length).fill(stack));
        } else {
            const inner = node.scope ? [...stack, node.scope] : stack;
            for (const child of node.children) {
                visit(child, inner);
            }
        }
    };
    visit(result._emitter.rootNode, []);
    const lines = [];
    let offset = 0;
    for (const line of code.split("\n")) {
        lines.push(scopes.slice(offset, offset + line.length));
        offset += line.length + 1;
    }
    return lines;
}

const has = (stack, scope) => stack.some((s) => matches(s, scope));

const testDir = new URL("../../vscode/tests/grammar/", import.meta.url);
for (const file of readdirSync(testDir).filter((f) => f.endsWith(".slint"))) {
    test(file, () => {
        const testCase = parseGrammarTestCase(
            readFileSync(new URL(file, testDir), "utf8"),
        );
        const lines = scopesPerCharacter(testCase.source.join("\n"));
        const failures = [];
        for (const {
            sourceLineNumber,
            scopeAssertions,
        } of testCase.assertions) {
            for (const { from, to, scopes, exclude } of scopeAssertions) {
                const expected = scopes.map(hljsScope).filter(Boolean);
                const excluded = exclude.map(hljsScope).filter(Boolean);
                const line = testCase.source[sourceLineNumber];
                for (
                    let column = from;
                    column < Math.min(to, line.length);
                    column++
                ) {
                    const stack = lines[sourceLineNumber][column];
                    if (
                        expected.some((s) => !has(stack, s)) ||
                        excluded.some((s) => has(stack, s))
                    ) {
                        failures.push(
                            `${file}:${sourceLineNumber + 2}:${column + 1} '${line[column]}' has [${stack}], expected [${expected}] without [${excluded}]`,
                        );
                    }
                }
            }
        }
        assert.deepEqual(failures, []);
    });
}
