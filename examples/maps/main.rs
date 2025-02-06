// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Tiles are 256x256px wide.
// Url is https://tile.openstreetmap.org/{zoom}/{x}/{y}.png
// zoom starts at 1.
// x and y go from 0 to 2^zoom - 1.

use core::cell::RefCell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use slint::{Rgba8Pixel, SharedPixelBuffer, VecModel};
use std::collections::BTreeMap;
use std::rc::Rc;

const TILE_SIZE: isize = 256;

slint::slint! {
import { Slider } from "std-widgets.slint";
export struct Tile { x: length, y: length, tile: image}

export component MainUI inherits Window {
    callback flicked(length, length);
    callback zoom-changed(float);
    callback zoom-in(length, length);
    callback zoom-out(length, length);
    callback link-clicked();
    min-height: 500px;
    min-width: 500px;

    out property <length> visible_width: fli.width;
    out property <length> visible_height: fli.height;

    in-out property <float> zoom <=> sli.value;

    in property <[Tile]> tiles;

    public function set_viewport(ox: length, oy: length, width: length, height: length) {
        fli.viewport-x = ox;
        fli.viewport-y = oy;
        fli.viewport-width = width;
        fli.viewport-height = height;
    }

    VerticalLayout {
        fli := Flickable {
            for t in tiles: Image {
                x: t.x;
                y: t.y;
                source: t.tile;
            }
            flicked => {
                root.flicked(fli.viewport-x, fli.viewport-y);
            }
            TouchArea {
                scroll-event(e) => {
                    if e.delta-y > 0 {
                        root.zoom-in(self.mouse-x + fli.viewport-x, self.mouse-y + fli.viewport-y);
                        return accept;
                    } else if e.delta-y < 0 {
                        root.zoom-out(self.mouse-x + fli.viewport-x, self.mouse-y + fli.viewport-y);
                        return accept;
                    }
                    return reject;
                }
            }
        }

        HorizontalLayout {
            sli := Slider {
                minimum: 1;
                maximum: 19;
                released => {
                    zoom-changed(self.value);
                }
            }
        }
    }

    Text {
        text: "Map data from OpenStreetMap";
        x: fli.x + (fli.width) - (self.width) - 3px;
        y: fli.y + (fli.height) - (self.height) - 3px;
        TouchArea {
            clicked => {
                root.link-clicked();
            }
        }
    }
}
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
struct TileCoordinate {
    z: u32,
    x: isize,
    y: isize,
}

struct World {
    client: reqwest::Client,
    loaded_tiles: BTreeMap<TileCoordinate, slint::Image>,
    loading_tiles: BTreeMap<TileCoordinate, Pin<Box<dyn Future<Output = slint::Image>>>>,
    osm_url: String,
    zoom_level: u32,
    visible_height: f64,
    visible_width: f64,
    offset_x: f64,
    offset_y: f64,
}

impl World {
    fn new() -> Self {
        World {
            client: reqwest::Client::new(),
            osm_url: std::env::var("OSM_TILES_URL")
                .unwrap_or("https://tile.openstreetmap.org".to_string()),
            loaded_tiles: Default::default(),
            loading_tiles: Default::default(),
            zoom_level: 1,
            visible_height: 0.,
            visible_width: 0.,
            offset_x: 0.,
            offset_y: 0.,
        }
    }

    fn set_zoom_level(&mut self, zoom_level: u32, ox: f64, oy: f64) {
        if self.zoom_level != zoom_level {
            self.loaded_tiles.clear();
            self.loaded_tiles.clear();
            let exp2 = f64::exp2(zoom_level as f64 - self.zoom_level as f64);
            self.offset_x += ox;
            self.offset_y += oy;
            self.offset_x *= exp2;
            self.offset_y *= exp2;
            self.offset_x -= ox;
            self.offset_y -= oy;
            self.zoom_level = zoom_level;
            self.reset_view();
        }
    }

    fn reset_view(&mut self) {
        let m = 1 << self.zoom_level;
        let min_x = (self.offset_x / TILE_SIZE as f64).floor() as isize;
        let min_y = (self.offset_y / TILE_SIZE as f64).floor() as isize;
        let max_x =
            (((self.offset_x + self.visible_width) / TILE_SIZE as f64).ceil() as isize + 1).min(m);
        let max_y =
            (((self.offset_y + self.visible_height) / TILE_SIZE as f64).ceil() as isize + 1).min(m);
        // remove tiles that is too far away
        const KEEP_CACHED_TILES: isize = 10;
        let keep = |coord: &TileCoordinate| {
            coord.z == self.zoom_level
                && (coord.x > min_x - KEEP_CACHED_TILES)
                && (coord.x < max_x + KEEP_CACHED_TILES)
                && (coord.y > min_y - KEEP_CACHED_TILES)
                && (coord.y < max_y + KEEP_CACHED_TILES)
        };
        self.loading_tiles.retain(|coord, _| keep(coord));
        self.loaded_tiles.retain(|coord, _| keep(coord));

        for x in min_x..max_x {
            for y in min_y..max_y {
                let coord = TileCoordinate { z: self.zoom_level, x, y };
                if self.loaded_tiles.contains_key(&coord) {
                    continue;
                }
                self.loading_tiles.entry(coord).or_insert_with(|| {
                    let url = format!("{}/{}/{}/{}.png", self.osm_url, coord.z, coord.x, coord.y);
                    let client = self.client.clone();
                    Box::pin(async move {
                        let response = client
                            .get(&url)
                            .header("User-Agent", "Slint Maps example")
                            .send()
                            .await;
                        let response = match response {
                            Ok(response) => response,
                            Err(err) => {
                                eprintln!("Error loading {url}: {err}");
                                return slint::Image::default();
                            }
                        };
                        if !response.status().is_success() {
                            eprintln!("Error loading {url}: {:?}", response.status());
                            return slint::Image::default();
                        }

                        let bytes = response.bytes().await.unwrap();
                        // Use spawn_blocking to offload the image decoding to a thread as to not block the UI
                        let buffer = tokio::task::spawn_blocking(move || {
                            let image = match image::load_from_memory(&bytes) {
                                Ok(image) => image,
                                Err(err) => {
                                    eprintln!("Error reading {url}: {err}");
                                    return None;
                                }
                            };
                            println!("Loaded {url}");
                            let image = image
                                .resize(
                                    TILE_SIZE as u32,
                                    TILE_SIZE as u32,
                                    image::imageops::FilterType::Nearest,
                                )
                                .into_rgba8();
                            let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                                image.as_raw(),
                                image.width(),
                                image.height(),
                            );
                            Some(buffer)
                        })
                        .await
                        .unwrap();
                        buffer.map(|buffer| slint::Image::from_rgba8(buffer)).unwrap_or_default()
                    })
                });
            }
        }
    }

    fn poll(&mut self, context: &mut Context, changed: &mut bool) {
        self.loading_tiles.retain(|coord, future| {
            let image = future.as_mut().poll(context);
            match image {
                Poll::Ready(image) => {
                    self.loaded_tiles.insert(*coord, image);
                    *changed = true;
                    false
                }
                Poll::Pending => true,
            }
        })
    }
}

