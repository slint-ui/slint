// Copyright © 2024 Slint project authors (see AUTHORS or LICENSE-MIT)
// SPDX-License-Identifier: MIT

//! # Slint + Bevy Integration Example
//!
//! This example demonstrates how to embed Slint UI components within a Bevy game engine application.
//! The Slint UI is rendered to a texture that can be displayed in the 3D scene, enabling rich UI
//! overlays and interactive elements within a game or 3D application.
//!
//! ## Architecture Overview
//!
//! The integration works by implementing a custom Slint platform backend that:
//! 1. Uses Slint's software renderer to render UI into a pixel buffer
//! 2. Copies that buffer to a Bevy texture every frame
//! 3. Displays the texture on a 3D mesh (in this case, a quad attached to a rotating cube)
//! 4. Handles mouse input by raycasting against the 3D mesh and converting to Slint coordinates
//!
//! ## Key Components
//!
//! - **BevyWindowAdapter**: Implements `slint::platform::WindowAdapter` to bridge Slint's
//!   windowing model to Bevy's texture-based rendering.
//! - **SlintBevyPlatform**: Implements `slint::platform::Platform` to create window adapters
//!   without opening native OS windows.
//! - **render_slint**: Bevy system that renders the Slint UI to a texture each frame.
//! - **handle_input**: Bevy system that performs raycasting to detect mouse interaction with
//!   the UI quad and forwards events to Slint.
//!
//! ## Usage Pattern
//!
//! This example can serve as a template for integrating Slint with any custom rendering backend:
//! 1. Implement the `Platform` and `WindowAdapter` traits
//! 2. Use `SoftwareRenderer` to render UI to a pixel buffer
//! 3. Upload the buffer to your graphics API (Vulkan, OpenGL, WebGPU, etc.)
//! 4. Handle input by converting your coordinate system to Slint's logical coordinates
//!
//! ## Running the Example
//!
//! ```bash
//! cargo run --release
//! ```
//!
//! Use arrow keys to rotate the cube. Click the button and interact with the slider on the UI.

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

use bevy::{
    input::{ButtonState, mouse::MouseButtonInput},
    math::primitives::InfinitePlane3d,
    prelude::*,
    render::render_resource::{
        Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    },
};
use slint::{
    LogicalPosition, PhysicalSize, platform::WindowEvent,
    platform::software_renderer::PremultipliedRgbaColor,
};

// Define the Slint UI component inline using the slint! macro.
// This macro compiles .slint markup at Rust compile time into native Rust structures.
// The resulting `Demo` struct can be instantiated and its properties/callbacks accessed from Rust.
//
// This example demonstrates:
// - Standard widget imports (VerticalBox, Button, Slider)
// - Property definitions (click-count, slider-value)
// - Event handlers (button clicked callback)
// - Animations (continuous oscillating slider animation)
// - Layout system (VerticalBox with alignment)
slint::slint! {

import { VerticalBox, Button, Slider } from "std-widgets.slint";

export component Demo inherits Window {
    background: #ff00ff3f;
    in-out property <int> click-count: 0;
    in-out property <float> slider-value: 0;

    VerticalBox {
        alignment: start;
        Text {
            text: "Clicks: " + click-count;
            color: white;
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
    }

    // Continuous oscillating animation
    animate slider-value {
        duration: 2s;
        easing: ease-in-out;
        iteration-count: -1; // infinite
        direction: alternate; // go back and forth
    }

    // Trigger animation on init
    init => {
        slider-value = 100;
    }
}
}

/// Bevy resource that holds the Slint UI instance and window adapter.
///
/// This is stored as a `NonSend` resource because Slint's types are not thread-safe
/// (they use Rc internally). Bevy systems that access this must run on the main thread.
///
/// The adapter reference is kept here so that Bevy systems can access the window adapter
/// to render the UI and handle input events.
struct SlintContext {
    /// The Slint component instance. Kept alive for the lifetime of the application.
    _instance: Demo,
    /// Shared reference to the window adapter, used by rendering and input systems.
    adapter: Rc<BevyWindowAdapter>,
}

impl FromWorld for SlintContext {
    /// Initializes the Slint context when Bevy starts up.
    ///
    /// This is called automatically by Bevy when the `NonSend<SlintContext>` resource
    /// is first accessed. It creates the Slint component instance and retrieves the
    /// window adapter from thread-local storage.
    fn from_world(_world: &mut World) -> Self {
        // Initialize Slint timers before creating component to ensure animations work
        // See: https://github.com/slint-ui/slint/issues/2809
        slint::platform::update_timers_and_animations();

        let instance = Demo::new().expect("Failed to create Slint Demo component");

        instance.window().show().expect("Failed to show Slint window");

        // Get the adapter from thread_local storage where it was stored by SlintBevyPlatform
        let adapter = SLINT_WINDOWS
            .with(|windows| windows.borrow().first().and_then(|w| w.upgrade()))
            .expect("Slint window adapter should be created when Demo is initialized");

        // Notify Slint that the window is active and ready to receive events
        adapter
            .slint_window
            .dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(true));

        Self { _instance: instance, adapter }
    }
}

