// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Sharper imagery where the camera looks, from map tiles.
//!
//! The globe's imagery is 4096 pixels wide, which is blurry from close up. Around the point
//! the camera looks at, `LEVELS` detail windows replace it, each at twice the resolution of
//! the one before, and each `WINDOW_TILES` tiles wide and high. The tiles are those of NASA's
//! GIBS, in the same projection as the globe's textures: at zoom level `z`, a pixel is
//! `0.5625 / 2^z` degrees.
//!
//! All windows share one array texture. A tile's place in its layer is its column and row
//! modulo `WINDOW_TILES`, and the texture repeats, so tiles stay where they are while the
//! window moves over the globe, and only the tiles that come into it are loaded.

use std::f32::consts::PI;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use glam::{Vec2, Vec3};
use slint::wgpu_30::wgpu;

use crate::renderer::{Imagery, Renderer};

pub const LEVELS: usize = 3;
/// The zoom level of the first, coarsest window. Its 0.035 degrees per pixel are 2.5 times as
/// fine as the globe's own imagery.
const FIRST_ZOOM: u32 = 4;
/// The width and height of a tile in pixels.
pub const TILE: u32 = 512;
pub const WINDOW_TILES: u32 = 4;
/// The mip levels of a tile, down to the 4 pixels of one compressed block.
pub const MIPS: u32 = 8;
/// What the shader gets for each window, see `Detail::update()`.
pub const WINDOW_FLOATS: usize = 4 + (WINDOW_TILES * WINDOW_TILES) as usize;
/// The degrees per pixel of the globe's own imagery.
const BASE_RESOLUTION: f32 = 360.0 / 4096.0;
/// How many tiles load at once.
const LOADING_AT_ONCE: usize = 6;
const FADE_SECONDS: f32 = 0.4;

/// Where the tiles come from.
pub trait Source: Send + Sync + 'static {
    /// The JPEG or PNG file of the tile, or `None` if there isn't one. Called on a thread of
    /// its own, so it may take a while.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    fn fetch(&self, zoom: u32, row: u32, column: u32) -> Option<Vec<u8>>;
}

/// Tiles in a directory, as `<zoom>/<row>/<column>.jpg`.
#[cfg(not(target_arch = "wasm32"))]
pub struct Directory(pub std::path::PathBuf);

#[cfg(not(target_arch = "wasm32"))]
impl Source for Directory {
    fn fetch(&self, zoom: u32, row: u32, column: u32) -> Option<Vec<u8>> {
        std::fs::read(self.0.join(format!("{zoom}/{row}/{column}.jpg"))).ok()
    }
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Wanted,
    Loading,
    /// On the GPU.
    Loaded,
    /// The source has no such tile.
    Missing,
}

/// One of a window's places for a tile.
#[derive(Clone, Copy)]
struct Slot {
    /// The column and row of the tile that belongs here.
    tile: (u32, u32),
    state: State,
    /// How far the tile has faded in, from 0 to 1.
    shown: f32,
}

struct Window {
    zoom: u32,
    columns: u32,
    rows: u32,
    /// The point the camera looks at, in tiles from the top left of the map.
    center: Vec2,
    slots: [Slot; (WINDOW_TILES * WINDOW_TILES) as usize],
}

impl Window {
    fn new(zoom: u32) -> Self {
        // GIBS covers the world with 2.5 by 1.25 tiles at zoom 0, rounded up, and at zoom 3
        // and beyond with a whole number of them.
        let columns = 10 << (zoom - 3);
        let empty = Slot { tile: (u32::MAX, u32::MAX), state: State::Wanted, shown: 0.0 };
        Self {
            zoom,
            columns,
            rows: columns / 2,
            center: Vec2::ZERO,
            slots: [empty; (WINDOW_TILES * WINDOW_TILES) as usize],
        }
    }

    fn resolution(&self) -> f32 {
        0.5625 / (1 << self.zoom) as f32
    }

    fn slot(column: u32, row: u32) -> usize {
        (column % WINDOW_TILES + row % WINDOW_TILES * WINDOW_TILES) as usize
    }

    /// Moves the window to `center`, and makes room for the tiles that come into it.
    fn move_to(&mut self, center: Vec2) {
        self.center = center;
        let half = WINDOW_TILES as f32 / 2.0;
        let first_column = (center.x - half).round() as i64;
        let first_row =
            ((center.y - half).round() as i64).clamp(0, (self.rows - WINDOW_TILES) as i64);
        for j in 0..WINDOW_TILES as i64 {
            for i in 0..WINDOW_TILES as i64 {
                let column = (first_column + i).rem_euclid(self.columns as i64) as u32;
                let row = (first_row + j) as u32;
                let slot = &mut self.slots[Self::slot(column, row)];
                if slot.tile != (column, row) {
                    *slot = Slot { tile: (column, row), state: State::Wanted, shown: 0.0 };
                }
            }
        }
    }
}

struct Arrived {
    window: usize,
    tile: (u32, u32),
    /// The tile's mip levels, or `None` if the source has no such tile.
    mips: Option<Vec<Vec<u8>>>,
}

/// The detail windows and the tiles on their way into them.
pub struct Detail {
    windows: [Window; LEVELS],
    source: Option<Arc<dyn Source>>,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    features: wgpu::Features,
    sender: Sender<Arrived>,
    receiver: Receiver<Arrived>,
    loading: usize,
}

impl Detail {
    /// Without a `source`, the globe shows its own imagery only.
    pub fn new(source: Option<Arc<dyn Source>>, features: wgpu::Features) -> Self {
        let (sender, receiver) = channel();
        Self {
            windows: std::array::from_fn(|index| Window::new(FIRST_ZOOM + index as u32)),
            source,
            features,
            sender,
            receiver,
            loading: 0,
        }
    }

