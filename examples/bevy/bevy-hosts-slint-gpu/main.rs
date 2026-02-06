// Copyright © 2026 Slint project authors (see AUTHORS or LICENSE-MIT)
// SPDX-License-Identifier: MIT

//! # Slint + Bevy GPU Integration Example
//!
//! This example demonstrates how to embed Slint UI within a Bevy application using
//! **GPU-accelerated rendering** via FemtoVG and WGPU. The Slint UI is rendered directly
//! to a GPU texture that can be displayed on 3D geometry in the scene.
//!
//! ## Architecture Overview
//!
//! The integration uses Slint's `FemtoVGWGPURenderer` to render UI directly to a WGPU texture:
//!
//! 1. Bevy creates a texture in its asset system
//! 2. The texture handle is extracted to Bevy's render world via `ExtractResourcePlugin`
//! 3. A channel passes the underlying WGPU texture from render world back to main world
//! 4. `FemtoVGWGPURenderer::render_to_texture()` renders the UI directly to the GPU texture
//! 5. Mouse input is handled by raycasting against the 3D quad and converting to Slint coordinates
//!
//! ## Key Difference from bevy-hosts-slint
//!
//! - **bevy-hosts-slint**: Uses `SoftwareRenderer` to CPU-render UI to a pixel buffer, then uploads to GPU
//! - **bevy-hosts-slint-gpu**: Uses `FemtoVGWGPURenderer` for direct GPU rendering (better performance)
//!
//! ## Key Components
//!
//! - [`BevyWindowAdapter`]: Implements `slint::platform::WindowAdapter` using `FemtoVGWGPURenderer`
//! - [`SlintBevyPlatform`]: Implements `slint::platform::Platform` to create window adapters
//! - [`SlintSharedTexture`]: Manages texture sharing between Bevy's main and render worlds
//! - [`render_slint`]: Bevy system that renders the Slint UI to the shared texture each frame
//! - [`handle_input`]: Bevy system that raycasts mouse input to the UI quad
//!
//! ## Usage Pattern
//!
//! This example can serve as a template for GPU-accelerated Slint integration:
//! 1. Implement the `Platform` and `WindowAdapter` traits with `FemtoVGWGPURenderer`
//! 2. Share the WGPU texture between Bevy's render world and your Slint renderer
//! 3. Call `render_to_texture()` each frame to render the UI directly on the GPU
//! 4. Handle input by converting your coordinate system to Slint's logical coordinates
//!
//! ## Running the Example
//!
//! ```bash
//! cargo run --release
//! ```
//!
//! Use arrow keys to rotate the cube. Click the button and interact with the slider on the UI.

use bevy::{
    input::{ButtonState, mouse::MouseButtonInput},
    prelude::*,
    render::{
        Render, RenderApp, RenderPlugin,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::RenderAssets,
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
        renderer::{RenderDevice, RenderInstance},
        settings::RenderCreation,
        texture::GpuImage,
    },
};
use i_slint_renderer_femtovg::FemtoVGWGPURenderer;
use slint::{LogicalPosition, PhysicalSize, platform::WindowEvent};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    sync::{Arc, Mutex},
};
use wgpu_28 as wgpu;

const UI_WIDTH: u32 = 800;
const UI_HEIGHT: u32 = 600;
const SCALE_FACTOR: f32 = 2.0; // HiDPI scale factor (2.0 for Retina displays)

slint::slint! {
    import { VerticalBox, Button, Slider, AboutSlint } from "std-widgets.slint";
    export component Demo inherits Window {
        // Fully transparent background (#RRGGBBAA with alpha=00) so the 3D scene
        // behind the UI quad shows through. This works together with the material's
        // alpha_mode: AlphaMode::Blend setting in the Bevy setup code.
        background: #00000000;
        in-out property <int> click-count: 0;
        in-out property <float> slider-value: 0;

        VerticalBox {
            alignment: start;
            Text {
                text: "Hello from Slint (GPU Rendered) - Clicks: " + click-count;
                color: green;
                font-size: 24px;
            }
            Button {
                text: "Press me";
                clicked => {
                    click-count += 1;
                }
            }
            Slider {
                maximum: 100;
                value: slider-value;
            }
            AboutSlint {}
        }

        animate slider-value {
            duration: 2s;
            easing: ease-in-out;
            iteration-count: -1;
            direction: alternate;
        }

        init => {
            slider-value = 100;
        }
    }
}

/// Window adapter that bridges Slint to Bevy using GPU rendering.
///
/// Instead of rendering to a native OS window, this adapter uses `FemtoVGWGPURenderer`
/// to render directly to a WGPU texture that Bevy displays on 3D geometry.
struct BevyWindowAdapter {
    size: Cell<slint::PhysicalSize>,
    scale_factor: Cell<f32>,
    slint_window: slint::Window,
    renderer: FemtoVGWGPURenderer,
}