/// Component that tracks the Bevy texture and material used for rendering the Slint UI.
///
/// This component is attached to an entity that holds handles to the texture and material.
/// The `render_slint` system uses these handles to update the texture with fresh UI pixels each frame.
#[derive(Component)]
struct SlintScene {
    /// Handle to the Bevy image/texture that contains the rendered Slint UI.
    image: Handle<Image>,
    /// Handle to the material that uses the Slint texture.
    material: Handle<StandardMaterial>,
}

/// Marker component for the rotating cube in the scene.
///
/// Used by the `rotate_cube` system to identify which entity to rotate in response to arrow keys.
#[derive(Component)]
struct Cube;

/// Custom window adapter that bridges Slint's windowing model to Bevy's texture-based rendering.
///
/// This adapter implements the `slint::platform::WindowAdapter` trait, which is the core
/// interface between Slint and the host platform. Instead of creating a native OS window,
/// this adapter renders to a pixel buffer that Bevy can upload to a texture.
///
/// ## Key Responsibilities
/// - Stores the current window size and scale factor
/// - Provides a `SoftwareRenderer` for rendering UI to a pixel buffer
/// - Responds to resize and scale factor change events
///
/// ## Thread Safety
/// This type uses `Cell` for interior mutability and is not thread-safe.
/// It must only be accessed from the main thread (Bevy's main world).
struct BevyWindowAdapter {
    /// Current physical size of the window in pixels.
    /// Updated when the texture size changes or when explicitly resized.
    size: Cell<slint::PhysicalSize>,
    /// Display scale factor (1.0 for standard displays, 2.0 for Retina/HiDPI).
    /// Used to convert between physical pixels and logical coordinates.
    scale_factor: Cell<f32>,
    /// The Slint window instance that receives events and manages the UI state.
    slint_window: slint::Window,
    /// Software renderer that renders the UI into a pixel buffer (RGBA8).
    software_renderer: slint::platform::software_renderer::SoftwareRenderer,
}

/// Implementation of Slint's WindowAdapter trait.
///
/// This is the core integration point where we tell Slint how to interact with our
/// custom "window" (which is really just a texture in Bevy).
impl slint::platform::WindowAdapter for BevyWindowAdapter {
    /// Returns a reference to the Slint window instance.
    /// Required by the WindowAdapter trait.
    fn window(&self) -> &slint::Window {
        &self.slint_window
    }

    /// Returns the current physical size of the window.
    /// Slint uses this to determine the rendering area.
    fn size(&self) -> slint::PhysicalSize {
        self.size.get()
    }

    /// Returns a reference to the renderer.
    /// In this case, we use Slint's software renderer which renders to a pixel buffer.
    fn renderer(&self) -> &dyn slint::platform::Renderer {
        &self.software_renderer
    }

    /// Called when Slint wants to show or hide the window.
    /// Since we're rendering to a texture, we don't need to do anything here.
    fn set_visible(&self, _visible: bool) -> Result<(), slint::PlatformError> {
        Ok(())
    }

    /// Called when Slint wants to request a redraw.
    /// In our case, we render every frame anyway, so this is a no-op.
    fn request_redraw(&self) {}
}

impl BevyWindowAdapter {
    /// Creates a new BevyWindowAdapter with default settings.
    ///
    /// Uses `Rc::new_cyclic` to create a self-referential structure where the
    /// `slint::Window` holds a weak reference to the adapter.
    ///
    /// Default settings:
    /// - Size: 800x600 pixels
    /// - Scale factor: 2.0 (optimized for Retina/HiDPI displays)
    fn new() -> Rc<Self> {
        Rc::new_cyclic(|self_weak: &Weak<Self>| Self {
            size: Cell::new(slint::PhysicalSize::new(800, 600)),
            scale_factor: Cell::new(2.0), // Default to 2.0 for Retina displays
            slint_window: slint::Window::new(self_weak.clone()),
            software_renderer: Default::default(),
        })
    }

