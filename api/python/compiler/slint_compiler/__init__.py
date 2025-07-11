# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import cached_path
import platform
import sys
import subprocess
import re
import os
from importlib.metadata import version, PackageNotFoundError


def main() -> None:
    try:
        package_version = version("slint-compiler")
        # Strip alpha/beta from for example "1.13.0b1"
        version_regex = re.search("([0-9]*)\\.([0-9]*)\\.([0-9]*).*", package_version)
        assert version_regex is not None
        major = version_regex.group(1)
        minor = version_regex.group(2)
        patch = version_regex.group(3)
        github_release = f"v{major}.{minor}.{patch}"
    except PackageNotFoundError:
        github_release = "nightly"

    # Permit override by environment variable
    github_release = os.environ.get("SLINT_COMPILER_VERSION") or github_release

    operating_system = {
        "darwin": "Darwin",
        "linux": "Linux",
        "win32": "Windows",
        "msys": "Windows",
    }[sys.platform]
    arch = {
        "aarch64": "aarch64",
        "amd64": "x86_64",
        "arm64": "aarch64",
        "x86_64": "x86_64",
    }[platform.machine().lower()]
    exe_suffix = ""

    if operating_system == "Windows":
        arch = {"aarch64": "ARM64", "x86_64": "AMD64"}[arch]
        exe_suffix = ".exe"
    elif operating_system == "Linux":
        pass
    elif operating_system == "Darwin":
        arch = {"aarch64": "arm64", "x86_64": "x86_64"}[arch]
    else:
        raise Exception(f"Unsupported operarating system: {operating_system}")

    platform_suffix = f"{operating_system}-{arch}"
    prebuilt_archive_filename = f"slint-compiler-{platform_suffix}.tar.gz"
    download_url = f"https://github.com/slint-ui/slint/releases/download/{github_release}/{prebuilt_archive_filename}"
    url_and_path_within_archive = f"{download_url}!slint-compiler{exe_suffix}"

    local_path = cached_path.cached_path(
        url_and_path_within_archive, extract_archive=True
    )
    args = [str(local_path)]
    args.extend(sys.argv[1:])
    subprocess.run(args)


if __name__ == "__main__":
    main()
