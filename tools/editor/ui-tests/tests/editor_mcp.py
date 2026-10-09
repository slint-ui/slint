# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json
import subprocess


def call_editor_tool(editor_binary, project, name, *, expected_error=None, **arguments):
    request = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": name,
            "arguments": {"workingDirectory": str(project), **arguments},
        },
    }
    result = subprocess.run(
        [str(editor_binary.with_name("slint-editor-mcp"))],
        input=json.dumps(request) + "\n",
        text=True,
        capture_output=True,
        check=True,
        timeout=35,
    )
    response = json.loads(result.stdout)["result"]
    if expected_error is None:
        assert not response.get("isError", False), response
    else:
        assert response.get("isError", False), response
        assert expected_error in response["content"][0]["text"], response
    return response
