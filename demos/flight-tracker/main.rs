// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore xorshift

use std::cell::RefCell;
use std::f32::consts::PI;
use std::fmt::Write;
use std::rc::Rc;
use std::time::Duration;

use glam::{Mat4, Quat, Vec2, Vec3, Vec4Swizzles};
use slint::wgpu_30::{WGPUConfiguration, WGPUSettings, wgpu};
use unicode_segmentation::UnicodeSegmentation;

use atlas::{Airport, City};
use camera::{Camera, FOV_Y, MIN_RANGE, Pose, surface};
use flight::{EARTH_RADIUS_KM, Flight};
use renderer::{Frame, Renderer};

mod adaptive;
mod atlas;
mod background;
mod camera;
mod clouds;
mod compress;
mod flight;
#[allow(dead_code)]
mod gibs;
mod map;
mod renderer;
mod tiles;

slint::include_modules!();

/// The views the kiosk cycles through during a flight, and how many seconds each one lasts.
const VIEW_CYCLE: [(View, f32); 3] = [(View::Follow, 14.0), (View::Route, 8.0), (View::Globe, 8.0)];
const BOARDING_SECONDS: f32 = 4.0;
const ARRIVED_SECONDS: f32 = 7.0;
/// How long the kiosk waits on the entry screen before it proposes a flight, and then before
/// it takes off.
const PROPOSE_SECONDS: f32 = 2.5;
const TAKE_OFF_SECONDS: f32 = 4.0;
/// How long the globe takes to grow from its place on the entry screen to the window.
const ZOOM_IN_SECONDS: f32 = 0.9;
/// How long the globe takes to shrink back into its place, while the camera turns to the entry
/// screen's view.
const ZOOM_OUT_SECONDS: f32 = 1.3;
/// How long the intro takes, from low above the night side until the sun has come up over the
/// horizon.
const INTRO_SECONDS: f32 = 8.0;
/// How long the intro takes to fade in from black.
const INTRO_FADE_SECONDS: f32 = 1.2;
/// How long the planner's slide trails the globe's zoom, so that the globe leads.
const PLANNER_LAG: f32 = 0.2;
/// How fast the globe on the entry screen turns, in radians per second.
const ENTRY_SPIN: f32 = 0.08;
/// How long the globe on the entry screen waits after the user turned it, before it turns on
/// its own again.
const ENTRY_SPIN_PAUSE: f32 = 4.0;
/// The latitude that the globe on the entry screen shows at the center.
const ENTRY_LATITUDE: f32 = 20.0 * PI / 180.0;
/// How much of its place on the entry screen the globe fills.
const ENTRY_FILL: f32 = 0.84;
/// After how long without input the kiosk takes over again.
const IDLE: Duration = Duration::from_secs(60);
/// The kiosk's flights are at least this long, and at most the other.
const KIOSK_KM: (f32, f32) = (1500.0, 13000.0);
const VIEW_TRANSITION: f32 = 1.5;
const MAX_LABELS: usize = 24;
const LABEL_FADE_SECONDS: f32 = 0.2;
/// How many logical pixels a label on the screen can overlap others or leave it before it goes.
const LABEL_SLACK: f32 = 4.0;
/// The most physical pixels per logical pixel the globe renders at, so that phones with a
/// scale factor of 3 render fewer than half the pixels of their full resolution.
const MAX_GLOBE_SCALE: f32 = 2.0;

/// The most physical pixels per logical pixel the globe renders at in `quality`, before the
/// adaptive render scale.
fn globe_density(window: &slint::Window, quality: Quality) -> f32 {
    let limit = if quality == Quality::Low { 1.0 } else { MAX_GLOBE_SCALE };
    window.scale_factor().min(limit)
}
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

