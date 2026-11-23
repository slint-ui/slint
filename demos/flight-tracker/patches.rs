// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! The globe as patches of a mesh that's fixed to it.
//!
//! Each patch shows one source of imagery: the globe's own, or a tile of a detail window,
//! see `tiles.rs`. Which source covers which part of the globe is decided here, once per frame,
//! so that a pixel takes one sample of imagery and no decisions.
//!
//! The patches follow the tiles' grid. A cell of the coarsest window's grid is one patch,
//! unless finer tiles cover part of it: then it's split in four, and so on.
//! A patch shows the finest tile that covers it whole and has faded in, or the globe's own
//! imagery. A tile that is still fading in is drawn over that, translucent.

use glam::{Vec2, Vec3};

use crate::gibs::{self, FIRST_ZOOM, LEVELS};
use crate::tiles::{WINDOW_FLOATS, WINDOW_TILES};

/// What `vs_patch` in `globe.wgsl` gets for each patch.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Patch {
    /// The part of the map it covers: its left, its top, its right, and its bottom.
    /// Neighbors get the same numbers for the edge they share.
    pub rect: [f32; 4],
    /// Its imagery: what scales the place on the map to the place in the texture, the layer of
    /// the detail window, and how much of it shows.
    pub source: [f32; 4],
}

/// The imagery of a patch.
#[derive(Clone, Copy)]
enum Source {
    /// The globe's own.
    Map,
    /// A tile of the detail window `level` of the cell's windows.
    Tile { level: usize },
}

/// How a patch is drawn.
#[derive(Clone, Copy)]
pub enum Kind {
    /// With the globe's own imagery.
    Map,
    /// With a tile of the day's imagery.
    Tile,
    /// With a tile of the night's lights, in the dark.
    Night,
    /// With a tile of the day's imagery that's fading in, over the others.
    FadingTile,
    /// With a tile of the night's lights that's fading in, over the others.
    FadingNight,
}

/// The patches of one frame, for each `Kind`, by the depth of their cells in the grid,
/// see `SEGMENTS`.
#[derive(Default)]
pub struct Patches(pub [[Vec<Patch>; LEVELS]; 5]);

/// The detail windows that a cell takes its tiles from.
struct Tiles<'a> {
    windows: &'a [Option<Window>],
    /// The layer of the first of them.
    first_layer: usize,
    tile: Kind,
    fading: Kind,
}

/// How many segments a patch has along each side, by the depth of its cell in the grid.
/// A cell is half as wide as the one above it, so that the vertices of neighbors meet.
pub const SEGMENTS: [u16; LEVELS] = [16, 8, 4];

/// A detail window, as `tiles::Detail::update()` describes it.
struct Window {
    columns: i64,
    rows: i64,
    first_column: i64,
    first_row: i64,
    shown: [f32; (WINDOW_TILES * WINDOW_TILES) as usize],
    loaded: u32,
}

impl Window {
    fn new(floats: &[f32; WINDOW_FLOATS]) -> Option<Self> {
        if floats[2] == 0.0 {
            return None;
        }
        let half = WINDOW_TILES as f32 / 2.0;
        let rows = floats[3] as i64;
        let mut shown = [0.0; (WINDOW_TILES * WINDOW_TILES) as usize];
        shown.copy_from_slice(&floats[4..4 + (WINDOW_TILES * WINDOW_TILES) as usize]);
        Some(Self {
            columns: floats[2] as i64,
            rows,
            // As `tiles::Window::move_to()` places the window.
            first_column: (floats[0] - half).round() as i64,
            first_row: ((floats[1] - half).round() as i64).clamp(0, rows - WINDOW_TILES as i64),
            shown,
            loaded: floats[4 + (WINDOW_TILES * WINDOW_TILES) as usize] as u32,
        })
    }

    /// How far the tile at `column` and `row` has faded in, or `None` if the window doesn't
    /// hold it.
    fn tile(&self, column: i64, row: i64) -> Option<f32> {
        let size = WINDOW_TILES as i64;
        let across = (column - self.first_column).rem_euclid(self.columns);
        if across >= size || row < self.first_row || row >= self.first_row + size {
            return None;
        }
        let slot = (column.rem_euclid(size) + row.rem_euclid(size) * size) as usize;
        (self.loaded >> slot & 1 == 1).then_some(self.shown[slot])
    }

    /// Whether the window holds a tile in the cell at `column` and `row` of a grid with
    /// `columns`, which are fewer than the window's.
    fn reaches(&self, columns: i64, column: i64, row: i64) -> bool {
        let scale = self.columns / columns;
        (0..scale)
            .any(|j| (0..scale).any(|i| self.tile(column * scale + i, row * scale + j).is_some()))
    }
}

