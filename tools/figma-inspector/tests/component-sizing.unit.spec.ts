// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { readFile } from "node:fs/promises";
import { expect, test } from "vitest";
import { normalizeSource } from "../src/plugin/normalize";
import { convertSnapshot } from "../src/preview/converter";

for (const name of ["conditional-root", "conditional-inner-row"]) {
    for (const target of ["preview", "export"] as const) {
        test(`${name}/${target}: structural alternatives have an unconditional layout owner`, async () => {
            const capture = JSON.parse(
                await readFile(`fixtures/source/${name}.json`, "utf8"),
            );
            const before = JSON.stringify(capture);
            const normalized = await normalizeSource(capture, target);
            if (!normalized.ok || normalized.empty)
                throw Error(JSON.stringify(normalized));
            const result = convertSnapshot(normalized.snapshot, { target });
            if (!result.ok) throw Error(JSON.stringify(result));
            expect(result.source).toMatch(/\n\s+FlexboxLayout \{/);
            if (name === "conditional-root")
                expect(result.source).not.toContain("preferred-height: 32px;");
            expect(JSON.stringify(capture)).toBe(before);
            expect(convertSnapshot(normalized.snapshot, { target })).toEqual(
                result,
            );
        });
    }
}
