// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import Testing
import Slint

@Suite("SlintString")
struct SlintStringTests {

    @Test func initFromString() {
        let s = SlintString("hello")
        #expect(s.stringValue == "hello")
    }

    @Test func emptyString() {
        let s = SlintString("")
        #expect(s.stringValue == "")
    }

    @Test func unicodeString() {
        let s = SlintString("Hello, 世界 🌍")
        #expect(s.stringValue == "Hello, 世界 🌍")
    }

    @Test func stringLiteralInit() {
        let s: SlintString = "literal"
        #expect(s.stringValue == "literal")
    }

    @Test func description() {
        let s = SlintString("test")
        #expect(s.description == "test")
    }

    @Test func equalStrings() {
        let a = SlintString("same")
        let b = SlintString("same")
        #expect(a == b)
    }

    @Test func unequalStrings() {
        let a = SlintString("foo")
        let b = SlintString("bar")
        #expect(a != b)
    }

    @Test func hashableUsableAsDictionaryKey() {
        let key = SlintString("key")
        var dict: [SlintString: Int] = [:]
        dict[key] = 42
        #expect(dict[SlintString("key")] == 42)
    }

    @Test func hashEqualityImpliesEqualHash() {
        let a = SlintString("hash-me")
        let b = SlintString("hash-me")
        var hasherA = Hasher()
        var hasherB = Hasher()
        a.hash(into: &hasherA)
        b.hash(into: &hasherB)
        #expect(hasherA.finalize() == hasherB.finalize())
    }

    @Test func cloneProducesEqualString() {
        let original = SlintString("original")
        let clone = SlintString(cloning: original)
        #expect(clone == original)
    }

    @Test func stringExtensionInit() {
        let slint = SlintString("from slint")
        let swift = String(slint)
        #expect(swift == "from slint")
    }

    @Test func stringExtensionRoundTrip() {
        let swift = "round-trip"
        let slint = SlintString(swift)
        #expect(String(slint) == swift)
    }
}