    /// Updates the window size and scale factor, dispatching events to Slint.
    ///
    /// This should be called whenever the Bevy texture size changes to keep
    /// Slint's layout system in sync with the actual rendering area.
    ///
    /// # Arguments
    /// * `new_size` - The new physical size in pixels
    /// * `scale_factor` - The display scale factor (1.0 = standard, 2.0 = Retina)
    fn resize(&self, new_size: PhysicalSize, scale_factor: f32) {
        self.size.set(new_size);
        self.scale_factor.set(scale_factor);
        // Notify Slint of the size change (in logical coordinates)
        self.slint_window.dispatch_event(slint::platform::WindowEvent::Resized {
            size: self.size.get().to_logical(scale_factor),
        });
        // Notify Slint of the scale factor change
        self.slint_window
            .dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor });
    }
}

// Thread-local storage for Slint window adapters.
//
// Since Slint uses Rc (not Arc), all Slint types must live on a single thread.
// We use thread-local storage to keep track of created window adapters so that
// Bevy systems can retrieve them after Slint creates them.
thread_local! {
    /// Storage for weak references to all created window adapters.
    /// When a new Slint component is created, the platform stores a weak reference here.
    static SLINT_WINDOWS: RefCell<Vec<Weak<BevyWindowAdapter>>> = RefCell::new(Vec::new());
}

/// Custom Slint platform implementation for Bevy integration.
///
/// This struct implements the `slint::platform::Platform` trait, which is the top-level
/// integration point between Slint and the host environment. The platform is responsible
/// for creating window adapters and providing timing information.
///
/// Register this platform before creating any Slint components using:
/// ```ignore
/// slint::platform::set_platform(Box::new(SlintBevyPlatform {})).unwrap();
/// ```
struct SlintBevyPlatform {}

impl slint::platform::Platform for SlintBevyPlatform {
    /// Creates a new window adapter when Slint needs to display a component.
    ///
    /// This is called automatically when you create a Slint component instance
    /// (e.g., `Demo::new()`). The adapter is stored in thread-local storage so
    /// Bevy systems can access it later.
    ///
    /// The adapter is initialized with resize and scale factor events to ensure
    /// Slint's layout engine has the correct initial dimensions.
    fn create_window_adapter(
        &self,
    ) -> Result<std::rc::Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        let adapter = BevyWindowAdapter::new();
        // Dispatch initial resize and scale factor events to initialize the window
        let scale_factor = adapter.scale_factor.get();
        adapter.slint_window.dispatch_event(slint::platform::WindowEvent::Resized {
            size: adapter.size.get().to_logical(scale_factor),
        });
        adapter
            .slint_window
            .dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor });
        // Store a weak reference so we can retrieve it later
        SLINT_WINDOWS.with(|windows| {
            windows.borrow_mut().push(Rc::downgrade(&adapter));
        });
        Ok(adapter)
    }
}

/// Marker component for the quad mesh that displays the Slint UI.
///
/// This component is attached to the quad entity that the UI texture is rendered onto.
/// The input handling system uses this marker to identify which mesh to raycast against.
#[derive(Component)]
struct SlintQuad;

/// Resource that tracks the current mouse cursor state relative to the Slint UI.
///
/// This is used to track whether the cursor is currently hovering over the UI quad
/// and what its position is in Slint's logical coordinate space. We need to track
/// this to properly send PointerExited events when the cursor leaves the UI area.
#[derive(Resource, Default)]
struct CursorState {
    /// Current cursor position in Slint's logical coordinates, if hovering over the UI.
    /// None if the cursor is not over the UI quad.
    position: Option<LogicalPosition>,
}

