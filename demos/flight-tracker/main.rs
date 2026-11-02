// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore xorshift

use std::cell::RefCell;
use std::f32::consts::PI;
use std::rc::Rc;
use std::time::Duration;

use glam::{Mat4, Vec2, Vec3, Vec4Swizzles};
use slint::wgpu_30::{WGPUConfiguration, WGPUSettings, wgpu};

use atlas::{Airport, City};
use camera::{Camera, FOV_Y, MIN_RANGE, Pose, surface};
use flight::{EARTH_RADIUS_KM, Flight};
use renderer::{Frame, Renderer};

mod atlas;
mod camera;
mod compress;
mod flight;
mod map;
mod renderer;

slint::include_modules!();

/// The views the kiosk cycles through during a flight, and how many seconds each one lasts.
const VIEW_CYCLE: [(View, f32); 3] = [(View::Follow, 14.0), (View::Route, 8.0), (View::Globe, 8.0)];
const BOARDING_SECONDS: f32 = 4.0;
const ARRIVED_SECONDS: f32 = 7.0;
/// How many flights the departures board shows.
const TILES: usize = 6;
/// How long the board shows before the kiosk picks a flight, and how long that flight's tile
/// is highlighted before the zoom.
const BOARD_SECONDS: f32 = 6.0;
const HIGHLIGHT_SECONDS: f32 = 1.5;
/// How long the globe takes to grow from its tile to the window, or back.
const ZOOM_SECONDS: f32 = 1.4;
/// How long a flown flight stays on the board before another one replaces it.
const REPLACE_SECONDS: f32 = 2.5;
/// How the globes on the board look at their routes: from the side, turning slowly.
const TILE_TILT_DEGREES: f32 = 22.0;
const TILE_SPIN: f32 = 0.12;
/// After how long without input the kiosk takes over again.
const IDLE: Duration = Duration::from_secs(60);
/// The kiosk's flights are at least this long, and at most the other.
const KIOSK_KM: (f32, f32) = (1500.0, 13000.0);
const VIEW_TRANSITION: f32 = 2.0;
const MAX_LABELS: usize = 24;
/// The most physical pixels per logical pixel the globe renders at, so that phones with a
/// scale factor of 3 render fewer than half the pixels of their full resolution.
const MAX_GLOBE_SCALE: f32 = 2.0;
/// The kiosk flies between airports within `HUB_KM` of a city with at least
/// `HUB_POPULATION` thousand people.
const HUB_KM: f32 = 60.0;
const HUB_POPULATION: f32 = 1500.0;

/// The direction towards the sun, `hours` after the current time.
fn sun_direction(hours: f32) -> Vec3 {
    let now = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map_or(0.0, |time| time.as_secs_f64());
    let seconds = now + hours as f64 * 3600.0;
    // Days since noon on the 1st of January 2000, and the day of the year from there.
    let days = (seconds - 946_728_000.0) / 86_400.0;
    let day_of_year = days.rem_euclid(365.2422) as f32;
    let declination = -23.44f32.to_radians() * (2.0 * PI / 365.2422 * (day_of_year + 10.0)).cos();
    let utc_hours = (seconds / 3600.0).rem_euclid(24.0) as f32;
    let lon = -(utc_hours - 12.0) * 15.0f32.to_radians();
    surface(lon, declination)
}

fn random_seed() -> u64 {
    let now = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map_or(0, |time| time.as_nanos() as u64);
    (now ^ (now >> 29)).wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1
}

/// Where `point` is on the screen, in logical pixels, if it's in front of the camera.
fn project(view_projection: Mat4, point: Vec3, size: Vec2) -> Option<Vec2> {
    let clip = view_projection * point.extend(1.0);
    if clip.w <= 1e-4 {
        return None;
    }
    let ndc = clip.xy() / clip.w;
    Some(Vec2::new((ndc.x * 0.5 + 0.5) * size.x, (0.5 - ndc.y * 0.5) * size.y))
}

/// Whether the globe hides `point`, which is on or above the ground, from `eye`.
fn occluded(eye: Vec3, point: Vec3) -> bool {
    let ray = point - eye;
    let a = ray.length_squared();
    let b = 2.0 * eye.dot(ray);
    let c = eye.length_squared() - 1.0;
    let discriminant = b * b - 4.0 * a * c;
    discriminant > 0.0 && {
        let t = (-b - discriminant.sqrt()) / (2.0 * a);
        t > 0.0 && t < 0.999
    }
}

/// Where a point slightly above the ground at `point` is on the screen, unless it's hidden.
fn screen_point(view_projection: Mat4, eye: Vec3, point: Vec3, size: Vec2) -> Option<Vec2> {
    let lifted = point * 1.0005;
    if occluded(eye, lifted) {
        return None;
    }
    project(view_projection, lifted, size)
}