    /// Whether tiles are loading or fading in, so that the globe needs to be drawn again.
    pub fn is_busy(&self) -> bool {
        self.loading > 0
            || self
                .windows
                .iter()
                .flat_map(|window| &window.slots)
                .any(|slot| slot.state == State::Loaded && slot.shown < 1.0)
    }

    /// Follows the camera, which looks at `target` on the globe, where a pixel of the frame
    /// covers `footprint` degrees. Puts the tiles that arrived onto the GPU, and asks for the
    /// ones that are missing.
    ///
    /// Returns what the shader needs for each window: where the camera looks, in tiles, the
    /// number of the map's columns and rows, which is 0 without tiles, and how far the tile of
    /// each slot has faded in.
    pub fn update(
        &mut self,
        renderer: &Renderer,
        target: Vec3,
        footprint: f32,
        dt: f32,
    ) -> [[f32; WINDOW_FLOATS]; LEVELS] {
        let mut floats = [[0.0; WINDOW_FLOATS]; LEVELS];
        let Some(source) = self.source.clone() else { return floats };

        while let Ok(arrived) = self.receiver.try_recv() {
            self.loading -= 1;
            let (column, row) = arrived.tile;
            let slot = &mut self.windows[arrived.window].slots[Window::slot(column, row)];
            // The window may have moved on meanwhile.
            if slot.tile != arrived.tile || slot.state != State::Loading {
                continue;
            }
            match arrived.mips {
                Some(mips) => {
                    let place = (column % WINDOW_TILES, row % WINDOW_TILES);
                    renderer.set_tile(arrived.window as u32, place, &mips);
                    slot.state = State::Loaded;
                }
                None => slot.state = State::Missing,
            }
        }

        let longitude = target.x.atan2(target.z);
        let latitude = target.y.clamp(-1.0, 1.0).asin();
        let position = Vec2::new(longitude / (2.0 * PI) + 0.5, 0.5 - latitude / PI);
        // The tiles to ask for, nearest to where the camera looks first.
        let mut wanted: Vec<(f32, usize, (u32, u32))> = Vec::new();
        for (index, window) in self.windows.iter_mut().enumerate() {
            window.move_to(position * Vec2::new(window.columns as f32, window.rows as f32));
            // A window is needed once the next coarser imagery is blurry at this zoom.
            let coarser = if index == 0 { BASE_RESOLUTION } else { window.resolution() * 2.0 };
            let needed = footprint < coarser * 1.15;
            for slot in &mut window.slots {
                if slot.state == State::Loaded {
                    slot.shown = (slot.shown + dt / FADE_SECONDS).min(1.0);
                } else if slot.state == State::Wanted && needed && slot.tile.0 != u32::MAX {
                    let tile = Vec2::new(slot.tile.0 as f32, slot.tile.1 as f32) + 0.5;
                    let mut offset = tile - window.center;
                    offset.x -= window.columns as f32 * (offset.x / window.columns as f32).round();
                    wanted.push((offset.length(), index, slot.tile));
                }
            }
            let out = &mut floats[index];
            out[0] = window.center.x;
            out[1] = window.center.y;
            out[2] = window.columns as f32;
            out[3] = window.rows as f32;
            for (slot, shown) in window.slots.iter().zip(&mut out[4..]) {
                *shown = slot.shown;
            }
        }

        wanted.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, index, tile) in wanted {
            if self.loading >= LOADING_AT_ONCE {
                break;
            }
            let window = &mut self.windows[index];
            window.slots[Window::slot(tile.0, tile.1)].state = State::Loading;
            let zoom = window.zoom;
            self.loading += 1;
            self.load(source.clone(), index, zoom, tile);
        }
        floats
    }

    /// Fetches and decodes a tile on a thread of its own.
    #[cfg(not(target_arch = "wasm32"))]
    fn load(&self, source: Arc<dyn Source>, window: usize, zoom: u32, tile: (u32, u32)) {
        let (sender, features) = (self.sender.clone(), self.features);
        std::thread::spawn(move || {
            let mips = source.fetch(zoom, tile.1, tile.0).and_then(|file| mips(&file, features));
            let _ = sender.send(Arrived { window, tile, mips });
        });
    }

    /// The web has no threads. Its tiles aren't supported yet.
    #[cfg(target_arch = "wasm32")]
    fn load(&self, _: Arc<dyn Source>, window: usize, _: u32, tile: (u32, u32)) {
        let _ = self.sender.send(Arrived { window, tile, mips: None });
    }
}

/// Decodes a tile and computes its `MIPS` mip levels, each in the format of the tile texture.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn mips(file: &[u8], features: wgpu::Features) -> Option<Vec<Vec<u8>>> {
    let image = image::load_from_memory(file).ok()?.to_rgba8();
    if image.dimensions() != (TILE, TILE) {
        return None;
    }
    let mut level = image.into_raw();
    let mut size = TILE as usize;
    let mut levels = Vec::with_capacity(MIPS as usize);
    for _ in 0..MIPS {
        let half = size / 2;
        let next: Vec<u8> = (0..half * half * 4)
            .map(|k| {
                let (pixel, channel) = (k / 4, k % 4);
                let (x, y) = (pixel % half * 2, pixel / half * 2);
                let at =
                    |dx: usize, dy: usize| level[((y + dy) * size + x + dx) * 4 + channel] as u32;
                ((at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1) + 2) / 4) as u8
            })
            .collect();
        levels.push(Imagery::compress_level(features, level, size));
        (level, size) = (next, half);
    }
    Some(levels)
}
