// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore metalness tonemapping

use std::cell::RefCell;
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};
use std::rc::Rc;

use glam::{Mat4, Quat, Vec2, Vec3, Vec3Swizzles, Vec4};
use slint::wgpu_30::{WGPUConfiguration, WGPUSettings, wgpu};

use course::{Course, HALL_HEIGHT, HALL_X, HALL_Z, SAMPLES};
use renderer::{
    Blend, Camera, FloorSpot, Geometry, Instance, LightMap, Material, Mesh, Node, Object, Renderer,
    Scene, SpotLight, SpotShape, Texture,
};
use rivals::{Neighbor, Rival};

mod course;
mod director;
mod geometry;
mod renderer;
mod rivals;
mod scores;
mod textures;

slint::include_modules!();

const BACKDROP: u32 = 0x07090d;
const SLINT_BLUE: u32 = 0x2379f4;

const CRUISE_SPEED: f32 = 10.0;
const BOOST_SPEED: f32 = 16.0;
const TURN_RATE: f32 = 2.6;
const CLIMB_RATE: f32 = 5.5;
const GROUND_CLEARANCE: f32 = 0.15;
const CEILING: f32 = 19.0;
// Seconds of flight on a full battery, while cruising and while boosting.
const BATTERY_CRUISE: f32 = 150.0;
const BATTERY_BOOST: f32 = 60.0;
const LAPS: u32 = 3;
// How long the autopilot's result stays on screen before the next heat.
const HEAT_RESULT_DISPLAY: f32 = 6.0;
// Seconds of boost on a full charge, the charge per second that comes back on its own, and the
// charge each gate passed inside its frame adds.
const BOOST_SECONDS: f32 = 3.0;
const BOOST_RECHARGE: f32 = 0.02;
const BOOST_PER_GATE: f32 = 0.12;
// Once used up, boost works again from this charge on.
const BOOST_RESUME: f32 = 0.25;
const COUNTDOWN: f32 = 3.0;
// How long "GO" stays on screen after the countdown.
const GO_DISPLAY: f32 = 0.8;
// How long a finished lap's result stays on screen, and how long the lap clock holds its time.
const LAP_RESULT_DISPLAY: f32 = 4.0;
const LAP_HOLD: f32 = 1.2;
// With assist, a missed gate counts, but costs this many seconds.
const MISSED_GATE_PENALTY: f32 = 2.0;
const PENALTY_DISPLAY: f32 = 1.6;
// How far before the start gate the launch pad sits, so the drone can climb onto the course.
const LAUNCH_DISTANCE: f32 = 25.0;
// The racing drone is drawn a little larger than a real one so it reads from across the hall.
const DRONE_SCALE: f32 = 1.3;
// The launch pads' edge length.
const PAD_SIZE: f32 = 1.8 * DRONE_SCALE;
const FLOOR_SIZE: Vec2 = Vec2::new(2.0 * HALL_X, 2.0 * HALL_Z);
const FLOOR_LIGHT_TEXELS_PER_METER: f32 = 4.0;
/// The light shafts whose pools low quality keeps.
const LOW_QUALITY_POOLS: usize = 3;
const TRAIL_POINTS: usize = 44;

fn yaw_towards(direction: Vec3) -> f32 {
    (-direction.x).atan2(-direction.z)
}

fn forward(yaw: f32) -> Vec3 {
    Quat::from_rotation_y(yaw) * Vec3::NEG_Z
}

/// How much of the way to its target a value easing at `rate` per second moves in `dt`
/// seconds, whatever the frame rate.
fn damp(rate: f32, dt: f32) -> f32 {
    1.0 - (-rate * dt).exp()
}