/// How much `point` on the ground faces `eye`, from 0 at the horizon to 1 straight below.
fn facing(eye: Vec3, point: Vec3) -> f32 {
    point.dot((eye - point).normalize())
}

fn thousands(value: f32) -> String {
    let digits = (value.max(0.0).round() as u64).to_string();
    let mut text = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            text.push(',');
        }
        text.push(digit);
    }
    text
}

fn format_hours(hours: f32) -> String {
    let minutes = (hours * 60.0).round() as u32;
    match (minutes / 60, minutes % 60) {
        (0, minutes) => format!("{minutes} min"),
        (hours, minutes) => format!("{hours} h {minutes:02} min"),
    }
}

fn compass(degrees: f32) -> &'static str {
    const POINTS: [&str; 16] = [
        "N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE", "S", "SSW", "SW", "WSW", "W", "WNW",
        "NW", "NNW",
    ];
    POINTS[((degrees / 22.5).round() as usize) % 16]
}

/// The sun's elevation in degrees at `point`, `hours` from now.
fn sun_elevation(point: Vec3, hours: f32) -> f32 {
    point.dot(sun_direction(hours)).clamp(-1.0, 1.0).asin().to_degrees()
}

/// When the sun is at `point`, `hours` from now: "in daylight", "at night", "at dawn", or "at dusk".
fn daylight(point: Vec3, hours: f32) -> &'static str {
    let elevation = sun_elevation(point, hours);
    if elevation > 0.0 {
        "in daylight"
    } else if elevation < -6.0 {
        "at night"
    } else if sun_elevation(point, hours + 0.25) > elevation {
        "at dawn"
    } else {
        "at dusk"
    }
}

/// Path commands through the projected `points`, leaving gaps where a point is hidden, and
/// with `dashed`, between every other pair of points.
fn path(points: impl Iterator<Item = Option<Vec2>>, dashed: bool) -> String {
    let mut commands = String::new();
    let mut previous: Option<Vec2> = None;
    for (index, point) in points.enumerate() {
        if let (Some(from), Some(to)) = (previous, point)
            && (!dashed || index % 2 == 1)
        {
            commands += &format!("M {:.1} {:.1} L {:.1} {:.1} ", from.x, from.y, to.x, to.y);
        }
        previous = point;
    }
    commands
}

/// How far from the ground the camera shows all of `flight`'s route.
fn route_range(flight: &Flight, aspect: f32) -> f32 {
    let half = flight.point(0.0).angle_between(flight.point(1.0)) / 2.0;
    // The angle from the screen's center that keeps the ends off the edges.
    let fit = 0.7 * ((FOV_Y / 2.0).tan() * aspect.min(1.0)).atan();
    (half.sin() / fit.tan() - (1.0 - half.cos())).clamp(MIN_RANGE, 4.5)
}

/// What the demo shows: the departures board, the zoom between a tile and the window, or a
/// flight. `Returning` turns the camera back to the flight's tile before zooming out.
#[derive(Clone, Copy, PartialEq)]
enum Stage {
    Board,
    ZoomingIn,
    Flight,
    Returning,
    ZoomingOut,
}

/// A flight on the departures board.
struct Tile {
    flight: Flight,
    /// The heading the globe on the tile had at the start.
    spin: f32,
    frames: Frames,
}

fn airport_item(airports: &[Airport], index: usize) -> AirportItem {
    let airport = &airports[index];
    AirportItem {
        index: index as i32,
        code: airport.code.as_str().into(),
        city: airport.city.as_str().into(),
        country: airport.country.as_str().into(),
        name: airport.name.as_str().into(),
    }
}

/// The kiosk's flights, the user's own, and the camera that watches them.
struct Demo {
    airports: Vec<Airport>,
    /// The airports the kiosk flies between.
    hubs: Vec<usize>,
    cities: Vec<City>,
    flight: Flight,
    phase: Phase,
    phase_time: f32,
    tiles: Vec<Tile>,
    stage: Stage,
    stage_time: f32,
    /// The tile that the flight came from, which the globe grows from and shrinks back into.
    focused: usize,
    /// The tile that the kiosk is about to pick, and when it picked it.
    highlighted: Option<(usize, f32)>,
    /// The tile of a flown flight, which another flight replaces after a while on the board.
    flown: Option<usize>,
    /// The next tile to render, since the board renders one tile each frame.
    next_tile: usize,
    camera: Camera,
    view: View,
    view_time: f32,
    /// Whether the user picked the view, which stops the kiosk from cycling through views.
    view_chosen: bool,
    /// How much the user zoomed into the current view.
    zoom: f32,
    dragging: bool,
    last_drag: web_time::Instant,
    /// Seconds since the start, for the stars' twinkle.
    time: f32,
    random: u64,
    /// Hands control back to the kiosk after a while without input.
    idle: slint::Timer,
}

