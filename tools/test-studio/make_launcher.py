# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Create a local macOS launcher for this checkout's Test Studio."""

import plistlib
from pathlib import Path


def main():
    directory = Path(__file__).resolve().parent
    bundle = directory / ".local/Slint Test Studio.app"
    contents = bundle / "Contents"
    executable = contents / "MacOS/TestStudio"
    executable.parent.mkdir(parents=True, exist_ok=True)
    with (contents / "Info.plist").open("wb") as stream:
        plistlib.dump(
            {
                "CFBundleExecutable": "TestStudio",
                "CFBundleIdentifier": "dev.slint.test-studio.local",
                "CFBundleName": "Slint Test Studio",
                "CFBundlePackageType": "APPL",
                "NSHighResolutionCapable": True,
            },
            stream,
        )
    executable.write_text("""#!/bin/sh
set -eu
studio_dir=$(CDPATH= cd -- "$(dirname -- "$0")/../../../.." && pwd)
unset SLINT_BACKEND SLINT_MCP_PORT SLINT_TEST_SERVER
exec "$studio_dir/run.command" > "$studio_dir/.local/desktop.log" 2>&1
""")
    executable.chmod(0o755)
    print(bundle)


if __name__ == "__main__":
    main()