/// `angle` wrapped to -π..π.
fn wrap_angle(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

fn random_seed() -> u32 {
    let now = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map_or(0, |time| time.as_nanos() as u64);
    // Mixes all bits, since some platforms only report microseconds.
    let mixed = (now ^ (now >> 29)).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    1000 + ((mixed >> 32) % 9000) as u32
}

/// A drone in the standings.
#[derive(Clone, Copy)]
struct Entry {
    name: &'static str,
    color: u32,
    distance: f32,
    speed: f32,
    player: bool,
}

/// How far a drone must get ahead of the one before it in the standings to pass it, in
/// meters, so drones side by side don't swap places back and forth.
const OVERTAKE_MARGIN: f32 = 0.5;

/// The standings, in `Race::order`.
fn standings(race: &Race) -> Vec<Entry> {
    let player = Entry {
        name: "You",
        color: SLINT_BLUE,
        distance: race.progress(),
        speed: race.speed,
        player: true,
    };
    let entries: Vec<Entry> = std::iter::once(player)
        .chain(race.rivals.iter().map(|rival| Entry {
            name: rival.pilot.name,
            color: rival.pilot.color,
            distance: rival.distance,
            speed: rival.speed,
            player: false,
        }))
        .collect();
    race.order.iter().map(|&index| entries[index]).collect()
}

/// The player's race, once finished.
#[derive(Clone, Copy)]
struct Outcome {
    /// From 1.
    place: usize,
    time: f32,
    /// In km/h.
    speed: f32,
}

#[derive(Clone, Copy)]
struct FinishedLap {
    lap: u32,
    time: f32,
    /// Against the best lap before this one.
    delta: Option<f32>,
    /// Seconds since the drone crossed the line.
    age: f32,
}

/// The race and the drone's flight state.
struct Race {
    phase: Phase,
    phase_time: f32,
    position: Vec3,
    yaw: f32,
    speed: f32,
    climb: f32,
    roll: f32,
    battery: f32,
    /// From 0 to 1.
    boost_charge: f32,
    /// Cleared when the charge runs out, until it reaches `BOOST_RESUME` again.
    boost_ready: bool,
    lap: u32,
    lap_time: f32,
    last_lap: Option<f32>,
    best_lap: Option<f32>,
    next_gate: usize,
    // The drone's side of the next gate's plane in the previous frame.
    gate_side: f32,
    // The course sample nearest to the drone.
    course_index: usize,
    /// How far along the course the drone is, counting every lap, at the nearest course
    /// sample.
    distance: f32,
    /// How far the drone is past the nearest course sample, along the course.
    past_sample: f32,
    /// The drones by place, the player as 0 and the rivals from 1.
    order: Vec<usize>,
    finished: Option<FinishedLap>,
    /// Seconds since the last missed gate's penalty.
    penalty: Option<f32>,
    message: &'static str,
    summary: String,
    /// The stick input the autopilot applies.
    autopilot_stick: Vec2,
    rivals: Vec<Rival>,
    /// Seconds since the start, with penalties.
    race_time: f32,
    /// Set when the autopilot flew any part of the race, which keeps it off the leaderboard.
    autopilot_used: bool,
    outcome: Option<Outcome>,
    /// The standings at the finish.
    final_standings: Vec<Entry>,
    /// Whether the race is on the leaderboard already.
    saved: bool,
}

impl Race {
    fn new(course: &Course) -> Self {
        let index = launch_index(course);
        Self {
            phase: Phase::Ready,
            phase_time: 0.0,
            position: launch_pad(course).with_y(GROUND_CLEARANCE),
            yaw: yaw_towards(course.tangents[index]),
            speed: 0.0,
            climb: 0.0,
            roll: 0.0,
            battery: 1.0,
            boost_charge: 1.0,
            boost_ready: true,
            lap: 1,
            lap_time: 0.0,
            last_lap: None,
            best_lap: None,
            next_gate: 0,
            gate_side: -1.0,
            course_index: index,
            distance: -LAUNCH_DISTANCE,
            past_sample: 0.0,
            // As on the grid.
            order: (0..=rivals::PILOTS.len()).collect(),
            finished: None,
            penalty: None,
            message: "",
            summary: String::new(),
            autopilot_stick: Vec2::ZERO,
            rivals: (1..=rivals::PILOTS.len()).map(|slot| Rival::new(course, slot)).collect(),
            race_time: 0.0,
            autopilot_used: false,
            outcome: None,
            final_standings: Vec::new(),
            saved: false,
        }
    }
}

impl Race {
    /// How far along the course the drone is, counting every lap, for the standings.
    fn progress(&self) -> f32 {
        self.distance + self.past_sample
    }

    /// Moves drones up the standings once they're `OVERTAKE_MARGIN` ahead of the one before.
    fn update_order(&mut self) {
        let distances: Vec<f32> = std::iter::once(self.progress())
            .chain(self.rivals.iter().map(|rival| rival.distance))
            .collect();
        let mut swapped = true;
        while swapped {
            swapped = false;
            for k in 1..self.order.len() {
                let (ahead, behind) = (self.order[k - 1], self.order[k]);
                if distances[behind] > distances[ahead] + OVERTAKE_MARGIN {
                    self.order.swap(k - 1, k);
                    swapped = true;
                }
            }
        }
    }
}

/// The course sample at the launch pad, `LAUNCH_DISTANCE` before the start gate.
fn launch_index(course: &Course) -> usize {
    SAMPLES - (LAUNCH_DISTANCE / course.spacing()).round() as usize
}

fn launch_pad(course: &Course) -> Vec3 {
    course.points[launch_index(course)].with_y(0.0)
}

/// The input and settings from the UI, and the race they drive.
struct State {
    stick: Vec2,
    boost: bool,
    autopilot: bool,
    /// Keeps the drone at the course's height and gently steers it along the course.
    assist: bool,
    start_requested: bool,
    reset_requested: bool,
    new_course_requested: bool,
    fpv: bool,
    /// While set, the race and everything in the hall stand still.
    paused: bool,
    camera_shift: f32,
    high_quality: bool,
    look: usize,
    course: Course,
    race: Race,
    scores: scores::Scores,
    /// The leaderboard row of the race saved last, to highlight it.
    saved_place: Option<usize>,
}

impl State {
    fn new() -> Self {
        // `DRONE_SEED` picks a fixed first course, to compare runs.
        let seed = std::env::var("DRONE_SEED").ok().and_then(|seed| seed.parse().ok());
        let course = course::generate(seed.unwrap_or_else(random_seed));
        let race = Race::new(&course);
        Self {
            stick: Vec2::ZERO,
            boost: false,
            autopilot: false,
            assist: true,
            start_requested: false,
            reset_requested: false,
            new_course_requested: false,
            fpv: false,
            paused: false,
            camera_shift: 0.0,
            high_quality: true,
            look: 0,
            course,
            race,
            scores: scores::Scores::load(),
            saved_place: None,
        }
    }

    /// Whether the player's finished race makes it onto the leaderboard.
    fn qualifies(&self) -> bool {
        let race = &self.race;
        !race.autopilot_used
            && !race.saved
            && race.outcome.is_some_and(|outcome| self.scores.qualifies(outcome.speed))
    }

    fn save_score(&mut self, initials: &str) {
        if let Some(outcome) = self.race.outcome
            && self.qualifies()
        {
            let score = scores::Score {
                initials: initials.into(),
                speed: outcome.speed,
                time: outcome.time,
            };
            self.saved_place = Some(self.scores.insert(score));
            self.race.saved = true;
        }
    }

    fn animating(&self) -> bool {
        !self.paused && (self.race.phase != Phase::Ready || self.start_requested)
    }

    /// Advances the race and the drone's flight.
    fn fly(&mut self, dt: f32) {
        let course = &self.course;
        let race = &mut self.race;
        if std::mem::take(&mut self.reset_requested) {
            *race = Race::new(course);
        }
        if std::mem::take(&mut self.start_requested)
            && matches!(race.phase, Phase::Ready | Phase::Finished)
        {
            *race = Race { phase: Phase::Countdown, best_lap: race.best_lap, ..Race::new(course) };
        }

        race.phase_time += dt;
        if let Some(finished) = &mut race.finished {
            finished.age += dt;
            if finished.age >= LAP_RESULT_DISPLAY {
                race.finished = None;
            }
        }
        race.penalty = race.penalty.map(|age| age + dt).filter(|&age| age < PENALTY_DISPLAY);
        race.autopilot_stick = Vec2::ZERO;
        match race.phase {
            Phase::Ready => {}
            Phase::Countdown => {
                if race.phase_time >= COUNTDOWN {
                    race.phase = Phase::Flying;
                    race.phase_time = 0.0;
                }
            }
            Phase::Flying | Phase::Finished => {
                let drones: Vec<Neighbor> = std::iter::once((race.position, race.speed))
                    .chain(race.rivals.iter().map(|rival| (rival.position, rival.speed)))
                    .map(|(position, speed)| Neighbor { position, speed })
                    .collect();
                for (index, rival) in race.rivals.iter_mut().enumerate() {
                    // A rival behind the player pushes a little harder.
                    let behind = race.distance - rival.distance;
                    let push = 1.0 + (behind / 40.0).clamp(0.0, 0.1);
                    // The player's drone comes first, so this rival is at `index + 1`.
                    let others: Vec<_> = (drones.iter().enumerate())
                        .filter(|&(other, _)| other != index + 1)
                        .map(|(_, neighbor)| *neighbor)
                        .collect();
                    rival.fly(course, race.phase_time, push, &others, dt);
                }
                track_progress(race, course);
                race.update_order();
                let guide = autopilot(race, course);
                let flying = race.phase == Phase::Flying;
                if flying {
                    race.race_time += dt;
                    race.autopilot_used |= self.autopilot;
                }
                let assisted = self.assist && !self.autopilot;
                // After the finish, the drone glides on along the course.
                let (stick, boost) = if self.autopilot || !flying {
                    race.autopilot_stick = guide;
                    (guide, false)
                } else if assisted {
                    // The height follows the course, and steering is pulled gently along it.
                    let pull = 0.4 * (1.0 - self.stick.x.abs());
                    let stick = Vec2::new(
                        (self.stick.x + guide.x * pull).clamp(-1.0, 1.0),
                        (guide.y + self.stick.y * 0.6).clamp(-1.0, 1.0),
                    );
                    (stick, self.boost)
                } else {
                    (self.stick, self.boost)
                };
                let boost = boost && race.boost_ready;
                steer(race, course, stick, boost, assisted, dt);
            }
        }
        // The autopilot races on its own, for an unattended demo. After a finish or an empty
        // battery, the next heat is on a new course.
        let heat_over = race.phase == Phase::Finished && race.phase_time > HEAT_RESULT_DISPLAY;
        if self.autopilot && (race.phase == Phase::Ready || heat_over) {
            self.new_course_requested |= heat_over || !race.message.is_empty();
            self.start_requested = true;
        }
    }

    fn countdown(&self) -> (&'static str, f32) {
        let race = &self.race;
        match race.phase {
            Phase::Countdown => {
                let remaining = COUNTDOWN - race.phase_time;
                let digit = ["1", "2", "3"][(remaining.ceil() as usize).clamp(1, 3) - 1];
                (digit, 1.0 - remaining.fract())
            }
            Phase::Flying if race.phase_time < GO_DISPLAY => ("GO", race.phase_time / GO_DISPLAY),
            _ => ("", 0.0),
        }
    }
}

/// Finds the course sample nearest to the drone, and how far along the course it got.
fn track_progress(race: &mut Race, course: &Course) {
    let distance = |i: usize| course.points[i].distance_squared(race.position);
    // Looks around the last known position, so a crossing doesn't jump to the other stretch,
    // or everywhere once off the course.
    let nearby = (0..140)
        .map(|step| (race.course_index + SAMPLES + step - 20) % SAMPLES)
        .min_by(|&a, &b| distance(a).total_cmp(&distance(b)))
        .filter(|&i| distance(i) < 64.0);
    let index = nearby.unwrap_or_else(|| {
        (0..SAMPLES).min_by(|&a, &b| distance(a).total_cmp(&distance(b))).unwrap_or(0)
    });
    let step = (index + SAMPLES - race.course_index) % SAMPLES;
    let step = if step > SAMPLES / 2 { step as f32 - SAMPLES as f32 } else { step as f32 };
    // Back on the course after a detour, the progress counts, unless the nearest stretch is
    // too far along to have been flown to.
    if nearby.is_some() || step.abs() * course.spacing() < 60.0 {
        race.distance += step * course.spacing();
    }
    race.course_index = index;
    let half = course.spacing() / 2.0;
    race.past_sample =
        (race.position - course.points[index]).dot(course.tangents[index]).clamp(-half, half);
}

/// The stick input that follows the course: it steers towards the point a few meters ahead
/// on the course and climbs or descends to its height. Gates stand on the course, so this
/// flies through each of them.
fn autopilot(race: &Race, course: &Course) -> Vec2 {
    let ahead = |meters: f32| {
        course.points[(race.course_index + (meters / course.spacing()).round() as usize) % SAMPLES]
    };
    let aim = ahead(5.5);
    let heading_error = wrap_angle(yaw_towards(aim - race.position) - race.yaw);
    // Climbs with the course's slope just ahead, and corrects towards the course's height here,
    // so the drone passes the gates at their centers.
    let (here, ahead) = (ahead(0.0), ahead(2.0));
    let climb = (ahead.y - here.y) / 2.0 * race.speed + (here.y - race.position.y) * 3.0;
    Vec2::new((-heading_error * 2.5).clamp(-1.0, 1.0), (climb / CLIMB_RATE).clamp(-1.0, 1.0))
}

/// The arcade flight model: the drone flies forward on its own, the stick turns and climbs.
/// With `forgiving`, a gate the drone passes outside of counts too, with a time penalty.
fn steer(race: &mut Race, course: &Course, stick: Vec2, boost: bool, forgiving: bool, dt: f32) {
    let ease = |current: f32, target: f32, rate: f32| current + (target - current) * damp(rate, dt);
    race.speed = ease(race.speed, if boost { BOOST_SPEED } else { CRUISE_SPEED }, 2.5);
    race.climb = ease(race.climb, stick.y * CLIMB_RATE, 4.0);
    race.roll = ease(race.roll, stick.x, 5.0);
    race.yaw -= stick.x * TURN_RATE * dt;

    let position = race.position + forward(race.yaw) * race.speed * dt + Vec3::Y * race.climb * dt;
    race.position = Vec3::new(
        position.x.clamp(-HALL_X + 1.0, HALL_X - 1.0),
        position.y.clamp(GROUND_CLEARANCE, CEILING),
        position.z.clamp(-HALL_Z + 1.0, HALL_Z - 1.0),
    );

    if race.phase != Phase::Flying {
        return;
    }
    race.lap_time += dt;
    race.battery -= dt / if boost { BATTERY_BOOST } else { BATTERY_CRUISE };
    race.boost_charge = if boost {
        (race.boost_charge - dt / BOOST_SECONDS).max(0.0)
    } else {
        (race.boost_charge + dt * BOOST_RECHARGE).min(1.0)
    };
    if race.boost_charge == 0.0 {
        race.boost_ready = false;
    } else if race.boost_charge >= BOOST_RESUME {
        race.boost_ready = true;
    }
    if race.battery <= 0.0 {
        let progress = match race.next_gate {
            0 => "before the first gate".to_string(),
            passed => format!("after {passed} of {} gates", course.gates.len()),
        };
        let summary = format!(
            "Out of power on lap {}, {progress}. Boosting drains the battery faster.",
            race.lap
        );
        *race = Race {
            best_lap: race.best_lap,
            message: "Battery empty",
            summary,
            ..Race::new(course)
        };
        return;
    }

    // A gate counts when the drone crosses its plane, in flight direction, inside a frame.
    let gate = &course.gates[race.next_gate];
    let offset = race.position - gate.center;
    let side = offset.dot(gate.forward);
    let lateral = offset.dot(gate.right).abs();
    let inside = lateral < gate.half - 0.2
        && gate
            .frame_heights()
            .iter()
            .any(|height| (race.position.y - height).abs() < gate.half - 0.2);
    let expected = (race.lap - 1) as f32 * course.length + gate.index as f32 * course.spacing();
    let missed = forgiving && race.distance > expected + 12.0;
    if missed {
        race.lap_time += MISSED_GATE_PENALTY;
        race.race_time += MISSED_GATE_PENALTY;
        race.penalty = Some(0.0);
    }
    let passed = race.gate_side < 0.0 && side >= 0.0 && inside;
    if passed {
        race.boost_charge = (race.boost_charge + BOOST_PER_GATE).min(1.0);
    }
    if passed || missed {
        race.next_gate += 1;
        if race.next_gate == course.gates.len() {
            race.next_gate = 0;
            race.finished = Some(FinishedLap {
                lap: race.lap,
                time: race.lap_time,
                delta: race.best_lap.map(|best| race.lap_time - best),
                age: 0.0,
            });
            race.last_lap = Some(race.lap_time);
            race.best_lap =
                Some(race.best_lap.map_or(race.lap_time, |best| best.min(race.lap_time)));
            race.lap_time = 0.0;
            if race.lap == LAPS {
                finish(race);
            } else {
                race.lap += 1;
            }
        }
        let next = &course.gates[race.next_gate];
        race.gate_side = (race.position - next.center).dot(next.forward);
    } else {
        race.gate_side = side;
    }
}

fn finish(race: &mut Race) {
    race.phase = Phase::Finished;
    race.phase_time = 0.0;
    race.final_standings = standings(race);
    let place = race.final_standings.iter().position(|entry| entry.player).unwrap_or(0) + 1;
    // The distance flown along the course, from the launch pad.
    let distance = race.distance + LAUNCH_DISTANCE;
    race.outcome =
        Some(Outcome { place, time: race.race_time, speed: distance / race.race_time * 3.6 });
}

fn format_time(seconds: f32) -> String {
    let hundredths = (seconds * 100.0).round() as u32;
    format!("{}:{:02}.{:02}", hundredths / 6000, hundredths / 100 % 60, hundredths % 100)
}

fn format_delta(seconds: f32) -> String {
    format!("{}{:.2}", if seconds < 0.0 { "\u{2212}" } else { "+" }, seconds.abs())
}

fn map_point(p: Vec2) -> MapPoint {
    MapPoint { x: p.x, y: p.y }
}

fn show_course(app: &AppWindow, course: &Course) {
    let model = |points: Vec<Vec2>| -> slint::ModelRc<MapPoint> {
        std::rc::Rc::new(slint::VecModel::from(
            points.into_iter().map(map_point).collect::<Vec<_>>(),
        ))
        .into()
    };
    let track: Vec<Vec2> =
        (0..SAMPLES).step_by(SAMPLES / 140).map(|i| course.points[i].xz()).collect();
    let gates: Vec<Vec2> = course.gates.iter().map(|gate| gate.center.xz()).collect();
    // The drone strays a little off the course, so the map keeps some room around it.
    let (lower, upper) = track
        .iter()
        .chain(&gates)
        .fold((Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)), |(lower, upper), &p| {
            (lower.min(p), upper.max(p))
        });
    app.set_map_lower(map_point(lower - 4.0));
    app.set_map_upper(map_point(upper + 4.0));
    app.set_course_text(
        format!(
            "Course {}, {} m, {} gates",
            course.seed,
            course.length.round(),
            course.gates.len()
        )
        .into(),
    );
    app.set_track(model(track));
    app.set_gates(model(gates));
}

