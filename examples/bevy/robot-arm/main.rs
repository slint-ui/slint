// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore khronos msaa softbox tonemapping

use std::sync::{Arc, Mutex};

use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::light::{
    CascadeShadowConfigBuilder, DirectionalLightShadowMap, GlobalAmbientLight,
    ShadowFilteringMethod,
};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy::render::render_resource::PipelineCache;
use bevy::render::{Render, RenderApp, RenderSystems};
use slint::Model;

#[path = "../slint-hosts-bevy/slint_bevy_adapter.rs"]
mod slint_bevy_adapter;
mod textures;

slint::include_modules!();

// Arm geometry in meters. The shoulder pivot sits on the turntable, every
// segment points along its local +Y axis and pitches around its local Z axis.
const SHOULDER_HEIGHT: f32 = 0.5;
const UPPER_ARM: f32 = 1.0;
const FOREARM: f32 = 0.8;
// From the wrist pivot to the center between the fingers.
const HAND: f32 = 0.19;

const CUBE_SIZE: f32 = 0.14;
const PAD_HEIGHT: f32 = 0.06;
const BACKDROP: Color = Color::srgb(0.106, 0.125, 0.145);
const SLINT_BLUE: Color = Color::srgb_u8(0x23, 0x79, 0xf4);
const PAD_RADIUS: f32 = 1.15;
const PAD_ANGLE: f32 = 55.0;
const HOVER: f32 = 0.35;
// The shoulder pivot's height above the turntable.
const SHOULDER_PIVOT: f32 = SHOULDER_HEIGHT - 0.2;
// Piston mounts on the turntable and, relative to the shoulder pivot, on the upper arm.
// They sit outside the shoulder bracket so the pistons clear the shoulder hub.
const PISTON_BASE: Vec3 = Vec3::new(-0.16, 0.12, 0.0);
const PISTON_ARM: Vec3 = Vec3::new(-0.14, 0.36, 0.0);
const PISTON_Z: f32 = 0.24;
const PISTON_SLEEVE: f32 = 0.3;
const PISTON_ROD: f32 = 0.32;
const FINGER_PAD: f32 = 0.012;
// Gripper opening where the finger pads touch the cube's sides.
const GRIP_CLOSED: f32 = 0.6;

const BASE: usize = 0;
const SHOULDER: usize = 1;
const ELBOW: usize = 2;
const WRIST: usize = 3;
const GRIPPER: usize = 4;
const JOINT_COUNT: usize = 5;

/// Joint angles in radians, except the gripper opening which goes from 0 to 1.
type Pose = [f32; JOINT_COUNT];

const HOME: Pose = [0.0, 0.15, 1.1, 1.0, 0.6];
const REACH: Pose = [0.0, 1.2, 0.35, 0.0, 1.0];
const FOLD: Pose = [-1.4, -0.5, 2.5, 1.4, GRIP_CLOSED];

/// Joint limits.
const LIMITS: [(f32, f32); JOINT_COUNT] = [
    (-170f32.to_radians(), 170f32.to_radians()),
    (-60f32.to_radians(), 100f32.to_radians()),
    (0.0, 150f32.to_radians()),
    (-120f32.to_radians(), 120f32.to_radians()),
    (0.0, 1.0),
];

/// Speed of each joint in radians per second (gripper: opening per second).
const SPEED: Pose = [1.4, 1.1, 1.3, 2.0, 1.5];

/// The state the Slint UI and the Bevy app share.
///
/// Slint writes targets, the camera, and render settings, and sets `animating` whenever it
/// changes one of them. Bevy writes `current` and clears `animating` once the arm is settled
/// and no render pipelines are compiling.
#[derive(Default)]
struct SharedState {
    targets: Pose,
    current: Pose,
    auto_mode: bool,
    animating: bool,
    camera_yaw: f32,
    camera_pitch: f32,
    camera_zoom: f32,
    /// Moves the robot left of the window's center, relative to half the window width.
    camera_shift: f32,
    msaa: Msaa,
    shadows: bool,
}

#[derive(Resource, Clone)]
struct Shared(Arc<Mutex<SharedState>>);

#[derive(Resource)]
struct AppWindowHandle(slint::Weak<AppWindow>);

#[derive(Component)]
struct JointNode(usize);

#[derive(Component)]
struct Finger(f32);

#[derive(Component)]
struct Tip;

#[derive(Component)]
struct Piston {
    z: f32,
    part: PistonPart,
}

#[derive(Clone, Copy)]
enum PistonPart {
    Sleeve,
    Rod,
}

/// The material that all status lights share.
#[derive(Resource)]
struct StatusLight(Handle<StandardMaterial>);

#[derive(Component)]
struct Workpiece;

/// The arm's state for the systems that draw it, changed only when it differs.
#[derive(Resource, Default, PartialEq)]
struct ArmPose {
    pose: Pose,
    auto_mode: bool,
    moving: bool,
}

