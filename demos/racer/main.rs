// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::cell::RefCell;
use std::f32::consts::{PI, TAU};
use std::rc::Rc;

use glam::{Quat, Vec2, Vec3, Vec3Swizzles};
use slint::wgpu_30::{WGPUConfiguration, WGPUSettings, wgpu};

use cars::{Car, Controls, Style};
use renderer::Renderer;
use track::{SAMPLES, Track};
use world::{CameraMode, FrameInput, StartLights, World, hex};

mod camera;
mod car_model;
mod cars;
mod geometry;
mod renderer;
mod textures;
mod track;
mod trees;
mod world;

slint::include_modules!();

const SLINT_BLUE: u32 = 0x2379f4;
const LAPS: i32 = 3;
const COUNTDOWN: f32 = 3.0;
/// How long "GO" stays on screen after the countdown.
const GO_DISPLAY: f32 = 0.8;
/// How long a finished lap's time stays on screen.
const LAP_FLASH: f32 = 3.5;
const BOOST_FLASH: f32 = 0.9;
/// How long the autopilot's result stays before the next race.
const RESULT_DISPLAY: f32 = 7.0;
/// The player's place on the grid.
const PLAYER_SLOT: usize = 2;
/// How far a car must get ahead of the one before it in the standings to pass it, so cars
/// side by side don't swap places back and forth.
const OVERTAKE_MARGIN: f32 = 0.5;

pub fn yaw_towards(direction: Vec3) -> f32 {
    (-direction.x).atan2(-direction.z)
}

pub fn forward(yaw: f32) -> Vec3 {
    Quat::from_rotation_y(yaw) * Vec3::NEG_Z
}

/// How much of the way to its target a value easing at `rate` per second moves in `dt`
/// seconds, whatever the frame rate.
pub fn damp(rate: f32, dt: f32) -> f32 {
    1.0 - (-rate * dt).exp()
}