impl slint::platform::WindowAdapter for BevyWindowAdapter {
    fn window(&self) -> &slint::Window {
        &self.slint_window
    }

    fn size(&self) -> slint::PhysicalSize {
        self.size.get()
    }

    fn renderer(&self) -> &dyn slint::platform::Renderer {
        &self.renderer
    }

    fn set_visible(&self, _visible: bool) -> Result<(), slint::PlatformError> {
        Ok(())
    }

    fn request_redraw(&self) {}
}

impl BevyWindowAdapter {
    fn new(instance: wgpu::Instance, device: wgpu::Device, queue: wgpu::Queue) -> Rc<Self> {
        // Create renderer using the new helper
        let renderer =
            FemtoVGWGPURenderer::new(instance, device, queue).expect("Failed to create renderer");

        Rc::new_cyclic(|self_weak: &Weak<Self>| Self {
            size: Cell::new(slint::PhysicalSize::new(UI_WIDTH, UI_HEIGHT)),
            scale_factor: Cell::new(SCALE_FACTOR),
            slint_window: slint::Window::new(self_weak.clone()),
            renderer,
        })
    }

    fn resize(&self, new_size: PhysicalSize, scale_factor: f32) {
        self.size.set(new_size);
        self.scale_factor.set(scale_factor);
        self.slint_window.dispatch_event(slint::platform::WindowEvent::Resized {
            size: self.size.get().to_logical(scale_factor),
        });
        self.slint_window
            .dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor });
    }
}

/// Slint platform implementation that creates GPU-rendered window adapters.
///
/// Registered via `slint::platform::set_platform()` before creating Slint components.
/// Stores the WGPU device and queue needed to create `FemtoVGWGPURenderer` instances.
struct SlintBevyPlatform {
    instance: wgpu::Instance,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl slint::platform::Platform for SlintBevyPlatform {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        let adapter =
            BevyWindowAdapter::new(self.instance.clone(), self.device.clone(), self.queue.clone());
        SLINT_WINDOWS.with(|windows| {
            windows.borrow_mut().push(Rc::downgrade(&adapter));
        });
        Ok(adapter)
    }
}

/// Bevy resource holding the Slint UI instance and window adapter.
///
/// Stored as `NonSend` because Slint uses `Rc` internally (not thread-safe).
struct SlintContext {
    /// Kept alive for the app's lifetime; the UI state lives inside.
    _instance: Demo,
    /// Reference to the window adapter for rendering and input handling.
    adapter: Rc<BevyWindowAdapter>,
}

/// Marker for the quad mesh displaying the Slint UI texture.
#[derive(Component)]
struct SlintQuad;

/// Marker for the rotating cube that the UI quad is attached to.
#[derive(Component)]
struct Cube;

/// Resource for passing the WGPU texture from render world to main world.
/// Lives in the render app.
#[derive(Resource)]
struct TextureSender(std::sync::mpsc::Sender<wgpu::Texture>);

/// Bevy image handle that gets extracted to the render world.
/// Used to look up the underlying GPU texture.
#[derive(Resource, Clone, Component, ExtractResource)]
struct SlintImageHandle(Handle<Image>);

/// Manages the shared WGPU texture between Bevy's render world and the Slint renderer.
/// Receives the texture from render world and stores it for use by `render_slint`.
#[derive(Resource)]
struct SlintSharedTexture {
    receiver: Mutex<std::sync::mpsc::Receiver<wgpu::Texture>>,
    texture: Arc<Mutex<Option<wgpu::Texture>>>,
}

/// Tracks cursor position over the Slint UI quad.
#[derive(Resource, Default)]
struct CursorState {
    position: Option<LogicalPosition>,
}

/// Raycasts from the camera through the cursor to find intersection with the UI quad.
/// Returns the Slint logical position if the cursor is over the quad.
fn raycast_slint(
    window: &Window,
    camera: (&Camera, &GlobalTransform),
    quad_global: &GlobalTransform,
    scale_factor: f32,
) -> Option<LogicalPosition> {
    let cursor_position = window.cursor_position()?;
    let (camera, camera_transform) = camera;
    let ray = camera.viewport_to_world(camera_transform, cursor_position).ok()?;

    let plane = InfinitePlane3d::new(quad_global.back());
    let intersection = ray.plane_intersection_point(quad_global.translation(), plane)?;
    let local_point = quad_global.affine().inverse().transform_point3(intersection);

    // Quad is 1.0x1.0, check if within bounds
    if local_point.x.abs() <= 0.5 && local_point.y.abs() <= 0.5 {
        let u = local_point.x + 0.5;
        let v = 1.0 - (local_point.y + 0.5);
        return Some(slint::LogicalPosition::new(
            u * UI_WIDTH as f32 / scale_factor,
            v * UI_HEIGHT as f32 / scale_factor,
        ));
    }
    None
}

