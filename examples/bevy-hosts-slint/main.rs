use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

use bevy::{
    prelude::*,
    render::{
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, PrimitiveTopology,
        },
    },
    input::{
        mouse::MouseButtonInput,
        ButtonState,
    },
    window::CursorMoved,
};
use slint::{
    platform::software_renderer::PremultipliedRgbaColor,
    PhysicalSize,
    platform::WindowEvent,
    LogicalPosition,
};

slint::slint! {

import { VerticalBox, Button, Slider } from "std-widgets.slint";
export component Demo inherits Window {
    background: #ff00ff3f;
    in-out property <int> click-count: 0;

    VerticalBox {
        alignment: start;
        Text {
            text: "Hello World - Clicks: " + click-count;
            color: green;
        }
        Button {
            text: "Press me";
            clicked => {
                click-count += 1;
            }
        }
        Slider {
            maximum: 100;
            value: 60;
        }
    }
}
}

struct SlintContext {
    _instance: Demo,
    adapter: Rc<BevyWindowAdapter>,
}

impl FromWorld for SlintContext {
    fn from_world(_world: &mut World) -> Self {
        let instance = Demo::new().unwrap();

        // Get the adapter from thread_local
        let adapter = SLINT_WINDOWS.with(|windows| {
            windows.borrow().first().and_then(|w| w.upgrade())
        }).expect("Slint window adapter should be created when Demo is initialized");

        Self {
            _instance: instance,
            adapter,
        }
    }
}

#[derive(Component)]
struct SlintScene(Handle<Image>);

#[derive(Component)]
struct ColorfulCube;

struct BevyWindowAdapter {
    size: Cell<slint::PhysicalSize>,
    scale_factor: Cell<f32>,
    slint_window: slint::Window,
    software_renderer: slint::platform::software_renderer::SoftwareRenderer,
}

impl slint::platform::WindowAdapter for BevyWindowAdapter {
    fn window(&self) -> &slint::Window {
        &self.slint_window
    }

    fn size(&self) -> slint::PhysicalSize {
        self.size.get()
    }

    fn renderer(&self) -> &dyn slint::platform::Renderer {
        &self.software_renderer
    }

    fn set_visible(&self, _visible: bool) -> Result<(), slint::PlatformError> {
        Ok(())
    }

    fn request_redraw(&self) {}
}

impl BevyWindowAdapter {
    fn new() -> Rc<Self> {
        Rc::new_cyclic(|self_weak: &Weak<Self>| Self {
            size: Cell::new(slint::PhysicalSize::new(800, 600)),
            scale_factor: Cell::new(2.0), // Default to 2.0 for Retina displays
            slint_window: slint::Window::new(self_weak.clone()),
            software_renderer: Default::default(),
        })
    }

    fn resize(&self, new_size: PhysicalSize, scale_factor: f32) {
        self.size.set(new_size);
        self.scale_factor.set(scale_factor);
        self.slint_window
            .dispatch_event(slint::platform::WindowEvent::Resized {
                size: self.size.get().to_logical(scale_factor),
            });
        self.slint_window
            .dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged {
                scale_factor,
            });
    }
}

thread_local! {
    static SLINT_WINDOWS: RefCell<Vec<Weak<BevyWindowAdapter>>> = RefCell::new(Vec::new());
}

struct SlintBevyPlatform {}

impl slint::platform::Platform for SlintBevyPlatform {
    fn create_window_adapter(
        &self,
    ) -> Result<std::rc::Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        let adapter = BevyWindowAdapter::new();
        // Dispatch initial resize and scale factor events to initialize the window
        let scale_factor = adapter.scale_factor.get();
        adapter.slint_window.dispatch_event(slint::platform::WindowEvent::Resized {
            size: adapter.size.get().to_logical(scale_factor),
        });
        adapter.slint_window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged {
            scale_factor,
        });
        SLINT_WINDOWS.with(|windows| {
            windows.borrow_mut().push(Rc::downgrade(&adapter));
        });
        Ok(adapter)
    }
}



#[derive(Resource, Default)]
struct CursorState {
    position: Option<LogicalPosition>,
}