/// `angle` wrapped to -π..π.
pub fn wrap_angle(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

/// The rotation that points -Z from `eye` at `target`, like a camera.
pub fn looking_at(eye: Vec3, target: Vec3) -> Quat {
    Quat::look_at_rh(eye, target, Vec3::Y).inverse()
}

fn random_seed() -> u32 {
    let now = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map_or(0, |time| time.as_nanos() as u64);
    // Mixes all bits, since some platforms only report microseconds.
    let mixed = (now ^ (now >> 29)).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    1000 + ((mixed >> 32) % 9000) as u32
}

struct Pilot {
    name: &'static str,
    color: u32,
    style: Style,
    /// Scales the top speed.
    power: f32,
    /// Shifts the pilot's wandering line, so the field spreads out.
    phase: f32,
}

const PILOTS: [Pilot; 4] = [
    Pilot {
        name: "Volta",
        color: 0xff8a3d,
        style: Style { line: 1.0, grip: 30.0, pads: true },
        power: 0.97,
        phase: 0.0,
    },
    Pilot {
        name: "Kestrel",
        color: 0xf2c94c,
        style: Style { line: -1.2, grip: 32.0, pads: false },
        power: 0.96,
        phase: 2.1,
    },
    Pilot {
        name: "Nix",
        color: 0x3fd0a8,
        style: Style { line: 0.4, grip: 28.0, pads: true },
        power: 0.95,
        phase: 4.0,
    },
    Pilot {
        name: "Wren",
        color: 0xd060e0,
        style: Style { line: -0.4, grip: 30.0, pads: true },
        power: 0.965,
        phase: 5.3,
    },
];

/// How the autopilot drives the player's car.
const AUTOPILOT: Style = Style { line: 0.0, grip: 31.0, pads: true };

/// The race: the cars, with the player's first, and the player's laps.
struct Race {
    phase: Phase,
    phase_time: f32,
    cars: Vec<Car>,
    /// The race time at which each car finished.
    finish: Vec<Option<f32>>,
    /// Seconds since the start.
    race_time: f32,
    /// The player's laps done, and when the current one began.
    laps_done: i32,
    lap_start: f32,
    best_lap: Option<f32>,
    /// The player's last finished lap: its number, time, and the best before it, with its age.
    lap_flash: Option<(i32, f32, Option<f32>, f32)>,
    /// Seconds since the player's last boost pad.
    boost_flash: f32,
    /// The cars by place.
    order: Vec<usize>,
}

impl Race {
    fn new(track: &Track) -> Self {
        let mut slots = (0..=PILOTS.len()).filter(|&slot| slot != PLAYER_SLOT);
        let mut cars = vec![grid_car(track, PLAYER_SLOT, 1.0)];
        for pilot in &PILOTS {
            cars.push(grid_car(track, slots.next().unwrap_or(0), pilot.power));
        }
        let mut order: Vec<usize> = (0..cars.len()).collect();
        order.sort_by(|&a, &b| cars[b].progress().total_cmp(&cars[a].progress()));
        Self {
            phase: Phase::Ready,
            phase_time: 0.0,
            finish: vec![None; cars.len()],
            cars,
            race_time: 0.0,
            laps_done: 0,
            lap_start: 0.0,
            best_lap: None,
            lap_flash: None,
            boost_flash: BOOST_FLASH,
            order,
        }
    }

    /// Moves cars up the standings: finished cars by their time, the others once they're
    /// `OVERTAKE_MARGIN` ahead of the car before them.
    fn update_order(&mut self) {
        let score = |i: usize| match self.finish[i] {
            Some(time) => 1e9 - time,
            None => self.cars[i].progress(),
        };
        let mut swapped = true;
        while swapped {
            swapped = false;
            for k in 1..self.order.len() {
                let (ahead, behind) = (self.order[k - 1], self.order[k]);
                if score(behind) > score(ahead) + OVERTAKE_MARGIN {
                    self.order.swap(k - 1, k);
                    swapped = true;
                }
            }
        }
    }

    fn place(&self) -> usize {
        self.order.iter().position(|&i| i == 0).unwrap_or(0) + 1
    }
}

fn grid_car(track: &Track, slot: usize, power: f32) -> Car {
    let place = cars::grid_slot(slot);
    Car::new(track, place.x, place.y, power)
}

/// The input and settings from the UI, and the race they drive.
struct State {
    stick: Vec2,
    autopilot: bool,
    assist: bool,
    camera: CameraMode,
    quality: Quality,
    /// While set, the race stands still.
    paused: bool,
    start_requested: bool,
    reset_requested: bool,
    new_track_requested: bool,
    track: Track,
    race: Race,
}

impl State {
    fn new() -> Self {
        // `RACER_SEED` picks a fixed first track, to compare runs.
        let seed = std::env::var("RACER_SEED").ok().and_then(|seed| seed.parse().ok());
        let track = track::generate(seed.unwrap_or_else(random_seed));
        let race = Race::new(&track);
        Self {
            stick: Vec2::ZERO,
            autopilot: true,
            assist: true,
            camera: CameraMode::Chase,
            quality: Quality::High,
            paused: false,
            start_requested: false,
            reset_requested: false,
            new_track_requested: false,
            track,
            race,
        }
    }

    fn animating(&self) -> bool {
        !self.paused && (self.race.phase != Phase::Ready || self.start_requested)
    }

    /// The player's controls: the autopilot's, the player's own, or with assist, the
    /// player's steering pulled gently along the track and the speed kept for the turns.
    fn player_controls(&self, guide: Controls) -> Controls {
        let race = &self.race;
        if self.autopilot || race.phase == Phase::Finished {
            return guide;
        }
        let stick = self.stick;
        if self.assist {
            let pull = 0.35 * (1.0 - stick.x.abs());
            Controls {
                throttle: if stick.y < -0.2 { stick.y } else { guide.throttle },
                steer: (stick.x + guide.steer * pull).clamp(-1.0, 1.0),
            }
        } else {
            Controls { throttle: stick.y, steer: stick.x }
        }
    }

    /// Advances the race by `dt` seconds.
    fn step(&mut self, dt: f32) {
        let track = &self.track;
        if std::mem::take(&mut self.reset_requested) {
            self.race = Race::new(track);
        }
        if std::mem::take(&mut self.start_requested)
            && matches!(self.race.phase, Phase::Ready | Phase::Finished)
        {
            let best_lap = self.race.best_lap;
            self.race = Race { phase: Phase::Countdown, best_lap, ..Race::new(track) };
        }

        let race = &mut self.race;
        race.phase_time += dt;
        race.boost_flash += dt;
        if let Some(flash) = &mut race.lap_flash {
            flash.3 += dt;
        }
        race.lap_flash = race.lap_flash.filter(|flash| flash.3 < LAP_FLASH);
        match race.phase {
            Phase::Ready => {}
            Phase::Countdown => {
                if race.phase_time >= COUNTDOWN {
                    race.phase = Phase::Racing;
                    race.phase_time = 0.0;
                }
            }
            Phase::Racing | Phase::Finished => self.drive(dt),
        }

        // The autopilot races on its own, for an unattended demo, on a new track each time.
        let race = &self.race;
        let over = race.phase == Phase::Finished && race.phase_time > RESULT_DISPLAY;
        if self.autopilot && (race.phase == Phase::Ready || over) {
            self.new_track_requested |= over;
            self.start_requested = true;
        }
    }

    fn drive(&mut self, dt: f32) {
        let track = &self.track;
        let race = &mut self.race;
        race.race_time += dt;
        let player_progress = race.cars[0].progress();
        let controls: Vec<Controls> = (0..race.cars.len())
            .map(|i| {
                let car = &race.cars[i];
                if i == 0 {
                    return cars::drive(car, track, AUTOPILOT, &race.cars);
                }
                let pilot = &PILOTS[i - 1];
                let style = cars::wander(pilot.style, race.race_time, pilot.phase);
                cars::drive(car, track, style, &race.cars)
            })
            .collect();
        let player = self.player_controls(controls[0]);
        let race = &mut self.race;
        for (i, car) in race.cars.iter_mut().enumerate() {
            if i > 0 {
                // A rival behind the player pushes a little harder.
                let behind = player_progress - car.progress();
                car.power = PILOTS[i - 1].power * (1.0 + (behind / 80.0).clamp(0.0, 0.05));
            }
            let controls = if i == 0 { player } else { controls[i] };
            if car.drive(track, controls, dt) && i == 0 {
                race.boost_flash = 0.0;
            }
        }
        cars::separate(&mut race.cars);

        for (i, car) in race.cars.iter().enumerate() {
            if race.finish[i].is_none() && car.progress() >= LAPS as f32 * track.length {
                race.finish[i] = Some(race.race_time);
            }
        }
        if race.phase == Phase::Racing {
            let done = (race.cars[0].progress() / track.length).floor() as i32;
            if done > race.laps_done {
                let time = race.race_time - race.lap_start;
                race.lap_flash = Some((done, time, race.best_lap, 0.0));
                race.best_lap = Some(race.best_lap.map_or(time, |best| best.min(time)));
                race.lap_start = race.race_time;
                race.laps_done = done;
                if done >= LAPS {
                    race.phase = Phase::Finished;
                    race.phase_time = 0.0;
                }
            }
        }
        race.update_order();
    }

    fn countdown(&self) -> (&'static str, f32) {
        let race = &self.race;
        match race.phase {
            Phase::Countdown => {
                let remaining = COUNTDOWN - race.phase_time;
                let digit = ["1", "2", "3"][(remaining.ceil() as usize).clamp(1, 3) - 1];
                (digit, 1.0 - remaining.fract())
            }
            Phase::Racing if race.phase_time < GO_DISPLAY => ("GO", race.phase_time / GO_DISPLAY),
            _ => ("", 0.0),
        }
    }

    fn start_lights(&self) -> StartLights {
        let race = &self.race;
        match race.phase {
            Phase::Countdown => {
                StartLights::Red(((race.phase_time / COUNTDOWN) * 5.0) as usize + 1)
            }
            Phase::Racing if race.phase_time < 2.0 => StartLights::Green,
            _ => StartLights::Off,
        }
    }

    fn colors(&self) -> Vec<Vec3> {
        racer_colors().map(|color| hex(color, 1.0)).collect()
    }
}

/// The cars' colors, the player's first.
fn racer_colors() -> impl Iterator<Item = u32> {
    std::iter::once(SLINT_BLUE).chain(PILOTS.iter().map(|pilot| pilot.color))
}

fn format_time(seconds: f32) -> String {
    let hundredths = (seconds * 100.0).round() as u32;
    format!("{}:{:02}.{:02}", hundredths / 6000, hundredths / 100 % 60, hundredths % 100)
}

fn format_delta(seconds: f32) -> String {
    format!("{}{:.2}", if seconds < 0.0 { "\u{2212}" } else { "+" }, seconds.abs())
}

fn slint_color(color: u32) -> slint::Color {
    slint::Color::from_argb_encoded(0xff00_0000 | color)
}

fn map_point(p: Vec2) -> MapPoint {
    MapPoint { x: p.x, y: p.y }
}

/// Path commands through the samples from `start`, `count` of them.
fn path(track: &Track, start: usize, count: usize, close: bool) -> String {
    let mut commands = String::new();
    for k in (0..=count).step_by(6).chain([count]) {
        let p = track.points[(start + k) % SAMPLES];
        let verb = if commands.is_empty() { "M" } else { "L" };
        commands += &format!("{verb} {:.1} {:.1} ", p.x, p.z);
    }
    if close {
        commands += "Z";
    }
    commands
}

fn show_track(app: &AppWindow, track: &Track) {
    let (lower, upper) = track
        .points
        .iter()
        .fold((Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)), |(lower, upper), p| {
            (lower.min(p.xz()), upper.max(p.xz()))
        });
    app.set_map_lower(map_point(lower - 4.0));
    app.set_map_upper(map_point(upper + 4.0));
    app.set_map_track(path(track, 0, SAMPLES, true).into());
    let tunnels: String = track
        .tunnels
        .iter()
        .map(|tunnel| path(track, tunnel.start, tunnel.samples - 1, false))
        .collect();
    app.set_map_tunnels(tunnels.into());
    let pads: Vec<MapPoint> = track
        .pads
        .iter()
        .map(|pad| map_point((track.points[pad.index] + track.rights[pad.index] * pad.offset).xz()))
        .collect();
    app.set_map_pads(Rc::new(slint::VecModel::from(pads)).into());
    let mut features = Vec::new();
    if track.crossing.is_some() {
        features.push("figure eight".to_string());
    }
    match track.tunnels.iter().filter(|tunnel| !tunnel.bridge).count() {
        0 => {}
        1 => features.push("a tunnel".into()),
        count => features.push(format!("{count} tunnels")),
    }
    let features: String = features.iter().map(|feature| format!(", {feature}")).collect();
    app.set_track_text(
        format!("Track {}, {} m{features}", track.seed, track.length.round()).into(),
    );
}