/// Where the cube is. A resting position is the center of the cube's bottom face.
#[derive(Resource)]
enum CubeState {
    Resting(Vec3),
    Held,
    /// Released by hand: falls onto the floor below where it was held.
    Dropped,
}

/// Progress through the automatic pick-and-place cycle.
#[derive(Resource, Default)]
struct Sequencer {
    steps: Vec<Step>,
    index: usize,
    dwell: f32,
}

#[derive(Clone, Copy)]
struct Step {
    pose: Pose,
    action: Action,
}

#[derive(Clone, Copy)]
enum Action {
    None,
    Grab,
    Release(Vec3),
}

/// The center of a pad's top face.
fn pad_position(angle_degrees: f32) -> Vec3 {
    let angle = angle_degrees.to_radians();
    Vec3::new(PAD_RADIUS * angle.cos(), PAD_HEIGHT, -PAD_RADIUS * angle.sin())
}

fn pads() -> [Vec3; 2] {
    [pad_position(PAD_ANGLE), pad_position(-PAD_ANGLE)]
}

fn other_pad(near: Vec3) -> Vec3 {
    let [a, b] = pads();
    if near.distance(a) < near.distance(b) { b } else { a }
}

/// Converts a joint value to the unit its slider shows: degrees, or percent for the gripper.
fn to_ui(index: usize, value: f32) -> f32 {
    if index == GRIPPER { value * 100.0 } else { value.to_degrees() }
}

fn from_ui(index: usize, value: f32) -> f32 {
    if index == GRIPPER { value / 100.0 } else { value.to_radians() }
}

/// Solves the joint angles that put the gripper center at `target`, pointing down.
fn inverse_kinematics(target: Vec3, gripper: f32) -> Pose {
    let base = (-target.z).atan2(target.x);
    let dx = Vec2::new(target.x, target.z).length();
    let dy = target.y + HAND - SHOULDER_HEIGHT;
    let distance = Vec2::new(dx, dy).length().clamp(0.05, UPPER_ARM + FOREARM - 1e-3);
    let cos_elbow = (distance * distance - UPPER_ARM * UPPER_ARM - FOREARM * FOREARM)
        / (2.0 * UPPER_ARM * FOREARM);
    let elbow = cos_elbow.clamp(-1.0, 1.0).acos();
    let shoulder = dx.atan2(dy) - (FOREARM * elbow.sin()).atan2(UPPER_ARM + FOREARM * elbow.cos());
    let wrist = std::f32::consts::PI - shoulder - elbow;
    [base, shoulder, elbow, wrist, gripper]
}

fn is_reachable(position: Vec3) -> bool {
    let reach = Vec2::new(position.x, position.z).length();
    (0.6..1.6).contains(&reach)
}

/// Builds the steps that pick up the cube at `at` (`Action::Grab`) or put it down there.
fn visit(at: Vec3, action: Action) -> [Step; 4] {
    let (before, after) =
        if matches!(action, Action::Grab) { (1.0, GRIP_CLOSED) } else { (GRIP_CLOSED, 1.0) };
    let pose =
        |lift: f32, gripper| inverse_kinematics(at + Vec3::Y * (CUBE_SIZE / 2.0 + lift), gripper);
    [
        Step { pose: pose(HOVER, before), action: Action::None },
        Step { pose: pose(0.0, before), action: Action::None },
        Step { pose: pose(0.0, after), action },
        Step { pose: pose(HOVER, after), action: Action::None },
    ]
}

/// Builds the steps that carry the cube from `from` to the pad it isn't on.
fn pick_and_place(from: Vec3) -> Vec<Step> {
    let to = other_pad(from);
    [visit(from, Action::Grab), visit(to, Action::Release(to))].concat()
}