fn handle_input(
    mut cursor_moved: MessageReader<CursorMoved>,
    mut mouse_button: MessageReader<MouseButtonInput>,
    windows: Query<&Window>,
    mut cursor_state: ResMut<CursorState>,
    slint_context: Option<NonSend<SlintContext>>,
    slint_scenes: Query<&SlintScene>,
    sprites: Query<(&Sprite, &Transform)>,
    images: Res<Assets<Image>>,
) {
    let Some(slint_context) = slint_context else {
        return;
    };

    let adapter = &slint_context.adapter;

    let Ok(window) = windows.single() else {
        return;
    };

    // Get the Slint texture size
    let Some(scene) = slint_scenes.iter().next() else {
        return;
    };
    let Some(image) = images.get(&scene.0) else {
        return;
    };

    // Find the sprite displaying the Slint UI to get its transform
    let mut sprite_transform = None;
    for (sprite, transform) in sprites.iter() {
        // Check if this sprite is using our Slint image
        if &sprite.image == &scene.0 {
            sprite_transform = Some(transform);
            break;
        }
    }

    let texture_width = image.texture_descriptor.size.width as f32;
    let texture_height = image.texture_descriptor.size.height as f32;
    let window_width = window.width();
    let window_height = window.height();
    let scale_factor = adapter.scale_factor.get();

    // Handle cursor movement
    for event in cursor_moved.read() {
        // Convert window coordinates (top-left origin) to Bevy world coordinates (center origin)
        // Bevy window coordinates: (0,0) at top-left
        // Bevy world coordinates: (0,0) at center
        let world_x = event.position.x - (window_width / 2.0);
        let world_y = (window_height / 2.0) - event.position.y;

        // Get sprite position (default to center if not found)
        let sprite_pos = sprite_transform.map(|t| t.translation).unwrap_or(Vec3::ZERO);

        // Calculate position relative to sprite
        let sprite_local_x = world_x - sprite_pos.x + (texture_width / 2.0);
        let sprite_local_y = world_y - sprite_pos.y + (texture_height / 2.0);

        // Flip Y for Slint (top-left origin)
        let slint_y = texture_height - sprite_local_y;

        // Convert to logical coordinates
        let position = LogicalPosition::new(
            sprite_local_x / scale_factor,
            slint_y / scale_factor,
        );
        cursor_state.position = Some(position);


        adapter.slint_window.dispatch_event(WindowEvent::PointerMoved {
            position,
        });
    }

    // Handle mouse button events
    for event in mouse_button.read() {
        let position = cursor_state.position.unwrap_or_else(|| LogicalPosition::new(0.0, 0.0));

        match event.state {
            ButtonState::Pressed => {
                let button = match event.button {
                    MouseButton::Left => slint::platform::PointerEventButton::Left,
                    MouseButton::Right => slint::platform::PointerEventButton::Right,
                    MouseButton::Middle => slint::platform::PointerEventButton::Middle,
                    _ => slint::platform::PointerEventButton::Other,
                };
                adapter.slint_window.dispatch_event(WindowEvent::PointerPressed { button, position });
            }
            ButtonState::Released => {
                let button = match event.button {
                    MouseButton::Left => slint::platform::PointerEventButton::Left,
                    MouseButton::Right => slint::platform::PointerEventButton::Right,
                    MouseButton::Middle => slint::platform::PointerEventButton::Middle,
                    _ => slint::platform::PointerEventButton::Other,
                };
                adapter.slint_window.dispatch_event(WindowEvent::PointerReleased { button, position });
            }
        }
    }
}

fn main() {
    slint::platform::set_platform(Box::new(SlintBevyPlatform {})).unwrap();
    App::new()
        .add_plugins(DefaultPlugins)
        .init_resource::<CursorState>()
        .add_systems(Startup, setup)
        .add_systems(Update, (handle_input, render_slint, rotate_cube).chain())
        .init_non_send_resource::<SlintContext>()
        .run();
}

fn rotate_cube(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut Transform, With<ColorfulCube>>,
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

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let size = Extent3d {
        width: 800,
        height: 600,
        ..default()
    };

    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("SlintUI"),
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST,
            view_formats: &[],
        },
        ..default()
    };

    image.resize(size);

    let image_handle = images.add(image);

    commands.spawn(SlintScene(image_handle.clone()));

    // Spawn our Slint UI texture sprite
    commands.spawn((
        Sprite::from_image(image_handle),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    // Camera with dark gray clear color so we can see the UI
    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::None,
            order: 2,
            ..default()
        },
    ));

    // Colorful Cube behind the UI
    let cube_mesh = meshes.add(create_colorful_cube());
    let cube_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: false,
        ..default()
    });
    
    commands.spawn((
        Mesh3d(cube_mesh),
        MeshMaterial3d(cube_material),
        Transform::from_xyz(0.0, 0.0, -2.0)
            .with_rotation(Quat::from_rotation_y(0.5))
            .with_scale(Vec3::splat(2.0)),
        ColorfulCube,
    ));

    // 3D Scene Setup
    commands.spawn((
        SceneRoot(assets.load("Monkey.gltf#Scene0")),
        Transform::from_scale(Vec3::splat(4.0)).with_translation(Vec3::new(4.0, 0.0, -5.0)),
    ));

    commands.spawn((
        PointLight {
            intensity: 2_000_000.0,
            range: 100.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(8.0, 16.0, 8.0),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.1)),
            ..default()
        },
    ));
}