impl Demo {
    fn new() -> Self {
        let airports = atlas::airports();
        let cities = atlas::cities(500.0);
        let near = HUB_KM / EARTH_RADIUS_KM;
        let hubs = (0..airports.len())
            .filter(|&index| {
                cities
                    .iter()
                    .take_while(|city| city.population >= HUB_POPULATION)
                    .any(|city| city.position.angle_between(airports[index].position) < near)
            })
            .collect();
        let position = airports[0].position;
        let mut demo = Self {
            flight: Flight::new(0, position, 0, position),
            hubs,
            cities,
            phase: Phase::Boarding,
            phase_time: 0.0,
            tiles: Vec::new(),
            stage: Stage::Board,
            stage_time: 0.0,
            focused: 0,
            highlighted: None,
            flown: None,
            next_tile: 0,
            camera: Camera::new(Pose::top_down(position, 3.0)),
            view: View::Route,
            view_time: 0.0,
            view_chosen: false,
            zoom: 1.0,
            dragging: false,
            last_drag: web_time::Instant::now(),
            time: 0.0,
            random: random_seed(),
            idle: slint::Timer::default(),
            airports,
        };
        demo.tiles = (0..TILES).map(|_| demo.random_tile()).collect();
        demo
    }

    /// A tile with a random flight between hubs.
    fn random_tile(&mut self) -> Tile {
        let origin = self.random_hub();
        let destination = self.random_destination(origin);
        self.tile(origin, destination)
    }

    fn tile(&mut self, origin: usize, destination: usize) -> Tile {
        let (from, to) = (self.airports[origin].position, self.airports[destination].position);
        let spin = (self.random % 6283) as f32 / 1000.0;
        Tile {
            flight: Flight::new(origin, from, destination, to),
            spin,
            frames: Frames { textures: Vec::new(), next: 0 },
        }
    }

    /// Where the camera looks at the route on tile `index`.
    fn tile_pose(&self, index: usize, aspect: f32) -> Pose {
        let flight = &self.tiles[index].flight;
        Pose {
            target: flight.point(0.5),
            heading: self.tiles[index].spin + self.time * TILE_SPIN,
            tilt: TILE_TILT_DEGREES.to_radians(),
            range: route_range(flight, aspect) * 1.15,
        }
    }

    /// Starts the flight on tile `index`, growing its globe to the window.
    fn enter(&mut self, index: usize, aspect: f32) {
        let pose = self.tile_pose(index, aspect);
        let (origin, destination) =
            (self.tiles[index].flight.origin, self.tiles[index].flight.destination);
        self.start_flight(origin, destination);
        self.camera = Camera::new(pose);
        self.view_chosen = false;
        self.set_view(View::Route);
        self.focused = index;
        self.highlighted = None;
        self.flown = None;
        self.set_stage(Stage::ZoomingIn);
    }

    /// Turns back to the flight's tile and shrinks the globe into it.
    fn leave(&mut self) {
        if self.stage == Stage::Flight {
            self.camera.transition(VIEW_TRANSITION);
            self.set_stage(Stage::Returning);
        }
    }

    fn set_stage(&mut self, stage: Stage) {
        self.stage = stage;
        self.stage_time = 0.0;
    }

    /// How far the globe has grown from its tile to the window, from 0 to 1.
    fn zoom_progress(&self) -> f32 {
        match self.stage {
            Stage::Board => 0.0,
            Stage::ZoomingIn => camera::ease(self.stage_time / ZOOM_SECONDS),
            Stage::Flight | Stage::Returning => 1.0,
            Stage::ZoomingOut => 1.0 - camera::ease(self.stage_time / ZOOM_SECONDS),
        }
    }

    fn random_hub(&mut self) -> usize {
        // xorshift64
        self.random ^= self.random << 13;
        self.random ^= self.random >> 7;
        self.random ^= self.random << 17;
        self.hubs[(self.random % self.hubs.len() as u64) as usize]
    }

    /// A random hub a long-haul flight away from `origin`.
    fn random_destination(&mut self, origin: usize) -> usize {
        let from = self.airports[origin].position;
        let mut candidate = origin;
        for _ in 0..500 {
            candidate = self.random_hub();
            let km = from.angle_between(self.airports[candidate].position) * EARTH_RADIUS_KM;
            if (KIOSK_KM.0..=KIOSK_KM.1).contains(&km) {
                break;
            }
        }
        candidate
    }

    fn start_flight(&mut self, origin: usize, destination: usize) {
        let (from, to) = (self.airports[origin].position, self.airports[destination].position);
        self.flight = Flight::new(origin, from, destination, to);
        self.phase = Phase::Boarding;
        self.phase_time = 0.0;
        if !self.view_chosen || self.view == View::Free {
            self.view_chosen = false;
            self.set_view(View::Route);
        }
    }

