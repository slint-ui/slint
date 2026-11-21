# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
# cSpell: ignore followlinks gpgsign NOSYSTEM

import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parent.parent
CRATE = ROOT / "api/rs/slint"
SKILLS = ROOT / "ai-plugins/skills"


def contents(root):
    return {
        (Path(directory) / name).relative_to(root): (
            Path(directory) / name
        ).read_bytes()
        for directory, _, names in os.walk(root, followlinks=True)
        for name in names
    }


def run(args, cwd):
    # Git's repository-routing variables override the subprocess cwd.
    env = {
        key: value for key, value in os.environ.items() if not key.startswith("GIT_")
    }
    env.update(GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1")
    result = subprocess.run(args, cwd=cwd, env=env, capture_output=True, text=True)
    if result.returncode:
        raise AssertionError(f"{args} failed:\n{result.stdout}{result.stderr}")
    return result


def commit_fixture(root):
    run(["git", "init", "-q"], root)
    run(["git", "add", "."], root)
    run(
        [
            "git",
            "-c",
            "user.name=Packaging test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            f"core.hooksPath={os.devnull}",
            "commit",
            "-qm",
            "Package fixture",
        ],
        root,
    )


class SymposiumPackageTests(unittest.TestCase):
    def test_cargo_includes_skills_and_existing_crate_files(self):
        result = run(
            ["cargo", "package", "-p", "slint", "--list", "--allow-dirty", "--locked"],
            ROOT,
        )
        packaged = set(result.stdout.splitlines())
        self.assertIn("Symposium.toml", packaged)
        expected_skills = {f"skills/{path.as_posix()}" for path in contents(SKILLS)}
        self.assertTrue(expected_skills, "no source skills found")
        self.assertTrue(expected_skills <= packaged, expected_skills - packaged)
        tracked = run(["git", "ls-files", "-z", "api/rs/slint"], ROOT)
        expected_existing = {
            Path(path).relative_to(CRATE.relative_to(ROOT)).as_posix()
            for path in tracked.stdout.split("\0")
            if path and not (ROOT / path).is_dir()
        }
        self.assertTrue(expected_existing <= packaged, expected_existing - packaged)

    def test_clean_cargo_archive_contains_self_contained_skills(self):
        self.check_clean_cargo_archive()

    def test_archive_ignores_external_cargo_target_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch.dict(os.environ, {"CARGO_TARGET_DIR": directory}):
                self.check_clean_cargo_archive()
            self.assertEqual(list(Path(directory).iterdir()), [])

    def test_archive_preserves_external_git_repository(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            sentinel = root / "sentinel.txt"
            sentinel.write_text("Keep this repository unchanged.\n")
            commit_fixture(root)
            before_head = run(["git", "rev-parse", "HEAD"], root).stdout
            index = root / ".git/index"
            before_index = index.read_bytes()
            for variables in [
                {"GIT_DIR": str(root / ".git")},
                {
                    "GIT_DIR": str(root / ".git"),
                    "GIT_COMMON_DIR": str(root / ".git"),
                    "GIT_WORK_TREE": str(root),
                    "GIT_INDEX_FILE": str(index),
                    "GIT_CONFIG_COUNT": "1",
                    "GIT_CONFIG_KEY_0": "core.bare",
                    "GIT_CONFIG_VALUE_0": "true",
                },
            ]:
                with self.subTest(variables=tuple(variables)):
                    try:
                        with patch.dict(os.environ, variables):
                            self.check_clean_cargo_archive()
                    finally:
                        self.assertEqual(
                            run(["git", "rev-parse", "HEAD"], root).stdout, before_head
                        )
                        self.assertEqual(index.read_bytes(), before_index)
                        self.assertEqual(
                            sentinel.read_text(), "Keep this repository unchanged.\n"
                        )

    def check_clean_cargo_archive(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            crate = root / "api/rs/slint"
            crate.mkdir(parents=True)
            # A dependency-free crate exercises Cargo packaging before Slint's
            # workspace versions have been published to crates.io.
            (crate / "Cargo.toml").write_text(
                '[package]\nname = "slint"\nversion = "0.0.0"\nedition = "2024"\n'
                '[lib]\npath = "lib.rs"\n'
            )
            (crate / "lib.rs").write_text("")
            shutil.copyfile(CRATE / "Symposium.toml", crate / "Symposium.toml")
            shutil.copytree(SKILLS, root / "ai-plugins/skills", symlinks=True)
            shutil.copytree(
                ROOT / "ai-plugins/mcp/guidance", root / "ai-plugins/mcp/guidance"
            )
            (crate / "skills").symlink_to(
                (CRATE / "skills").readlink(), target_is_directory=True
            )
            commit_fixture(root)
            target = root / "target"
            run(
                [
                    "cargo",
                    "package",
                    "--no-verify",
                    "--offline",
                    "--target-dir",
                    str(target),
                ],
                crate,
            )
            unpacked = root / "unpacked"
            with tarfile.open(target / "package/slint-0.0.0.crate") as archive:
                manifest = archive.extractfile("slint-0.0.0/Symposium.toml").read()
                self.assertEqual(manifest, (CRATE / "Symposium.toml").read_bytes())
                for entry in archive.getmembers():
                    prefix = "slint-0.0.0/skills/"
                    if not entry.name.startswith(prefix):
                        continue
                    self.assertTrue(entry.isfile(), entry.name)
                    path = unpacked / entry.name.removeprefix(prefix)
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(archive.extractfile(entry).read())
            self.assertEqual(contents(SKILLS), contents(unpacked))
            self.assertTrue((unpacked / "slint/SKILL.md").is_file())
            for path in unpacked.rglob("*.md"):
                markdown = re.sub(r"(?ms)^```.*?^```[^\n]*$", "", path.read_text())
                markdown = re.sub(r"`[^`\n]+`", "", markdown)
                for link in re.findall(r"\]\(([^\s)]+)\)", markdown):
                    url = urlsplit(link)
                    if url.scheme or url.netloc or not url.path:
                        continue
                    target = (path.parent / unquote(url.path)).resolve()
                    self.assertTrue(
                        target.is_relative_to(unpacked.resolve()), (path, link)
                    )
                    self.assertTrue(target.exists(), (path, link))


if __name__ == "__main__":
    unittest.main()