/// Builds the steps that put a held cube on the pad the arm isn't facing.
fn place_held(current: &Pose) -> Vec<Step> {
    let facing = Vec3::new(current[BASE].cos(), 0.0, -current[BASE].sin()) * PAD_RADIUS;
    let to = other_pad(facing);
    visit(to, Action::Release(to)).to_vec()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::rc::Rc;

    let shared = Arc::new(Mutex::new(SharedState {
        targets: HOME,
        current: HOME,
        animating: true,
        ..Default::default()
    }));

    let mut wgpu_settings = slint::wgpu_29::WGPUSettings::default();
    wgpu_settings.device_required_limits = slint::wgpu_29::wgpu::Limits::default()
        .using_resolution(slint::wgpu_29::wgpu::Limits::downlevel_defaults());
    slint::BackendSelector::new()
        .require_wgpu_29(slint::wgpu_29::WGPUConfiguration::Automatic(wgpu_settings))
        .select()?;
    let app_window = AppWindow::new()?;

    let joints = Rc::new(slint::VecModel::from_iter(
        ["Base", "Shoulder", "Elbow", "Wrist", "Gripper"].into_iter().enumerate().map(
            |(index, name)| JointData {
                name: name.into(),
                unit: if index == GRIPPER { "%" } else { "°" }.into(),
                minimum: to_ui(index, LIMITS[index].0),
                maximum: to_ui(index, LIMITS[index].1),
                step: if index == GRIPPER { 10.0 } else { 5.0 },
                target: to_ui(index, HOME[index]),
                current: to_ui(index, HOME[index]),
            },
        ),
    ));
    app_window.set_joints(joints.clone().into());

    let update = {
        let shared = shared.clone();
        let app_weak = app_window.as_weak();
        move |change: &dyn Fn(&mut SharedState)| {
            let mut state = shared.lock().unwrap();
            change(&mut state);
            state.animating = true;
            if let Some(app) = app_weak.upgrade() {
                app.window().request_redraw();
            }
        }
    };
    app_window.on_joint_changed({
        let update = update.clone();
        move |index, value| {
            let index = index as usize;
            let (min, max) = LIMITS[index];
            update(&|state| {
                state.auto_mode = false;
                state.targets[index] = from_ui(index, value).clamp(min, max);
            })
        }
    });
    app_window.on_mode_changed({
        let update = update.clone();
        move |auto_mode| {
            update(&|state| {
                state.auto_mode = auto_mode;
                state.targets = state.current;
            })
        }
    });
    app_window.on_preset({
        let update = update.clone();
        move |index| {
            update(&|state| {
                state.auto_mode = false;
                state.targets = [HOME, REACH, FOLD][index as usize];
            })
        }
    });
    app_window.on_view_changed({
        let update = update.clone();
        move |yaw, pitch, zoom, shift| {
            update(&|state| {
                (state.camera_yaw, state.camera_pitch) = (yaw, pitch);
                (state.camera_zoom, state.camera_shift) = (zoom, shift);
            })
        }
    });
    app_window.on_render_settings_changed(move |msaa, shadows| {
        update(&|state| {
            state.msaa = if msaa { Msaa::Sample4 } else { Msaa::Off };
            state.shadows = shadows;
        })
    });
    app_window.invoke_publish_settings();

    let mut bevy_channels = None;
    // Bevy renders ahead into up to three textures, so after the scene settles a few more
    // textures need to arrive until the final one is on screen.
    let mut flush_frames = 0u32;
    let mut frame_count = 0u32;
    let mut fps_window_start = std::time::Instant::now();
    let mut idle = false;
    let mut texture_size = (0u32, 0u32);

    let app_weak = app_window.as_weak();
    app_window.window().set_rendering_notifier(move |state, graphics_api| match state {
        slint::RenderingState::RenderingSetup => {
            let slint::GraphicsAPI::WGPU29 { instance, device, queue, .. } = graphics_api else {
                return;
            };
            let info = device.adapter_info();
            app_weak.unwrap().set_gpu_text(format!("{} ({:?})", info.name, info.backend).into());

            let shared = Shared(shared.clone());
            let app_window_handle = AppWindowHandle(app_weak.clone());
            let channels = slint_bevy_adapter::run_bevy_app_with_slint(
                instance.clone(),
                device.clone(),
                queue.clone(),
                |_| {},
                move |mut app| {
                    app.sub_app_mut(RenderApp)
                        .insert_resource(shared.clone())
                        .insert_resource(app_window_handle)
                        .add_systems(
                            Render,
                            keep_animating_while_compiling.in_set(RenderSystems::Cleanup),
                        );
                    app.insert_resource(shared)
                        .insert_resource(ClearColor(BACKDROP))
                        // The environment map provides the ambient light.
                        .insert_resource(GlobalAmbientLight::NONE)
                        .insert_resource(DirectionalLightShadowMap { size: 1024 })
                        .insert_resource(ArmPose::default())
                        .insert_resource(CubeState::Resting(pads()[0]))
                        .insert_resource(Sequencer::default())
                        .add_systems(Startup, setup)
                        .add_systems(
                            Update,
                            (
                                sync_shared_state,
                                (
                                    apply_pose.run_if(resource_changed::<ArmPose>),
                                    update_camera,
                                    update_cube,
                                    update_status_lights,
                                ),
                            )
                                .chain(),
                        )
                        .run();
                },
            );
            bevy_channels = Some(channels);
        }
        slint::RenderingState::BeforeRendering => {
            let Some(app) = app_weak.upgrade() else { return };
            let Some((new_texture_receiver, control_message_sender)) = &bevy_channels else {
                return;
            };

            let (current, targets, animating) = {
                let state = shared.lock().unwrap();
                (state.current, state.targets, state.animating)
            };
            if animating {
                flush_frames = 4;
                if std::mem::take(&mut idle) {
                    frame_count = 0;
                    fps_window_start = std::time::Instant::now();
                }
            }
            for index in 0..JOINT_COUNT {
                let mut row = joints.row_data(index).unwrap();
                let (target, current) =
                    (to_ui(index, targets[index]), to_ui(index, current[index]));
                if row.target != target || row.current != current {
                    row.target = target;
                    row.current = current;
                    joints.set_row_data(index, row);
                }
            }
            app.set_moving(animating);

            let send = |message| {
                let sender = control_message_sender.clone();
                slint::spawn_local(async move {
                    // A closed channel means Bevy shut down.
                    sender.send(message).await.ok();
                })
                .unwrap();
            };

            let window_size = app.window().size();
            let scale = app.get_render_scale();
            let size = (
                (window_size.width as f32 * scale).round() as u32,
                (window_size.height as f32 * scale).round() as u32,
            );
            if size.0 > 0 && size.1 > 0 && std::mem::replace(&mut texture_size, size) != size {
                send(slint_bevy_adapter::ControlMessage::ResizeBuffers {
                    width: size.0,
                    height: size.1,
                });
            }

            if let Ok(new_texture) = new_texture_receiver.try_recv() {
                if let Some(old_texture) = app.get_texture().to_wgpu_29_texture() {
                    send(slint_bevy_adapter::ControlMessage::ReleaseFrontBufferTexture {
                        texture: old_texture,
                    });
                }
                if let Ok(image) = new_texture.try_into() {
                    app.set_texture(image);
                }
                frame_count += 1;
                flush_frames = flush_frames.saturating_sub(1);
            }

            let elapsed = fps_window_start.elapsed().as_secs_f32();
            if elapsed >= 0.5 && !idle {
                let fps = frame_count as f32 / elapsed;
                app.set_fps_text(format!("{fps:.0} fps · {:.1} ms", 1000.0 / fps.max(1.0)).into());
                frame_count = 0;
                fps_window_start = std::time::Instant::now();
            }

            if flush_frames > 0 {
                app.window().request_redraw();
            } else if !std::mem::replace(&mut idle, true) {
                app.set_fps_text("Idle · 0 fps".into());
            }
        }
        _ => {}
    })?;

    app_window.run()?;
    Ok(())
}