/// The lists the HUD shows, kept across frames, so that only rows that change update.
struct HudModels {
    standings: Rc<slint::VecModel<Standing>>,
    cars: Rc<slint::VecModel<CarDot>>,
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

fn show_telemetry(app: &AppWindow, state: &State, models: &HudModels) {
    let race = &state.race;
    let player = &race.cars[0];
    app.set_phase(race.phase);
    app.set_laps(LAPS);
    app.set_lap(race.laps_done + 1);
    let (countdown, progress) = state.countdown();
    app.set_countdown_text(countdown.into());
    app.set_countdown_progress(progress);
    let clock = match race.phase {
        Phase::Racing => race.race_time - race.lap_start,
        Phase::Finished => race.finish[0].unwrap_or(race.race_time),
        _ => 0.0,
    };
    app.set_lap_time(format_time(clock).into());
    app.set_best_lap(race.best_lap.map_or(String::new(), format_time).into());
    app.set_lap_flash(race.lap_flash.map_or_else(Default::default, |(lap, time, best, _)| {
        LapFlash {
            shown: true,
            lap,
            time: format_time(time).into(),
            delta: best.map_or(String::new(), |best| format_delta(time - best)).into(),
            improved: best.is_some_and(|best| time < best),
        }
    }));
    app.set_speed(player.speed * 3.6);
    app.set_boost(player.boost / cars::BOOST_TIME);
    app.set_boost_flash(race.boost_flash < BOOST_FLASH && race.phase != Phase::Ready);
    app.set_place(race.place() as i32);
    app.set_result(RaceResult {
        place: race.place() as i32,
        time: format_time(race.finish[0].unwrap_or(0.0)).into(),
        best_lap: race.best_lap.map_or(String::new(), format_time).into(),
    });

    let names: Vec<&str> = std::iter::once("You").chain(PILOTS.iter().map(|p| p.name)).collect();
    let colors: Vec<u32> = racer_colors().collect();
    let leader = &race.cars[race.order[0]];
    let racing = matches!(race.phase, Phase::Racing | Phase::Finished);
    let standings = race
        .order
        .iter()
        .enumerate()
        .map(|(place, &i)| {
            let gap = match (race.finish[i], race.finish[race.order[0]]) {
                (Some(time), Some(winner)) if place > 0 => format!("+{:.1}", time - winner),
                _ if place > 0 && racing => format!(
                    "+{:.1}",
                    ((leader.progress() - race.cars[i].progress()) / leader.speed.max(10.0))
                        .max(0.0)
                ),
                _ => String::new(),
            };
            Standing {
                name: names[i].into(),
                color: slint_color(colors[i]),
                gap: gap.into(),
                player: i == 0,
            }
        })
        .collect();
    update_rows(&models.standings, standings);
    let dots = race
        .cars
        .iter()
        .zip(&colors)
        .enumerate()
        .rev()
        .map(|(i, (car, &color))| CarDot {
            x: car.position.x,
            y: car.position.z,
            color: slint_color(color),
            player: i == 0,
        })
        .collect();
    update_rows(&models.cars, dots);
}

/// Renders the scene into one of two textures, so Slint can show the previous one meanwhile.
struct Frames {
    textures: Vec<wgpu::Texture>,
    next: usize,
}

impl Frames {
    fn texture(&mut self, device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        let size = |texture: &wgpu::Texture| (texture.width(), texture.height());
        if self.textures.first().is_none_or(|texture| size(texture) != (width, height)) {
            self.textures = (0..2)
                .map(|_| {
                    device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("racer frame"),
                        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        // The renderer writes through an sRGB view, so the texture holds
                        // sRGB-encoded bytes, which Slint shows as they are.
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

const FRAME_VIEW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen(start))]
pub fn main() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();
    run().unwrap();
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    // The scene renders on Slint's device, so Slint can draw the rendered frames directly.
    slint::BackendSelector::new()
        .require_wgpu_30(WGPUConfiguration::Automatic(WGPUSettings::default()))
        .select()?;

    let app_window = AppWindow::new()?;
    // Phones and browsers often run on mid-range GPUs.
    if cfg!(any(target_arch = "wasm32", target_os = "android", target_os = "ios")) {
        app_window.set_quality(Quality::Medium);
    }
    for arg in std::env::args() {
        match arg.as_str() {
            "--low" => app_window.set_quality(Quality::Low),
            "--medium" => app_window.set_quality(Quality::Medium),
            _ => {}
        }
    }

    let state = Rc::new(RefCell::new(State::new()));
    let update = {
        let state = state.clone();
        let app_weak = app_window.as_weak();
        move |change: &dyn Fn(&mut State)| {
            change(&mut state.borrow_mut());
            if let Some(app) = app_weak.upgrade() {
                app.window().request_redraw();
            }
        }
    };
    app_window.on_input_changed({
        let update = update.clone();
        move |x, y| update(&|state| state.stick = Vec2::new(x, y))
    });
    app_window.on_start({
        let update = update.clone();
        move || update(&|state| state.start_requested = true)
    });
    app_window.on_reset({
        let update = update.clone();
        move || update(&|state| state.reset_requested = true)
    });
    app_window.on_new_track({
        let update = update.clone();
        move || update(&|state| state.new_track_requested = true)
    });
    app_window.on_paused_changed({
        let update = update.clone();
        move |paused| update(&|state| state.paused = paused)
    });
    let app_weak = app_window.as_weak();
    let publish_settings = move || {
        let Some(app) = app_weak.upgrade() else { return };
        update(&|state| {
            state.autopilot = app.get_autopilot();
            state.assist = app.get_assist();
            state.quality = app.get_quality();
            state.camera =
                if app.get_bumper_camera() { CameraMode::Bumper } else { CameraMode::Chase };
        })
    };
    publish_settings();
    app_window.on_settings_changed(publish_settings);
    let models = HudModels { standings: Default::default(), cars: Default::default() };
    app_window.set_standings(models.standings.clone().into());
    app_window.set_cars(models.cars.clone().into());
    show_track(&app_window, &state.borrow().track);

    // The renderer, the scene, and Slint's device, once Slint set up rendering.
    let mut graphics: Option<(Renderer, World, wgpu::Device)> = None;
    let mut frames = Frames { textures: Vec::new(), next: 0 };
    let mut last_frame = web_time::Instant::now();
    let mut frame_count = 0u32;
    let mut fps_window_start = web_time::Instant::now();

    let app_weak = app_window.as_weak();
    app_window.window().set_rendering_notifier(move |rendering_state, graphics_api| {
        let Some(app) = app_weak.upgrade() else { return };
        if let slint::RenderingState::RenderingSetup = rendering_state {
            let slint::GraphicsAPI::WGPU30 { device, queue, .. } = graphics_api else {
                return;
            };
            let info = device.adapter_info();
            app.set_gpu_text(format!("{} ({:?})", info.name, info.backend).into());
            let renderer = Renderer::new(device, queue, FRAME_VIEW_FORMAT);
            let mut world = World::new(&renderer);
            world.show_track(&renderer, &state.borrow().track);
            graphics = Some((renderer, world, device.clone()));
            return;
        }
        if !matches!(rendering_state, slint::RenderingState::BeforeRendering) {
            return;
        }
        let Some((renderer, world, device)) = &mut graphics else { return };
        let now = web_time::Instant::now();
        // After an idle period the first frame would otherwise jump.
        let dt = (now - last_frame).as_secs_f32().min(1.0 / 20.0);
        last_frame = now;

        let mut state = state.borrow_mut();
        if std::mem::take(&mut state.new_track_requested) {
            state.track = track::generate(random_seed());
            state.race = Race::new(&state.track);
            show_track(&app, &state.track);
            world.show_track(renderer, &state.track);
        }
        let dt = if state.paused { 0.0 } else { dt };
        state.step(dt);
        show_telemetry(&app, &state, &models);

        let window_size = app.window().size();
        let size = (window_size.width.max(1), window_size.height.max(1));
        let colors = state.colors();
        let frame = FrameInput {
            track: &state.track,
            cars: &state.race.cars,
            colors: &colors,
            start_lights: state.start_lights(),
            camera: state.camera,
            cutting: state.autopilot && state.race.phase == Phase::Racing,
            quality: state.quality,
        };
        world.update(&frame, dt, size.0 as f32 / size.1 as f32);
        let texture = frames.texture(device, size.0, size.1);
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(FRAME_VIEW_FORMAT),
            ..Default::default()
        });
        renderer.render(&world.scene, &world.camera, &view, size, state.quality);
        if let Ok(image) = texture.try_into() {
            app.set_texture(image);
        }

        frame_count += 1;
        let elapsed = fps_window_start.elapsed().as_secs_f32();
        if elapsed >= 0.5 {
            let fps = frame_count as f32 / elapsed;
            app.set_fps_text(format!("{fps:.0} fps · {:.1} ms", 1000.0 / fps.max(1.0)).into());
            frame_count = 0;
            fps_window_start = now;
        }
        if state.animating() {
            app.window().request_redraw();
        }
    })?;

    app_window.run()?;
    Ok(())
}
