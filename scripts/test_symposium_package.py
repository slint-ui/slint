# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
# cSpell: ignore followlinks gpgsign NOSYSTEM

"""Check the Slint crate's file list and its packaged skill's contents and links."""

import os
import re
import shutil
import subprocess
import tarfile
import tempfile
import unittest
from pathlib import Path
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parent.parent
CRATE = ROOT / "api/rs/slint"
SKILLS = ROOT / "ai-plugins/skills"


def files(root):
    return {
        (Path(directory) / name).relative_to(root)
        for directory, _, names in os.walk(root, followlinks=True)
        for name in names
    }


def run(args, cwd):
    # Git's repository-routing variables override the subprocess cwd.
    env = {
        key: value for key, value in os.environ.items() if not key.startswith("GIT_")
    }
    env.update(GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1")
    result = subprocess.run(
        args, cwd=cwd, env=env, capture_output=True, text=True, check=False
    )
    if result.returncode:
        raise AssertionError(f"{args} failed:\n{result.stdout}{result.stderr}")
    return result


class SymposiumPackageTests(unittest.TestCase):
    def test_crate_file_list_includes_skills_and_existing_files(self):
        result = run(
            ["cargo", "package", "-p", "slint", "--list", "--allow-dirty", "--locked"],
            ROOT,
        )
        packaged = set(result.stdout.splitlines())
        self.assertIn("Symposium.toml", packaged)
        expected_skills = {f"skills/{path.as_posix()}" for path in files(SKILLS)}
        self.assertTrue(expected_skills, "no source skills found")
        self.assertTrue(expected_skills <= packaged, expected_skills - packaged)
        tracked = run(["git", "ls-files", "-z", "api/rs/slint"], ROOT)
        expected_existing = {
            Path(path).relative_to(CRATE.relative_to(ROOT)).as_posix()
            for path in tracked.stdout.split("\0")
            if path and not (ROOT / path).is_dir()
        }
        self.assertTrue(expected_existing <= packaged, expected_existing - packaged)

    def test_clean_archive_materializes_skill_links_and_preserves_contents(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            crate = root / "api/rs/slint"
            crate.mkdir(parents=True)
            # Slint's workspace versions aren't published yet, so use a
            # dependency-free crate to package the same manifest and skill tree.
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
                manifest = archive.extractfile("slint-0.0.0/Symposium.toml")
                assert manifest is not None
                with manifest:
                    self.assertEqual(
                        manifest.read(), (CRATE / "Symposium.toml").read_bytes()
                    )
                for entry in archive.getmembers():
                    prefix = "slint-0.0.0/skills/"
                    if not entry.name.startswith(prefix):
                        continue
                    self.assertTrue(entry.isfile(), entry.name)
                    path = unpacked / entry.name.removeprefix(prefix)
                    self.assertTrue(
                        path.resolve().is_relative_to(unpacked.resolve()), entry.name
                    )
                    path.parent.mkdir(parents=True, exist_ok=True)
                    contents = archive.extractfile(entry)
                    assert contents is not None
                    with contents:
                        path.write_bytes(contents.read())
            expected_files = files(SKILLS)
            self.assertEqual(expected_files, files(unpacked))
            self.assertTrue((unpacked / "slint/SKILL.md").is_file())
            for relative in expected_files:
                self.assertEqual(
                    (SKILLS / relative).read_bytes(),
                    (unpacked / relative).read_bytes(),
                    relative,
                )
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