    fn set_view(&mut self, view: View) {
        self.camera.transition(VIEW_TRANSITION);
        self.view = view;
        self.view_time = 0.0;
        self.zoom = 1.0;
    }

    /// How far from the ground `view` puts the camera before the user zooms.
    fn base_range(&self, view: View, aspect: f32) -> f32 {
        match view {
            View::Follow => 0.6,
            View::Route => route_range(&self.flight, aspect),
            View::Globe => 2.6,
            View::Free => self.camera.pose.range,
        }
    }

    /// Where `view` puts the camera.
    fn goal(&self, view: View, aspect: f32) -> Pose {
        let flight = &self.flight;
        let range = self.base_range(view, aspect) * self.zoom;
        match view {
            View::Follow => Pose {
                target: flight.position(),
                heading: flight.heading_degrees().to_radians(),
                tilt: 50f32.to_radians(),
                range,
            },
            View::Route => Pose::top_down(flight.point(0.5), range),
            View::Globe => Pose::top_down(flight.position(), range),
            View::Free => self.camera.pose,
        }
    }

    /// Zooms in by `factor`, down to `MIN_RANGE`.
    fn zoom_by(&mut self, factor: f32, aspect: f32) {
        if self.view == View::Free {
            self.camera.pose.range = Camera::zoomed(self.camera.pose.range, factor);
        } else {
            let base = self.base_range(self.view, aspect);
            self.zoom = (self.zoom / factor).clamp(MIN_RANGE / base, 3.0);
        }
    }

    fn step(&mut self, kiosk: bool, aspect: f32, dt: f32) {
        self.time += dt;
        self.stage_time += dt;
        match self.stage {
            Stage::Board => {
                if let Some(index) = self.flown
                    && self.stage_time >= REPLACE_SECONDS
                {
                    self.tiles[index] = self.random_tile();
                    self.flown = None;
                }
                match self.highlighted {
                    None if kiosk && self.stage_time >= BOARD_SECONDS => {
                        let shift = 1 + (self.random_hub() % (TILES - 1));
                        self.highlighted = Some(((self.focused + shift) % TILES, self.stage_time));
                    }
                    Some((index, since)) if self.stage_time - since >= HIGHLIGHT_SECONDS => {
                        self.enter(index, aspect);
                    }
                    _ => {}
                }
                return;
            }
            Stage::ZoomingIn if self.stage_time >= ZOOM_SECONDS => self.set_stage(Stage::Flight),
            Stage::Returning if self.stage_time >= VIEW_TRANSITION => {
                self.set_stage(Stage::ZoomingOut)
            }
            Stage::ZoomingOut if self.stage_time >= ZOOM_SECONDS => {
                self.set_stage(Stage::Board);
                self.flown = Some(self.focused);
                return;
            }
            _ => {}
        }
        self.phase_time += dt;
        match self.phase {
            Phase::Boarding => {
                if self.phase_time >= BOARDING_SECONDS {
                    self.phase = Phase::Flying;
                    self.phase_time = 0.0;
                    if !self.view_chosen {
                        self.set_view(View::Follow);
                    }
                }
            }
            Phase::Flying => {
                self.flight.advance(dt);
                if self.flight.landed() {
                    self.phase = Phase::Arrived;
                    self.phase_time = 0.0;
                    if !self.view_chosen && self.view != View::Follow {
                        self.set_view(View::Follow);
                    }
                } else if kiosk && !self.view_chosen && self.stage == Stage::Flight {
                    self.view_time += dt;
                    let current = VIEW_CYCLE.iter().position(|(view, _)| *view == self.view);
                    let index = current.unwrap_or(0);
                    if current.is_none() || self.view_time >= VIEW_CYCLE[index].1 {
                        self.set_view(VIEW_CYCLE[(index + 1) % VIEW_CYCLE.len()].0);
                    }
                }
            }
            Phase::Arrived => {
                if kiosk && self.phase_time >= ARRIVED_SECONDS {
                    self.leave();
                }
            }
        }
    }

    fn update_camera(&mut self, aspect: f32, height: f32, dt: f32) {
        if matches!(self.stage, Stage::Returning | Stage::ZoomingOut) {
            let goal = self.tile_pose(self.focused, aspect);
            self.camera.follow(goal, dt);
        } else if self.view == View::Free {
            if !self.dragging {
                self.camera.coast(height, dt);
            }
        } else {
            let goal = self.goal(self.view, aspect);
            self.camera.follow(goal, dt);
        }
    }