/// Handles mouse input by raycasting to the UI quad and forwarding events to Slint.
fn handle_input(
    mut mouse_button: MessageReader<MouseButtonInput>,
    windows: Query<&Window>,
    mut cursor_state: ResMut<CursorState>,
    slint_context: Option<NonSend<SlintContext>>,
    quad_query: Query<&GlobalTransform, With<SlintQuad>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
) {
    let Some(slint_context) = slint_context else { return };
    let adapter = &slint_context.adapter;

    let Some(window) = windows.iter().next() else { return };
    let scale_factor = adapter.scale_factor.get();

    let Some(camera) = camera_query.iter().next() else { return };
    let Some(quad_global) = quad_query.iter().next() else { return };

    let new_pos = raycast_slint(window, camera, quad_global, scale_factor);

    match (cursor_state.position, new_pos) {
        (_, Some(pos)) => {
            cursor_state.position = Some(pos);
            adapter.slint_window.dispatch_event(WindowEvent::PointerMoved { position: pos });
        }
        (Some(_), None) => {
            cursor_state.position = None;
            adapter.slint_window.dispatch_event(WindowEvent::PointerExited);
        }
        _ => {}
    }

    for event in mouse_button.read() {
        if let Some(position) = cursor_state.position {
            let button = match event.button {
                MouseButton::Left => slint::platform::PointerEventButton::Left,
                MouseButton::Right => slint::platform::PointerEventButton::Right,
                MouseButton::Middle => slint::platform::PointerEventButton::Middle,
                _ => slint::platform::PointerEventButton::Other,
            };
            match event.state {
                ButtonState::Pressed => {
                    adapter
                        .slint_window
                        .dispatch_event(WindowEvent::PointerPressed { button, position });
                }
                ButtonState::Released => {
                    adapter
                        .slint_window
                        .dispatch_event(WindowEvent::PointerReleased { button, position });
                }
            }
        }
    }
}

/// Creates the 3D scene: cube with UI quad, camera, and lighting.
fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let size = Extent3d { width: UI_WIDTH, height: UI_HEIGHT, ..default() };

    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("SlintUI"),
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        },
        ..default()
    };
    image.resize(size);

    let image_handle = images.add(image);
    commands.insert_resource(SlintImageHandle(image_handle.clone()));

    // Create a material for the Slint UI with special properties for transparency:
    // - unlit: true -> No lighting calculations, UI appears flat and consistent
    // - alpha_mode: Blend -> Enables transparency so the Slint UI's transparent background
    //   (set via `background: #00000000` in the Slint component) shows through, allowing
    //   the 3D scene behind the UI quad to be visible
    // - cull_mode: None -> Visible from both sides (useful as the quad rotates with the cube)
    let material_handle = materials.add(StandardMaterial {
        base_color_texture: Some(image_handle.clone()),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    });

    let cube_mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let quad_mesh = meshes.add(Mesh::from(Rectangle::new(1.0, 1.0)));

    commands
        .spawn((
            Mesh3d(cube_mesh),
            MeshMaterial3d(
                materials.add(StandardMaterial { base_color: Color::WHITE, ..default() }),
            ),
            Transform::from_xyz(0.0, 0.0, -0.5)
                .with_rotation(Quat::from_rotation_y(0.5))
                .with_scale(Vec3::splat(2.0)),
            Cube,
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(quad_mesh),
                MeshMaterial3d(material_handle),
                Transform::from_xyz(0.0, 0.0, 0.5001),
                SlintQuad,
            ));
        });

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        PointLight { intensity: 2_000_000.0, range: 100.0, shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(8.0, 16.0, 8.0),
    ));

    // Instructions overlay
    commands.spawn((
        Text::new("Arrow keys: rotate cube | Mouse: interact with UI"),
        TextFont { font_size: 20.0, ..default() },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
    ));
}

/// Receives the WGPU texture from the render world (sent via channel).
/// This runs once when the texture becomes available.
fn receive_texture(shared: Res<SlintSharedTexture>) {
    if let Ok(texture) = shared.receiver.lock().unwrap().try_recv() {
        *shared.texture.lock().unwrap() = Some(texture);
    }
}

