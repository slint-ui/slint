#!/usr/bin/env python3
# -*- coding: utf-8 -*-

# Copyright © SixtyFPS GmbH <info@slint-ui.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

# Usage:
# ```sh
# # install tree-sitter-tooling
# cargo install tree-sitter-cli
# # generate corpus from existing tests
# find ../../tests/cases -type d -exec ./test-to-corpus.py --tests-directory {} \;
# # generate parser and update contents of test with current state
# tree-sitter generate && tree-sitter test -u
# # Count ERROR in generated outputs:
# rg ERROR corpus | wc -l
# ````

import argparse
import os


def header(file_name):
    case_name = os.path.basename(file_name)
    assert case_name.endswith(".slint")

    case_name = case_name[:-6]

    return f"\n==================\n{case_name}\n==================\n\n"


def process_file(input, corpus):
    corpus.write(header(input))

    test_case = ""
    in_comment = False
    in_code = False
    comment = ""
    with open(input, "r") as reader:
        line_number = 0
        for line in reader.readlines():
            line_number += 1
            strip_line = line.strip()
            if (
                line.startswith("// Copyright") or line.startswith("// SPDX-")
            ) and line_number <= 4:
                continue
            if (strip_line == "") and line_number <= 4:
                continue
            if line == "/*\n":
                comment = ""
                in_comment = True
                continue
            if line == "*/\n":
                in_comment = False
                if comment.strip() != "":
                    test_case += f"/*\n{comment}\n*/\n"
                continue
            if line.startswith("```") and in_comment:
                in_code = not in_code
                continue
            if in_code:
                continue
            if in_comment:
                comment += line
            else:
                test_case += line

    corpus.write(test_case)

    corpus.write("---\n\n(sourcefile)\n")


parser = argparse.ArgumentParser(
    description="Convert slint tests to corpus files for tree-sitter."
)
parser.add_argument(
    "--tests-directory",
    dest="tests_dir",
    action="store",
    required=True,
    help="The directory containing the tests to convert",
)
parser.add_argument(
    "--corpus-directory",
    dest="corpus_dir",
    action="store",
    default="./corpus",
    help="The directory containing the corpus data",
)

args = parser.parse_args()

tests_dir = os.path.realpath(args.tests_dir)
corpus_dir = os.path.realpath(args.corpus_dir)

corpus_file = os.path.join(corpus_dir, os.path.basename(tests_dir) + ".txt")

with open(corpus_file, "w") as corpus:
    corpus.write(
        "// Copyright © SixtyFPS GmbH <info@slint-ui.com>\n// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial\n"
    )

    for file in os.listdir(tests_dir):
        filename = os.fsdecode(file)
        if filename.endswith(".slint"):
            process_file(os.path.join(tests_dir, filename), corpus)