fn render_slint(
    mut images: ResMut<Assets<Image>>,
    slint_scenes: Query<&SlintScene>,
    slint_context: Option<NonSend<SlintContext>>,
    windows: Query<&Window>,
) {
    let Some(slint_context) = slint_context else {
        return;
    };

    let adapter = &slint_context.adapter;

    // Process pending Slint events and updates BEFORE rendering
    slint::platform::update_timers_and_animations();

    for scene in slint_scenes.iter() {
        let image = images.get_mut(&scene.0).unwrap();

        let requested_size = slint::PhysicalSize::new(
            image.texture_descriptor.size.width,
            image.texture_descriptor.size.height,
        );

        // Get the actual scale factor from the Bevy window
        let scale_factor = windows.single().map(|w| w.scale_factor()).unwrap_or(2.0);

        // Update adapter if size or scale factor changed
        if requested_size != adapter.size.get() || scale_factor != adapter.scale_factor.get() {
            adapter.resize(requested_size, scale_factor);
        }

        let width = requested_size.width;
        let height = requested_size.height;

        // Always render, not just when needs_redraw is set
        let mut buffer =
            vec![PremultipliedRgbaColor::default(); width as usize * height as usize];
        adapter.software_renderer.render(
            buffer.as_mut_slice(),
            image.texture_descriptor.size.width as usize,
        );

        if let Some(data) = image.data.as_mut() {
            data.clone_from_slice(bytemuck::cast_slice(buffer.as_slice()));
        }
    }
}

fn create_colorful_cube() -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default());

    // Vertices for a cube (24 vertices for hard edges)
    // 6 faces * 4 vertices
    let raw_vertices = vec![
        // Front (z+)
        [-0.5, -0.5, 0.5], [0.5, -0.5, 0.5], [0.5, 0.5, 0.5], [-0.5, 0.5, 0.5],
        // Back (z-)
        [-0.5, 0.5, -0.5], [0.5, 0.5, -0.5], [0.5, -0.5, -0.5], [-0.5, -0.5, -0.5],
        // Right (x+)
        [0.5, -0.5, -0.5], [0.5, 0.5, -0.5], [0.5, 0.5, 0.5], [0.5, -0.5, 0.5],
        // Left (x-)
        [-0.5, -0.5, 0.5], [-0.5, 0.5, 0.5], [-0.5, 0.5, -0.5], [-0.5, -0.5, -0.5],
        // Top (y+)
        [-0.5, 0.5, 0.5], [0.5, 0.5, 0.5], [0.5, 0.5, -0.5], [-0.5, 0.5, -0.5],
        // Bottom (y-)
        [-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, -0.5, 0.5], [-0.5, -0.5, 0.5],
    ];

    let raw_colors = vec![
        // Front - Red
        [1.0, 0.0, 0.0, 1.0], [1.0, 0.0, 0.0, 1.0], [1.0, 0.0, 0.0, 1.0], [1.0, 0.0, 0.0, 1.0],
        // Back - Green
        [0.0, 1.0, 0.0, 1.0], [0.0, 1.0, 0.0, 1.0], [0.0, 1.0, 0.0, 1.0], [0.0, 1.0, 0.0, 1.0],
        // Right - Blue
        [0.0, 0.0, 1.0, 1.0], [0.0, 0.0, 1.0, 1.0], [0.0, 0.0, 1.0, 1.0], [0.0, 0.0, 1.0, 1.0],
        // Left - Yellow
        [1.0, 1.0, 0.0, 1.0], [1.0, 1.0, 0.0, 1.0], [1.0, 1.0, 0.0, 1.0], [1.0, 1.0, 0.0, 1.0],
        // Top - Cyan
        [0.0, 1.0, 1.0, 1.0], [0.0, 1.0, 1.0, 1.0], [0.0, 1.0, 1.0, 1.0], [0.0, 1.0, 1.0, 1.0],
        // Bottom - Magenta
        [1.0, 0.0, 1.0, 1.0], [1.0, 0.0, 1.0, 1.0], [1.0, 0.0, 1.0, 1.0], [1.0, 0.0, 1.0, 1.0],
    ];

    let raw_normals = vec![
        // Front
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        // Back
        [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0],
        // Right
        [1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 0.0],
        // Left
        [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0],
        // Top
        [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0],
        // Bottom
        [0.0, -1.0, 0.0], [0.0, -1.0, 0.0], [0.0, -1.0, 0.0], [0.0, -1.0, 0.0],
    ];

    let indices = vec![
        0, 1, 2, 2, 3, 0, // Front
        4, 5, 6, 6, 7, 4, // Back
        8, 9, 10, 10, 11, 8, // Right
        12, 13, 14, 14, 15, 12, // Left
        16, 17, 18, 18, 19, 16, // Top
        20, 21, 22, 22, 23, 20, // Bottom
    ];

    let mut vertices = Vec::new();
    let mut colors = Vec::new();
    let mut normals = Vec::new();

    for i in indices {
        vertices.push(raw_vertices[i]);
        colors.push(raw_colors[i]);
        normals.push(raw_normals[i]);
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);

    mesh
}
