#!/usr/bin/python3

# LICENSE BEGIN
# Copyright (c) 2021 Tobias Hunger <tobias.hunger@gmail.com>
#
# SPDX-License-Identifier: GPL-3.0-only
# This file is also available under commercial licensing terms.
# Please contact info@sixtyfps.io for more information.
# LICENSE END

import datetime
from re import sub
import subprocess
import typing


def parse_message(input: str) -> typing.Tuple[str, str]:
    subject = ""
    message = ""

    for l in input.rstrip().split("\n"):
        if not l.startswith("    "):
            continue
        if subject == "":
            subject = l[4:]
            continue

        message += f"{l[4:]}\n"

    return (subject, message.strip())


def get_git_log(
    start_date: datetime.datetime, end_date: datetime.datetime
) -> typing.Sequence[typing.Tuple[str, str, str, str]]:
    result = subprocess.run(
        [
            "/usr/bin/git",
            "log",
            f"--since={start_date.isoformat()}",
            f"--until={end_date.isoformat()}",
            "--pretty=format:%w(0)Subject: %s%nAuthor: %ae%nHash: %H%nBody:%n%w(80,4,4)%b%n%w(0)END_OF_BODY%n%n",
        ],
        capture_output=True,
        check=True,
    )

    return _parse_git_log(result.stdout.decode("utf-8"))


def _parse_git_log(log_data: str) -> typing.Sequence[typing.Tuple[str, str, str, str]]:
    result: typing.List[typing.Tuple[str, str, str, str]] = []

    author = ""
    subject = ""
    commit = ""
    message = ""

    for l in log_data.split("\n"):
        if l == "":
            message += "\n"
        elif l.startswith("Subject: "):
            subject = l[9:].strip()
        elif l.startswith("Author: "):
            author = l[8:].strip()
        elif l.startswith("Hash: "):
            commit = l[6:].strip()
        elif l.startswith("Body:"):
            message = ""
        elif l.startswith("    "):
            message = f"{message}{l[4:]}\n"
        elif l.startswith("END_OF_BODY"):
            message = message.strip()
            message_str = message
            message_str.replace("\n", "\\\n")

            assert commit and author and subject

            result.append((commit, author, subject, message))
            commit = ""
            author = ""
            subject = ""
            message = ""
        else:
            print(f'ERROR: Unknown line "{l}".')

    return result


def make_ordinal(n):
    '''
    Convert an integer into its ordinal representation::

        make_ordinal(0)   => '0th'
        make_ordinal(3)   => '3rd'
        make_ordinal(122) => '122nd'
        make_ordinal(213) => '213th'

    This code was taken from
    https://stackoverflow.com/questions/9647202/ordinal-numbers-replacement

    under this license:
    https://creativecommons.org/licenses/by-sa/4.0/
    '''
    n = int(n)
    suffix = ['th', 'st', 'nd', 'rd', 'th'][min(n % 10, 4)]
    if 11 <= (n % 100) <= 13:
        suffix = 'th'
    return str(n) + suffix


def date_string(date: datetime.datetime) -> str:
    month_names = [ "INVALID", "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December" ]
    return "{0} of {1} of {2}".format(make_ordinal(date.day), month_names[date.month], date.year)

def generate_header(start_date: datetime.datetime, end_date: datetime.datetime, timestamp: datetime.datetime) -> str:
    start_str = date_string(start_date)
    end_str = date_string(end_date)
    timestamp_str = f"{timestamp.year}-{timestamp.month:02}-{timestamp.day:02}T{timestamp.hour:02}:{timestamp.minute:02}:{timestamp.second:02}+01:00"
    return f"""<!DOCTYPE html>
<html lang="en">

<head>
    <meta name="description" content="Changelog for {end_str}" />
    <title>{start_str} to {end_str}</title>
</head>
<!--
    <date>{timestamp_str}</date>
    <discussion_id></discussion_id>
    <author></author>
-->

<body>

<div></div>

"""


def codify(text: str) -> str:
    tags = ["<code>", "</code>"]
    tick_count = 0

    result = ""
    for c in text:
        if c == "`":
            result += tags[tick_count % 2]
            tick_count += 1
        else:
            result += c

    return result


def link_ify(text: str) -> str:
    return sub(
        r"#(\d+)",
        lambda x: f'<a href="https://github.com/sixtyfpsui/sixtyfps/issues/{x.group(1)}">#{x.group(1)}</a>',
        text,
    )


def generate_progress(git: typing.Sequence[typing.Tuple[str, str, str, str]]) -> str:
    result = """<h3>Progress</h3>

<p>Progress was made in the following areas:</p>

<ul>
"""
    for (commit, _, summary, message) in git:
        msg_str = ""
        summary = codify(summary)
        short_commit = commit[0:6]

        if message != "":
            tmp = link_ify(codify(message)).replace("\n\n", "</p>\n<p>")
            msg_str = f"\n\n    <p>{tmp}</p>\n"
        result = f'{result}<li>{summary} (<a href="https://github.com/sixtyfpsui/sixtyfps/commit/{commit}">{short_commit}</a>){msg_str}</li>\n\n'
    result = f"{result}</ul>\n"

    return result


def generate_statistics(git: typing.Sequence[typing.Tuple[str, str, str, str]]) -> str:
    commit_count = len(git)

    authors: typing.Dict[str, int] = {}
    for (_, author, _, _) in git:
        if author not in authors:
            authors[author] = 1
        else:
            authors[author] += 1

    return f"""<h3>Statistics</h3>

<p>{commit_count} patches were committed by {len(authors)} authors.</p>

"""


def generate_footer():
    return "</body>"


end_date = datetime.datetime.now().replace(
    hour=23, minute=59, second=59, microsecond=999999
)
start_date = (end_date - datetime.timedelta(days=7)).replace(
    hour=0, minute=0, second=0, microsecond=0
)

git_log = get_git_log(start_date, end_date)

timestamp = end_date + datetime.timedelta(days=1)

result = generate_header(start_date, end_date, timestamp)
result += generate_progress(git_log)
result += generate_statistics(git_log)
result += generate_footer()

with open(f"{timestamp.year}-{timestamp.month:02}-{timestamp.day:02}.html", "w") as f:
    f.write(result)
