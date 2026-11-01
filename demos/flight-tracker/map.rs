// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore equirectangular texel

//! The map of the earth that the globe shader samples, rasterized at startup from the
//! Natural Earth outlines in `data/`.
//!
//! The map is equirectangular: x runs from 180° west to 180° east, and y from the north pole to
//! the south pole. Red holds the signed distance to the coast, 0.5 on the coast and more on
//! land, so that the shader draws sharp coasts however far it magnifies the map. Green holds
//! the distance to the nearest border, 1 on a border, and blue the lights of the cities.

pub const WIDTH: u32 = 4096;
pub const HEIGHT: u32 = WIDTH / 2;
/// How many texels the distances in the map reach.
pub const SPREAD: f32 = 6.0;

/// The map and its mip levels, each in RGBA.
pub struct Map {
    pub levels: Vec<Vec<u8>>,
}

/// Longitude and latitude in degrees.
type Point = (f32, f32);

/// The rings or lines of one of the files in `data/`.
fn rings(text: &str) -> impl Iterator<Item = Vec<Point>> + '_ {
    text.lines().filter(|line| !line.starts_with('#')).map(|line| {
        let mut values = line.split(' ').map(|value| value.parse::<i32>().unwrap_or(0));
        let mut point = (0, 0);
        let mut ring = Vec::new();
        while let (Some(lon), Some(lat)) = (values.next(), values.next()) {
            point = (point.0 + lon, point.1 + lat);
            ring.push((point.0 as f32 / 100.0, point.1 as f32 / 100.0));
        }
        ring
    })
}

/// The longitude, latitude, and population in thousands of each city.
pub fn cities() -> impl Iterator<Item = (f32, f32, f32)> {
    include_str!("data/cities.txt").lines().filter(|line| !line.starts_with('#')).filter_map(
        |line| {
            let mut values = line.split(' ').map(|value| value.parse::<f32>().ok());
            Some((values.next()?? / 100.0, values.next()?? / 100.0, values.next()??))
        },
    )
}

fn to_map((lon, lat): Point) -> (f32, f32) {
    ((lon + 180.0) / 360.0 * WIDTH as f32, (90.0 - lat) / 180.0 * HEIGHT as f32)
}

/// Whether the center of each texel lies inside the polygons, by the even-odd rule.
fn inside(polygons: impl Iterator<Item = Vec<Point>>) -> Vec<bool> {
    let rows = HEIGHT as usize;
    // Where the edges cross the middle of each row.
    let mut crossings: Vec<Vec<f32>> = vec![Vec::new(); rows];
    for ring in polygons {
        let points: Vec<(f32, f32)> = ring.into_iter().map(to_map).collect();
        for (i, &(x0, y0)) in points.iter().enumerate() {
            let (x1, y1) = points[(i + 1) % points.len()];
            if y0 == y1 {
                continue;
            }
            let (top, bottom) = (y0.min(y1), y0.max(y1));
            let first = (top - 0.5).ceil().max(0.0) as usize;
            let last = ((bottom - 0.5).ceil() as usize).min(rows);
            for (row, xs) in crossings.iter_mut().enumerate().take(last).skip(first) {
                let t = (row as f32 + 0.5 - y0) / (y1 - y0);
                xs.push(x0 + (x1 - x0) * t);
            }
        }
    }

    let width = WIDTH as usize;
    let mut inside = vec![false; width * rows];
    for (row, xs) in crossings.iter_mut().enumerate() {
        xs.sort_by(f32::total_cmp);
        let line = &mut inside[row * width..][..width];
        for span in xs.chunks_exact(2) {
            let first = ((span[0] - 0.5).ceil().max(0.0) as usize).min(width);
            let last = ((span[1] - 0.5).ceil().max(0.0) as usize).min(width);
            line[first..last.max(first)].fill(true);
        }
    }
    inside
}