/// Inverts `Tonemapping::KhronosPbrNeutral` for colors in its dark toe, where it subtracts
/// `m - 6.25 m²` from every channel, `m` being the smallest channel.
fn before_neutral_tonemapping(color: Color) -> Color {
    let LinearRgba { red, green, blue, .. } = color.to_linear();
    let target_min = red.min(green).min(blue);
    let m = (target_min / 6.25).sqrt();
    let offset = m - target_min;
    Color::linear_rgb(red + offset, green + offset, blue + offset)
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    use std::f32::consts::FRAC_PI_2;

    let mut paint = |color: Color, roughness: f32, metallic: f32| {
        materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: roughness,
            metallic,
            ..default()
        })
    };
    // A light shell like a collaborative robot, with Slint-blue accents.
    let shell = paint(Color::srgb_u8(0xc9, 0xce, 0xd5), 0.35, 0.0);
    let slint_blue = paint(SLINT_BLUE, 0.35, 0.0);
    let graphite = paint(Color::srgb_u8(0x3c, 0x42, 0x4a), 0.4, 0.3);
    let dark = paint(Color::srgb_u8(0x1a, 0x1e, 0x23), 0.55, 0.3);
    let steel = paint(Color::srgb_u8(0xc3, 0xc9, 0xd1), 0.28, 1.0);
    let rubber = paint(Color::srgb_u8(0x10, 0x11, 0x14), 0.9, 0.0);
    let fixture = paint(Color::srgb_u8(0xa4, 0xab, 0xb4), 0.35, 1.0);

    let mut textured = |image: Image, roughness: f32, metallic: f32, repeat: Vec2| {
        materials.add(StandardMaterial {
            base_color_texture: Some(images.add(image)),
            perceptual_roughness: roughness,
            metallic,
            uv_transform: bevy::math::Affine2::from_scale(repeat),
            ..default()
        })
    };
    // The floor disc is 40 m wide, and one texture tile covers 0.5 m.
    let floor = textured(textures::concrete(), 0.85, 0.0, Vec2::splat(80.0));
    // The annulus maps u across the ring and v around it.
    let hazard = textured(textures::hazard(), 0.7, 0.0, Vec2::new(1.0, 64.0));
    let crate_face = textured(textures::crate_face(), 0.55, 0.0, Vec2::ONE);
    let fixture_top = textured(textures::fixture_top(), 0.4, 1.0, Vec2::ONE);

    let status_light = materials.add(StandardMaterial { base_color: Color::BLACK, ..default() });
    commands.insert_resource(StatusLight(status_light.clone()));

    let part = |mesh: Handle<Mesh>, material: &Handle<StandardMaterial>, transform: Transform| {
        (Mesh3d(mesh), MeshMaterial3d(material.clone()), transform)
    };
    let flat = Quat::from_rotation_x(-FRAC_PI_2);
    // Pivots around the Z axis show their axle as a cylinder lying along Z.
    let axle = Quat::from_rotation_x(FRAC_PI_2);
    let thin_ring = |major: f32, minor: f32| {
        Torus::new(major - minor, major + minor)
            .mesh()
            .minor_resolution(8)
            .major_resolution(48)
            .build()
    };

    // Cell floor, safety zone, and fixtures
    commands.spawn(part(
        meshes.add(Circle::new(20.0).mesh().resolution(64).build()),
        &floor,
        Transform::from_rotation(flat),
    ));
    commands.spawn(part(
        meshes.add(Annulus::new(1.72, 1.78).mesh().resolution(128).build()),
        &hazard,
        Transform::from_xyz(0.0, 0.002, 0.0).with_rotation(flat),
    ));
    let fixture_body = meshes.add(Cuboid::new(0.36, PAD_HEIGHT, 0.36));
    let fixture_face = meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(0.18)));
    for angle in [PAD_ANGLE, -PAD_ANGLE] {
        let top = pad_position(angle);
        // Square fixtures face the robot.
        let facing = Quat::from_rotation_y(angle.to_radians());
        commands.spawn(part(
            fixture_body.clone(),
            &fixture,
            Transform::from_translation(top - Vec3::Y * PAD_HEIGHT / 2.0).with_rotation(facing),
        ));
        commands.spawn(part(
            fixture_face.clone(),
            &fixture_top,
            Transform::from_translation(top + Vec3::Y * 0.001).with_rotation(facing),
        ));
    }

    commands.spawn((
        part(
            meshes.add(Cuboid::from_length(CUBE_SIZE)),
            &crate_face,
            Transform::from_translation(pads()[0] + Vec3::Y * CUBE_SIZE / 2.0),
        ),
        Workpiece,
    ));

    // Fixed base: floor plate with bolts, pedestal, and status light ring
    commands.spawn(part(
        meshes.add(Cylinder::new(0.48, 0.04)),
        &dark,
        Transform::from_xyz(0.0, 0.02, 0.0),
    ));
    let bolt = meshes.add(Cylinder::new(0.022, 0.02).mesh().resolution(10).build());
    for i in 0..8 {
        let (sin, cos) = (i as f32 * std::f32::consts::TAU / 8.0 + 0.4).sin_cos();
        commands.spawn(part(
            bolt.clone(),
            &steel,
            Transform::from_xyz(0.41 * cos, 0.05, 0.41 * sin),
        ));
    }
    commands.spawn(part(
        meshes.add(Cylinder::new(0.32, 0.16)),
        &graphite,
        Transform::from_xyz(0.0, 0.12, 0.0),
    ));
    commands.spawn(part(
        meshes.add(thin_ring(0.323, 0.012)),
        &status_light,
        Transform::from_xyz(0.0, 0.17, 0.0),
    ));

    let turntable = commands
        .spawn((Transform::from_xyz(0.0, 0.2, 0.0), Visibility::default(), JointNode(BASE)))
        .id();
    let shoulder = commands
        .spawn((
            Transform::from_xyz(0.0, SHOULDER_PIVOT, 0.0),
            Visibility::default(),
            JointNode(SHOULDER),
        ))
        .id();
    let elbow = commands
        .spawn((Transform::from_xyz(0.0, UPPER_ARM, 0.0), Visibility::default(), JointNode(ELBOW)))
        .id();
    let wrist = commands
        .spawn((Transform::from_xyz(0.0, FOREARM, 0.0), Visibility::default(), JointNode(WRIST)))
        .id();

    // Turntable: rotating disc, shoulder bracket, motor, and the piston mounts
    let piston_sleeve = meshes.add(Cylinder::new(0.034, PISTON_SLEEVE));
    let piston_rod = meshes.add(Cylinder::new(0.018, PISTON_ROD).mesh().resolution(16).build());
    let bracket = meshes.add(Cuboid::new(0.32, 0.36, 0.05));
    let piston_mount = meshes.add(Cuboid::new(0.08, 0.05, 0.06));
    let hub_cap = meshes.add(Cylinder::new(0.09, 0.02));
    commands.entity(turntable).add_child(shoulder).with_children(|parent| {
        parent.spawn(part(
            meshes.add(Cylinder::new(0.29, 0.1)),
            &slint_blue,
            Transform::from_xyz(0.0, 0.05, 0.0),
        ));
        parent.spawn(part(
            meshes.add(Cylinder::new(0.24, 0.02)),
            &dark,
            Transform::from_xyz(0.0, 0.11, 0.0),
        ));
        for side in [-1.0, 1.0] {
            let z = side * PISTON_Z;
            parent.spawn(part(
                bracket.clone(),
                &shell,
                Transform::from_xyz(0.0, 0.28, side * 0.17),
            ));
            parent.spawn(part(
                hub_cap.clone(),
                &slint_blue,
                Transform::from_xyz(0.0, SHOULDER_PIVOT, side * 0.205).with_rotation(axle),
            ));
            parent.spawn(part(
                piston_mount.clone(),
                &dark,
                Transform::from_translation(PISTON_BASE.with_z(z) - Vec3::Y * 0.02),
            ));
            parent.spawn((
                part(piston_sleeve.clone(), &dark, Transform::default()),
                Piston { z, part: PistonPart::Sleeve },
            ));
            parent.spawn((
                part(piston_rod.clone(), &steel, Transform::default()),
                Piston { z, part: PistonPart::Rod },
            ));
        }
    });

    // Upper arm: hub, tapered link, cable conduit, and the piston clevis
    let conduit =
        |length: f32| Capsule3d::new(0.022, length).mesh().longitudes(12).latitudes(6).build();
    commands.entity(shoulder).add_child(elbow).with_children(|parent| {
        parent.spawn(part(
            meshes.add(Cylinder::new(0.14, 0.29)),
            &graphite,
            Transform::from_rotation(axle),
        ));
        parent.spawn(part(
            meshes.add(ConicalFrustum {
                radius_top: 0.075,
                radius_bottom: 0.11,
                height: UPPER_ARM,
            }),
            &shell,
            Transform::from_xyz(0.0, UPPER_ARM / 2.0, 0.0),
        ));
        parent.spawn(part(
            meshes.add(conduit(0.6)),
            &rubber,
            Transform::from_xyz(-0.105, 0.5, 0.0).with_rotation(Quat::from_rotation_z(0.03)),
        ));
        parent.spawn(part(
            meshes.add(Cuboid::new(0.05, 0.05, 2.0 * PISTON_Z + 0.06)),
            &dark,
            Transform::from_translation(PISTON_ARM),
        ));
    });

    // Forearm: elbow hub with motor, rear counterweight, tapered link, conduit
    commands.entity(elbow).add_child(wrist).with_children(|parent| {
        parent.spawn(part(
            meshes.add(Cylinder::new(0.11, 0.24)),
            &graphite,
            Transform::from_rotation(axle),
        ));
        parent.spawn(part(
            meshes.add(Cylinder::new(0.075, 0.1)),
            &slint_blue,
            Transform::from_xyz(0.0, 0.0, 0.17).with_rotation(axle),
        ));
        parent.spawn(part(
            meshes.add(Capsule3d::new(0.085, 0.12).mesh().longitudes(16).latitudes(8).build()),
            &graphite,
            Transform::from_xyz(0.0, -0.1, 0.0),
        ));
        parent.spawn(part(
            meshes.add(ConicalFrustum { radius_top: 0.055, radius_bottom: 0.08, height: FOREARM }),
            &shell,
            Transform::from_xyz(0.0, FOREARM / 2.0, 0.0),
        ));
        parent.spawn(part(
            meshes.add(conduit(0.45)),
            &rubber,
            Transform::from_xyz(-0.08, 0.42, 0.0).with_rotation(Quat::from_rotation_z(0.03)),
        ));
    });

    // Wrist and gripper
    let finger = meshes.add(Cuboid::new(0.03, 0.16, 0.08));
    let pad = meshes.add(Cuboid::new(FINGER_PAD, 0.09, 0.07));
    let rail = meshes.add(Cuboid::new(0.34, 0.012, 0.016));
    commands.entity(wrist).with_children(|parent| {
        parent.spawn(part(
            meshes.add(Cylinder::new(0.075, 0.18)),
            &graphite,
            Transform::from_rotation(axle),
        ));
        parent.spawn(part(
            meshes.add(Cylinder::new(0.065, 0.03)),
            &steel,
            Transform::from_xyz(0.0, 0.065, 0.0),
        ));
        parent.spawn(part(
            meshes.add(Cuboid::new(0.36, 0.05, 0.1)),
            &dark,
            Transform::from_xyz(0.0, 0.1, 0.0),
        ));
        for z in [-0.032, 0.032] {
            parent.spawn(part(rail.clone(), &steel, Transform::from_xyz(0.0, 0.127, z)));
        }
        parent.spawn(part(
            meshes.add(Sphere::new(0.016).mesh().uv(12, 8)),
            &status_light,
            Transform::from_xyz(0.0, 0.1, 0.05),
        ));
        for side in [-1.0, 1.0] {
            parent
                .spawn((
                    part(finger.clone(), &graphite, Transform::from_xyz(0.0, 0.2, 0.0)),
                    Finger(side),
                ))
                .with_children(|finger| {
                    finger.spawn(part(
                        pad.clone(),
                        &rubber,
                        Transform::from_xyz(-side * (0.015 + FINGER_PAD / 2.0), 0.0, 0.0),
                    ));
                });
        }
        parent.spawn((Transform::from_xyz(0.0, HAND, 0.0), Tip));
    });

    // The key light comes from the studio's main softbox, see `textures::studio`.
    commands.spawn((
        DirectionalLight { illuminance: 4_000.0, shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(0.25, 1.0, 0.35).looking_at(Vec3::ZERO, Vec3::Y),
        // One cascade covering the cell keeps the shadow pass to a single small render.
        CascadeShadowConfigBuilder { num_cascades: 1, maximum_distance: 7.0, ..default() }.build(),
    ));
    commands.spawn((
        Camera3d::default(),
        Msaa::Off,
        // Runs inside the PBR shader for non-HDR cameras, so it needs no extra pass.
        Tonemapping::KhronosPbrNeutral,
        ShadowFilteringMethod::Hardware2x2,
        EnvironmentMapLight {
            diffuse_map: images.add(textures::studio_diffuse()),
            specular_map: images.add(textures::studio_specular()),
            intensity: 3_500.0,
            ..default()
        },
        // Fog fades the floor into the background, which the tone curve doesn't apply to.
        DistanceFog {
            color: before_neutral_tonemapping(BACKDROP),
            falloff: FogFalloff::Linear { start: 7.0, end: 18.0 },
            ..default()
        },
    ));
}

