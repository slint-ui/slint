# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location(
    "validator", Path(__file__).resolve().parents[1] / "scripts/check-source.py"
)
validator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(validator)


class DiagnosticClient(validator.LspClient):
    def __init__(self, entry_first):
        super().__init__("")
        self.entry_first = entry_first
        self.messages = []

    async def send(self, message):
        if message.get("method") == "textDocument/didOpen":
            document = message["params"]["textDocument"]
            entry = {
                "uri": document["uri"],
                "version": document["version"],
                "diagnostics": [],
            }
            dependency = {
                "uri": "file:///card.slint",
                "diagnostics": [{"severity": 1, "message": "invalid expression"}],
            }
            ordered = [entry, dependency] if self.entry_first else [dependency, entry]
            self.messages = [
                {"method": "textDocument/publishDiagnostics", "params": item}
                for item in ordered
            ]
        else:
            self.messages.append({"id": message["id"], "result": []})

    async def receive(self):
        return self.messages.pop(0)


class DiagnosticTests(unittest.IsolatedAsyncioTestCase):
    async def test_import_errors_in_either_notification_order(self):
        for entry_first in [True, False]:
            result = await DiagnosticClient(entry_first).check(
                "main.slint", "source", 1
            )
            self.assertEqual(result["status"], "error")
            self.assertEqual(len(result["diagnostics"]), 1)


if __name__ == "__main__":
    unittest.main()