    fn flight_info(&self) -> FlightInfo {
        let flight = &self.flight;
        let destination = &self.airports[flight.destination];
        let heading = flight.heading_degrees();
        FlightInfo {
            phase: self.phase,
            origin: airport_item(&self.airports, flight.origin),
            destination: airport_item(&self.airports, flight.destination),
            progress: flight.progress(),
            remaining_time: format_hours(flight.remaining_hours()).into(),
            remaining_distance: format!("{} km", thousands(flight.remaining_km())).into(),
            flown_distance: format!("{} km", thousands(flight.flown_km())).into(),
            total_distance: format!("{} km", thousands(flight.distance_km)).into(),
            speed: format!("{} km/h", thousands(flight.ground_speed_kmh())).into(),
            altitude: format!("{} m", thousands(flight.altitude_m())).into(),
            // Adding 0 turns a rounded -0 into 0.
            temperature: format!("{} °C", flight.outside_celsius().round() + 0.0)
                .replace('-', "\u{2212}")
                .into(),
            heading: format!("{heading:.0}° {}", compass(heading)).into(),
            arrival_light: if flight.landed() {
                format!("Landed {}", daylight(destination.position, 0.0))
            } else {
                format!("Arriving {}", daylight(destination.position, flight.remaining_hours()))
            }
            .into(),
        }
    }

    /// The names of the largest cities on the screen that don't overlap each other or `taken`,
    /// and hidden ones up to `MAX_LABELS`.
    fn labels(&self, view_projection: Mat4, eye: Vec3, size: Vec2, taken: &[Vec2]) -> Vec<Label> {
        let height = (eye.length() - 1.0).max(0.01);
        let min_population = if height > 2.0 {
            6000.0
        } else if height > 0.8 {
            2500.0
        } else {
            500.0
        };
        let mut placed: Vec<(Vec2, Vec2)> = taken
            .iter()
            .map(|point| (*point - Vec2::new(24.0, 24.0), *point + Vec2::new(60.0, 24.0)))
            .collect();
        let mut labels = Vec::new();
        for city in self.cities.iter().take_while(|city| city.population >= min_population) {
            let facing = facing(eye, city.position);
            if facing < 0.15 {
                continue;
            }
            let Some(point) = project(view_projection, city.position, size) else { continue };
            let width = city.name.chars().count() as f32 * 6.5 + 14.0;
            let (low, high) = (point - Vec2::new(6.0, 10.0), point + Vec2::new(width, 10.0));
            if low.x < 0.0 || low.y < 0.0 || high.x > size.x || high.y > size.y {
                continue;
            }
            if placed
                .iter()
                .any(|(a, b)| low.x < b.x && a.x < high.x && low.y < b.y && a.y < high.y)
            {
                continue;
            }
            placed.push((low, high));
            labels.push(Label {
                x: point.x,
                y: point.y,
                name: city.name.as_str().into(),
                opacity: ((facing - 0.15) / 0.2).clamp(0.0, 1.0),
            });
            if labels.len() == MAX_LABELS {
                break;
            }
        }
        // Changing the number of rows while Slint renders would remove elements mid-frame.
        labels.resize(MAX_LABELS, Label::default());
        labels
    }
}

/// Tile `index` of the board, with the route drawn over its globe in the window's coordinates.
fn tile_item(
    demo: &Demo,
    index: usize,
    view_projection: Mat4,
    eye: Vec3,
    size: Vec2,
    texture: wgpu::Texture,
) -> TileItem {
    let flight = &demo.tiles[index].flight;
    let screen = |t: f32| screen_point(view_projection, eye, flight.point(t), size);
    let samples = (flight.distance_km / 60.0).clamp(24.0, 400.0) as usize;
    let route = path((0..=samples).map(|i| screen(i as f32 / samples as f32)), true);
    let end = |t: f32| screen(t).map_or(Coordinates::default(), |p| Coordinates { x: p.x, y: p.y });
    TileItem {
        image: texture.try_into().unwrap_or_default(),
        route: route.into(),
        from: end(0.0),
        to: end(1.0),
        origin: airport_item(&demo.airports, flight.origin),
        destination: airport_item(&demo.airports, flight.destination),
        distance: format!("{} km", thousands(flight.distance_km)).into(),
        duration: format_hours(flight.remaining_hours()).into(),
    }
}

fn marker(view_projection: Mat4, eye: Vec3, airport: &Airport, size: Vec2) -> Marker {
    let screen = screen_point(view_projection, eye, airport.position, size);
    Marker {
        shown: screen.is_some(),
        x: screen.map_or(0.0, |p| p.x),
        y: screen.map_or(0.0, |p| p.y),
        code: airport.code.as_str().into(),
    }
}

/// Renders into one of two textures, so Slint can show the previous one meanwhile.
struct Frames {
    textures: Vec<wgpu::Texture>,
    next: usize,
}

const FRAME_VIEW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

