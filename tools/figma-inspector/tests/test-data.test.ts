// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import { sanitizeSlintPropertyName } from "../backend/utils/test-data";

test("keeps valid property name starting with letter", () => {
    const name = "validName";
    const result = sanitizeSlintPropertyName(name);
    expect(result).toBe(name);
});

test("keeps valid property name starting with underscore", () => {
    const name = "_validName";
    const result = sanitizeSlintPropertyName(name);
    expect(result).toBe(name);
});

test("adds underscore to name starting with number", () => {
    const result = sanitizeSlintPropertyName("123invalid");
    expect(result).toBe("_123invalid");
});

test("adds underscore to name starting with special character", () => {
    const result = sanitizeSlintPropertyName("@invalid");
    expect(result).toBe("invalid");
});

test("removes spaces from property name", () => {
    const result = sanitizeSlintPropertyName("my property name");
    expect(result).toBe("mypropertyname");
});

test("preserves hyphens in property name", () => {
    const result = sanitizeSlintPropertyName("my-property-name");
    expect(result).toBe("my-property-name");
});

test("removes em dashes and other special characters", () => {
    const result = sanitizeSlintPropertyName("my—property—name");
    expect(result).toBe("mypropertyname");
});

test("removes non-ASCII characters", () => {
    const result = sanitizeSlintPropertyName("my-π-property");
    expect(result).toBe("my--property");
});

test("handles multiple consecutive hyphens", () => {
    const result = sanitizeSlintPropertyName("my--property--name");
    expect(result).toBe("my--property--name");
});

test("handles mixed valid and invalid characters", () => {
    const result = sanitizeSlintPropertyName("my@#$%property&*()name");
    expect(result).toBe("mypropertyname");
});

test("handles property name with numbers in middle", () => {
    const result = sanitizeSlintPropertyName("property123name");
    expect(result).toBe("property123name");
});

test("handles property name with underscores in middle", () => {
    const result = sanitizeSlintPropertyName("property_name");
    expect(result).toBe("property_name");
});

test("handles property name with hyphens and underscores", () => {
    const result = sanitizeSlintPropertyName("property-name_with-mixed");
    expect(result).toBe("property-name_with-mixed");
});

test("handles empty string", () => {
    const result = sanitizeSlintPropertyName("");
    expect(result).toBe("_");
});

test("handles string with only invalid characters", () => {
    const result = sanitizeSlintPropertyName("@#$%^&*()");
    expect(result).toBe("_");
});

test("trims spaces from start and end", () => {
    const result = sanitizeSlintPropertyName("  my property name  ");
    expect(result).toBe("mypropertyname");
});

test("converts forward slashes to hyphens", () => {
    const result = sanitizeSlintPropertyName("my/property/name");
    expect(result).toBe("my-property-name");
});
