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

        instance.window().show().unwrap();

        // Get the adapter from thread_local
        let adapter = SLINT_WINDOWS.with(|windows| {
            windows.borrow().first().and_then(|w| w.upgrade())
        }).expect("Slint window adapter should be created when Demo is initialized");

        adapter.slint_window.dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(true));

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



#[derive(Component)]
struct SlintQuad;

#[derive(Resource, Default)]
struct CursorState {
    position: Option<LogicalPosition>,
}

fn handle_input(
    mut mouse_button: MessageReader<MouseButtonInput>,
    windows: Query<&Window>,
    mut cursor_state: ResMut<CursorState>,
    slint_context: Option<NonSend<SlintContext>>,
    slint_scenes: Query<&SlintScene>,
    mut quad_query: Query<(&GlobalTransform, &mut Transform), With<SlintQuad>>,
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

    // Get the Slint texture size
    let Some(scene) = slint_scenes.iter().next() else {
        return;
    };
    let Some(image) = images.get(&scene.0) else {
        return;
    };

    let texture_width = image.texture_descriptor.size.width as f32;
    let texture_height = image.texture_descriptor.size.height as f32;
    let scale_factor = adapter.scale_factor.get();

    // Get camera and quad
    let Some((camera, camera_transform)) = camera_query.iter().next() else { return };
    let Some((quad_global, _quad_local)) = quad_query.iter_mut().next() else { return };

    // Continuous Raycasting
    if let Some(cursor_position) = window.cursor_position() {
        let mut hit = false;
        if let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_position) {
            // Intersect with Quad (Plane)
            let plane_normal = quad_global.back();
            let plane_point = quad_global.translation();
            
            let denominator = ray.direction.dot(*plane_normal);
            if denominator.abs() > f32::EPSILON {
                let t = (plane_point - ray.origin).dot(*plane_normal) / denominator;
                if t >= 0.0 {
                    let intersection_point = ray.origin + ray.direction * t;
                    
                    // Convert to local coordinates of the quad
                    let local_point = quad_global.affine().inverse().transform_point3(intersection_point);
                    
                    // Quad size is 1.0 x 1.0 (defined in setup)
                    let quad_width = 1.0;
                    let quad_height = 1.0;
                    
                    if local_point.x.abs() <= quad_width / 2.0 && local_point.y.abs() <= quad_height / 2.0 {
                        // Normalize to 0..1 (UV)
                        // Local x: -w/2 .. w/2 -> 0 .. 1
                        let u = (local_point.x + quad_width / 2.0) / quad_width;
                        // Local y: -h/2 .. h/2 -> 1 .. 0 (Slint is top-down)
                        let v = 1.0 - (local_point.y + quad_height / 2.0) / quad_height;
                        
                        let slint_x = u * texture_width;
                        let slint_y = v * texture_height;
                        
                        let position = LogicalPosition::new(
                            slint_x / scale_factor,
                            slint_y / scale_factor,
                        );
                        
                        // Only dispatch if position changed significantly? 
                        // Slint filters duplicates efficiently, so it's fine to send.
                        cursor_state.position = Some(position);
                        adapter.slint_window.dispatch_event(WindowEvent::PointerMoved { position });
                        hit = true;
                    } else {
                        // println!("Outside bounds: local_point={:?}", local_point);
                    }
                }
            }
        }
        
        if !hit && cursor_state.position.is_some() {
            cursor_state.position = None;
            adapter.slint_window.dispatch_event(WindowEvent::PointerExited);
        }
    }

    // Handle mouse button events
    for event in mouse_button.read() {
        if let Some(position) = cursor_state.position {
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

    // Slint UI Material
    let material_handle = materials.add(StandardMaterial {
        base_color_texture: Some(image_handle),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    });

    // Colorful Cube Material
    let cube_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: false,
        ..default()
    });

    let cube_mesh = meshes.add(create_colorful_cube());
    let quad_mesh = meshes.add(Mesh::from(Rectangle::new(1.0, 1.0)));

    // Spawn Cube with Slint UI as child
    commands.spawn((
        Mesh3d(cube_mesh),
        MeshMaterial3d(cube_material),
        Transform::from_xyz(0.0, 0.0, -2.0)
            .with_rotation(Quat::from_rotation_y(0.5))
            .with_scale(Vec3::splat(2.0)),
        ColorfulCube,
    )).with_children(|parent| {
        // Spawn Slint UI on the front face (Z+)
        parent.spawn((
            Mesh3d(quad_mesh),
            MeshMaterial3d(material_handle),
            Transform::from_xyz(0.0, 0.0, 0.51), // Slightly in front to avoid z-fighting
            SlintQuad,
        ));
    });

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

        // Mark the image as changed so Bevy knows to update the GPU texture
        image.texture_descriptor.label = Some("SlintUI");
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