/// The distance in texels from the center of each texel to the nearest of the `lines`, up to
/// `SPREAD`. Natural Earth cuts polygons along the date line and the south pole; those edges
/// aren't coasts, so they don't count.
fn distances(lines: impl Iterator<Item = Vec<Point>>) -> Vec<f32> {
    let mut distance = vec![SPREAD; (WIDTH * HEIGHT) as usize];
    let reach = SPREAD + 1.0;
    for line in lines {
        for pair in line.windows(2) {
            let ((lon0, lat0), (lon1, lat1)) = (pair[0], pair[1]);
            if (lon0.abs() >= 179.99 && lon1.abs() >= 179.99) || (lat0 <= -89.99 && lat1 <= -89.99)
            {
                continue;
            }
            let ((x0, y0), (x1, y1)) = (to_map(pair[0]), to_map(pair[1]));
            let (dx, dy) = (x1 - x0, y1 - y0);
            let length_squared = (dx * dx + dy * dy).max(1e-6);
            let x_range = (x0.min(x1) - reach).floor() as i32..=(x0.max(x1) + reach) as i32;
            let y_range = ((y0.min(y1) - reach).floor() as i32).max(0)
                ..=((y0.max(y1) + reach) as i32).min(HEIGHT as i32 - 1);
            for y in y_range {
                for x in x_range.clone() {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let t = (((px - x0) * dx + (py - y0) * dy) / length_squared).clamp(0.0, 1.0);
                    let (ex, ey) = (px - (x0 + dx * t), py - (y0 + dy * t));
                    let texel = &mut distance
                        [(y as u32 * WIDTH + x.rem_euclid(WIDTH as i32) as u32) as usize];
                    *texel = texel.min((ex * ex + ey * ey).sqrt());
                }
            }
        }
    }
    distance
}

/// Glows around the cities, larger and brighter for larger ones.
fn lights() -> Vec<f32> {
    let mut light = vec![0.0f32; (WIDTH * HEIGHT) as usize];
    for (lon, lat, population) in cities() {
        let (cx, cy) = to_map((lon, lat));
        let millions = population / 1000.0;
        let sigma = 0.8 + 1.0 * millions.sqrt().min(3.0);
        let brightness = (0.35 + 0.2 * millions.max(0.05).log10()).clamp(0.15, 1.0);
        // Pixels get narrower towards the poles.
        let stretch = 1.0 / lat.to_radians().cos().max(0.2);
        let (rx, ry) = (3.0 * sigma * stretch, 3.0 * sigma);
        for y in (cy - ry) as i32..=(cy + ry) as i32 {
            if y < 0 || y >= HEIGHT as i32 {
                continue;
            }
            for x in (cx - rx) as i32..=(cx + rx) as i32 {
                let dx = (x as f32 + 0.5 - cx) / stretch;
                let dy = y as f32 + 0.5 - cy;
                let value = brightness * (-(dx * dx + dy * dy) / (2.0 * sigma * sigma)).exp();
                light[(y as u32 * WIDTH + x.rem_euclid(WIDTH as i32) as u32) as usize] += value;
            }
        }
    }
    light
}

impl Map {
    pub fn new() -> Self {
        let land = rings(include_str!("data/land.txt"));
        let lakes = rings(include_str!("data/lakes.txt"));
        let land_inside = inside(rings(include_str!("data/land.txt")));
        let lake_inside = inside(rings(include_str!("data/lakes.txt")));
        let coast = distances(land.chain(lakes));
        let borders = distances(rings(include_str!("data/borders.txt")));
        let lights = lights();
        let byte = |value: f32| (value.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        let mut base = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
        for i in 0..(WIDTH * HEIGHT) as usize {
            let sign = if land_inside[i] && !lake_inside[i] { 1.0 } else { -1.0 };
            let land = 0.5 + 0.5 * sign * coast[i] / SPREAD;
            let border = 1.0 - borders[i] / SPREAD;
            base.extend_from_slice(&[byte(land), byte(border), byte(lights[i]), 255]);
        }

        let mut levels = vec![base];
        let (mut width, mut height) = (WIDTH as usize, HEIGHT as usize);
        while width > 1 || height > 1 {
            let (next_width, next_height) = ((width / 2).max(1), (height / 2).max(1));
            let previous = levels.last().unwrap();
            let mut level = vec![0u8; next_width * next_height * 4];
            for y in 0..next_height {
                for x in 0..next_width {
                    for channel in 0..4 {
                        let texel = |sx: usize, sy: usize| {
                            let (sx, sy) = (sx.min(width - 1), sy.min(height - 1));
                            previous[(sy * width + sx) * 4 + channel] as u32
                        };
                        let sum = texel(2 * x, 2 * y)
                            + texel(2 * x + 1, 2 * y)
                            + texel(2 * x, 2 * y + 1)
                            + texel(2 * x + 1, 2 * y + 1);
                        level[(y * next_width + x) * 4 + channel] = ((sum + 2) / 4) as u8;
                    }
                }
            }
            levels.push(level);
            (width, height) = (next_width, next_height);
        }
        Self { levels }
    }
}