/// Bevy system that handles mouse input and forwards it to Slint.
///
/// This system performs the following steps each frame:
/// 1. Gets the current mouse position from Bevy's window
/// 2. Performs a raycast from the camera through the mouse position
/// 3. Checks if the ray intersects the Slint UI quad
/// 4. If intersecting, converts the 3D intersection point to 2D UV coordinates
/// 5. Converts UV coordinates to Slint's logical coordinate space
/// 6. Dispatches PointerMoved/PointerPressed/PointerReleased events to Slint
///
/// This enables mouse interaction with Slint UI elements even though they're rendered
/// on a 3D mesh in the scene rather than directly on the screen.
fn handle_input(
    mut mouse_button: MessageReader<MouseButtonInput>,
    windows: Query<&Window>,
    mut cursor_state: ResMut<CursorState>,
    slint_context: Option<NonSend<SlintContext>>,
    slint_scenes: Query<&SlintScene>,
    quad_query: Query<&GlobalTransform, With<SlintQuad>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    images: Res<Assets<Image>>,
) {
    let Some(slint_context) = slint_context else {
        return;
    };

    let adapter = &slint_context.adapter;

    let Ok(window) = windows.single() else {
        return;
    };

    // Get the Slint texture size - we need this to convert UV coordinates to pixel coordinates
    let Some(scene) = slint_scenes.iter().next() else {
        return;
    };
    let Some(image) = images.get(&scene.image) else {
        return;
    };

    let texture_width = image.texture_descriptor.size.width as f32;
    let texture_height = image.texture_descriptor.size.height as f32;
    let scale_factor = adapter.scale_factor.get();

    // Get camera and quad transforms for raycasting
    let Some((camera, camera_transform)) = camera_query.iter().next() else { return };
    let Some(quad_global) = quad_query.iter().next() else { return };

    // Perform raycasting every frame to detect cursor position on the UI quad
    if let Some(cursor_position) = window.cursor_position() {
        let mut hit = false;
        if let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_position) {
            // Use Bevy's built-in ray-plane intersection
            // The quad's back direction is its normal (pointing toward the camera)
            let plane_normal = quad_global.back();
            let plane_origin = quad_global.translation();
            let plane = InfinitePlane3d::new(*plane_normal);

            if let Some(intersection_point) = ray.plane_intersection_point(plane_origin, plane) {
                // Transform from world space to the quad's local coordinate system
                let local_point =
                    quad_global.affine().inverse().transform_point3(intersection_point);

                // The quad mesh is 1.0 x 1.0 units (centered at origin in local space)
                let quad_width = 1.0;
                let quad_height = 1.0;

                // Check if the intersection point is within the quad's bounds
                if local_point.x.abs() <= quad_width / 2.0
                    && local_point.y.abs() <= quad_height / 2.0
                {
                    // Convert local coordinates to UV coordinates (0..1 range)
                    // Local x: -0.5 .. 0.5 -> UV u: 0 .. 1
                    let u = (local_point.x + quad_width / 2.0) / quad_width;
                    // Local y: -0.5 .. 0.5 -> UV v: 1 .. 0 (flip Y because Slint's origin is top-left)
                    let v = 1.0 - (local_point.y + quad_height / 2.0) / quad_height;

                    // Convert UV coordinates to pixel coordinates
                    let slint_x = u * texture_width;
                    let slint_y = v * texture_height;

                    // Convert physical pixels to logical coordinates using the scale factor
                    let position =
                        LogicalPosition::new(slint_x / scale_factor, slint_y / scale_factor);

                    // Update cursor state and notify Slint of pointer movement
                    cursor_state.position = Some(position);
                    adapter.slint_window.dispatch_event(WindowEvent::PointerMoved { position });
                    hit = true;
                }
            }
        }

        // If the cursor was previously over the quad but is no longer, send a PointerExited event
        if !hit && cursor_state.position.is_some() {
            cursor_state.position = None;
            adapter.slint_window.dispatch_event(WindowEvent::PointerExited);
        }
    }

    // Handle mouse button clicks and forward them to Slint
    // Only dispatch events if the cursor is currently over the UI quad
    for event in mouse_button.read() {
        if let Some(position) = cursor_state.position {
            // Convert Bevy's mouse button to Slint's button enum
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

/// Main entry point for the Bevy + Slint integration example.
///
/// This function sets up the integration in the correct order:
/// 1. Register the custom Slint platform (MUST be done before creating any Slint components)
/// 2. Create the Bevy app with default plugins (windowing, rendering, input, etc.)
/// 3. Initialize resources and systems
/// 4. Initialize the Slint context as a NonSend resource (main thread only)
///
/// The systems run in this order each frame:
/// - `handle_input`: Raycasts mouse position and forwards events to Slint
/// - `render_slint`: Renders the Slint UI to a texture
/// - `rotate_cube`: Rotates the cube in response to arrow keys
fn main() {
    // CRITICAL: Set the platform BEFORE creating any Slint components
    slint::platform::set_platform(Box::new(SlintBevyPlatform {})).unwrap();

    App::new()
        .add_plugins(DefaultPlugins)
        .init_resource::<CursorState>()
        .add_systems(Startup, setup)
        // Chain systems to ensure they run in a deterministic order
        .add_systems(Update, (handle_input, render_slint, rotate_cube).chain())
        // Initialize Slint context as NonSend (must run on main thread)
        .init_non_send_resource::<SlintContext>()
        .run();
}

/// Bevy system that rotates the cube based on arrow key input.
///
/// This is a simple demo feature to show that the 3D scene continues to update
/// independently of the Slint UI overlay.
///
/// Controls:
/// - Arrow Up: Rotate around X axis (pitch up)
/// - Arrow Down: Rotate around X axis (pitch down)
/// - Arrow Left: Rotate around Y axis (yaw left)
/// - Arrow Right: Rotate around Y axis (yaw right)
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

/// Bevy startup system that creates the 3D scene and sets up the Slint UI texture.
///
/// This system runs once at startup and performs the following setup:
///
/// 1. **Create Slint UI Texture**: Creates an 800x600 RGBA texture that Slint will render into
/// 2. **Create Material**: Creates a Bevy material with the Slint texture, configured for
///    transparency and unlit rendering (so the UI appears flat and consistent)
/// 3. **Create Cube**: Spawns a rotating cube at (0, 0, -0.5)
/// 4. **Attach UI Quad**: Adds a child quad mesh to the cube's front face that displays the Slint UI
/// 5. **Load 3D Model**: Loads a cow model from GLTF
/// 6. **Add Lighting**: Places a point light to illuminate the 3D scene
/// 7. **Setup Camera**: Creates a 3D camera at (0, 0, 6) looking at the origin
///
/// ## Important Details
///
/// - The UI quad is a child of the cube, so it rotates with the cube
/// - The quad is positioned at z=0.51 in local space (slightly in front of the cube face) to avoid z-fighting
/// - The material has `unlit: true` so the UI brightness doesn't change as the cube rotates
/// - The material has `alpha_mode: AlphaMode::Blend` to support UI transparency
/// - The material has `cull_mode: None` so the UI is visible from both sides
fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Define the size of the Slint UI texture
    let size = Extent3d { width: 600, height: 600, ..default() };

    // Create a Bevy image/texture for the Slint UI to render into
    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("SlintUI"),
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm, // 8-bit RGBA to match Slint's output
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        },
        ..default()
    };
    image.resize(size);

    let image_handle = images.add(image);

    // Create a material for the Slint UI with special properties:
    // - unlit: true -> No lighting calculations, UI appears flat and consistent
    // - alpha_mode: Blend -> Support transparency (Slint UI can have transparent backgrounds)
    // - cull_mode: None -> Visible from both sides (useful as the quad rotates)
    let material_handle = materials.add(StandardMaterial {
        base_color_texture: Some(image_handle.clone()),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    });

    // Spawn an entity to track the Slint texture and material
    // The render_slint system will query for this component to find the texture to update
    commands.spawn(SlintScene { image: image_handle, material: material_handle.clone() });

    // Create a material for the cube with a distinct color
    let cube_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.7, 0.9), // Cyan/teal color
        unlit: false,
        ..default()
    });

    // Create meshes using Bevy's built-in primitives
    let cube_mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let quad_mesh = meshes.add(Rectangle::new(1.0, 1.0));

    // Spawn the cube with the Slint UI as a child entity
    commands
        .spawn((
            Mesh3d(cube_mesh),
            MeshMaterial3d(cube_material),
            Transform::from_xyz(0.0, 0.0, -0.5)
                .with_rotation(Quat::from_rotation_y(0.5))
                .with_scale(Vec3::splat(2.0)),
            Cube,
        ))
        .with_children(|parent| {
            // Attach the UI quad as a child of the cube
            // Being a child means it inherits the cube's transform (rotation, scale)
            parent.spawn((
                Mesh3d(quad_mesh),
                MeshMaterial3d(material_handle),
                // Position on the front face (+Z in local space)
                // 0.5001 is slightly in front of the cube face (which is at 0.5 in a 1x1 cube)
                // to prevent z-fighting artifacts
                Transform::from_xyz(0.0, 0.0, 0.5001),
                SlintQuad, // Marker for input handling system to find this quad
            ));
        });

    // Load and spawn the cow model
    commands.spawn((
        SceneRoot(assets.load("cow.gltf#Scene0")),
        Transform::from_scale(Vec3::splat(4.0)).with_translation(Vec3::new(-2.0, 2.0, -30.0)),
    ));

    // Add a point light
    commands.spawn((
        PointLight { intensity: 2_000_000.0, range: 100.0, shadows_enabled: true, ..default() },
        Transform::from_xyz(8.0, 16.0, 8.0),
    ));

    // Create the 3D camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
        Camera { clear_color: ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.1)), ..default() },
    ));

    // Create a static info overlay using Bevy's built-in UI system
    // This demonstrates that Slint and Bevy UI can coexist
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(10.0),
            top: Val::Px(10.0),
            padding: UiRect::all(Val::Px(10.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .insert(BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Slint + Bevy Integration Demo"),
                TextFont { font_size: 16.0, ..default() },
                TextColor(Color::WHITE),
            ));
            parent.spawn((
                Text::new("UI rendered via Slint software renderer"),
                TextFont { font_size: 12.0, ..default() },
                TextColor(Color::srgb(0.8, 0.8, 0.8)),
            ));
            parent.spawn((
                Text::new("Use arrow keys to rotate the cube"),
                TextFont { font_size: 12.0, ..default() },
                TextColor(Color::srgb(0.8, 0.8, 0.8)),
            ));
        });
}

