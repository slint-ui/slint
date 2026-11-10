// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore equirectangular texel

//! The map of the earth that the globe shader samples, rasterized at startup from the
//! Natural Earth outlines in `data/`.
//!
//! The map is equirectangular: x runs from 180° west to 180° east, and y from the north pole to
//! the south pole. It holds the signed distance to the coast, 0.5 on the coast and more on
//! land, so that the shader draws sharp coasts however far it magnifies the map.

use crate::camera::surface;
use crate::renderer::Imagery;

pub const WIDTH: u32 = 4096;
pub const HEIGHT: u32 = WIDTH / 2;
/// How many texels the distances in the map reach.
const SPREAD: f32 = 6.0;

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
                    let length = (ex * ex + ey * ey).sqrt();
                    let i = (y as u32 * WIDTH + x.rem_euclid(WIDTH as i32) as u32) as usize;
                    distance[i] = distance[i].min(length);
                }
            }
        }
    }
    distance
}

/// The segments of the land borders, as pairs of points on the unit sphere.
pub fn border_segments() -> Vec<[[f32; 3]; 2]> {
    let point = |(lon, lat): Point| surface(lon.to_radians(), lat.to_radians()).to_array();
    rings(include_str!("data/borders.txt"))
        .flat_map(|line| {
            line.windows(2).map(|pair| [point(pair[0]), point(pair[1])]).collect::<Vec<_>>()
        })
        .collect()
}

/// The map, with the signed distance to the coast in one byte per texel.
pub fn coasts() -> Imagery {
    let outlines = |text| rings(text).collect::<Vec<_>>();
    let (land, lakes) =
        (outlines(include_str!("data/land.txt")), outlines(include_str!("data/lakes.txt")));
    let land_inside = inside(land.iter().cloned());
    let lake_inside = inside(lakes.iter().cloned());
    let coast = distances(land.into_iter().chain(lakes));
    let base: Vec<u8> = (0..(WIDTH * HEIGHT) as usize)
        .map(|i| {
            let sign = if land_inside[i] && !lake_inside[i] { 1.0 } else { -1.0 };
            let land = 0.5 + 0.5 * sign * coast[i] / SPREAD;
            (land.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
        })
        .collect();
    Imagery::gray(WIDTH, HEIGHT, base)
}