/// Exchanges state with the Slint UI, runs the auto mode, and moves the joints.
fn sync_shared_state(
    shared: Res<Shared>,
    time: Res<Time>,
    mut pose: ResMut<ArmPose>,
    mut sequencer: ResMut<Sequencer>,
    mut cube: ResMut<CubeState>,
    mut cameras: Query<&mut Msaa, With<Camera3d>>,
    mut lights: Query<&mut DirectionalLight>,
) {
    let mut state = shared.0.lock().unwrap();
    // After an idle period the first frame would otherwise jump.
    let dt = time.delta_secs().min(1.0 / 20.0);

    if state.auto_mode {
        if sequencer.steps.is_empty() {
            sequencer.steps = match *cube {
                CubeState::Held => place_held(&state.current),
                CubeState::Resting(position) if is_reachable(position) => pick_and_place(position),
                CubeState::Resting(_) | CubeState::Dropped => {
                    *cube = CubeState::Resting(pads()[0]);
                    pick_and_place(pads()[0])
                }
            };
            sequencer.index = 0;
            sequencer.dwell = 0.0;
        }
        state.targets = sequencer.steps[sequencer.index].pose;
    } else {
        sequencer.steps.clear();
    }

    let remaining: Pose = std::array::from_fn(|i| state.targets[i] - state.current[i]);
    // Move all joints in sync so they arrive together, easing out over the last 150 ms.
    let duration = (0..JOINT_COUNT).map(|i| remaining[i].abs() / SPEED[i]).fold(0.0, f32::max);
    let settled = duration < 1e-3;
    let factor = (dt / duration.max(0.15)).min(1.0);
    state.current = if settled {
        state.targets
    } else {
        std::array::from_fn(|i| state.current[i] + remaining[i] * factor)
    };

    if state.auto_mode && settled {
        sequencer.dwell += dt;
        if sequencer.dwell > 0.25 {
            match sequencer.steps[sequencer.index].action {
                Action::None => {}
                Action::Grab => *cube = CubeState::Held,
                Action::Release(position) => *cube = CubeState::Resting(position),
            }
            sequencer.dwell = 0.0;
            sequencer.index += 1;
            if sequencer.index == sequencer.steps.len() {
                sequencer.steps.clear();
            }
        }
    }

    if !state.auto_mode && state.current[GRIPPER] > 0.75 && matches!(*cube, CubeState::Held) {
        *cube = CubeState::Dropped;
    }

    for mut msaa in &mut cameras {
        msaa.set_if_neq(state.msaa);
    }
    for light in &mut lights {
        light.map_unchanged(|light| &mut light.shadow_maps_enabled).set_if_neq(state.shadows);
    }

    state.animating = state.auto_mode || !settled;
    pose.set_if_neq(ArmPose { pose: state.current, auto_mode: state.auto_mode, moving: !settled });
}

