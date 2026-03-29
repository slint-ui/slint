// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import Testing
import Slint

@Suite("SlintCallback")
struct SlintCallbackTests {

    @Test func invokeWithNoHandlerIsNoop() {
        let cb = SlintCallback()
        // Should not crash when no handler is set
        cb.invoke()
    }

    @Test func handlerIsCalledOnInvoke() {
        let cb = SlintCallback()
        var called = false
        cb.setHandler { called = true }
        cb.invoke()
        #expect(called)
    }

    @Test func handlerIsCalledEachTime() {
        let cb = SlintCallback()
        var count = 0
        cb.setHandler { count += 1 }
        cb.invoke()
        cb.invoke()
        cb.invoke()
        #expect(count == 3)
    }

    @Test func replacingHandlerCallsNewHandler() {
        let cb = SlintCallback()
        var whichHandler = ""
        cb.setHandler { whichHandler = "first" }
        cb.invoke()
        #expect(whichHandler == "first")
        cb.setHandler { whichHandler = "second" }
        cb.invoke()
        #expect(whichHandler == "second")
    }

    @Test func replacingHandlerDoesNotCallOldHandler() {
        let cb = SlintCallback()
        var oldCalled = false
        var newCalled = false
        cb.setHandler { oldCalled = true }
        cb.setHandler { newCalled = true }
        cb.invoke()
        #expect(!oldCalled)
        #expect(newCalled)
    }

    @Test func handlerCanMutateOuterVariable() {
        let cb = SlintCallback()
        var accumulator = 0
        cb.setHandler { accumulator += 10 }
        cb.invoke()
        cb.invoke()
        #expect(accumulator == 20)
    }

    @Test func multipleCallbacksAreIndependent() {
        let cb1 = SlintCallback()
        let cb2 = SlintCallback()
        var result = ""
        cb1.setHandler { result += "A" }
        cb2.setHandler { result += "B" }
        cb1.invoke()
        cb2.invoke()
        cb1.invoke()
        #expect(result == "ABA")
    }
}