/// The lists the HUD shows, kept across frames, so that only rows that change update.
struct HudModels {
    standings: Rc<slint::VecModel<Standing>>,
    rivals: Rc<slint::VecModel<RivalDot>>,
    scores: Rc<slint::VecModel<ScoreRow>>,
}

impl HudModels {
    fn new(app: &AppWindow) -> Self {
        let models = Self {
            standings: Default::default(),
            rivals: Default::default(),
            scores: Default::default(),
        };
        app.set_standings(models.standings.clone().into());
        app.set_rivals(models.rivals.clone().into());
        app.set_scores(models.scores.clone().into());
        models
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

fn show_telemetry(app: &AppWindow, state: &State, models: &HudModels) {
    let race = &state.race;
    let time_or_empty = |time: Option<f32>| time.map_or(String::new(), format_time);
    let held = race.finished.filter(|finished| finished.age < LAP_HOLD);
    app.set_phase(race.phase);
    app.set_laps(LAPS as i32);
    app.set_result(race.outcome.map_or_else(Default::default, |outcome| RaceResult {
        place: outcome.place as i32,
        time: format_time(outcome.time).into(),
        speed: format!("{:.1}", outcome.speed).into(),
        qualifies: state.qualifies(),
        autopilot: race.autopilot_used,
    }));
    let rows: Vec<ScoreRow> = state
        .scores
        .list()
        .iter()
        .take(5)
        .enumerate()
        .map(|(place, score)| ScoreRow {
            initials: score.initials.as_str().into(),
            speed: format!("{:.1}", score.speed).into(),
            highlight: race.saved && state.saved_place == Some(place),
        })
        .collect();
    update_rows(&models.scores, rows);
    let (countdown_text, countdown_progress) = state.countdown();
    app.set_countdown_text(countdown_text.into());
    app.set_countdown_progress(countdown_progress);
    app.set_lap(held.map_or(race.lap, |finished| finished.lap) as i32);
    // After the finish, the clock shows the race's time.
    let clock = match race.outcome {
        Some(outcome) => outcome.time,
        None => held.map_or(race.lap_time, |finished| finished.time),
    };
    app.set_lap_time(format_time(clock).into());
    app.set_last_lap(time_or_empty(race.last_lap).into());
    app.set_best_lap(time_or_empty(race.best_lap).into());
    app.set_lap_result(race.finished.map_or_else(Default::default, |finished| LapResult {
        shown: true,
        lap: finished.lap as i32,
        time: format_time(finished.time).into(),
        delta: finished.delta.map_or(String::new(), format_delta).into(),
        improved: finished.delta.is_some_and(|delta| delta < 0.0),
        best: finished.delta.is_none_or(|delta| delta < 0.0),
        held: held.is_some(),
        progress: finished.age / LAP_RESULT_DISPLAY,
    }));
    app.set_next_gate(race.next_gate as i32);
    app.set_speed(race.speed * 3.6);
    app.set_altitude(race.position.y - GROUND_CLEARANCE);
    app.set_battery(race.battery);
    app.set_boost_charge(race.boost_charge);
    app.set_boost_ready(race.boost_ready);
    app.set_drone_position(map_point(race.position.xz()));
    let heading = forward(race.yaw);
    app.set_drone_heading(heading.x.atan2(-heading.z).to_degrees());
    app.set_message(race.message.into());
    app.set_summary(race.summary.as_str().into());
    app.set_autopilot_stick(MapPoint { x: race.autopilot_stick.x, y: race.autopilot_stick.y });
    app.set_penalty(match race.penalty {
        Some(_) => format!("Missed gate +{MISSED_GATE_PENALTY:.1} s").into(),
        None => Default::default(),
    });

    // The standings, frozen at the finish, with gaps in seconds to the leader.
    let standings = match race.phase {
        Phase::Finished => race.final_standings.clone(),
        _ => self::standings(race),
    };
    let (leader_distance, leader_speed) = (standings[0].distance, standings[0].speed.max(8.0));
    app.set_place(standings.iter().position(|entry| entry.player).unwrap_or(0) as i32 + 1);
    let standings: Vec<Standing> = standings
        .iter()
        .enumerate()
        .map(|(place, entry)| Standing {
            name: entry.name.into(),
            color: slint_color(entry.color),
            gap: if place > 0 && matches!(race.phase, Phase::Flying | Phase::Finished) {
                format!("+{:.1}", ((leader_distance - entry.distance) / leader_speed).max(0.0))
                    .into()
            } else {
                Default::default()
            },
            player: entry.player,
        })
        .collect();
    update_rows(&models.standings, standings);
    let rivals: Vec<RivalDot> = race
        .rivals
        .iter()
        .map(|rival| RivalDot {
            x: rival.position.x,
            y: rival.position.z,
            color: slint_color(rival.pilot.color),
        })
        .collect();
    update_rows(&models.rivals, rivals);
}

fn slint_color(color: u32) -> slint::Color {
    slint::Color::from_argb_encoded(0xff00_0000 | color)
}

/// The scene's light colors for one look of the UI.
struct Palette {
    perimeter: u32,
    ceiling: u32,
    shaft: u32,
    shaft_pool: u32,
    route: u32,
    gate: u32,
    start_gate: u32,
    tower: u32,
}

fn palette(look: usize) -> Palette {
    match look {
        // Sodium-lit warehouse: amber LEDs, warm light, hazard-yellow towers.
        1 => Palette {
            perimeter: 0xc97a1e,
            ceiling: 0xa8946e,
            shaft: 0x3d3324,
            shaft_pool: 0x4a3f2e,
            route: 0x5c4523,
            gate: 0xffe2b8,
            start_gate: 0xff9f1c,
            tower: 0xffcf33,
        },
        // Cool cyan with muted orchid for the start gate and the towers.
        2 => Palette {
            perimeter: 0x1aa6c9,
            ceiling: 0x6a9fbd,
            shaft: 0x12344d,
            shaft_pool: 0x1a3c55,
            route: 0x145a6e,
            gate: 0xa6eef7,
            start_gate: 0xd65ad0,
            tower: 0xc856c4,
        },
        _ => Palette {
            perimeter: 0x1f6f86,
            ceiling: 0x7c8794,
            shaft: 0x2b3644,
            shaft_pool: 0x38414d,
            route: 0x2c4552,
            gate: 0xbfe8ff,
            start_gate: 0xffd27a,
            tower: 0xffb347,
        },
    }
}

/// An sRGB hex color in linear light, scaled.
fn hex(color: u32, scale: f32) -> Vec3 {
    let channel = |shift: u32| textures::srgb_to_linear((color >> shift) as u8);
    Vec3::new(channel(16), channel(8), channel(0)) * scale
}

/// The rotation that points -Z from `eye` at `target`, like a camera.
fn looking_at(eye: Vec3, target: Vec3) -> Quat {
    Quat::look_at_rh(eye, target, Vec3::Y).inverse()
}

/// A glow that adds to what's behind it: `texture`'s alpha, tinted with `tint`.
fn additive(texture: &Rc<Texture>, tint: Vec3) -> Material {
    Material {
        texture: Some(texture.clone()),
        blend: Blend::Additive,
        depth_write: false,
        double_sided: true,
        ..Material::unlit(tint)
    }
}

fn standard(color: Vec3, roughness: f32, metalness: f32, texture: Option<Rc<Texture>>) -> Material {
    Material { texture, ..Material::lit(color, roughness, metalness) }
}

/// A flat ribbon on the floor along the course, with the dash texture repeating every 1.6 m,
/// around the course's center, which is returned with it. See `geometry::boxes` for why.
fn route_ribbon(course: &Course) -> (Geometry, Vec3) {
    let repeats = (course.length / 1.6).round();
    let center = (course.points.iter().sum::<Vec3>() / SAMPLES as f32).with_y(0.025);
    let mut geometry = Geometry::default();
    for i in 0..=SAMPLES {
        let (p, r) =
            (course.points[i % SAMPLES].with_y(0.025) - center, course.rights[i % SAMPLES]);
        geometry.positions.push((p - r * 0.12).to_array());
        geometry.positions.push((p + r * 0.12).to_array());
        let v = i as f32 / SAMPLES as f32 * repeats;
        geometry.uvs.push([0.0, v]);
        geometry.uvs.push([1.0, v]);
    }
    geometry.normals = vec![[0.0, 1.0, 0.0]; geometry.positions.len()];
    geometry.indices = (0..SAMPLES as u32)
        .flat_map(|i| {
            let k = i * 2;
            [k, k + 1, k + 2, k + 1, k + 3, k + 2]
        })
        .collect();
    (geometry, center)
}

/// A round soft shade on the floor, `radius` meters across, taking away up to `shade` of
/// its light.
fn round_shade(center: Vec2, radius: f32, shade: f32) -> FloorSpot {
    FloorSpot {
        center,
        along: Vec2::X,
        half: Vec2::splat(radius),
        light: Vec3::ZERO,
        shade,
        shape: SpotShape::Round,
    }
}

/// The shade on the floor along the walls, darkest where it meets them.
fn wall_shade() -> Vec<FloorSpot> {
    let (hx, hz) = (HALL_X, HALL_Z);
    [
        (Vec2::new(0.0, -hz), Vec2::X, hx),
        (Vec2::new(0.0, hz), Vec2::X, hx),
        (Vec2::new(-hx, 0.0), Vec2::Y, hz),
        (Vec2::new(hx, 0.0), Vec2::Y, hz),
    ]
    .map(|(center, along, length)| FloorSpot {
        center,
        along,
        // Half of it is inside the wall.
        half: Vec2::new(length, 5.0),
        light: Vec3::ZERO,
        shade: 0.6,
        shape: SpotShape::Soft(Vec2::new(0.01, 1.0)),
    })
    .into()
}

/// One gate's meshes, recolored to show which gate is next.
struct GateMeshes {
    /// Where the frame's bars are, as transforms of a unit cube.
    bars: Vec<Mat4>,
    halos: Vec<Node>,
    pool: Node,
    start: bool,
    base: Vec3,
}

/// The propellers' places on the airframe, and which way each spins. Diagonal propellers
/// spin the same way, so the drone doesn't yaw.
fn propeller_places() -> [(Vec3, f32); 4] {
    [(1.0, -1.0), (-1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .map(|(sx, sz)| (Vec3::new(sx * DRONE_ARM, 0.1, sz * DRONE_ARM), sx * sz))
}

/// How far the motors are from the drone's center, sideways and lengthwise.
const DRONE_ARM: f32 = 0.42 * 0.72;
const PROPELLER_RADIUS: f32 = 0.24;
/// How wide a drone is with its propellers, in meters.
const DRONE_SPAN: f32 = 2.0 * (DRONE_ARM + PROPELLER_RADIUS) * DRONE_SCALE;

/// All drones, with each part drawn once for all of them.
struct Drones {
    /// The carbon frame with the camera and the battery, and the motors.
    bodies: [Node; 2],
    /// The parts in each drone's team color: the motor bells and the battery strap.
    anodized: Node,
    /// The lights on the airframe and their colors, times the team color where set.
    lights: Vec<(Mat4, Vec3, bool)>,
    propellers: Node,
    /// The soft shadows below the drones.
    blobs: Node,
    /// The angles of each drone's propellers.
    angles: Vec<[f32; 4]>,
}

impl Drones {
    /// Places a drone for each of `flights`, in its team color, and adds its lights to
    /// `glows`. The first is the player's, whose propellers show only with
    /// `player_propellers`.
    fn place(
        &mut self,
        scene: &mut Scene,
        flights: &[(Flight, Vec3)],
        spin: bool,
        player_propellers: bool,
        dt: f32,
        glows: &mut Vec<Instance>,
    ) {
        let (mut bodies, mut anodized, mut propellers, mut blobs) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        self.angles.resize(flights.len(), [0.0; 4]);
        for (index, (&(flight, team), angles)) in flights.iter().zip(&mut self.angles).enumerate() {
            // Bank into turns and lean the nose down with speed.
            let attitude = Quat::from_rotation_y(flight.yaw)
                * Quat::from_rotation_z(-flight.roll * 0.5)
                * Quat::from_rotation_x(-flight.speed / BOOST_SPEED * 0.35);
            let airframe = Mat4::from_scale_rotation_translation(
                Vec3::splat(DRONE_SCALE),
                attitude,
                flight.position,
            );
            bodies.push(Instance { transform: airframe, color: Vec4::ONE });
            anodized.push(Instance { transform: airframe, color: team.extend(1.0) });
            glows.extend(self.lights.iter().map(|&(place, color, tinted)| Instance {
                transform: airframe * place,
                color: (if tinted { color * team } else { color }).extend(1.0),
            }));
            for ((place, direction), angle) in propeller_places().into_iter().zip(angles) {
                if spin {
                    *angle += direction * (40.0 + flight.speed * 3.0) * dt;
                }
                if index > 0 || player_propellers {
                    let rotation =
                        Quat::from_rotation_x(-FRAC_PI_2) * Quat::from_rotation_z(*angle);
                    propellers.push(Instance {
                        transform: airframe * Mat4::from_rotation_translation(rotation, place),
                        color: Vec4::ONE,
                    });
                }
            }
            // A soft shadow that grows and fades with height.
            let height = flight.position.y;
            let size = 1.2 + height * 0.18;
            blobs.push(Instance {
                transform: Mat4::from_scale_rotation_translation(
                    Vec3::new(size, 1.0, size),
                    Quat::IDENTITY,
                    flight.position.with_y(0.035),
                ),
                color: Vec3::ONE.extend((0.75 - height * 0.05).clamp(0.08, 0.75)),
            });
        }
        for node in self.bodies {
            scene.object(node).instances = Some(bodies.clone());
        }
        scene.object(self.anodized).instances = Some(anodized);
        scene.object(self.propellers).instances = Some(propellers);
        scene.object(self.blobs).instances = Some(blobs);
    }
}

/// Where a drone is and how it flies, for drawing it.
#[derive(Clone, Copy)]
struct Flight {
    position: Vec3,
    yaw: f32,
    roll: f32,
    speed: f32,
}

/// What colors one of the hall's glowing boxes, from the look's palette.
#[derive(Clone, Copy)]
enum HallLight {
    CeilingBar,
    ShaftFixture,
    FloorEdge,
    Screen,
}

/// The 3D scene: the hall, the current course, and the drone.
struct World {
    scene: Scene,
    camera: Camera,
    floor_spot: Rc<Mesh>,
    halo: Rc<Mesh>,
    halo_texture: Rc<Texture>,
    radial_texture: Rc<Texture>,
    route: Material,
    tower: Material,
    tower_pool: Material,
    /// The glowing boxes: the hall's lights and screens, the gates' frames, and the drones'
    /// lights.
    glows: Node,
    /// The hall's glowing boxes, to color them for the look into `hall_glows`.
    hall_lights: Vec<(Mat4, HallLight)>,
    hall_glows: Vec<Instance>,
    /// The steel boxes: the roof trusses and the screens' frames, and the gates' posts.
    steel_boxes: Node,
    hall_steel: Vec<Instance>,
    shafts: Vec<Node>,
    shaft_pools: Vec<Node>,
    high_quality_only: Vec<Node>,
    floor: Node,
    ceiling: Node,
    floor_light: LightMap,
    /// The spots in `floor_light`, to draw it again when they change.
    baked_spots: Vec<FloorSpot>,
    /// The shade on the floor along the walls, and below the gate posts and the towers.
    shade_spots: Vec<FloorSpot>,
    course: Node,
    gates: Vec<GateMeshes>,
    route_mesh: Option<Node>,
    /// The LED towers and the launch pads.
    towers: Node,
    pads: Node,
    tower_pools: Vec<Node>,
    drones: Drones,
    trail: Rc<Mesh>,
    trail_node: Node,
    trail_geometry: Geometry,
    trail_points: Vec<Vec3>,
    director: director::Director,
    time: f32,
    look: Option<usize>,
    high_quality: Option<bool>,
    switches: Switches,
}

/// Debug switches to compare the low-quality savings on a device.
#[derive(Clone, Copy)]
struct Switches {
    /// `DRONE_ALL_HALOS`: every gate keeps its halos in low quality.
    all_halos: bool,
    /// `DRONE_SHAFTS`: low quality keeps three light shafts.
    shafts: bool,
    /// `DRONE_NO_NEAR_FADE`: halos don't fade out near the camera.
    no_near_fade: bool,
    /// `DRONE_BLEND_POOLS`: low quality blends the pools of light on the floor.
    blend_pools: bool,
}

impl Switches {
    fn from_env() -> Self {
        let set = |name| std::env::var_os(name).is_some();
        Self {
            all_halos: set("DRONE_ALL_HALOS"),
            shafts: set("DRONE_SHAFTS"),
            no_near_fade: set("DRONE_NO_NEAR_FADE"),
            blend_pools: set("DRONE_BLEND_POOLS"),
        }
    }
}

impl World {
    /// The hall: floor, walls, roof trusses with light bars, light shafts, wall screens, and
    /// lights.
    fn new(renderer: &Renderer) -> Self {
        let mut scene = Scene::default();

        let floor_texture = renderer.create_texture(&textures::floor());
        let wall_texture = renderer.create_texture(&textures::wall());
        let repeated =
            |material: Material, x: f32, y: f32| Material { uv_scale: Vec2::new(x, y), ..material };
        let floor_light = renderer.create_light_map(FLOOR_SIZE, FLOOR_LIGHT_TEXELS_PER_METER);
        let floor = Material {
            light_map: Some(floor_light.texture.clone()),
            ..repeated(standard(Vec3::ONE, 0.32, 0.15, Some(floor_texture)), 16.0, 9.0)
        };
        let wall = standard(Vec3::ONE, 0.8, 0.2, Some(wall_texture));
        let ceiling = standard(hex(0x0c0e11, 1.0), 1.0, 0.0, None);
        let steel = standard(hex(0x3c434d, 1.0), 0.45, 0.6, None);

        let (hx, hz, h) = (HALL_X, HALL_Z, HALL_HEIGHT);
        let flat = Quat::from_rotation_x(-FRAC_PI_2);
        let hall_floor = renderer.create_mesh(&geometry::plane(FLOOR_SIZE.x, FLOOR_SIZE.y));
        let floor = scene.add_mesh(None, &hall_floor, &floor, Vec3::ZERO, flat, Vec3::ONE);
        let ceiling = scene.add_mesh(
            None,
            &hall_floor,
            &ceiling,
            Vec3::new(0.0, h, 0.0),
            Quat::from_rotation_x(FRAC_PI_2),
            Vec3::ONE,
        );
        // The walls' textures repeat 10 times along the long walls and 5.6 times along the
        // short ones. The walls are brightest where they meet the floor and darken towards the
        // roof, through rows of vertex colors.
        let wall_rows = [(0.0, 3.5), (2.5, 2.0), (8.0, 0.85), (h, 0.3)];
        let wall_plane = |width: f32, repeat: f32| {
            let mut plane = Geometry::default();
            for (height, brightness) in wall_rows {
                for x in [-width / 2.0, width / 2.0] {
                    plane.positions.push([x, height - h / 2.0, 0.0]);
                    plane.uvs.push([(x / width + 0.5) * repeat, height / h * 2.0]);
                    plane.colors.push([brightness; 3]);
                }
            }
            plane.normals = vec![[0.0, 0.0, 1.0]; plane.positions.len()];
            // Counter-clockwise when seen from the front, like `geometry::plane`.
            plane.indices = (0..wall_rows.len() as u32 - 1)
                .flat_map(|row| {
                    let (low, high) = (row * 2, row * 2 + 2);
                    [high, low, high + 1, low, low + 1, high + 1]
                })
                .collect();
            plane
        };
        let (long_wall, short_wall) = (wall_plane(hx * 2.0, 10.0), wall_plane(hz * 2.0, 5.6));
        let walls = geometry::merge(
            [
                (Vec3::Z, Vec3::new(0.0, h / 2.0, -hz), &long_wall),
                (Vec3::NEG_Z, Vec3::new(0.0, h / 2.0, hz), &long_wall),
                (Vec3::X, Vec3::new(-hx, h / 2.0, 0.0), &short_wall),
                (Vec3::NEG_X, Vec3::new(hx, h / 2.0, 0.0), &short_wall),
            ]
            .map(|(normal, center, plane)| {
                // Planes face +Z before rotating, so their texture runs along the wall.
                let rotation = Quat::from_rotation_arc(Vec3::Z, normal);
                (plane, (center, rotation, Vec3::ONE), Vec3::ONE)
            }),
        );
        let walls = renderer.create_mesh(&walls);
        scene.add_mesh(None, &walls, &wall, Vec3::ZERO, Quat::IDENTITY, Vec3::ONE);

        // Roof trusses, the rows of light bars between them, and lines along the floor's edge.
        let unit_cube =
            renderer.create_mesh(&geometry::boxes(&[(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE)]).0);
        let beam = |translation: Vec3, scale: Vec3| {
            Mat4::from_scale_rotation_translation(scale, Quat::IDENTITY, translation)
        };
        let steel_box = |transform: Mat4| Instance { transform, color: Vec4::ONE };
        let mut hall_steel = Vec::new();
        for z in (-32..=32).step_by(8) {
            let truss = beam(Vec3::new(0.0, 20.5, z as f32), Vec3::new(hx * 2.0, 0.9, 0.35));
            hall_steel.push(steel_box(truss));
        }
        for x in (-60..=60).step_by(10) {
            let truss = beam(Vec3::new(x as f32, 21.3, 0.0), Vec3::new(0.35, 0.6, hz * 2.0));
            hall_steel.push(steel_box(truss));
        }
        let mut hall_lights = Vec::new();
        for x in (-55..=55).step_by(10) {
            for z in (-28..=28).step_by(8) {
                let bar = beam(Vec3::new(x as f32, 19.9, z as f32), Vec3::new(6.0, 0.12, 0.35));
                hall_lights.push((bar, HallLight::CeilingBar));
            }
        }
        for line in [
            beam(Vec3::new(0.0, 0.03, -hz + 1.0), Vec3::new(hx * 2.0 - 2.0, 0.05, 0.12)),
            beam(Vec3::new(0.0, 0.03, hz - 1.0), Vec3::new(hx * 2.0 - 2.0, 0.05, 0.12)),
            beam(Vec3::new(-hx + 1.0, 0.03, 0.0), Vec3::new(0.12, 0.05, hz * 2.0 - 2.0)),
            beam(Vec3::new(hx - 1.0, 0.03, 0.0), Vec3::new(0.12, 0.05, hz * 2.0 - 2.0)),
        ] {
            hall_lights.push((line, HallLight::FloorEdge));
        }

        // Light shafts from the roof, with pools of light where they land. Low quality keeps
        // three pools and no shafts; the fixtures show where the light comes from.
        let switches = Switches::from_env();
        let radial_texture = renderer.create_texture(&textures::radial());
        let shaft = additive(&renderer.create_texture(&textures::shaft()), hex(0x2b3644, 1.0));
        let shaft_pool = additive(&radial_texture, hex(0x38414d, 1.0));
        let cone = renderer.create_mesh(&geometry::open_cone(0.5, 5.5, 19.6, 24));
        let floor_spot = renderer.create_mesh(&geometry::floor_spot());
        let mut shafts = Vec::new();
        let mut shaft_pools = Vec::new();
        let mut high_quality_only = Vec::new();
        let shaft_spots = [
            (-30.0, -12.0),
            (30.0, 12.0),
            (0.0, 0.0),
            (-34.0, 16.0),
            (34.0, -16.0),
            (-8.0, -24.0),
            (14.0, 22.0),
        ];
        for (index, &(x, z)) in shaft_spots.iter().enumerate() {
            let shaft_mesh = scene.add_mesh(
                None,
                &cone,
                &shaft,
                Vec3::new(x, 19.7, z),
                Quat::IDENTITY,
                Vec3::ONE,
            );
            let pool_mesh = scene.add_mesh(
                None,
                &floor_spot,
                &shaft_pool,
                Vec3::new(x, 0.02, z),
                Quat::IDENTITY,
                Vec3::new(14.0, 1.0, 14.0),
            );
            if index >= LOW_QUALITY_POOLS || !switches.shafts {
                high_quality_only.push(shaft_mesh);
            }
            if index >= LOW_QUALITY_POOLS {
                high_quality_only.push(pool_mesh);
            }
            shafts.push(shaft_mesh);
            shaft_pools.push(pool_mesh);
        }
        for &(x, z) in &shaft_spots {
            let fixture = beam(Vec3::new(x, 19.75, z), Vec3::new(1.2, 0.15, 1.2));
            hall_lights.push((fixture, HallLight::ShaftFixture));
        }
        // Spot lights under the first five shafts, as many as the shader has room for.
        scene.lights.spots = shaft_spots[..5]
            .iter()
            .map(|&(x, z)| SpotLight {
                position: Vec3::new(x, 20.0, z),
                target: Vec3::new(x, 0.0, z),
                color: hex(0xcfd9e6, 1.0),
                intensity: 3.2 * PI,
                distance: 46.0,
                angle: 0.55,
                penumbra: 0.7,
            })
            .collect();

        // Two big screens on the walls with a dimmed Slint logo, like signage in a venue.
        let logo_material = Material {
            texture: Some(renderer.create_texture(&textures::logo())),
            blend: Blend::Alpha,
            ..Material::unlit(hex(0xffffff, 0.45))
        };
        // The logo's aspect ratio, 423 x 126.
        let logo_mesh = renderer.create_mesh(&geometry::plane(12.0, 12.0 * 126.0 / 423.0));
        let logos = scene.add_instanced(&logo_mesh, &logo_material);
        for (position, rotation) in [
            (Vec3::new(0.0, 12.0, -hz + 0.2), Quat::IDENTITY),
            (Vec3::new(-hx + 0.2, 12.0, 0.0), Quat::from_rotation_y(FRAC_PI_2)),
        ] {
            let facing = rotation * Vec3::Z;
            let place = |offset: f32, scale: Vec3| {
                Mat4::from_scale_rotation_translation(scale, rotation, position + facing * offset)
            };
            hall_steel.push(steel_box(place(-0.15, Vec3::new(20.6, 10.6, 0.3))));
            hall_lights.push((place(0.01, Vec3::new(20.0, 10.0, 0.004)), HallLight::Screen));
            let logo = Instance { transform: place(0.02, Vec3::ONE), color: Vec4::ONE };
            scene.object(logos).instances.get_or_insert_default().push(logo);
        }

        scene.background = hex(BACKDROP, 1.0);
        scene.lights.sky = hex(0x8fa3b8, 1.0);
        // Light bouncing off the floor, so the trusses and the undersides of things show.
        scene.lights.ground = hex(0x2a3038, 1.0);
        scene.lights.sun_direction = Vec3::new(0.3, 1.0, 0.2);
        scene.lights.sun_color = hex(0xdfe8f2, 1.0);

        let course = scene.group(None);

        let halo_texture = renderer.create_texture(&textures::halo());
        let route = additive(&renderer.create_texture(&textures::dash()), hex(0x2c4552, 1.0));
        let tower = Material {
            texture: Some(renderer.create_texture(&textures::stripes())),
            ..Material::unlit(hex(0xffb347, 1.2))
        };
        let tower_pool = additive(&radial_texture, hex(0xffb347, 0.35));
        let pad = standard(Vec3::ONE, 0.6, 0.0, Some(renderer.create_texture(&textures::pad())));
        let glows = scene.add_instanced(&unit_cube, &Material::unlit(Vec3::ONE));
        let steel_boxes = scene.add_instanced(&unit_cube, &steel);
        let towers = scene.add_instanced(
            &renderer.create_mesh(&geometry::cylinder(0.55, 0.55, 14.0, 16)),
            &tower,
        );
        let pads =
            scene.add_instanced(&renderer.create_mesh(&geometry::plane(PAD_SIZE, PAD_SIZE)), &pad);

        let drones = Self::drones(&mut scene, renderer, &floor_spot, &radial_texture);

        let trail_geometry = Self::trail_geometry();
        let trail = renderer.create_mesh(&trail_geometry);
        let trail_material = Material {
            blend: Blend::Additive,
            depth_write: false,
            double_sided: true,
            ..Material::unlit(Vec3::ONE)
        };
        let trail_node = scene.add(Object::new(None, Some(trail.clone()), trail_material));

        let camera = Camera {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            fov_degrees: 65.0,
            aspect: 1.6,
            // As far out as the onboard camera allows, for the 16-bit depth of some GPUs.
            near: 0.25,
            far: 400.0,
        };

        Self {
            scene,
            camera,
            floor_spot,
            halo: renderer.create_mesh(&geometry::square_ring(
                textures::HALO_OUTLINE - textures::HALO_REACH,
                textures::HALO_OUTLINE + textures::HALO_REACH,
            )),
            halo_texture,
            radial_texture,
            route,
            tower,
            tower_pool,
            glows,
            hall_lights,
            hall_glows: Vec::new(),
            steel_boxes,
            hall_steel,
            shafts,
            shaft_pools,
            high_quality_only,
            floor,
            ceiling,
            floor_light,
            baked_spots: Vec::new(),
            shade_spots: Vec::new(),
            course,
            gates: Vec::new(),
            route_mesh: None,
            towers,
            pads,
            tower_pools: Vec::new(),
            drones,
            trail,
            trail_node,
            trail_geometry,
            trail_points: vec![Vec3::ZERO; TRAIL_POINTS],
            director: Default::default(),
            time: 0.0,
            look: None,
            high_quality: None,
            switches,
        }
    }

    /// The racing drones, about 0.9 m across including propellers, with their noses towards
    /// -Z, and their shadows. They're built like FPV racers: a carbon frame with a camera in
    /// front, a battery strapped on top, and motors with anodized bells.
    fn drones(
        scene: &mut Scene,
        renderer: &Renderer,
        floor_spot: &Rc<Mesh>,
        radial_texture: &Rc<Texture>,
    ) -> Drones {
        // The parts' colors are in their vertices, so each material is one mesh.
        let carbon = standard(Vec3::ONE, 0.45, 0.35, None);
        let metal = standard(Vec3::ONE, 0.25, 0.95, None);
        let anodized_metal = standard(Vec3::ONE, 0.3, 0.8, None);
        let white_led = Vec3::new(2.2, 2.3, 2.5);
        let red_led = Vec3::new(2.5, 0.15, 0.1);

        let (cube, _) = geometry::boxes(&[(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE)]);
        let cylinder = geometry::cylinder(1.0, 1.0, 1.0, 16);
        let flat = Quat::IDENTITY;
        let round = |radius: f32, height: f32| Vec3::new(radius, height, radius);
        // Tilted up, as FPV pilots fly fast and nose down.
        let camera_tilt = Quat::from_rotation_x(0.35);
        let lens_axis = camera_tilt * Quat::from_rotation_x(-FRAC_PI_2);

        let mut frame = vec![
            (&cube, Vec3::new(0.0, 0.02, 0.0), flat, Vec3::new(0.15, 0.016, 0.3), 0x3a3f47),
            (&cube, Vec3::new(0.0, 0.13, 0.0), flat, Vec3::new(0.13, 0.012, 0.21), 0x3a3f47),
            (&cube, Vec3::new(0.0, 0.075, -0.12), camera_tilt, Vec3::splat(0.07), 0x16181c),
            (&cylinder, Vec3::new(0.0, 0.09, -0.162), lens_axis, round(0.024, 0.03), 0x050608),
            (&cube, Vec3::new(0.0, 0.18, 0.015), flat, Vec3::new(0.095, 0.07, 0.2), 0x25282e),
            (
                &cylinder,
                Vec3::new(0.0, 0.198, 0.16),
                Quat::from_rotation_x(0.6),
                round(0.006, 0.14),
                0x101215,
            ),
        ];
        for angle in [FRAC_PI_4, -FRAC_PI_4] {
            let arm = Quat::from_rotation_y(angle);
            frame.push((
                &cube,
                Vec3::new(0.0, 0.02, 0.0),
                arm,
                Vec3::new(0.055, 0.016, 0.86),
                0x3a3f47,
            ));
        }
        let mut metal_parts = Vec::new();
        let mut anodized_parts = vec![(
            &cube,
            Vec3::new(0.0, 0.18, 0.015),
            flat,
            Vec3::new(0.1, 0.075, 0.025),
            0xcccccc,
        )];
        for (x, z) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
            let standoff = Vec3::new(x * 0.05, 0.078, z * 0.08);
            metal_parts.push((&cylinder, standoff, flat, round(0.006, 0.1), 0x8a9099));
        }
        let mut lights = vec![(
            (Vec3::new(0.0, 0.03, 0.155), flat, Vec3::new(0.05, 0.012, 0.012)),
            red_led,
            false,
        )];
        lights.push((
            (Vec3::new(0.0, 0.14, 0.095), flat, Vec3::new(0.03, 0.008, 0.03)),
            Vec3::splat(1.5),
            true,
        ));
        for (place, _) in propeller_places() {
            let (x, z) = (place.x, place.z);
            frame.push((&cylinder, Vec3::new(x, 0.02, z), flat, round(0.055, 0.016), 0x3a3f47));
            metal_parts.push((
                &cylinder,
                Vec3::new(x, 0.043, z),
                flat,
                round(0.042, 0.03),
                0x4a5059,
            ));
            metal_parts.push((
                &cylinder,
                Vec3::new(x, 0.108, z),
                flat,
                round(0.013, 0.02),
                0xc0c6cf,
            ));
            anodized_parts.push((
                &cylinder,
                Vec3::new(x, 0.072, z),
                flat,
                round(0.048, 0.035),
                0xcccccc,
            ));
            // LED strips under the arms: white at the front, team color at the back.
            let strip = (
                Vec3::new(x * 0.6, 0.006, z * 0.6),
                Quat::from_rotation_y(f32::atan2(x, z)),
                Vec3::new(0.025, 0.008, 0.22),
            );
            if z < 0.0 {
                lights.push((strip, white_led, false));
            } else {
                lights.push((strip, Vec3::splat(1.6), true));
            }
        }

        let mesh = |parts: Vec<(&Geometry, Vec3, Quat, Vec3, u32)>| {
            renderer.create_mesh(&geometry::merge(parts.into_iter().map(
                |(part, translation, rotation, scale, color)| {
                    (part, (translation, rotation, scale), hex(color, 1.0))
                },
            )))
        };
        let bodies = [
            scene.add_instanced(&mesh(frame), &carbon),
            scene.add_instanced(&mesh(metal_parts), &metal),
        ];
        let anodized = scene.add_instanced(&mesh(anodized_parts), &anodized_metal);
        let lights = lights
            .into_iter()
            .map(|((translation, rotation, scale), color, tinted)| {
                (Mat4::from_scale_rotation_translation(scale, rotation, translation), color, tinted)
            })
            .collect();
        let propeller = Material {
            texture: Some(renderer.create_texture(&textures::propeller())),
            blend: Blend::Alpha,
            depth_write: false,
            double_sided: true,
            ..Material::unlit(Vec3::ONE)
        };
        let propellers = scene.add_instanced(
            &renderer.create_mesh(&geometry::circle(PROPELLER_RADIUS, 24)),
            &propeller,
        );
        let blob = Material {
            texture: Some(radial_texture.clone()),
            blend: Blend::Alpha,
            depth_write: false,
            ..Material::unlit(Vec3::ZERO)
        };
        let blobs = scene.add_instanced(floor_spot, &blob);
        scene.object(propellers).render_order = 2;
        Drones { bodies, anodized, lights, propellers, blobs, angles: Vec::new() }
    }

    fn trail_geometry() -> Geometry {
        let blue = hex(SLINT_BLUE, 1.0);
        let colors = (0..TRAIL_POINTS)
            .flat_map(|k| {
                let fade = (1.0 - k as f32 / TRAIL_POINTS as f32).powf(1.6) * 0.35;
                let c = (blue * fade).to_array();
                [c, c]
            })
            .collect();
        let indices = (0..TRAIL_POINTS as u32 - 1)
            .flat_map(|k| {
                let a = k * 2;
                [a, a + 1, a + 2, a + 1, a + 3, a + 2]
            })
            .collect();
        Geometry {
            positions: vec![[0.0; 3]; TRAIL_POINTS * 2],
            colors,
            indices,
            ..Default::default()
        }
    }

    fn add(
        &mut self,
        mesh: &Rc<Mesh>,
        material: &Material,
        translation: Vec3,
        rotation: Quat,
        scale: Vec3,
    ) -> Node {
        self.scene.add_mesh(Some(self.course), mesh, material, translation, rotation, scale)
    }

    /// Replaces the gates with halos and light pools, the LED towers, the route line, and the
    /// launch pad.
    fn show_course(&mut self, renderer: &Renderer, course: &Course) {
        self.scene.clear(self.course);
        self.gates.clear();
        self.tower_pools.clear();
        let colors = palette(self.look.unwrap_or(0));
        let mut steel = self.hall_steel.clone();
        let mut shade = wall_shade();
        for gate in &course.gates {
            let base = hex(if gate.start { colors.start_gate } else { colors.gate }, 1.0);
            let halo_material = additive(&self.halo_texture, base);
            let pool_material = additive(&self.radial_texture, base);
            let rotation = Quat::from_rotation_y(gate.yaw());
            let w = gate.half;
            let mut frame_bars = Vec::new();
            let mut halos = Vec::new();
            for height in gate.frame_heights() {
                for (offset, y, size_x, size_y) in [
                    (0.0, height + w, 2.0 * w + 0.36, 0.18),
                    (0.0, height - w, 2.0 * w + 0.36, 0.18),
                    (-w, height, 0.18, 2.0 * w),
                    (w, height, 0.18, 2.0 * w),
                ] {
                    let position = (gate.center + gate.right * offset).with_y(y);
                    frame_bars.push(Mat4::from_scale_rotation_translation(
                        Vec3::new(size_x, size_y, 0.18),
                        rotation,
                        position,
                    ));
                }
                let halo = self.halo.clone();
                halos.push(self.add(
                    &halo,
                    &halo_material,
                    gate.center.with_y(height),
                    rotation,
                    Vec3::new(w * 4.4, w * 4.4, 1.0),
                ));
            }
            let bottom = gate.center.y - w;
            if bottom > 0.2 {
                for side in [-1.0, 1.0] {
                    let position =
                        (gate.center + gate.right * side * (w + 0.25)).with_y(bottom / 2.0);
                    shade.push(round_shade(position.xz(), 1.0, 0.55));
                    steel.push(Instance {
                        transform: Mat4::from_scale_rotation_translation(
                            Vec3::new(0.14, bottom, 0.14),
                            rotation,
                            position,
                        ),
                        color: Vec4::ONE,
                    });
                }
            }
            let floor_spot = self.floor_spot.clone();
            let pool = self.add(
                &floor_spot,
                &pool_material,
                gate.center.with_y(0.03),
                Quat::IDENTITY,
                Vec3::new(w * 5.0, 1.0, w * 5.0),
            );
            self.gates.push(GateMeshes { bars: frame_bars, halos, pool, start: gate.start, base });
        }
        self.scene.object(self.steel_boxes).instances = Some(steel);
        let towers = course.towers.iter().map(|position| Instance {
            transform: Mat4::from_translation(*position + Vec3::Y * 7.0),
            color: Vec4::ONE,
        });
        self.scene.object(self.towers).instances = Some(towers.collect());
        shade.extend(course.towers.iter().map(|position| round_shade(position.xz(), 2.4, 0.6)));
        self.shade_spots = shade;
        for position in &course.towers {
            let (floor_spot, tower_pool) = (self.floor_spot.clone(), self.tower_pool.clone());
            let pool = self.add(
                &floor_spot,
                &tower_pool,
                position.with_y(0.03),
                Quat::IDENTITY,
                Vec3::new(7.0, 1.0, 7.0),
            );
            self.tower_pools.push(pool);
        }

        let (route_geometry, center) = route_ribbon(course);
        let route_mesh = renderer.create_mesh(&route_geometry);
        let route = self.route.clone();
        self.route_mesh = Some(self.add(&route_mesh, &route, center, Quat::IDENTITY, Vec3::ONE));

        let rotation =
            Quat::from_rotation_y(yaw_towards(course.tangents[launch_index(course)]) + PI)
                * Quat::from_rotation_x(-FRAC_PI_2);
        let pads = (0..=rivals::PILOTS.len()).map(|slot| {
            let (position, _) = rivals::grid_slot(course, slot);
            Instance {
                // Below the light pools, so they light it.
                transform: Mat4::from_rotation_translation(rotation, position.with_y(0.015)),
                color: Vec4::ONE,
            }
        });
        self.scene.object(self.pads).instances = Some(pads.collect());
        // Makes `apply_look` color the new course.
        self.look = None;
    }

    /// Recolors the hall's lights and the gates when the look changes.
    fn apply_look(&mut self, look: usize) {
        if self.look == Some(look) {
            return;
        }
        self.look = Some(look);
        let colors = palette(look);
        let scene = &mut self.scene;
        let mut recolor = |nodes: &[Node], color: Vec3| {
            for &node in nodes {
                scene.object(node).material.color = color;
            }
        };
        self.hall_glows = (self.hall_lights.iter())
            .map(|&(transform, light)| {
                let color = match light {
                    HallLight::CeilingBar => hex(colors.ceiling, 0.85),
                    HallLight::ShaftFixture => hex(colors.ceiling, 1.0),
                    HallLight::FloorEdge => hex(colors.perimeter, 1.0),
                    HallLight::Screen => hex(0x0a1018, 1.0),
                };
                Instance { transform, color: color.extend(1.0) }
            })
            .collect();
        recolor(&self.shafts, hex(colors.shaft, 1.4));
        recolor(&self.shaft_pools, hex(colors.shaft_pool, 1.0));
        self.route.color = hex(colors.route, 1.0);
        self.tower.color = hex(colors.tower, 0.9);
        self.tower_pool.color = hex(colors.tower, 0.25);
        recolor(self.route_mesh.as_slice(), self.route.color);
        recolor(&[self.towers], self.tower.color);
        recolor(&self.tower_pools, self.tower_pool.color);
        for gate in &mut self.gates {
            gate.base = hex(if gate.start { colors.start_gate } else { colors.gate }, 1.0);
        }
    }

    fn apply_quality(&mut self, high: bool) {
        if self.high_quality == Some(high) {
            return;
        }
        self.high_quality = Some(high);
        for &node in &self.high_quality_only {
            self.scene.object(node).visible = high;
        }
        // Low quality samples only the light map on the floor, so the floor takes its
        // texture's base color. Each texture sample on the floor costs about 1.5 ms per frame
        // at 1080p on an i.MX 8M Plus.
        self.scene.object(self.floor).material.color =
            if high { Vec3::ONE } else { hex(textures::FLOOR_BASE, 1.0) };
        // Low quality's ceiling is as dark as the backdrop, which stands in for it.
        self.scene.object(self.ceiling).visible = high;
        // Both qualities share the flat lights; high quality's spot lights add to them.
        self.scene.lights.hemisphere_intensity = 0.9 * PI;
        self.scene.lights.sun_intensity = 0.9 * PI;
    }

    /// Adds the gates' frames to `glows`, showing which gate is next: it glows Slint blue and
    /// brighter than the others, and gates already passed dim. Low quality draws no halos.
    /// Halos fade out near the camera, where they'd cover most of the view.
    fn light_gates(&mut self, race: &Race, high_quality: bool, glows: &mut Vec<Instance>) {
        let camera = self.camera.translation;
        for (index, gate) in self.gates.iter().enumerate() {
            let next = race.phase == Phase::Flying && index == race.next_gate;
            let color = if race.phase != Phase::Flying || index > race.next_gate {
                gate.base
            } else if next {
                hex(SLINT_BLUE, 1.0)
            } else {
                gate.base * 0.3
            };
            // The frame, its halo, and the pool of light below it.
            let (frame, halo, pool) = if next { (1.6, 0.6, 0.5) } else { (1.15, 0.4, 0.35) };
            let tint = (color * frame).extend(1.0);
            glows.extend(gate.bars.iter().map(|&transform| Instance { transform, color: tint }));
            let shown = high_quality || self.switches.all_halos;
            for &node in &gate.halos {
                let object = self.scene.object(node);
                let size = object.scale.x;
                let fade = if self.switches.no_near_fade {
                    1.0
                } else {
                    textures::smoothstep(
                        0.8 * size,
                        1.8 * size,
                        object.translation.distance(camera),
                    )
                };
                object.material.color = color * halo * fade;
                object.visible = shown && fade > 0.0;
            }
            self.scene.object(gate.pool).material.color = color * pool;
        }
    }

    /// Draws the floor's light map: the light below the ceiling bars and the shade along
    /// the walls and below the gate posts and towers, and in low quality the pools of light.
    /// Low quality bakes the pools instead of blending them over the floor, which is the
    /// same with its flat lighting.
    fn bake_floor(&mut self, renderer: &Renderer, high_quality: bool) {
        let bake = !high_quality && !self.switches.blend_pools;
        let nodes = self.shaft_pools[..LOW_QUALITY_POOLS]
            .iter()
            .chain(self.gates.iter().map(|gate| &gate.pool))
            .chain(&self.tower_pools);
        let mut spots = Vec::new();
        for &node in nodes {
            let object = self.scene.object(node);
            object.visible = !bake;
            if bake {
                spots.push(FloorSpot {
                    center: object.translation.xz(),
                    along: Vec2::X,
                    half: Vec2::splat(object.scale.x / 2.0),
                    light: object.material.color,
                    shade: 0.0,
                    shape: SpotShape::Round,
                });
            }
        }
        let ceiling = hex(palette(self.look.unwrap_or(0)).ceiling, 0.12);
        for &(transform, light) in &self.hall_lights {
            if let HallLight::CeilingBar = light {
                spots.push(FloorSpot {
                    center: transform.w_axis.truncate().xz(),
                    along: Vec2::X,
                    half: Vec2::new(5.0, 2.0),
                    light: ceiling,
                    shade: 0.0,
                    shape: SpotShape::Soft(Vec2::new(0.6, 1.0)),
                });
            }
        }
        spots.extend_from_slice(&self.shade_spots);
        if spots != self.baked_spots {
            renderer.draw_light_map(&self.floor_light, &spots);
            self.baked_spots = spots;
        }
    }

    /// Moves the drone, its shadow, the propellers, the trail, and the camera.
    fn update(&mut self, renderer: &Renderer, state: &State, dt: f32, aspect: f32) {
        let race = &state.race;
        self.time += dt;
        self.apply_quality(state.high_quality);
        self.apply_look(state.look);
        if race.phase != Phase::Ready
            && let Some(route) = self.route_mesh
        {
            // Scrolls the dashes of the route line in flight direction.
            self.scene.object(route).material.uv_offset =
                Vec2::new(0.0, -(self.time * 0.6).fract());
        }

        let spin = race.phase != Phase::Ready;
        let player =
            Flight { position: race.position, yaw: race.yaw, roll: race.roll, speed: race.speed };
        let flights: Vec<(Flight, Vec3)> = std::iter::once((player, hex(SLINT_BLUE, 1.0)))
            .chain(race.rivals.iter().map(|rival| {
                let flight = Flight {
                    position: rival.position,
                    yaw: rival.yaw,
                    roll: rival.roll,
                    speed: rival.speed,
                };
                (flight, hex(rival.pilot.color, 1.0))
            }))
            .collect();
        // From the onboard camera, the player's propellers would blur over the view.
        let mut glows = self.hall_glows.clone();
        self.drones.place(&mut self.scene, &flights, spin, !state.fpv, dt, &mut glows);

        let ahead = forward(race.yaw);
        let (eye, rotation, fov) = if state.fpv {
            self.director.cut();
            let rotation = Quat::from_rotation_y(race.yaw)
                * Quat::from_rotation_z(-race.roll * 0.3)
                * Quat::from_rotation_x(-0.06);
            (race.position + ahead * 0.35 + Vec3::Y * 0.2, rotation, 100f32)
        } else {
            let cutting = state.autopilot && race.phase == Phase::Flying;
            let view =
                self.director.view(race, &state.course, cutting, state.camera_shift, aspect, dt);
            (view.eye, view.rotation, view.fov_degrees)
        };
        self.camera.translation = eye;
        self.camera.rotation = rotation;
        // The field of view is vertical, so tall windows widen it to see as much to the sides.
        self.camera.fov_degrees = if aspect < 1.0 {
            (2.0 * ((fov.to_radians() / 2.0).tan() / aspect.max(0.5)).atan())
                .to_degrees()
                .min(100.0)
        } else {
            fov
        };
        self.camera.aspect = aspect;
        self.light_gates(race, state.high_quality, &mut glows);
        self.scene.object(self.glows).instances = Some(glows);
        self.bake_floor(renderer, state.high_quality);

        // While paused, the trail keeps its shape.
        if dt > 0.0 {
            self.update_trail(renderer, race, eye);
        }
    }

    /// A ribbon of light behind the drone that always faces the camera.
    fn update_trail(&mut self, renderer: &Renderer, race: &Race, camera: Vec3) {
        let tail = race.position
            + Quat::from_rotation_y(race.yaw) * Vec3::new(0.0, 0.03, 0.1) * DRONE_SCALE;
        if race.phase != Phase::Flying {
            self.trail_points.fill(tail);
        } else {
            self.trail_points.rotate_right(1);
            self.trail_points[0] = tail;
        }
        let points = &self.trail_points;
        let count = points.len();
        for k in 0..count {
            let point = points[k];
            let along = points[k.saturating_sub(1)] - points[(k + 1).min(count - 1)];
            let along = if along.length_squared() < 1e-8 { Vec3::Z } else { along };
            let side = (camera - point).cross(along).normalize_or_zero()
                * 0.045
                * (1.0 - k as f32 / count as f32);
            // Around the drone, see `geometry::boxes` for why.
            self.trail_geometry.positions[k * 2] = (point - tail + side).to_array();
            self.trail_geometry.positions[k * 2 + 1] = (point - tail - side).to_array();
        }
        self.scene.object(self.trail_node).translation = tail;
        renderer.update_mesh(&self.trail, &self.trail_geometry);
    }
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
                        label: Some("drone course frame"),
                        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        // The renderer writes through an sRGB view, so the texture holds
                        // sRGB-encoded bytes, which Slint shows as they are. Sampling an sRGB
                        // texture instead is several times slower on some embedded GPUs, such
                        // as NXP's Vivante ones.
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
    if std::env::args().any(|arg| arg == "--low") {
        app_window.set_quality_index(0);
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
        move |x, y, boost| update(&|state| (state.stick, state.boost) = (Vec2::new(x, y), boost))
    });
    app_window.on_start({
        let update = update.clone();
        move || update(&|state| state.start_requested = true)
    });
    app_window.on_reset({
        let update = update.clone();
        move || update(&|state| state.reset_requested = true)
    });
    app_window.on_new_course({
        let update = update.clone();
        move || update(&|state| state.new_course_requested = true)
    });
    app_window.on_autopilot_changed({
        let update = update.clone();
        move |on| update(&|state| state.autopilot = on)
    });
    app_window.on_save_score({
        let update = update.clone();
        move |initials| update(&|state| state.save_score(&initials))
    });
    app_window.on_letter_index(|text| {
        let mut chars = text.chars();
        match (chars.next(), chars.next()) {
            (Some(letter), None) if letter.is_ascii_alphabetic() => {
                (letter.to_ascii_uppercase() as u8 - b'A') as i32
            }
            _ => -1,
        }
    });
    app_window.on_assist_changed({
        let update = update.clone();
        move |on| update(&|state| state.assist = on)
    });
    app_window.on_camera_changed({
        let update = update.clone();
        move |fpv| update(&|state| state.fpv = fpv)
    });
    app_window.on_view_changed({
        let update = update.clone();
        move |shift| update(&|state| state.camera_shift = shift)
    });
    app_window.on_look_changed({
        let update = update.clone();
        move |look| update(&|state| state.look = look as usize)
    });
    app_window.on_paused_changed({
        let update = update.clone();
        move |paused| update(&|state| state.paused = paused)
    });
    app_window.on_quality_changed(move |high| update(&|state| state.high_quality = high));
    app_window.invoke_publish_settings();
    let models = HudModels::new(&app_window);
    show_course(&app_window, &state.borrow().course);

    // The renderer, the scene, and Slint's device, once Slint set up rendering.
    let mut graphics: Option<(Renderer, World, wgpu::Device)> = None;
    let mut frames = Frames { textures: Vec::new(), next: 0 };
    let mut last_frame = web_time::Instant::now();
    let mut frame_count = 0u32;
    let mut fps_window_start = web_time::Instant::now();
    let mut idle = false;

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
            world.show_course(&renderer, &state.borrow().course);
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
        if std::mem::take(&mut state.new_course_requested) {
            state.course = course::generate(random_seed());
            state.race = Race::new(&state.course);
            show_course(&app, &state.course);
            world.show_course(renderer, &state.course);
        }
        let dt = if state.paused { 0.0 } else { dt };
        state.fly(dt);
        show_telemetry(&app, &state, &models);

        let window_size = app.window().size();
        let size = (window_size.width.max(1), window_size.height.max(1));
        world.update(renderer, &state, dt, size.0 as f32 / size.1 as f32);
        let texture = frames.texture(device, size.0, size.1);
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(FRAME_VIEW_FORMAT),
            ..Default::default()
        });
        renderer.render(&world.scene, &world.camera, &view, size, state.high_quality);
        if let Ok(image) = texture.try_into() {
            app.set_texture(image);
        }

        if state.animating() {
            if std::mem::take(&mut idle) {
                frame_count = 0;
                fps_window_start = now;
            }
            frame_count += 1;
            let elapsed = fps_window_start.elapsed().as_secs_f32();
            if elapsed >= 0.5 {
                let fps = frame_count as f32 / elapsed;
                app.set_fps_text(format!("{fps:.0} fps · {:.1} ms", 1000.0 / fps.max(1.0)).into());
                frame_count = 0;
                fps_window_start = now;
            }
            app.window().request_redraw();
        } else if !std::mem::replace(&mut idle, true) {
            app.set_fps_text("-- fps".into());
        }
    })?;

    app_window.run()?;
    Ok(())
}
