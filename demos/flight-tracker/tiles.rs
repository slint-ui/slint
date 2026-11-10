// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Sharper imagery where the camera looks, from map tiles.
//!
//! The globe's imagery is 4096 pixels wide, which is blurry from close up. Around the point
//! the camera looks at, `LEVELS` detail windows replace it, each at twice the resolution of
//! the one before, and each `WINDOW_TILES` tiles wide and high. The lights of the night side
//! have such windows of their own. The tiles are those of NASA's
//! GIBS, in the same projection as the globe's textures: at zoom level `z`, a pixel is
//! `0.5625 / 2^z` degrees.
//!
//! The tiles come from GIBS over the network, and natively stay in a cache directory, or from
//! a directory of tiles alone, or from the binary, see `Source`.
//!
//! All windows share one array texture. A tile's place in its layer is its column and row
//! modulo `WINDOW_TILES`, and the texture repeats, so tiles stay where they are while the
//! window moves over the globe, and only the tiles that come into it are loaded.

use std::f32::consts::PI;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use glam::{Vec2, Vec3};
use slint::wgpu_30::wgpu;

use crate::gibs::{FIRST_ZOOM, Kind, LEVELS};
use crate::renderer::{Imagery, Renderer};

/// The windows of the day's imagery, then those of the night's lights.
pub const WINDOWS: usize = LEVELS * 2;
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
/// How long no tiles are asked for after one failed to load, such as without a network.
const RETRY_AFTER: Duration = Duration::from_secs(20);
/// After this many failures without a single tile, the source counts as unreachable for good,
/// as on a web page that may not fetch from other sites.
const GIVE_UP_AFTER: u32 = 6;
/// The tiles that `build.rs` put into the binary.
#[cfg(embedded_tiles)]
mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded_tiles.rs"));
}

/// Where the tiles come from.
#[derive(Clone)]
pub enum Source {
    /// NASA's GIBS. Natively, the tiles stay in the directory `cache` once fetched, as in
    /// `Directory`; on the web, the browser caches them.
    #[cfg(any(target_arch = "wasm32", feature = "online-imagery"))]
    #[cfg_attr(embedded_tiles, allow(dead_code))]
    Online {
        #[cfg(not(target_arch = "wasm32"))]
        cache: Option<std::path::PathBuf>,
    },
    /// A directory of tiles, as `Kind::path()` names them, for use without a network.
    #[cfg(not(target_arch = "wasm32"))]
    Directory(std::path::PathBuf),
    /// The tiles in the binary, for use without a network or a disk.
    #[cfg(embedded_tiles)]
    Embedded,
}

/// What became of a tile that was asked for.
enum Fetched<T> {
    Tile(T),
    /// The source has no such tile.
    Missing,
    /// The source couldn't be reached.
    #[cfg_attr(not(any(target_arch = "wasm32", feature = "online-imagery")), allow(dead_code))]
    Failed,
}

#[cfg(not(target_arch = "wasm32"))]
impl Source {
    /// The file of a tile. This may take a while.
    fn fetch(&self, kind: Kind, zoom: u32, row: u32, column: u32) -> Fetched<Vec<u8>> {
        let path = kind.path(zoom, row, column);
        match self {
            #[cfg(embedded_tiles)]
            Source::Embedded => {
                let tile = (kind as u8, zoom as u8, row as u8, column as u8);
                let found =
                    embedded::INDEX.binary_search_by_key(&tile, |&(k, z, r, c, ..)| (k, z, r, c));
                match found.map(|index| embedded::INDEX[index]) {
                    Ok((.., offset, length)) => {
                        Fetched::Tile(embedded::DATA[offset as usize..][..length as usize].to_vec())
                    }
                    Err(_) => Fetched::Missing,
                }
            }
            Source::Directory(directory) => match std::fs::read(directory.join(path)) {
                Ok(file) => Fetched::Tile(file),
                Err(_) => Fetched::Missing,
            },
            #[cfg(feature = "online-imagery")]
            Source::Online { cache } => {
                let cached = cache.as_ref().map(|cache| cache.join(path));
                if let Some(file) = cached.as_ref().and_then(|path| std::fs::read(path).ok()) {
                    return Fetched::Tile(file);
                }
                let fetched = download(&kind.url(zoom, row, column));
                if let (Fetched::Tile(file), Some(path)) = (&fetched, cached) {
                    // Renamed into place, so that a tile cut short isn't taken for a whole one.
                    let part = path.with_extension("part");
                    let _ = path.parent().map(std::fs::create_dir_all);
                    if std::fs::write(&part, file).is_ok() {
                        let _ = std::fs::rename(&part, &path);
                    }
                }
                fetched
            }
        }
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "online-imagery"))]
fn download(url: &str) -> Fetched<Vec<u8>> {
    use std::sync::OnceLock;
    static CLIENT: OnceLock<Option<reqwest::blocking::Client>> = OnceLock::new();
    let client = CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .user_agent("slint-flight-tracker-demo")
            .timeout(Duration::from_secs(20))
            .build()
            .ok()
    });
    let Some(Ok(response)) = client.as_ref().map(|client| client.get(url).send()) else {
        return Fetched::Failed;
    };
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Fetched::Missing;
    }
    match response.error_for_status().and_then(|response| response.bytes()) {
        Ok(file) => Fetched::Tile(file.to_vec()),
        Err(_) => Fetched::Failed,
    }
}

