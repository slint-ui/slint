// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import Testing
import Slint

@Suite("SlintArrayModel")
struct SlintModelTests {

    // MARK: - Init

    @Test func emptyModelHasZeroRowCount() {
        let model = SlintArrayModel<Int>()
        #expect(model.rowCount == 0)
        #expect(model.count == 0)
        #expect(model.isEmpty)
    }

    @Test func initWithElementsSetsRowCount() {
        let model = SlintArrayModel([1, 2, 3])
        #expect(model.rowCount == 3)
        #expect(model.count == 3)
        #expect(!model.isEmpty)
    }

    // MARK: - rowData

    @Test func rowDataReturnsCorrectElement() {
        let model = SlintArrayModel(["apple", "banana", "cherry"])
        #expect(model[0] == "apple")
        #expect(model[1] == "banana")
        #expect(model[2] == "cherry")
    }

    @Test func rowDataOutOfBoundsReturnsNil() {
        let model = SlintArrayModel([10, 20])
        #expect(model[-1] == nil)
        #expect(model[2] == nil)
        #expect(model[100] == nil)
    }

    // MARK: - setRowData

    @Test func setRowDataUpdatesElement() {
        let model = SlintArrayModel([1, 2, 3])
        model[1] = 99
        #expect(model[1] == 99)
    }

    @Test func setRowDataOutOfBoundsIsNoop() {
        let model = SlintArrayModel([1, 2, 3])
        model[5] = 99
        #expect(model.rowCount == 3)
    }

    // MARK: - append

    @Test func appendIncreasesRowCount() {
        let model = SlintArrayModel<String>()
        model.append("x")
        #expect(model.rowCount == 1)
        model.append("y")
        #expect(model.rowCount == 2)
    }

    @Test func appendedElementIsAccessible() {
        let model = SlintArrayModel<Int>()
        model.append(42)
        #expect(model[0] == 42)
    }

    // MARK: - insert

    @Test func insertAtBeginning() {
        let model = SlintArrayModel([2, 3, 4])
        model.insert(1, at: 0)
        #expect(model[0] == 1)
        #expect(model[1] == 2)
        #expect(model.rowCount == 4)
    }

    @Test func insertAtEnd() {
        let model = SlintArrayModel([1, 2])
        model.insert(3, at: 2)
        #expect(model[2] == 3)
        #expect(model.rowCount == 3)
    }

    @Test func insertOutOfBoundsIsNoop() {
        let model = SlintArrayModel([1, 2])
        model.insert(99, at: 5)
        #expect(model.rowCount == 2)
    }

    // MARK: - remove

    @Test func removeDecreasesRowCount() {
        let model = SlintArrayModel([10, 20, 30])
        model.remove(at: 1)
        #expect(model.rowCount == 2)
    }

    @Test func removeDeletesCorrectElement() {
        let model = SlintArrayModel(["a", "b", "c"])
        model.remove(at: 1)
        #expect(model[0] == "a")
        #expect(model[1] == "c")
    }

    @Test func removeOutOfBoundsIsNoop() {
        let model = SlintArrayModel([1, 2])
        model.remove(at: 5)
        #expect(model.rowCount == 2)
    }

    @Test func removeFromEmptyModelIsNoop() {
        let model = SlintArrayModel<Int>()
        model.remove(at: 0)
        #expect(model.rowCount == 0)
    }

    // MARK: - reset

    @Test func resetReplacesAllElements() {
        let model = SlintArrayModel([1, 2, 3])
        model.reset(with: [10, 20])
        #expect(model.rowCount == 2)
        #expect(model[0] == 10)
        #expect(model[1] == 20)
    }

    @Test func resetWithEmptyArrayClearsModel() {
        let model = SlintArrayModel([1, 2, 3])
        model.reset(with: [])
        #expect(model.isEmpty)
    }
}