/// Bevy compiles pipelines in the background and skips meshes until they're ready, such as
/// at startup or after switching MSAA.
fn keep_animating_while_compiling(
    shared: Res<Shared>,
    pipeline_cache: Res<PipelineCache>,
    app_window: Res<AppWindowHandle>,
) {
    if pipeline_cache.waiting_pipelines().next().is_some() {
        shared.0.lock().unwrap().animating = true;
        // The UI may have gone idle already.
        app_window.0.upgrade_in_event_loop(|app| app.window().request_redraw()).ok();
    }
}

fn apply_pose(
    pose: Res<ArmPose>,
    mut joints: Query<(&JointNode, &mut Transform), (Without<Finger>, Without<Piston>)>,
    mut fingers: Query<(&Finger, &mut Transform), (Without<JointNode>, Without<Piston>)>,
    mut pistons: Query<(&Piston, &mut Transform), (Without<JointNode>, Without<Finger>)>,
) {
    let pose = &pose.pose;
    let shoulder = Quat::from_rotation_z(-pose[SHOULDER]);
    for (JointNode(index), mut transform) in &mut joints {
        transform.rotation = match *index {
            BASE => Quat::from_rotation_y(pose[BASE]),
            // Positive angles tip the segment forward, towards the turntable's +X.
            index => Quat::from_rotation_z(-pose[index]),
        };
    }
    for (Finger(side), mut transform) in &mut fingers {
        transform.translation.x = side * (0.02 + 0.13 * pose[GRIPPER]);
    }
    // The pistons live in the turntable's space and span from their mount to the upper arm.
    let arm_mount = Vec3::Y * SHOULDER_PIVOT + shoulder * PISTON_ARM;
    for (piston, mut transform) in &mut pistons {
        let (from, to) = (PISTON_BASE.with_z(piston.z), arm_mount.with_z(piston.z));
        let direction = (to - from).normalize();
        let rotation = Quat::from_rotation_arc(Vec3::Y, direction);
        let center = match piston.part {
            PistonPart::Sleeve => from + direction * PISTON_SLEEVE / 2.0,
            PistonPart::Rod => to - direction * PISTON_ROD / 2.0,
        };
        *transform = Transform::from_translation(center).with_rotation(rotation);
    }
}

