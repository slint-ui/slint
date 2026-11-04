# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

# cSpell: ignore subsetter

"""Subsets JetBrains Mono NL Bold to the characters that the demo shows in it.

Download JetBrains Mono 2.242 from https://github.com/JetBrains/JetBrainsMono/releases,
then run, with fontTools installed:

    python3 subset.py <fonts/ttf/JetBrainsMonoNL-Bold.ttf from the download>

It writes JetBrainsMonoNL-Bold.ttf next to this script.
"""

import pathlib
import sys

from fontTools import subset
from fontTools.ttLib import TTFont

HERE = pathlib.Path(__file__).parent
DEMO = HERE.parent.parent


def characters():
    # The flaps and the buttons show the readings, labels, and options that the sources spell out.
    text = {chr(c) for c in range(0x20, 0x7F)}
    for source in [HERE.parent / "app.slint", *DEMO.glob("*.rs")]:
        text |= {c for c in source.read_text() if ord(c) > 0x7E}
    # The planner shows the codes, cities, and countries in capitals.
    for line in (DEMO / "data" / "airports.txt").read_text().splitlines():
        if line and not line.startswith("#"):
            code, _, _, names = line.split(" ", 3)
            city, country, _ = names.split("|")
            text |= set(code + city.upper() + country.upper())
    return "".join(sorted(c for c in text if c.isprintable() or c == " "))


def main():
    font = TTFont(sys.argv[1], recalcTimestamp=False)
    options = subset.Options()
    options.hinting = False
    subsetter = subset.Subsetter(options)
    subsetter.populate(text=characters())
    subsetter.subset(font)
    font.save(HERE / "JetBrainsMonoNL-Bold.ttf")


if __name__ == "__main__":
    main()
