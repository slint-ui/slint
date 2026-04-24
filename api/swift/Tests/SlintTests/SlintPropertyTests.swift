// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import Testing
import Slint

@Suite("SlintProperty")
struct SlintPropertyTests {

    // MARK: - Int32 property

    @Test func int32InitialValue() {
        let p = SlintProperty<Int32>(42)
        #expect(p.value == 42)
    }

    @Test func int32SetAndGet() {
        let p = SlintProperty<Int32>(0)
        p.value = 99
        #expect(p.value == 99)
    }

    @Test func int32MultipleSetsCaptureLatest() {
        let p = SlintProperty<Int32>(1)
        p.value = 10
        p.value = 20
        p.value = 30
        #expect(p.value == 30)
    }

    @Test func int32Negative() {
        let p = SlintProperty<Int32>(0)
        p.value = -1_000_000
        #expect(p.value == -1_000_000)
    }

    // MARK: - Float property

    @Test func floatInitialValue() {
        let p = SlintProperty<Float>(3.14)
        #expect(p.value == 3.14)
    }

    @Test func floatSet() {
        let p = SlintProperty<Float>(0)
        p.value = 2.718
        #expect(p.value == 2.718)
    }

    // MARK: - Bool property

    @Test func boolInitialTrue() {
        let p = SlintProperty<Bool>(true)
        #expect(p.value == true)
    }

    @Test func boolInitialFalse() {
        let p = SlintProperty<Bool>(false)
        #expect(p.value == false)
    }

    @Test func boolToggle() {
        let p = SlintProperty<Bool>(false)
        p.value = true
        #expect(p.value == true)
        p.value = false
        #expect(p.value == false)
    }

    // MARK: - Binding

    @Test func bindingIsEvaluatedOnGet() {
        let source = SlintProperty<Int32>(7)
        let derived = SlintProperty<Int32>(0)
        derived.setBinding { source.value * 2 }
        #expect(derived.value == 14)
    }

    @Test func bindingUpdatesWhenSourceChanges() {
        let source = SlintProperty<Int32>(1)
        let derived = SlintProperty<Int32>(0)
        derived.setBinding { source.value + 100 }
        #expect(derived.value == 101)
        source.value = 5
        #expect(derived.value == 105)
    }

    @Test func setReplacesBinding() {
        let source = SlintProperty<Int32>(10)
        let p = SlintProperty<Int32>(0)
        p.setBinding { source.value }
        #expect(p.value == 10)
        // Replacing with a direct set should remove the binding
        p.value = 99
        source.value = 999          // source change should no longer affect p
        #expect(p.value == 99)
    }

    @Test func bindingReplacesBinding() {
        let a = SlintProperty<Int32>(3)
        let b = SlintProperty<Int32>(4)
        let p = SlintProperty<Int32>(0)
        p.setBinding { a.value }
        #expect(p.value == 3)
        p.setBinding { b.value }
        #expect(p.value == 4)
        a.value = 100               // old binding should no longer matter
        #expect(p.value == 4)
    }

    @Test func floatBinding() {
        let base = SlintProperty<Float>(1.0)
        let scaled = SlintProperty<Float>(0)
        scaled.setBinding { base.value * 0.5 }
        #expect(scaled.value == 0.5)
        base.value = 10.0
        #expect(scaled.value == 5.0)
    }

    @Test func boolBinding() {
        let flag = SlintProperty<Bool>(false)
        let negated = SlintProperty<Bool>(false)
        negated.setBinding { !flag.value }
        #expect(negated.value == true)
        flag.value = true
        #expect(negated.value == false)
    }
}