/// The SVG commands for a line through `points`, with gaps where a point is hidden. Each point comes with the index of its sample
/// along the route, and a dashed line draws the segments that end at an odd sample.
fn path(points: impl Iterator<Item = (usize, Option<Vec2>)>, dashed: bool) -> String {
    let mut commands = String::new();
    let mut previous: Option<Vec2> = None;
    for (sample, point) in points {
        if let (Some(from), Some(to)) = (previous, point)
            && (!dashed || sample % 2 == 1)
        {
            let _ = write!(commands, "M {:.1} {:.1} L {:.1} {:.1} ", from.x, from.y, to.x, to.y);
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

/// What the demo shows: the entry screen, the zoom between the globe's place there and the
/// window, or a flight.
#[derive(Clone, Copy, PartialEq)]
enum Stage {
    /// The camera rises from the dark above the night side until the sun comes up over the
    /// horizon, then the globe shrinks into its place on the entry screen.
    Intro,
    Entry,
    ZoomingIn,
    Flight,
    ZoomingOut,
}

/// A flight number with Slint's airline code, the same for every flight on a route.
fn flight_number(origin: usize, destination: usize) -> String {
    format!("SL {}", 100 + (origin * 7919 + destination * 104_729) % 900)
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

#[derive(Clone, Copy, Default)]
struct LabelState {
    shown: bool,
    fade: f32,
}

/// The kiosk's flights, the user's own, and the camera that watches them.
struct Demo {
    airports: Vec<Airport>,
    /// The airports the kiosk flies between.
    hubs: Vec<usize>,
    cities: Vec<City>,
    /// One for each of `cities`.
    label_states: Vec<LabelState>,
    flight: Flight,
    phase: Phase,
    phase_time: f32,
    stage: Stage,
    stage_time: f32,
    /// The origin and the destination picked on the entry screen.
    planned: (Option<usize>, Option<usize>),
    /// The flight proposed on the entry screen, which the UI has yet to show.
    proposal: Option<(usize, Option<usize>)>,
    /// Whether the kiosk proposed a flight since the entry screen showed.
    proposed: bool,
    /// Whether the zoom out to the entry screen comes from the intro, rather than a flight.
    from_intro: bool,
    /// How much of the window's height, and of its width, the globe's place on the entry
    /// screen shows of a frame rendered for the whole window.
    entry_view: Vec2,
    /// The point at the center of the globe on the entry screen.
    entry_target: Vec3,
    /// The compass direction that points up on the globe on the entry screen.
    entry_heading: f32,
    /// How much the user zoomed into the globe on the entry screen, as a part of the range at
    /// which it fills its place.
    entry_zoom: f32,
    /// Seconds since the user last moved the globe on the entry screen.
    entry_held: f32,
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
        Self {
            flight: Flight::new(0, position, 0, position),
            hubs,
            label_states: vec![LabelState::default(); cities.len()],
            cities,
            phase: Phase::Boarding,
            phase_time: 0.0,
            stage: Stage::Intro,
            stage_time: 0.0,
            planned: (None, None),
            proposal: None,
            proposed: false,
            from_intro: false,
            entry_view: Vec2::ONE,
            entry_target: surface(0.0, ENTRY_LATITUDE),
            entry_heading: 0.0,
            entry_zoom: 1.0,
            entry_held: ENTRY_SPIN_PAUSE,
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
        }
    }

    /// Shows `origin` and `destination` on the entry screen's globe, where they come around as it
    /// turns.
    fn plan(&mut self, origin: Option<usize>, destination: Option<usize>) {
        if self.planned == (origin, destination) {
            return;
        }
        self.planned = (origin, destination);
        if let Some(origin) = origin {
            let destination = destination.unwrap_or(origin);
            let (from, to) = (self.airports[origin].position, self.airports[destination].position);
            self.flight = Flight::new(origin, from, destination, to);
        }
    }

    /// The camera's range at which the globe fills its place on the entry screen.
    fn entry_fit_range(&self) -> f32 {
        // The globe's radius on the screen, as a part of half the frame's height.
        let fit = ENTRY_FILL * self.entry_view.y.min(self.entry_view.x);
        1.0 / (fit * (FOV_Y / 2.0).tan()).atan().sin() - 1.0
    }

    /// Where the camera looks at the turning globe on the entry screen.
    fn entry_pose(&self) -> Pose {
        let range = (self.entry_fit_range() * self.entry_zoom).max(MIN_RANGE);
        Pose { target: self.entry_target, heading: self.entry_heading, tilt: 0.0, range }
    }

    /// Turns the globe on the entry screen upright, zoomed out, with the longitude of `point`
    /// at the center.
    fn reset_entry(&mut self, point: Vec3) {
        self.entry_target = surface(camera::coordinates(point).0, ENTRY_LATITUDE);
        self.entry_heading = 0.0;
        self.entry_zoom = 1.0;
        self.entry_held = ENTRY_SPIN_PAUSE;
    }

    /// Stops the globe on the entry screen from turning on its own for a while, and stops the
    /// kiosk from taking off under the user's hands.
    fn hold_entry(&mut self, app: &AppWindow) {
        self.entry_held = 0.0;
        app.set_kiosk(false);
    }

    /// Hands the camera to the user, who starts to move the globe.
    fn grab(&mut self, app: &AppWindow) {
        if self.stage == Stage::Entry {
            self.hold_entry(app);
        } else {
            self.view = View::Free;
            self.view_chosen = true;
        }
    }

    /// Starts the planned flight, growing the globe to the window.
    fn take_off(&mut self) {
        let (Some(origin), Some(destination)) = self.planned else { return };
        if origin == destination || self.stage != Stage::Entry {
            return;
        }
        let (from, to) = (self.airports[origin].position, self.airports[destination].position);
        self.flight = Flight::new(origin, from, destination, to);
        self.phase = Phase::Boarding;
        self.phase_time = 0.0;
        self.view_chosen = false;
        self.set_view(View::Route);
        self.set_stage(Stage::ZoomingIn);
    }

    /// Turns back to the entry screen's view and shrinks the globe into its place there.
    fn leave(&mut self) {
        if self.stage == Stage::Flight {
            // The globe turns on from over the aircraft.
            self.reset_entry(self.flight.position());
            self.camera.transition(ZOOM_OUT_SECONDS);
            self.set_stage(Stage::ZoomingOut);
        }
    }

    /// Where the camera is during the intro: it rises from low above the night side, looking
    /// towards the sun, which comes up over the horizon on the way.
    fn intro_pose(&self) -> Pose {
        let sun = sun_direction(0.0);
        let side = sun.cross(Vec3::Y).try_normalize().unwrap_or(Vec3::X);
        let s = (self.stage_time / INTRO_SECONDS).clamp(0.0, 1.0);
        // The point looked at, on the great circle through the point under the sun, moves from
        // the night side towards the line between day and night, which brings the sun up over
        // the horizon after about three seconds. The camera keeps looking at the horizon for a
        // while, then pulls back and looks down at the globe.
        let angle = (107.0 - 11.0 * camera::ease(s / 0.45)).to_radians();
        let target = sun * angle.cos() + side * angle.sin();
        let towards_sun = sun * angle.sin() - side * angle.cos();
        let log_lerp = |from: f32, to: f32, t: f32| (from.ln() + (to.ln() - from.ln()) * t).exp();
        let range = if s < 0.62 {
            log_lerp(0.05, 0.18, s / 0.62)
        } else {
            log_lerp(0.18, 2.4, ((s - 0.62) / 0.38).powf(1.5))
        };
        let tilt = 80.0 - 60.0 * camera::ease((s - 0.74) / 0.26);
        Pose {
            target,
            heading: camera::bearing(target, towards_sun),
            tilt: tilt.to_radians(),
            range,
        }
    }

    /// Plays the intro again, from wherever the demo is.
    fn restart(&mut self) {
        self.from_intro = false;
        self.set_stage(Stage::Intro);
    }

    /// Ends the intro, zooming out to the entry screen from where the camera is.
    fn finish_intro(&mut self) {
        if self.stage == Stage::Intro {
            self.reset_entry(self.camera.pose.target);
            self.camera.transition(ZOOM_OUT_SECONDS);
            self.from_intro = true;
            self.set_stage(Stage::ZoomingOut);
        }
    }

    /// How dark the intro still is as it fades in, from 1 to 0.
    fn curtain(&self) -> f32 {
        match self.stage {
            Stage::Intro => 1.0 - camera::ease(self.stage_time / INTRO_FADE_SECONDS),
            _ => 0.0,
        }
    }

    fn set_stage(&mut self, stage: Stage) {
        self.stage = stage;
        self.stage_time = 0.0;
        self.proposed = false;
    }

    /// How far the globe has grown from its place on the entry screen to the window, from 0 to 1.
    fn zoom_progress(&self) -> f32 {
        match self.stage {
            Stage::Entry => 0.0,
            Stage::ZoomingIn => camera::ease(self.stage_time / ZOOM_IN_SECONDS),
            Stage::Intro | Stage::Flight => 1.0,
            Stage::ZoomingOut => 1.0 - camera::ease(self.stage_time / ZOOM_OUT_SECONDS),
        }
    }

    /// How far the planner has slid away, from 0 to 1. It leaves ahead of the globe's zoom, and
    /// comes back behind it, landing together with the globe.
    fn planner_away(&self) -> f32 {
        match self.stage {
            Stage::Entry => 0.0,
            Stage::ZoomingIn => camera::ease(self.stage_time / (ZOOM_IN_SECONDS - PLANNER_LAG)),
            Stage::Intro | Stage::Flight => 1.0,
            Stage::ZoomingOut => {
                let t = (self.stage_time - PLANNER_LAG) / (ZOOM_OUT_SECONDS - PLANNER_LAG);
                1.0 - camera::ease(t)
            }
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

    fn set_view(&mut self, view: View) {
        self.camera.transition(VIEW_TRANSITION);
        self.view = view;
        self.view_time = 0.0;
        self.zoom = 1.0;
    }

    /// How far from the ground the view puts the camera before the user zooms.
    fn base_range(&self, aspect: f32) -> f32 {
        match self.view {
            View::Follow => 0.6,
            View::Route => route_range(&self.flight, aspect),
            View::Globe => 2.6,
            View::Free => self.camera.pose.range,
        }
    }

    /// Where the view puts the camera.
    fn goal(&self, aspect: f32) -> Pose {
        let flight = &self.flight;
        let range = self.base_range(aspect) * self.zoom;
        match self.view {
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
        if self.stage == Stage::Entry {
            let closest = MIN_RANGE / self.entry_fit_range();
            self.entry_zoom = (self.entry_zoom / factor).clamp(closest.min(1.0), 1.0);
        } else if self.view == View::Free {
            self.camera.pose.range = Camera::zoomed(self.camera.pose.range, factor);
        } else {
            let base = self.base_range(aspect);
            self.zoom = (self.zoom / factor).clamp(MIN_RANGE / base, 3.0);
        }
    }

    fn step(&mut self, kiosk: bool, dt: f32) {
        self.time += dt;
        self.stage_time += dt;
        match self.stage {
            Stage::Intro => {
                if self.stage_time >= INTRO_SECONDS {
                    self.finish_intro();
                }
                return;
            }
            Stage::Entry => {
                self.entry_held += dt;
                let resume = camera::ease((self.entry_held - ENTRY_SPIN_PAUSE) / 2.0);
                let spin = ENTRY_SPIN * self.entry_zoom * resume;
                self.entry_target = Quat::from_rotation_y(-spin * dt) * self.entry_target;
                if kiosk && self.stage_time >= PROPOSE_SECONDS && !self.proposed {
                    self.proposed = true;
                    let origin = match self.planned {
                        (Some(origin), _) => origin,
                        _ => self.random_hub(),
                    };
                    let destination = self.random_destination(origin);
                    self.proposal = Some((origin, Some(destination)));
                    self.plan(Some(origin), Some(destination));
                }
                if kiosk && self.stage_time >= PROPOSE_SECONDS + TAKE_OFF_SECONDS {
                    self.take_off();
                }
                return;
            }
            Stage::ZoomingIn if self.stage_time >= ZOOM_IN_SECONDS => self.set_stage(Stage::Flight),
            Stage::ZoomingOut if self.stage_time >= ZOOM_OUT_SECONDS => {
                self.set_stage(Stage::Entry);
                if !std::mem::take(&mut self.from_intro) {
                    // The next flight starts where this one landed.
                    let origin = self.flight.destination;
                    self.proposal = Some((origin, None));
                    self.plan(Some(origin), None);
                }
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
        if self.stage == Stage::Intro {
            self.camera.pose = self.intro_pose();
        } else if self.stage == Stage::Entry && (self.dragging || self.camera.is_coasting()) {
            if !self.dragging {
                self.camera.coast(height, dt);
            }
            self.camera.pose.range = self.entry_pose().range;
            self.entry_target = self.camera.pose.target;
            self.entry_heading = self.camera.pose.heading;
            self.entry_held = 0.0;
        } else if matches!(self.stage, Stage::Entry | Stage::ZoomingOut) {
            let goal = self.entry_pose();
            self.camera.follow(goal, dt);
        } else if self.view == View::Free {
            if !self.dragging {
                self.camera.coast(height, dt);
            }
        } else {
            let goal = self.goal(aspect);
            self.camera.follow(goal, dt);
        }
    }

    fn flight_info(&self) -> FlightInfo {
        let flight = &self.flight;
        let destination = &self.airports[flight.destination];
        let heading = flight.heading_degrees();
        FlightInfo {
            phase: self.phase,
            number: flight_number(flight.origin, flight.destination).into(),
            origin: airport_item(&self.airports, flight.origin),
            destination: airport_item(&self.airports, flight.destination),
            progress: flight.progress(),
            remaining_time: format_hours(flight.remaining_hours()).into(),
            remaining_clock: {
                let minutes = (flight.remaining_hours() * 60.0).round() as u32;
                format!("{}:{:02}", minutes / 60, minutes % 60).into()
            },
            remaining_distance: thousands(flight.remaining_km()).into(),
            flown_distance: thousands(flight.flown_km()).into(),
            total_distance: thousands(flight.distance_km).into(),
            speed: thousands(flight.ground_speed_kmh()).into(),
            altitude: thousands(flight.altitude_m()).into(),
            // Adding 0 turns a rounded -0 into 0.
            temperature: format!("{}", flight.outside_celsius().round() + 0.0)
                .replace('-', "\u{2212}")
                .into(),
            heading: format!("{heading:.0}°").into(),
            heading_compass: compass(heading).into(),
            arrival_light: if flight.landed() {
                format!("Landed {}", daylight(destination.position, 0.0))
            } else {
                format!("Arriving {}", daylight(destination.position, flight.remaining_hours()))
            }
            .into(),
        }
    }

    /// The names of the largest cities on the screen, at most `MAX_LABELS`, that don't overlap
    /// each other or `taken`. They fade in and out, and those on the screen keep their place
    /// over new ones, so that a moving camera doesn't make them flicker.
    fn labels(
        &mut self,
        view_projection: Mat4,
        eye: Vec3,
        size: Vec2,
        taken: &[Vec2],
        dt: f32,
    ) -> Vec<Label> {
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
        let mut count = 0;
        let was_shown: Vec<bool> =
            self.label_states.iter_mut().map(|state| std::mem::take(&mut state.shown)).collect();
        for (slack, keep_shown) in [(LABEL_SLACK, true), (0.0, false)] {
            for (index, city) in self.cities.iter().enumerate() {
                if was_shown[index] != keep_shown
                    || city.population < min_population
                    || count == MAX_LABELS
                    || facing(eye, city.position) < 0.15
                {
                    continue;
                }
                let Some(point) = project(view_projection, city.position, size) else { continue };
                let width = city.name.chars().count() as f32 * 6.5 + 14.0;
                let (low, high) =
                    (point - Vec2::new(6.0, 10.0) + slack, point + Vec2::new(width, 10.0) - slack);
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
                self.label_states[index].shown = true;
                count += 1;
            }
        }
        let mut labels = Vec::new();
        for (city, state) in self.cities.iter().zip(&mut self.label_states) {
            let step = dt / LABEL_FADE_SECONDS;
            state.fade = (state.fade + if state.shown { step } else { -step }).clamp(0.0, 1.0);
            let facing = facing(eye, city.position);
            if state.fade == 0.0 || facing < 0.15 {
                continue;
            }
            let Some(point) = project(view_projection, city.position, size) else { continue };
            labels.push(Label {
                x: point.x,
                y: point.y,
                name: city.name.as_str().into(),
                opacity: state.fade * ((facing - 0.15) / 0.2).clamp(0.0, 1.0),
            });
        }
        labels
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

/// How strongly the borders show on a globe seen from `eye` in a frame `height` logical pixels
/// high: not at all where the globe is small, where the lines would crowd together.
fn border_strength(eye: Vec3, height: f32) -> f32 {
    let distance = eye.length().max(1.0 + 1e-4);
    let radius = height / 2.0 / ((distance * distance - 1.0).sqrt() * (FOV_Y / 2.0).tan());
    camera::smoothstep(250.0, 450.0, radius)
}

/// How strongly the clouds show on a globe seen from `eye`: they fade out as the camera comes
/// close, where they would hide the route and the cities.
fn cloud_strength(eye: Vec3) -> f32 {
    camera::smoothstep(0.9, 2.0, eye.length() - 1.0)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    /// The page's `flightTrackerReady()`, see `index.html`. It's an error where the page has none.
    #[wasm_bindgen(catch, js_name = flightTrackerReady)]
    fn flight_tracker_ready() -> Result<(), wasm_bindgen::JsValue>;
}

/// Renders into one of two textures, so Slint can show the previous one meanwhile.
/// They're as large as the largest frame, so that a change in the render scale or in the globe's
/// place doesn't create new ones.
struct Frames {
    textures: Vec<wgpu::Texture>,
    next: usize,
    /// The format the globe renders through: an sRGB view of the textures where the device
    /// supports one, so Slint shows the bytes as they are.
    view_format: wgpu::TextureFormat,
}

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
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: if self.view_format.is_srgb() {
                            &[wgpu::TextureFormat::Rgba8UnormSrgb]
                        } else {
                            &[]
                        },
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
    // Without a reset, the rows that stay keep their elements.
    for index in (rows.len()..model.row_count()).rev() {
        model.remove(index);
    }
    for (index, row) in rows.into_iter().enumerate() {
        if index >= model.row_count() {
            model.push(row);
        } else if model.row_data(index).as_ref() != Some(&row) {
            model.set_row_data(index, row);
        }
    }
}

fn aspect(size: Vec2) -> f32 {
    size.x / size.y.max(1.0)
}

fn logical_size(window: &slint::Window) -> Vec2 {
    let size = window.size().to_logical(window.scale_factor());
    Vec2::new(size.width, size.height)
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
    app.set_online_imagery_available(tiles::Source::detect().is_some());
    app.set_entry_fill(ENTRY_FILL);
    app.global::<Characters>().on_at(|text, index| {
        let character = usize::try_from(index).ok().and_then(|i| text.graphemes(true).nth(i));
        character.unwrap_or_default().into()
    });
    let demo = Rc::new(RefCell::new(Demo::new()));
    let labels = Rc::new(slint::VecModel::<Label>::default());
    app.set_labels(labels.clone().into());
    let results = Rc::new(slint::VecModel::<AirportItem>::default());
    app.set_results(results.clone().into());
    results.set_vec(
        atlas::search(&demo.borrow().airports, "")
            .into_iter()
            .map(|index| airport_item(&demo.borrow().airports, index))
            .collect::<Vec<_>>(),
    );

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
                let mut demo = demo.borrow_mut();
                demo.view_chosen = false;
                if demo.stage == Stage::Entry {
                    // Starts the kiosk's wait for proposing a flight over.
                    demo.set_stage(Stage::Entry);
                    let target = demo.entry_target;
                    demo.reset_entry(target);
                    demo.camera.transition(VIEW_TRANSITION);
                }
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
            interact(&|demo, app| {
                demo.grab(app);
                demo.dragging = true;
                demo.last_drag = web_time::Instant::now();
                demo.camera.hold();
            })
        }
    });
    app.on_twisted({
        let interact = interact.clone();
        move |degrees| {
            interact(&|demo, app| {
                demo.grab(app);
                if demo.stage == Stage::Entry {
                    demo.entry_heading -= degrees.to_radians();
                }
                demo.camera.twist(degrees.to_radians());
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
                if demo.stage == Stage::Entry {
                    demo.hold_entry(app);
                }
                demo.zoom_by(factor, aspect(logical_size(app.window())));
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
    app.on_show_planner({
        let interact = interact.clone();
        move || {
            interact(&|demo, app| {
                app.set_kiosk(false);
                demo.leave();
            })
        }
    });
    app.on_planned_changed({
        let interact = interact.clone();
        move || {
            interact(&|demo, app| {
                app.set_kiosk(false);
                let index = |item: AirportItem| usize::try_from(item.index).ok();
                demo.plan(index(app.get_planned_origin()), index(app.get_planned_destination()));
            })
        }
    });
    app.on_restart({
        let interact = interact.clone();
        move || {
            interact(&|demo, app| {
                app.set_kiosk(true);
                demo.restart();
            })
        }
    });
    app.on_skip_intro({
        let demo = demo.clone();
        move || demo.borrow_mut().finish_intro()
    });
    app.on_take_off({
        let interact = interact.clone();
        move || {
            interact(&|demo, app| {
                app.set_kiosk(false);
                demo.take_off();
            })
        }
    });
    let app_weak = app.as_weak();

    let mut graphics: Option<(Renderer, wgpu::Device)> = None;
    let mut frames =
        Frames { textures: Vec::new(), next: 0, view_format: wgpu::TextureFormat::Rgba8Unorm };
    let mut last_frame = web_time::Instant::now();
    let mut night_loading: Option<background::Background<renderer::Imagery>> = None;
    let mut day_loading: Option<background::Background<renderer::Imagery>> = None;
    let mut cloud_generation: Option<background::Background<renderer::Imagery>> = None;
    let mut detail: Option<tiles::Detail> = None;
    #[cfg(target_arch = "wasm32")]
    let mut page_told = false;
    let mut clouds_since: Option<web_time::Instant> = None;
    let mut frame_count = 0u32;
    let mut fps_window_start = web_time::Instant::now();
    let mut adaptive = adaptive::Adaptive::new(app.get_quality(), true);
    // Only a landed flight outside the kiosk stands still, once the UI shows the landing.
    let animating = |app: &AppWindow, demo: &Demo| {
        let waiting = demo.stage == Stage::Flight
            && demo.phase == Phase::Arrived
            && demo.phase_time > 1.0
            && !app.get_kiosk();
        !waiting || demo.camera.is_moving()
    };
    let mut request_redraw = move |app: &AppWindow, demo: &Demo, scale: f32| {
        frame_count += 1;
        let elapsed = fps_window_start.elapsed().as_secs_f32();
        if elapsed >= 0.5 {
            let fps = frame_count as f32 / elapsed;
            let quality = format!("{:?}", app.get_quality()).to_lowercase();
            let scale = (scale * 100.0).round();
            app.set_fps_text(format!("{quality} · {scale}% · {fps:.0} fps").into());
            frame_count = 0;
            fps_window_start = web_time::Instant::now();
        }
        if animating(app, demo) {
            app.window().request_redraw();
        }
    };

    app.window().set_rendering_notifier(move |rendering_state, graphics_api| {
        let Some(app) = app_weak.upgrade() else { return };
        if let slint::RenderingState::RenderingSetup = rendering_state {
            let slint::GraphicsAPI::WGPU30 { device, queue, .. } = graphics_api else {
                return;
            };
            // The map is only needed until it's on the GPU.
            let map = map::coasts();
            // The imagery and the clouds follow while the intro plays. The intro starts on the
            // night side, so its lights come first.
            let features = device.features();
            night_loading = Some(background::Background::start(1, move || {
                // NASA's Black Marble, see `data/convert.py`.
                renderer::Imagery::decode(include_bytes!("data/night.jpg"))
                    .expect("the night imagery is a valid JPEG")
                    .compress(features)
            }));
            day_loading = Some(background::Background::start(2, move || {
                // NASA's Blue Marble for July.
                renderer::Imagery::decode(include_bytes!("data/earth.jpg"))
                    .expect("the imagery is a valid JPEG")
                    .compress(features)
            }));
            cloud_generation = Some(background::Background::start(3, clouds::generate));
            let tiles = tiles::Source::detect();
            let with_tiles = tiles.is_some();
            detail = Some(tiles::Detail::new(tiles, features));
            let info = device.adapter_info();
            // OpenGL, and WebGL in a browser without WebGPU, can't view a texture as sRGB.
            if info.backend != wgpu::Backend::Gl {
                frames.view_format = wgpu::TextureFormat::Rgba8UnormSrgb;
            }
            app.set_gpu_text(
                format!(
                    "{} ({:?}, {:?})",
                    info.name,
                    info.backend,
                    renderer::Imagery::compressed_format(features)
                )
                .into(),
            );
            graphics = Some((
                Renderer::new(device, queue, &map, frames.view_format, with_tiles),
                device.clone(),
            ));
            return;
        }
        if !matches!(rendering_state, slint::RenderingState::BeforeRendering) {
            return;
        }
        let Some((renderer, device)) = &mut graphics else { return };
        if let Some(night) = night_loading.as_mut().and_then(background::Background::poll) {
            renderer.set_night(&night);
        }
        if let Some(day) = day_loading.as_mut().and_then(background::Background::poll) {
            renderer.set_day(&day);
        }
        if let Some(clouds) = cloud_generation.as_mut().and_then(background::Background::poll) {
            renderer.set_clouds(&clouds);
            clouds_since = Some(web_time::Instant::now());
        }
        let now = web_time::Instant::now();
        // After an idle period the first frame would otherwise jump.
        let dt = (now - last_frame).as_secs_f32().min(1.0 / 20.0);
        last_frame = now;
        // On the web, the work above holds up the first frames. The intro waits for it behind
        // the page's loading indicator, which the page hides once it's told.
        #[cfg(target_arch = "wasm32")]
        let dt = {
            let loaded = night_loading.as_ref().is_some_and(|work| work.is_done())
                && day_loading.as_ref().is_some_and(|work| work.is_done())
                && cloud_generation.as_ref().is_some_and(|work| work.is_done());
            if loaded && !page_told {
                page_told = true;
                let _ = flight_tracker_ready();
            }
            if loaded { dt } else { 0.0 }
        };

        let mut demo = demo.borrow_mut();
        let demo = &mut *demo;
        let window = app.window();
        let size = logical_size(window);
        let aspect = aspect(size);
        let entry = Vec2::new(app.get_entry_globe_width(), app.get_entry_globe_height());
        demo.entry_view = entry / (size * (entry / size).max_element());
        demo.step(app.get_kiosk(), dt);
        demo.update_camera(aspect, size.y, dt);

        let quality = app.get_quality();
        if let Some(lower) =
            adaptive.frame(quality, app.get_auto_quality(), true, animating(&app, demo))
        {
            app.set_quality(lower);
        }
        let scale = adaptive.scale();
        let full_density = globe_density(window, app.get_quality());
        let density = full_density * scale;
        let zoom_progress = demo.zoom_progress();

        // The globe's place, which grows from the entry screen's to the window, centers the globe.
        let entry_center =
            Vec2::new(app.get_entry_globe_x(), app.get_entry_globe_y()) + entry / 2.0;
        let center = entry_center.lerp(size / 2.0, zoom_progress);
        let shift = (center - size / 2.0) / size * Vec2::new(2.0, -2.0);
        let view_projection =
            Mat4::from_translation(shift.extend(0.0)) * demo.camera.pose.view_projection(aspect);
        let eye = demo.camera.pose.eye();
        let sun = if app.get_sunlight() {
            sun_direction(0.0)
        } else {
            // Lit from over the viewer's left shoulder.
            let view = Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y).inverse();
            (view.transform_vector3(Vec3::new(-0.5, 0.4, 1.0))).normalize()
        };

        let flight = &demo.flight;
        let mut markers = [Marker::default(), Marker::default()];
        let mut paths = [String::new(), String::new()];
        let mut aircraft = None;
        // The intro shows the globe alone, without the placeholder flight it starts with.
        if demo.stage != Stage::Intro && !demo.from_intro {
            markers = [flight.origin, flight.destination]
                .map(|airport| marker(view_projection, eye, &demo.airports[airport], size));

            // The route on the ground, solid where the aircraft flew, and dashed ahead of it.
            let samples = (flight.distance_km / 60.0).clamp(24.0, 400.0) as usize;
            let progress = flight.progress();
            let screen = |t: f32| screen_point(view_projection, eye, flight.point(t), size);
            let steps = (0..=samples).map(|i| (i, i as f32 / samples as f32));
            let flown = steps.clone().take_while(|(_, t)| *t < progress).chain([(0, progress)]);
            let ahead =
                [(0, progress)].into_iter().chain(steps.skip_while(|(_, t)| *t <= progress));
            let on_screen = |(sample, t): (usize, f32)| (sample, screen(t));
            paths = [path(flown.map(on_screen), false), path(ahead.map(on_screen), true)];

            let nose = (flight.position() + flight.track() * 0.005).normalize();
            aircraft = screen(progress).zip(project(view_projection, nose * 1.0005, size));
        }
        let taken: Vec<Vec2> = markers
            .iter()
            .filter(|marker| marker.shown)
            .map(|marker| Vec2::new(marker.x, marker.y))
            .chain(aircraft.map(|(at, _)| at))
            .collect();
        update_rows(&labels, demo.labels(view_projection, eye, size, &taken, dt));
        let [origin, destination] = markers;
        app.set_origin_marker(origin);
        app.set_destination_marker(destination);
        let [flown, remaining] = paths;
        app.set_flown_path(flown.into());
        app.set_remaining_path(remaining.into());
        app.set_aircraft(match aircraft {
            Some((at, ahead)) => Aircraft {
                shown: true,
                x: at.x,
                y: at.y,
                angle: (ahead.x - at.x).atan2(at.y - ahead.y).to_degrees(),
            },
            None => Aircraft::default(),
        });
        app.set_flight(demo.flight_info());
        app.set_view(demo.view);
        app.set_screen(match demo.stage {
            Stage::Entry => Screen::Entry,
            Stage::Intro | Stage::ZoomingIn | Stage::ZoomingOut => Screen::Zoom,
            Stage::Flight => Screen::Flight,
        });
        app.set_zoom_progress(zoom_progress);
        app.set_intro(demo.stage == Stage::Intro);
        app.set_curtain(demo.curtain());
        app.set_planner_away(demo.planner_away());
        if let Some((origin, destination)) = demo.proposal.take() {
            let item = |index: Option<usize>| {
                index.map_or(AirportItem::default(), |index| airport_item(&demo.airports, index))
            };
            app.invoke_propose(item(Some(origin)), item(destination));
        }
        // The frame covers the whole window, at the resolution the globe's place needs.
        let shown = entry.lerp(size, zoom_progress);
        let largest = (size * full_density).ceil().max(Vec2::ONE);
        let texture = frames.texture(device, largest.x as u32, largest.y as u32);
        let pixels =
            (size * (shown / size).max_element() * density).floor().clamp(Vec2::ONE, largest);
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(frames.view_format),
            ..Default::default()
        });
        let footprint =
            (demo.camera.pose.range * 2.0 * (FOV_Y / 2.0).tan() / pixels.y).to_degrees();
        let windows = match &mut detail {
            Some(detail) => {
                let target = demo.camera.pose.target;
                // The intro shows next to nothing of the imagery, and the day side no lights.
                let fetching = demo.stage != Stage::Intro;
                let dark = app.get_sunlight() && target.dot(sun) < 0.15;
                let kinds = (fetching, fetching && dark);
                detail.update(renderer, target, footprint, app.get_online_imagery(), kinds, dt)
            }
            None => Default::default(),
        };
        renderer.render(
            &Frame {
                view_projection,
                eye,
                sun,
                night: if app.get_sunlight() { 1.0 } else { 0.0 },
                borders: if app.get_borders() {
                    border_strength(eye, pixels.y / density)
                } else {
                    0.0
                },
                border_width: density,
                viewport: pixels,
                time: demo.time,
                space: zoom_progress,
                windows,
                clouds: match clouds_since {
                    Some(since) if app.get_clouds() => {
                        cloud_strength(eye) * since.elapsed().as_secs_f32().min(1.0)
                    }
                    _ => 0.0,
                },
            },
            &view,
        );
        if let Ok(image) = texture.try_into() {
            app.set_texture(image);
            app.set_texture_used_width(pixels.x as i32);
            app.set_texture_used_height(pixels.y as i32);
        }

        request_redraw(&app, demo, scale);
        // Tiles that arrive or fade in show in the next frame, also while nothing else moves.
        if detail.as_ref().is_some_and(tiles::Detail::is_busy) {
            app.window().request_redraw();
        }
    })?;

    app.run()?;
    Ok(())
}