#[cfg(target_arch = "wasm32")]
async fn download(url: String) -> Fetched<Vec<u8>> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    let Some(window) = web_sys::window() else { return Fetched::Failed };
    let Ok(response) = JsFuture::from(window.fetch_with_str(&url)).await else {
        return Fetched::Failed;
    };
    let Ok(response) = response.dyn_into::<web_sys::Response>() else { return Fetched::Failed };
    if response.status() == 404 {
        return Fetched::Missing;
    }
    if !response.ok() {
        return Fetched::Failed;
    }
    let Ok(buffer) = response.array_buffer() else { return Fetched::Failed };
    match JsFuture::from(buffer).await {
        Ok(buffer) => Fetched::Tile(js_sys::Uint8Array::new(&buffer).to_vec()),
        Err(_) => Fetched::Failed,
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
    kind: Kind,
    zoom: u32,
    columns: u32,
    rows: u32,
    /// The point the camera looks at, in tiles from the top left of the map.
    center: Vec2,
    slots: [Slot; (WINDOW_TILES * WINDOW_TILES) as usize],
}

impl Window {
    fn new(kind: Kind, zoom: u32) -> Self {
        let columns = crate::gibs::columns(zoom);
        let empty = Slot { tile: (u32::MAX, u32::MAX), state: State::Wanted, shown: 0.0 };
        Self {
            kind,
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
    /// Natively, the tile's mip levels, ready for the GPU. On the web, its file.
    tile_data: Fetched<Payload>,
}

#[cfg(not(target_arch = "wasm32"))]
type Payload = Vec<Vec<u8>>;
#[cfg(target_arch = "wasm32")]
type Payload = Vec<u8>;

/// A tile on the web, which has no threads: its mip levels are compressed a strip at a time,
/// over several frames.
#[cfg(target_arch = "wasm32")]
struct Job {
    window: usize,
    tile: (u32, u32),
    /// The mip levels as RGBA pixels.
    levels: Vec<Vec<u8>>,
    /// The compressed levels, the last of which may be unfinished up to `row`.
    done: Vec<Vec<u8>>,
    row: usize,
}

#[cfg(target_arch = "wasm32")]
impl Job {
    /// The rows of pixels compressed in one step.
    const STRIP: usize = 64;

    /// Compresses the next strip. Returns whether the tile is finished.
    fn step(&mut self, features: wgpu::Features) -> bool {
        let level = if self.row == 0 { self.done.len() } else { self.done.len() - 1 };
        let size = TILE as usize >> level;
        let rows = Self::STRIP.min(size - self.row);
        let pixels = &self.levels[level][self.row * size * 4..(self.row + rows) * size * 4];
        let strip = Imagery::compress_level(features, pixels, size, rows);
        if self.row == 0 {
            self.done.push(strip);
        } else {
            self.done[level].extend(strip);
        }
        self.row = (self.row + rows) % size;
        self.row == 0 && self.done.len() == MIPS as usize
    }
}

/// The detail windows and the tiles on their way into them.
pub struct Detail {
    windows: [Window; WINDOWS],
    source: Option<Source>,
    features: wgpu::Features,
    sender: Sender<Arrived>,
    receiver: Receiver<Arrived>,
    loading: usize,
    /// Tiles that failed to load in a row.
    failures: u32,
    /// Whether any tile ever loaded.
    reached: bool,
    /// Until when no tiles are asked for, after one failed to load.
    paused_until: Option<web_time::Instant>,
    /// The tiles that arrived, and the one being compressed.
    #[cfg(target_arch = "wasm32")]
    files: std::collections::VecDeque<(usize, (u32, u32), Vec<u8>)>,
    #[cfg(target_arch = "wasm32")]
    job: Option<Job>,
}

impl Detail {
    /// Without a `source`, the globe shows its own imagery only.
    pub fn new(source: Option<Source>, features: wgpu::Features) -> Self {
        let (sender, receiver) = channel();
        Self {
            windows: std::array::from_fn(|index| {
                let kind = if index < LEVELS { Kind::Day } else { Kind::Night };
                Window::new(kind, FIRST_ZOOM + (index % LEVELS) as u32)
            }),
            source,
            features,
            sender,
            receiver,
            loading: 0,
            failures: 0,
            reached: false,
            paused_until: None,
            #[cfg(target_arch = "wasm32")]
            files: Default::default(),
            #[cfg(target_arch = "wasm32")]
            job: None,
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
    /// covers `footprint` degrees. Puts the tiles that arrived onto the GPU. While `enabled`,
    /// the windows show, and the tiles that are missing are asked for, those of the day's
    /// imagery while `day` and those of the night's lights while `night`.
    ///
    /// Returns what the shader needs for each window: where the camera looks, in tiles, the
    /// number of the map's columns and rows, which is 0 without tiles, and how far the tile of
    /// each slot has faded in.
    pub fn update(
        &mut self,
        renderer: &Renderer,
        target: Vec3,
        footprint: f32,
        enabled: bool,
        (day, night): (bool, bool),
        dt: f32,
    ) -> [[f32; WINDOW_FLOATS]; WINDOWS] {
        let mut floats = [[0.0; WINDOW_FLOATS]; WINDOWS];

        while let Ok(arrived) = self.receiver.try_recv() {
            match arrived.tile_data {
                Fetched::Tile(payload) => {
                    self.arrived(renderer, arrived.window, arrived.tile, payload)
                }
                Fetched::Missing => {
                    self.loading -= 1;
                    if let Some(slot) = self.loading_slot(arrived.window, arrived.tile) {
                        slot.state = State::Missing;
                    }
                }
                Fetched::Failed => {
                    self.loading -= 1;
                    if let Some(slot) = self.loading_slot(arrived.window, arrived.tile) {
                        slot.state = State::Wanted;
                    }
                    self.failures += 1;
                    self.paused_until = Some(web_time::Instant::now() + RETRY_AFTER);
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        self.compress(renderer);
        if !self.reached && self.failures >= GIVE_UP_AFTER {
            self.source = None;
        }
        let Some(source) = self.source.clone() else { return floats };
        if !enabled {
            return floats;
        }

        let longitude = target.x.atan2(target.z);
        let latitude = target.y.clamp(-1.0, 1.0).asin();
        let position = Vec2::new(longitude / (2.0 * PI) + 0.5, 0.5 - latitude / PI);
        // The tiles to ask for, nearest to where the camera looks first.
        let mut wanted: Vec<(f32, usize, (u32, u32))> = Vec::new();
        for (index, window) in self.windows.iter_mut().enumerate() {
            window.move_to(position * Vec2::new(window.columns as f32, window.rows as f32));
            // A window is needed once the next coarser imagery is blurry at this zoom.
            let coarser =
                if window.zoom == FIRST_ZOOM { BASE_RESOLUTION } else { window.resolution() * 2.0 };
            let fetching = if window.kind == Kind::Day { day } else { night };
            let needed = fetching && footprint < coarser * 1.15;
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

        if self.paused_until.is_some_and(|until| web_time::Instant::now() < until) {
            return floats;
        }
        self.paused_until = None;
        wanted.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, index, tile) in wanted {
            if self.loading >= LOADING_AT_ONCE {
                break;
            }
            let window = &mut self.windows[index];
            window.slots[Window::slot(tile.0, tile.1)].state = State::Loading;
            let (kind, zoom) = (window.kind, window.zoom);
            self.loading += 1;
            self.load(source.clone(), index, kind, zoom, tile);
        }
        floats
    }

    /// The slot of `tile`, if it's still waiting for that tile: the window may have moved on.
    fn loading_slot(&mut self, window: usize, tile: (u32, u32)) -> Option<&mut Slot> {
        let slot = &mut self.windows[window].slots[Window::slot(tile.0, tile.1)];
        (slot.tile == tile && slot.state == State::Loading).then_some(slot)
    }

    /// Puts a tile's mip levels onto the GPU.
    fn show(&mut self, renderer: &Renderer, window: usize, tile: (u32, u32), mips: &[Vec<u8>]) {
        self.loading -= 1;
        self.reached = true;
        self.failures = 0;
        if let Some(slot) = self.loading_slot(window, tile) {
            slot.state = State::Loaded;
            renderer.set_tile(window as u32, (tile.0 % WINDOW_TILES, tile.1 % WINDOW_TILES), mips);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn arrived(&mut self, renderer: &Renderer, window: usize, tile: (u32, u32), mips: Payload) {
        self.show(renderer, window, tile, &mips);
    }

    /// Fetches, decodes, and compresses a tile on a thread of its own.
    #[cfg(not(target_arch = "wasm32"))]
    fn load(&self, source: Source, window: usize, kind: Kind, zoom: u32, tile: (u32, u32)) {
        let (sender, features) = (self.sender.clone(), self.features);
        std::thread::spawn(move || {
            let tile_data = match source.fetch(kind, zoom, tile.1, tile.0) {
                Fetched::Tile(file) => match rgba_mips(&file) {
                    Some(levels) => Fetched::Tile(
                        levels
                            .iter()
                            .enumerate()
                            .map(|(level, pixels)| {
                                let size = TILE as usize >> level;
                                Imagery::compress_level(features, pixels, size, size)
                            })
                            .collect(),
                    ),
                    None => Fetched::Missing,
                },
                Fetched::Missing => Fetched::Missing,
                Fetched::Failed => Fetched::Failed,
            };
            let _ = sender.send(Arrived { window, tile, tile_data });
        });
    }

    #[cfg(target_arch = "wasm32")]
    fn arrived(&mut self, _: &Renderer, window: usize, tile: (u32, u32), file: Payload) {
        self.files.push_back((window, tile, file));
    }

    /// Fetches a tile. Its file arrives in a later frame.
    #[cfg(target_arch = "wasm32")]
    fn load(&self, _: Source, window: usize, kind: Kind, zoom: u32, tile: (u32, u32)) {
        let sender = self.sender.clone();
        let url = kind.url(zoom, tile.1, tile.0);
        wasm_bindgen_futures::spawn_local(async move {
            let tile_data = download(url).await;
            let _ = sender.send(Arrived { window, tile, tile_data });
        });
    }

    /// Decodes and compresses the tiles that arrived, for a few milliseconds of each frame.
    #[cfg(target_arch = "wasm32")]
    fn compress(&mut self, renderer: &Renderer) {
        let start = web_time::Instant::now();
        while start.elapsed() < Duration::from_millis(4) {
            let Some(job) = &mut self.job else {
                let Some((window, tile, file)) = self.files.pop_front() else { break };
                match rgba_mips(&file) {
                    Some(levels) if self.loading_slot(window, tile).is_some() => {
                        self.job = Some(Job { window, tile, levels, done: Vec::new(), row: 0 });
                    }
                    levels => {
                        self.loading -= 1;
                        if let (None, Some(slot)) = (levels, self.loading_slot(window, tile)) {
                            slot.state = State::Missing;
                        }
                    }
                }
                continue;
            };
            if job.step(self.features) {
                let job = self.job.take().unwrap();
                self.show(renderer, job.window, job.tile, &job.done);
            }
        }
    }
}

/// Decodes a tile and computes its `MIPS` mip levels, as RGBA pixels.
fn rgba_mips(file: &[u8]) -> Option<Vec<Vec<u8>>> {
    let image = image::load_from_memory(file).ok()?.to_rgba8();
    if image.dimensions() != (TILE, TILE) {
        return None;
    }
    let mut levels = vec![image.into_raw()];
    let mut size = TILE as usize;
    while levels.len() < MIPS as usize {
        let (last, half) = (levels.last().unwrap(), size / 2);
        let next = (0..half * half * 4)
            .map(|k| {
                let (pixel, channel) = (k / 4, k % 4);
                let (x, y) = (pixel % half * 2, pixel / half * 2);
                let at =
                    |dx: usize, dy: usize| last[((y + dy) * size + x + dx) * 4 + channel] as u32;
                ((at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1) + 2) / 4) as u8
            })
            .collect();
        levels.push(next);
        size = half;
    }
    Some(levels)
}