struct State {
    world: RefCell<World>,
    main_ui: MainUI,
    poll_handle: RefCell<Option<slint::JoinHandle<()>>>,
}

impl State {
    fn do_poll(self: Rc<Self>) {
        if let Some(handle) = self.poll_handle.take() {
            handle.abort();
        }
        self.refresh_model();
        slint::spawn_local(async move {
            std::future::poll_fn(|context| {
                let mut changed = false;
                self.world.borrow_mut().poll(context, &mut changed);
                if changed {
                    self.refresh_model();
                }
                if self.world.borrow().loading_tiles.is_empty() {
                    Poll::Ready(())
                } else {
                    Poll::Pending
                }
            })
            .await;
        })
        .unwrap();
    }

    fn refresh_model(&self) {
        let vec = VecModel::from(
            self.world
                .borrow()
                .loaded_tiles
                .iter()
                .map(|(coord, image)| Tile {
                    tile: image.clone(),
                    x: (coord.x * TILE_SIZE) as f32,
                    y: (coord.y * TILE_SIZE) as f32,
                })
                .collect::<Vec<Tile>>(),
        );
        self.main_ui.set_tiles(slint::ModelRc::new(vec));
    }

    fn set_viewport_size(&self) {
        let world = self.world.borrow();
        let zoom = world.zoom_level;
        self.main_ui.set_zoom(zoom as _);
        let world_size = (TILE_SIZE * (1 << zoom)) as f32;
        self.main_ui.invoke_set_viewport(
            -world.offset_x as f32,
            -world.offset_y as f32,
            world_size,
            world_size,
        );
    }
}

fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _tokio = rt.enter();

    let state = Rc::new(State {
        world: RefCell::new(World::new()),
        main_ui: MainUI::new().unwrap(),
        poll_handle: None.into(),
    });

    let state_weak = Rc::downgrade(&state);
    state.main_ui.on_flicked(move |ox, oy| {
        let state = state_weak.upgrade().unwrap();
        let mut world = state.world.borrow_mut();
        world.offset_x = -ox as f64;
        world.offset_y = -oy as f64;
        world.visible_width = state.main_ui.get_visible_width() as f64;
        world.visible_height = state.main_ui.get_visible_height() as f64;
        world.reset_view();
        drop(world);
        state.do_poll();
    });
    let state_weak = Rc::downgrade(&state);
    state.main_ui.on_zoom_changed(move |zoom| {
        let state = state_weak.upgrade().unwrap();
        let mut world = state.world.borrow_mut();
        world.visible_width = state.main_ui.get_visible_width() as f64;
        world.visible_height = state.main_ui.get_visible_height() as f64;
        let (vw, vh) = (world.visible_width, world.visible_height);
        world.set_zoom_level(zoom as _, vw / 2., vh / 2.);
        drop(world);
        state.set_viewport_size();
        state.do_poll();
    });
    let state_weak = Rc::downgrade(&state);
    state.main_ui.on_zoom_in(move |ox, oy| {
        let state = state_weak.upgrade().unwrap();
        let mut world = state.world.borrow_mut();
        let z = (world.zoom_level + 1).min(19);
        world.visible_width = state.main_ui.get_visible_width() as f64;
        world.visible_height = state.main_ui.get_visible_height() as f64;
        world.set_zoom_level(z as _, ox as f64, oy as f64);
        drop(world);
        state.set_viewport_size();
        state.do_poll();
    });
    let state_weak = Rc::downgrade(&state);
    state.main_ui.on_zoom_out(move |ox, oy| {
        let state = state_weak.upgrade().unwrap();
        let mut world = state.world.borrow_mut();
        let z = (world.zoom_level - 1).max(1);
        world.visible_width = state.main_ui.get_visible_width() as f64;
        world.visible_height = state.main_ui.get_visible_height() as f64;
        world.set_zoom_level(z as _, ox as f64, oy as f64);
        drop(world);
        state.set_viewport_size();
        state.do_poll();
    });

    {
        let state = state.clone();
        slint::spawn_local(async move {
            let mut world = state.world.borrow_mut();
            world.visible_width = state.main_ui.get_visible_width() as f64;
            world.visible_height = state.main_ui.get_visible_height() as f64;
            world.reset_view();
            drop(world);
            state.set_viewport_size();
            state.clone().do_poll();
        })
        .unwrap();
    }

    state.main_ui.run().unwrap();
}