/// Runs in Bevy's render world: extracts the GPU texture and sends it to the main world.
/// Only runs once (uses `Local<bool>` to track).
fn send_slint_texture(
    handle: Option<Res<SlintImageHandle>>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    sender: Res<TextureSender>,
    mut sent: Local<bool>,
) {
    if *sent {
        return;
    }
    if let Some(handle) = handle {
        if let Some(gpu_image) = gpu_images.get(&handle.0) {
            let texture = (*gpu_image.texture).clone();
            let _ = sender.0.send(texture);
            *sent = true;
        }
    }
}

/// Renders the Slint UI to the shared GPU texture each frame.
fn render_slint(slint_context: Option<NonSend<SlintContext>>, shared: Res<SlintSharedTexture>) {
    let Some(ctx) = slint_context else { return };
    slint::platform::update_timers_and_animations();
    if let Some(texture) = shared.texture.lock().unwrap().as_ref() {
        let _ = ctx.adapter.renderer.render_to_texture(texture);
    }
}

/// Initializes the Slint platform and creates the Demo UI component.
/// This runs as a startup system after `setup` to ensure Bevy's render device is available.
fn initialize_slint(
    render_instance: &RenderInstance,
    render_device: &RenderDevice,
    render_queue: &bevy::render::renderer::RenderQueue,
) -> impl Fn(&mut World) + use<> {
    let instance = (**render_instance.0).clone();
    let device = render_device.wgpu_device().clone();
    let queue = (**render_queue.0).clone();
    move |world: &mut World| {
        let platform = SlintBevyPlatform {
            instance: instance.clone(),
            device: device.clone(),
            queue: queue.clone(),
        };
        slint::platform::set_platform(Box::new(platform)).unwrap();

        let instance = Demo::new().unwrap();
        instance.window().show().unwrap();

        // Retrieve the adapter that was created when Demo::new() called create_window_adapter()
        let adapter = SLINT_WINDOWS
            .with(|w| w.borrow().first().and_then(|a| a.upgrade()))
            .expect("Window adapter should have been created");

        instance.window().dispatch_event(WindowEvent::WindowActiveChanged(true));
        adapter.resize(slint::PhysicalSize::new(UI_WIDTH, UI_HEIGHT), SCALE_FACTOR);

        // The SlintSharedTexture was already inserted in main() with the channel receiver
        world.insert_non_send_resource(SlintContext {
            // Keep instance alive for the app's lifetime
            _instance: instance,
            adapter,
        });
    }
}

// Thread-local storage for window adapters created by the platform.
// Used to retrieve the adapter after Demo::new() creates it internally.
thread_local! {
    static SLINT_WINDOWS: RefCell<Vec<Weak<BevyWindowAdapter>>> = RefCell::new(Vec::new());
}

/// Rotates the cube based on arrow key input.
fn rotate_cube(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut Transform, With<Cube>>,
) {
    for mut transform in query.iter_mut() {
        let speed = 2.0;
        let delta = speed * time.delta_secs();
        if keyboard.pressed(KeyCode::ArrowUp) {
            transform.rotate_x(delta);
        }
        if keyboard.pressed(KeyCode::ArrowDown) {
            transform.rotate_x(-delta);
        }
        if keyboard.pressed(KeyCode::ArrowLeft) {
            transform.rotate_y(delta);
        }
        if keyboard.pressed(KeyCode::ArrowRight) {
            transform.rotate_y(-delta);
        }
    }
}

fn main() {
    let (tx, rx) = std::sync::mpsc::channel();

    let backends = wgpu::Backends::from_env().unwrap_or_default();

    let bevy::render::settings::RenderResources(
        render_device,
        render_queue,
        adapter_info,
        adapter,
        instance,
    ) = spin_on::spin_on(bevy::render::renderer::initialize_renderer(
        backends,
        None,
        &bevy::render::settings::WgpuSettings::default(),
    ));

    let slint_init = initialize_slint(&instance, &render_device, &render_queue);

    let mut app = App::new();

    app.add_plugins(DefaultPlugins.set(RenderPlugin {
        render_creation: RenderCreation::manual(
            render_device,
            render_queue,
            adapter_info,
            adapter,
            instance,
        ),
        ..default()
    }))
    .insert_resource(SlintSharedTexture {
        receiver: Mutex::new(rx),
        texture: Arc::new(Mutex::new(None)),
    })
    .init_resource::<CursorState>()
    .add_plugins(ExtractResourcePlugin::<SlintImageHandle>::default())
    .add_systems(Startup, (setup, slint_init).chain())
    .add_systems(Update, (receive_texture, handle_input, render_slint, rotate_cube).chain());

    let render_app = app.sub_app_mut(RenderApp);
    render_app.insert_resource(TextureSender(tx));
    render_app.add_systems(Render, send_slint_texture);

    app.run();
}