impl Frames {
    fn texture(&mut self, device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        let size = |texture: &wgpu::Texture| (texture.width(), texture.height());
        if self.textures.first().is_none_or(|texture| size(texture) != (width, height)) {
            self.textures = (0..2)
                .map(|_| {
                    device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("globe frame"),
                        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        // Written through an sRGB view, so Slint shows the bytes as they are.
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[FRAME_VIEW_FORMAT],
                    })
                })
                .collect();
        }
        self.next = (self.next + 1) % self.textures.len();
        self.textures[self.next].clone()
    }
}

fn update_rows<T: Clone + PartialEq + 'static>(model: &slint::VecModel<T>, rows: Vec<T>) {
    use slint::Model;
    if model.row_count() != rows.len() {
        model.set_vec(rows);
        return;
    }
    for (index, row) in rows.into_iter().enumerate() {
        if model.row_data(index).as_ref() != Some(&row) {
            model.set_row_data(index, row);
        }
    }
}

fn window_aspect(app: &AppWindow) -> f32 {
    let size = logical_size(app.window());
    size.x / size.y.max(1.0)
}

fn logical_size(window: &slint::Window) -> Vec2 {
    let physical = window.size();
    Vec2::new(physical.width as f32, physical.height as f32) / window.scale_factor()
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen(start))]
pub fn main() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();
    run().unwrap();
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    // The globe renders on Slint's device, so Slint can draw the rendered frames directly.
    slint::BackendSelector::new()
        .require_wgpu_30(WGPUConfiguration::Automatic(WGPUSettings::default()))
        .select()?;

    let app = AppWindow::new()?;
    let demo = Rc::new(RefCell::new(Demo::new()));
    let labels = Rc::new(slint::VecModel::<Label>::default());
    app.set_labels(labels.clone().into());
    let results = Rc::new(slint::VecModel::<AirportItem>::default());
    app.set_results(results.clone().into());
    let tiles = Rc::new(slint::VecModel::<TileItem>::default());
    app.set_tiles(tiles.clone().into());

    // Input restarts the timer that hands control back to the kiosk.
    let interact = {
        let demo = demo.clone();
        let app_weak = app.as_weak();
        move |change: &dyn Fn(&mut Demo, &AppWindow)| {
            let Some(app) = app_weak.upgrade() else { return };
            let mut demo_ref = demo.borrow_mut();
            change(&mut demo_ref, &app);
            let demo_weak = Rc::downgrade(&demo);
            let app_weak = app.as_weak();
            demo_ref.idle.start(slint::TimerMode::SingleShot, IDLE, move || {
                let (Some(app), Some(demo)) = (app_weak.upgrade(), demo_weak.upgrade()) else {
                    return;
                };
                app.set_kiosk(true);
                app.set_planner_open(false);
                let mut demo = demo.borrow_mut();
                demo.view_chosen = false;
                if demo.view == View::Free {
                    demo.set_view(View::Route);
                }
                app.window().request_redraw();
            });
            app.window().request_redraw();
        }
    };
    app.on_interacted({
        let interact = interact.clone();
        move || interact(&|_, _| {})
    });
    app.on_drag_started({
        let interact = interact.clone();
        move || {
            interact(&|demo, _| {
                demo.view = View::Free;
                demo.view_chosen = true;
                demo.dragging = true;
                demo.last_drag = web_time::Instant::now();
                demo.camera.hold();
            })
        }
    });
    app.on_dragged({
        let interact = interact.clone();
        move |dx, dy| {
            interact(&|demo, app| {
                let now = web_time::Instant::now();
                let dt = (now - demo.last_drag).as_secs_f32();
                demo.last_drag = now;
                let height = logical_size(app.window()).y;
                demo.camera.drag(Vec2::new(dx, dy), height, dt);
            })
        }
    });
    app.on_drag_ended({
        let interact = interact.clone();
        move || {
            interact(&|demo, _| {
                demo.dragging = false;
                // A pause before letting go means the ground shouldn't keep moving.
                if demo.last_drag.elapsed() > Duration::from_millis(80) {
                    demo.camera.hold();
                }
            })
        }
    });
    app.on_zoom({
        let interact = interact.clone();
        move |factor| {
            interact(&|demo, app| {
                demo.zoom_by(factor, window_aspect(app));
            })
        }
    });
    app.on_choose_view({
        let interact = interact.clone();
        move |view| {
            interact(&|demo, _| {
                demo.view_chosen = true;
                demo.set_view(view);
            })
        }
    });
    app.on_search({
        let interact = interact.clone();
        let results = results.clone();
        move |query| {
            interact(&|demo, _| {
                let rows = atlas::search(&demo.airports, &query)
                    .into_iter()
                    .map(|index| airport_item(&demo.airports, index))
                    .collect();
                update_rows(&results, rows);
            })
        }
    });
    app.on_open_planner({
        let interact = interact.clone();
        move || {
            interact(&|demo, app| {
                app.set_kiosk(false);
                // During a flight, a new flight starts where the current one lands.
                app.set_planned_origin(if demo.stage == Stage::Board {
                    AirportItem { index: -1, ..Default::default() }
                } else {
                    airport_item(&demo.airports, demo.flight.destination)
                });
                app.set_planned_destination(AirportItem { index: -1, ..Default::default() });
                app.set_planner_open(true);
            })
        }
    });
    app.on_take_off({
        let interact = interact.clone();
        move |origin, destination| {
            interact(&|demo, app| {
                app.set_kiosk(false);
                app.set_planner_open(false);
                demo.view_chosen = false;
                let (origin, destination) = (origin as usize, destination as usize);
                if demo.stage == Stage::Board {
                    // The flight takes the place of the one flown last, or the first one.
                    let index = demo.flown.unwrap_or(0);
                    demo.tiles[index] = demo.tile(origin, destination);
                    demo.enter(index, window_aspect(app));
                } else {
                    demo.start_flight(origin, destination);
                }
            })
        }
    });
    app.on_select_tile({
        let interact = interact.clone();
        move |index| {
            interact(&|demo, app| {
                if demo.stage == Stage::Board {
                    app.set_kiosk(false);
                    demo.enter(index as usize, window_aspect(app));
                }
            })
        }
    });
    app.on_show_departures({
        let interact = interact.clone();
        move || interact(&|demo, _| demo.leave())
    });
    let app_weak = app.as_weak();
    app.on_settings_changed({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                app.window().request_redraw();
            }
        }
    });

    // The renderer and Slint's device, once Slint set up rendering.
    let mut graphics: Option<(Renderer, wgpu::Device)> = None;
    let mut frames = Frames { textures: Vec::new(), next: 0 };
    let mut last_frame = web_time::Instant::now();
    let mut frame_count = 0u32;
    let mut fps_window_start = web_time::Instant::now();
    let mut tile_rows = vec![TileItem::default(); TILES];
    // Counts the frames for the frame rate, and asks for the next frame unless nothing moves.
    let mut request_redraw = move |app: &AppWindow, demo: &Demo| {
        frame_count += 1;
        let elapsed = fps_window_start.elapsed().as_secs_f32();
        if elapsed >= 0.5 {
            let fps = frame_count as f32 / elapsed;
            app.set_fps_text(format!("{fps:.0} fps").into());
            frame_count = 0;
            fps_window_start = web_time::Instant::now();
        }
        // Only a landed flight outside the kiosk stands still, once the UI shows the landing.
        let waiting = demo.stage == Stage::Flight
            && demo.phase == Phase::Arrived
            && demo.phase_time > 1.0
            && !app.get_kiosk();
        if !waiting || demo.camera.is_moving() {
            app.window().request_redraw();
        }
    };

    app.window().set_rendering_notifier(move |rendering_state, graphics_api| {
        let Some(app) = app_weak.upgrade() else { return };
        if let slint::RenderingState::RenderingSetup = rendering_state {
            let slint::GraphicsAPI::WGPU30 { device, queue, .. } = graphics_api else {
                return;
            };
            // The map and the imagery are only needed until they're on the GPU.
            let map = map::Map::new();
            // NASA's Blue Marble for July, see `data/convert.py`.
            let imagery = renderer::Imagery::decode(include_bytes!("data/earth.jpg"))
                .expect("the imagery is a valid JPEG")
                .compress(device.features());
            let info = device.adapter_info();
            app.set_gpu_text(
                format!("{} ({:?}, {:?})", info.name, info.backend, imagery.format).into(),
            );
            graphics = Some((
                Renderer::new(device, queue, &map, &imagery, FRAME_VIEW_FORMAT),
                device.clone(),
            ));
            return;
        }
        if !matches!(rendering_state, slint::RenderingState::BeforeRendering) {
            return;
        }
        let Some((renderer, device)) = &graphics else { return };
        let now = web_time::Instant::now();
        // After an idle period the first frame would otherwise jump.
        let dt = (now - last_frame).as_secs_f32().min(1.0 / 20.0);
        last_frame = now;

        let mut demo = demo.borrow_mut();
        let demo = &mut *demo;
        let window = app.window();
        let size = logical_size(window);
        let aspect = size.x / size.y.max(1.0);
        demo.step(app.get_kiosk(), aspect, dt);
        demo.update_camera(aspect, size.y, dt);

        let view_projection = demo.camera.pose.view_projection(aspect);
        let eye = demo.camera.pose.eye();
        let sun = if app.get_sunlight() {
            sun_direction(0.0)
        } else {
            // Lit from over the viewer's left shoulder.
            let view = Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y).inverse();
            (view.transform_vector3(Vec3::new(-0.5, 0.4, 1.0))).normalize()
        };

        let flight = &demo.flight;
        let origin = marker(view_projection, eye, &demo.airports[flight.origin], size);
        let destination = marker(view_projection, eye, &demo.airports[flight.destination], size);

        // The route on the ground, solid where the aircraft flew, and dashed ahead of it.
        let samples = (flight.distance_km / 60.0).clamp(24.0, 400.0) as usize;
        let progress = flight.progress();
        let screen = |t: f32| screen_point(view_projection, eye, flight.point(t), size);
        let steps = (0..=samples).map(|i| i as f32 / samples as f32);
        let flown = steps.clone().take_while(|t| *t < progress).chain([progress]);
        let ahead = [progress].into_iter().chain(steps.skip_while(|t| *t <= progress));
        app.set_flown_path(path(flown.map(screen), false).into());
        app.set_remaining_path(path(ahead.map(screen), true).into());

        let position = flight.position();
        let nose = (position + flight.track() * 0.005).normalize();
        let aircraft = screen(progress).zip(project(view_projection, nose * 1.0005, size));
        app.set_aircraft(match aircraft {
            Some((at, ahead)) => Aircraft {
                shown: true,
                x: at.x,
                y: at.y,
                angle: (ahead.x - at.x).atan2(at.y - ahead.y).to_degrees(),
            },
            None => Aircraft::default(),
        });

        let taken: Vec<Vec2> = [&origin, &destination]
            .iter()
            .filter(|marker| marker.shown)
            .map(|marker| Vec2::new(marker.x, marker.y))
            .chain(aircraft.map(|(at, _)| at))
            .collect();
        update_rows(&labels, demo.labels(view_projection, eye, size, &taken));
        app.set_origin_marker(origin);
        app.set_destination_marker(destination);
        app.set_flight(demo.flight_info());
        app.set_view(demo.view);
        app.set_screen(match demo.stage {
            Stage::Board => Screen::Board,
            Stage::ZoomingIn | Stage::Returning | Stage::ZoomingOut => Screen::Zoom,
            Stage::Flight => Screen::Flight,
        });
        app.set_zoom_progress(demo.zoom_progress());
        app.set_focused_tile(demo.focused as i32);
        app.set_highlighted_tile(demo.highlighted.map_or(-1, |(index, _)| index as i32));
        let sun_or_light = sun;
        let renderer_frame = |view_projection, eye| Frame {
            view_projection,
            eye,
            sun: sun_or_light,
            night: if app.get_sunlight() { 1.0 } else { 0.0 },
            borders: app.get_borders(),
            time: demo.time,
        };

        // The board renders one tile each frame, and the tile the globe shrinks into.
        if demo.stage != Stage::Flight {
            let tile_size = Vec2::new(app.get_tile_width(), app.get_tile_height());
            // Each tile shows the middle of a globe rendered for the whole window.
            let cover = (tile_size / size).max_element();
            let pixels = size * cover * window.scale_factor().min(MAX_GLOBE_SCALE);
            let mut render = vec![demo.next_tile];
            demo.next_tile = (demo.next_tile + 1) % TILES;
            if demo.stage == Stage::ZoomingOut {
                render.push(demo.focused);
            }
            render.extend((0..TILES).filter(|&index| demo.tiles[index].frames.textures.is_empty()));
            for index in render {
                let pose = demo.tile_pose(index, aspect);
                let tile_view_projection = pose.view_projection(aspect);
                let tile_eye = pose.eye();
                let texture = demo.tiles[index].frames.texture(
                    device,
                    (pixels.x as u32).max(1),
                    (pixels.y as u32).max(1),
                );
                let view = texture.create_view(&wgpu::TextureViewDescriptor {
                    format: Some(FRAME_VIEW_FORMAT),
                    ..Default::default()
                });
                renderer.render(&renderer_frame(tile_view_projection, tile_eye), &view);
                let item = tile_item(demo, index, tile_view_projection, tile_eye, size, texture);
                tile_rows[index] = item;
            }
            update_rows(&tiles, tile_rows.clone());
        }
        if demo.stage == Stage::Board {
            request_redraw(&app, demo);
            return;
        }

        // The `Image` under the UI stretches the frame over the window.
        let pixels = size * window.scale_factor().min(MAX_GLOBE_SCALE);
        let texture = frames.texture(device, (pixels.x as u32).max(1), (pixels.y as u32).max(1));
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(FRAME_VIEW_FORMAT),
            ..Default::default()
        });
        renderer.render(&renderer_frame(view_projection, eye), &view);
        if let Ok(image) = texture.try_into() {
            app.set_texture(image);
        }

        request_redraw(&app, demo);
    })?;

    app.run()?;
    Ok(())
}