/// The patches that show the globe from `eye`, with the detail windows in `windows`:
/// those of the day's imagery, then those of the night's lights.
/// The lights show where the ground is dark, with the `sun` in its direction, unless the whole
/// globe is lit.
pub fn patches(eye: Vec3, sun: Option<Vec3>, windows: &[[f32; WINDOW_FLOATS]]) -> Patches {
    let windows: Vec<Option<Window>> = windows.iter().map(Window::new).collect();
    let (day, night) = windows.split_at(LEVELS);
    let day = Tiles { windows: day, first_layer: 0, tile: Kind::Tile, fading: Kind::FadingTile };
    let night =
        Tiles { windows: night, first_layer: LEVELS, tile: Kind::Night, fading: Kind::FadingNight };
    let lights = sun.filter(|_| night.windows.iter().any(Option::is_some));
    let mut patches = Patches::default();
    // The grid of the coarsest window, which the others halve.
    let (columns, rows) = (gibs::columns(FIRST_ZOOM) as i64, gibs::rows(FIRST_ZOOM) as i64);
    let distance = eye.length();
    let towards_eye = eye / distance;
    // The angle from the point below the camera to the horizon.
    let horizon = (1.0 / distance).min(1.0).acos();
    for row in 0..rows {
        for column in 0..columns {
            let size = Vec2::new(1.0 / columns as f32, 1.0 / rows as f32);
            let corner = Vec2::new(column as f32, row as f32) * size;
            // A cell is in view where its middle is no further beyond the horizon than its
            // corners are from its middle.
            let middle = on_globe(corner + size / 2.0);
            let reach = [Vec2::ZERO, Vec2::X, Vec2::Y, Vec2::ONE]
                .map(|at| on_globe(corner + size * at).dot(middle))
                .into_iter()
                .fold(1.0, f32::min)
                .acos();
            if middle.dot(towards_eye).clamp(-1.0, 1.0).acos() > horizon + reach {
                continue;
            }
            // No sunlight reaches the ground from 0.06 below the horizon on, see `vs_patch`.
            // A cell that's all in the dark shows the lights' tiles, since the day's imagery
            // is too dark there to tell its tiles from the globe's own.
            let dark = lights.is_some_and(|sun| {
                middle.dot(sun).clamp(-1.0, 1.0).acos() - reach > (-0.06f32).acos()
            });
            let tiles = if dark { &night } else { &day };
            cell(&mut patches, tiles, 0, (columns, rows), (column, row), Source::Map);
        }
    }
    patches
}

/// The point of the globe at `uv` on the map.
fn on_globe(uv: Vec2) -> Vec3 {
    let longitude = (uv.x - 0.5) * std::f32::consts::TAU;
    let latitude = (0.5 - uv.y) * std::f32::consts::PI;
    Vec3::new(latitude.cos() * longitude.sin(), latitude.sin(), latitude.cos() * longitude.cos())
}

/// Adds the patches of the cell at `column` and `row` of the grid at `depth`, which has
/// `columns` and `rows`. `above` is the imagery that the cell shows unless it has its own.
fn cell(
    patches: &mut Patches,
    tiles: &Tiles,
    depth: usize,
    (columns, rows): (i64, i64),
    (column, row): (i64, i64),
    above: Source,
) {
    let windows = tiles.windows;
    let edge = |index: i64, count: i64| index as f32 / count as f32;
    let rect =
        [edge(column, columns), edge(row, rows), edge(column + 1, columns), edge(row + 1, rows)];
    let patch = |source: Source, shown: f32| {
        let source = match source {
            Source::Map => [1.0, 1.0, 0.0, shown],
            Source::Tile { level } => {
                let window = windows[level].as_ref().unwrap();
                let slots = WINDOW_TILES as f32;
                let layer = (tiles.first_layer + level) as f32;
                [window.columns as f32 / slots, window.rows as f32 / slots, layer, shown]
            }
        };
        Patch { rect, source }
    };
    // The grid at `depth` is that of the window at `depth`.
    let tile = windows[depth]
        .as_ref()
        .filter(|window| window.columns == columns)
        .and_then(|window| window.tile(column, row));
    let mut source = above;
    match tile {
        Some(shown) if shown >= 1.0 => source = Source::Tile { level: depth },
        Some(shown) => patches.0[tiles.fading as usize][depth]
            .push(patch(Source::Tile { level: depth }, shown)),
        None => {}
    }
    let finer =
        windows[depth + 1..].iter().flatten().any(|window| window.reaches(columns, column, row));
    if finer && depth + 1 < LEVELS {
        for (i, j) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let at = (column * 2 + i, row * 2 + j);
            cell(patches, tiles, depth + 1, (columns * 2, rows * 2), at, source);
        }
        return;
    }
    let kind = match source {
        Source::Map => Kind::Map,
        Source::Tile { .. } => tiles.tile,
    };
    patches.0[kind as usize][depth].push(patch(source, 1.0));
}

/// The stars, as `vs_star` in `globe.wgsl` takes them: where each is, then its brightness, its
/// tint, and what tells its size and its twinkling from the others'.
pub fn stars(count: usize) -> Vec<[f32; 6]> {
    // A small generator of the same numbers every time.
    let mut state = 0x9e3779b97f4a7c15u64;
    let mut random = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 40) as f32 / (1u64 << 24) as f32
    };
    (0..count)
        .map(|_| {
            // Evenly over the sphere.
            let (height, turn) = (random() * 2.0 - 1.0, random() * std::f32::consts::TAU);
            let around = (1.0 - height * height).sqrt();
            let brightness = (random().powi(2) * 2.0).min(1.0);
            [around * turn.cos(), height, around * turn.sin(), brightness, random(), random()]
        })
        .collect()
}