/// Shows the mode on the status lights: pulsing Slint blue in auto mode, amber while moving by
/// hand, and steady green when idle.
fn update_status_lights(
    time: Res<Time>,
    pose: Res<ArmPose>,
    light: Res<StatusLight>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    const LED_INTENSITY: f32 = 1.5;
    let color = LED_INTENSITY
        * if pose.auto_mode {
            let pulse = 0.75 + 0.25 * (time.elapsed_secs() * 3.0).sin();
            SLINT_BLUE.to_linear() * pulse
        } else if pose.moving {
            LinearRgba::rgb(1.0, 0.45, 0.03)
        } else {
            LinearRgba::rgb(0.12, 0.75, 0.3)
        };
    if materials.get(&light.0).is_some_and(|m| m.emissive != color)
        && let Some(mut material) = materials.get_mut(&light.0)
    {
        material.emissive = color;
    }
}

fn update_camera(
    shared: Res<Shared>,
    mut cameras: Query<(&mut Transform, &Projection), With<Camera3d>>,
) {
    let (yaw, pitch, zoom, shift) = {
        let state = shared.0.lock().unwrap();
        let (yaw, pitch) = (state.camera_yaw.to_radians(), state.camera_pitch.to_radians());
        (yaw, pitch, state.camera_zoom, state.camera_shift)
    };
    let focus = Vec3::new(0.3, 0.55, 0.0);
    let distance = 4.2 / zoom;
    let offset =
        Vec3::new(pitch.cos() * yaw.sin(), pitch.sin(), pitch.cos() * yaw.cos()) * distance;
    for (mut transform, projection) in &mut cameras {
        let mut view = Transform::from_translation(focus + offset).looking_at(focus, Vec3::Y);
        // Pan sideways so the robot is centered in the part of the scene the panel leaves free.
        if let Projection::Perspective(perspective) = projection {
            let half_width = distance * (perspective.fov / 2.0).tan() * perspective.aspect_ratio;
            view.translation += view.right() * half_width * shift;
        }
        *transform = view;
    }
}

fn update_cube(
    mut cube_state: ResMut<CubeState>,
    tips: Query<&GlobalTransform, With<Tip>>,
    mut cubes: Query<&mut Transform, With<Workpiece>>,
) {
    let Ok(mut cube) = cubes.single_mut() else { return };
    match *cube_state {
        CubeState::Held => {
            if let Ok(tip) = tips.single() {
                *cube = tip.compute_transform();
            }
        }
        CubeState::Dropped => {
            let ground = Vec3::new(cube.translation.x, 0.0, cube.translation.z);
            let (yaw, _, _) = cube.rotation.to_euler(EulerRot::YXZ);
            *cube_state = CubeState::Resting(ground);
            *cube = Transform::from_translation(ground + Vec3::Y * CUBE_SIZE / 2.0)
                .with_rotation(Quat::from_rotation_y(yaw));
        }
        CubeState::Resting(position) if cube_state.is_changed() => {
            cube.translation = position + Vec3::Y * CUBE_SIZE / 2.0;
        }
        CubeState::Resting(_) => {}
    }
}