/// Bevy system that renders the Slint UI to a texture each frame.
///
/// This is the core rendering loop that bridges Slint's software renderer to Bevy's texture system.
///
/// ## Rendering Pipeline
///
/// Each frame, this system:
/// 1. Updates Slint's timers and animations (drives the animation system)
/// 2. Checks if the window size or scale factor changed and updates the adapter if needed
/// 3. Renders the Slint UI into a pixel buffer using `SoftwareRenderer`
/// 4. Copies the pixel buffer to the Bevy texture's CPU-side data
/// 5. Triggers Bevy's change detection to ensure the GPU texture is updated
///
/// ## Performance Notes
///
/// - We render every frame (not just when `needs_redraw` is true) to support continuous animations
/// - The software renderer produces premultiplied RGBA pixels, which we copy directly using `bytemuck`
/// - We trigger material change detection as a workaround for Bevy issue #17350 to force GPU upload
///
/// ## Important
///
/// This system MUST run after `handle_input` to ensure input events are processed before rendering.
fn render_slint(
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    slint_scenes: Query<&SlintScene>,
    slint_context: Option<NonSend<SlintContext>>,
    windows: Query<&Window>,
) {
    let Some(slint_context) = slint_context else {
        return;
    };

    // CRITICAL: Update timers and animations BEFORE rendering
    // This processes all pending events, updates animations, and advances timers
    // Must be called at least once per frame for animations to work
    slint::platform::update_timers_and_animations();

    // Get the actual scale factor from the Bevy window
    let scale_factor = windows.single().map(|w| w.scale_factor()).unwrap_or(2.0);

    let adapter = &slint_context.adapter;

    // Only one SlintScene is spawned, so we use .next() to make this explicit
    let Some(scene) = slint_scenes.iter().next() else { return };
    let image = images.get_mut(&scene.image).unwrap();

    let requested_size = slint::PhysicalSize::new(
        image.texture_descriptor.size.width,
        image.texture_descriptor.size.height,
    );

    // If the texture size or DPI scale changed, notify Slint's layout engine
    // This triggers a re-layout of the UI at the new size
    if requested_size != adapter.size.get() || scale_factor != adapter.scale_factor.get() {
        adapter.resize(requested_size, scale_factor);
    }

    // Render the Slint UI directly into the Bevy texture's CPU-side storage.
    // We use bytemuck::cast_slice_mut to safely reinterpret the &mut [u8] as &mut [PremultipliedRgbaColor].
    if let Some(data) = image.data.as_mut() {
        // Render the Slint UI into the pixel buffer
        // The second parameter is the stride (pixels per row)
        adapter.software_renderer.render(
            bytemuck::cast_slice_mut::<u8, PremultipliedRgbaColor>(data),
            image.texture_descriptor.size.width as usize,
        );
    }

    // WORKAROUND: Force GPU texture re-upload by accessing the material mutably
    // This triggers Bevy's change detection, which schedules a GPU upload
    // Without this, the texture may not update on the GPU even though CPU data changed
    // See: https://github.com/bevyengine/bevy/issues/17350
    materials.get_mut(&scene.material);
}
