# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

# cSpell: ignore bathy Peucker texel topo

"""Converts Natural Earth GeoJSON files into the compact text files that the demo embeds.

Download the GeoJSON files from
https://github.com/nvkelso/natural-earth-vector/tree/master/geojson,
airports.csv and countries.csv from https://ourairports.com/data/, and NASA's Blue Marble
for July, world.topo.bathy.200407.3x5400x2700.jpg from https://visibleearth.nasa.gov/, then run:

    python3 convert.py <geojson directory> <airports directory> <blue marble> <output directory>

The Blue Marble needs Pillow.
"""

import csv
import json
import os
import sys

# Coordinates are stored in hundredths of a degree.
UNIT = 100
# The simplification tolerance in degrees, about a third of a texel of the 4096 wide map.
TOLERANCE = 0.03


def simplify(points, tolerance):
    """Douglas-Peucker, iterative so that long coastlines don't hit the recursion limit."""
    if len(points) < 3:
        return points
    keep = [False] * len(points)
    keep[0] = keep[-1] = True
    stack = [(0, len(points) - 1)]
    while stack:
        first, last = stack.pop()
        (ax, ay), (bx, by) = points[first], points[last]
        dx, dy = bx - ax, by - ay
        length = (dx * dx + dy * dy) ** 0.5
        best, best_distance = None, tolerance
        for i in range(first + 1, last):
            px, py = points[i]
            if length == 0:
                distance = ((px - ax) ** 2 + (py - ay) ** 2) ** 0.5
            else:
                distance = abs(dy * (px - ax) - dx * (py - ay)) / length
            if distance > best_distance:
                best, best_distance = i, distance
        if best is not None:
            keep[best] = True
            stack += [(first, best), (best, last)]
    return [p for p, k in zip(points, keep) if k]


def load(path):
    with open(path) as file:
        return json.load(file)


def lines(path, kind):
    data = load(path)
    for feature in data["features"]:
        geometry = feature["geometry"]
        coordinates = geometry["coordinates"]
        parts = {
            "Polygon": [coordinates],
            "MultiPolygon": coordinates,
            "LineString": [[coordinates]],
            "MultiLineString": [coordinates],
        }[geometry["type"]]
        for part in parts:
            for ring in part:
                ring = simplify(ring, TOLERANCE)
                if kind == "polygon" and len(ring) < 4:
                    continue
                yield ring


def encode(ring):
    values = []
    previous = (0, 0)
    for lon, lat in ring:
        point = (round(lon * UNIT), round(lat * UNIT))
        if values and point == previous:
            continue
        values += [point[0] - previous[0], point[1] - previous[1]]
        previous = point
    return " ".join(map(str, values))


def write(name, rings, header):
    with open(name, "w") as out:
        out.write(header)
        out.writelines(encode(ring) + "\n" for ring in rings)


def main():
    source, airports, imagery, target = sys.argv[1:5]
    write_imagery(imagery, target)
    header = (
        "# Natural Earth {layer}, simplified by convert.py.\n"
        "# One ring per line: the first point, then the steps to each next one,\n"
        "# as longitude and latitude in hundredths of a degree.\n"
    )
    for layer, kind, output in [
        ("ne_50m_land", "polygon", "land.txt"),
        ("ne_110m_lakes", "polygon", "lakes.txt"),
        ("ne_50m_admin_0_boundary_lines_land", "line", "borders.txt"),
    ]:
        rings = list(lines(os.path.join(source, layer + ".geojson"), kind))
        write(os.path.join(target, output), rings, header.format(layer=layer))

    places = load(os.path.join(source, "ne_50m_populated_places_simple.geojson"))
    with open(os.path.join(target, "cities.txt"), "w") as out:
        out.write(
            "# Natural Earth ne_50m_populated_places_simple, converted by convert.py.\n"
            "# cSpell: disable\n"
            "# Longitude and latitude in hundredths of a degree, the population in thousands,\n"
            "# and the name.\n"
        )
        for feature in places["features"]:
            p = feature["properties"]
            out.write(
                f"{round(p['longitude'] * UNIT)} {round(p['latitude'] * UNIT)} "
                f"{max(1, round(p['pop_max'] / 1000))} {p['name']}\n"
            )

    write_airports(airports, target)


def write_imagery(path, target):
    from PIL import Image

    image = Image.open(path).convert("RGB").resize((4096, 2048), Image.LANCZOS)
    image.save(
        os.path.join(target, "earth.jpg"), quality=80, optimize=True, progressive=True
    )


def read_csv(path):
    with open(path, encoding="utf-8") as file:
        return list(csv.DictReader(file))


def write_airports(source, target):
    countries = {
        row["code"]: row["name"]
        for row in read_csv(os.path.join(source, "countries.csv"))
    }
    airports = [
        row
        for row in read_csv(os.path.join(source, "airports.csv"))
        if row["type"] == "large_airport"
        and row["scheduled_service"] == "yes"
        and row["iata_code"]
    ]
    airports.sort(key=lambda row: row["iata_code"])
    with open(os.path.join(target, "airports.txt"), "w", encoding="utf-8") as out:
        out.write(
            "# The large airports with scheduled service from OurAirports, converted by convert.py.\n"
            "# cSpell: disable\n"
            "# The IATA code, the longitude and latitude in hundredths of a degree,\n"
            "# then the city, the country, and the airport's name, separated by |.\n"
        )
        for row in airports:
            # Turns "Sydney (Mascot)" and "Honolulu, Oahu" into the city's name.
            city = row["municipality"].split("(")[0].split(",")[0].strip()
            name = row["name"]
            country = countries.get(row["iso_country"], row["iso_country"])
            lon = round(float(row["longitude_deg"]) * UNIT)
            lat = round(float(row["latitude_deg"]) * UNIT)
            out.write(
                f"{row['iata_code']} {lon} {lat} {city or name}|{country}|{name}\n"
            )


if __name__ == "__main__":
    main()
