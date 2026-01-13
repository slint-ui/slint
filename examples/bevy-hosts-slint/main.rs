use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

use bevy::{
    prelude::*,
    render::{
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
    },
};
use slint::{
    platform::software_renderer::PremultipliedRgbaColor,
    PhysicalSize,
};

slint::slint! {

import { VerticalBox, Button, Slider } from "std-widgets.slint";
export component Demo inherits Window {
    background: #ff00ff3f;
    VerticalBox {
        alignment: start;
        Text {
            text: "Hello World";
            color: green;
        }
        Button {
            text: "Press me";
        }
        Slider {
            maximum: 100;
            value: 60;
        }
    }
}
}

#[allow(dead_code)]
struct DemoResource(Demo);

impl FromWorld for DemoResource {
    fn from_world(_world: &mut World) -> Self {
        let instance = Demo::new().unwrap();
        Self(instance)
    }
}

#[derive(Component)]
struct SlintScene(Handle<Image>);

struct BevyWindowAdapter {
    size: Cell<slint::PhysicalSize>,
    slint_window: slint::Window,
    software_renderer: slint::platform::software_renderer::SoftwareRenderer,
    needs_redraw: Cell<bool>,
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
        // We don't create a native window - rendering happens to texture
        // Dispatch initial resize event to set up the window size
        self.slint_window
            .dispatch_event(slint::platform::WindowEvent::Resized {
                size: self.size.get().to_logical(1.),
            });
        Ok(())
    }

    fn request_redraw(&self) {
        self.needs_redraw.set(true);
    }
}

impl BevyWindowAdapter {
    fn new() -> Rc<Self> {
        Rc::new_cyclic(|self_weak: &Weak<Self>| {
            let adapter = Self {
                size: Cell::new(slint::PhysicalSize::new(256, 256)),
                slint_window: slint::Window::new(self_weak.clone()),
                software_renderer: Default::default(),
                needs_redraw: Cell::new(true),
            };
            // Dispatch initial resize to initialize the window
            adapter.slint_window.dispatch_event(slint::platform::WindowEvent::Resized {
                size: adapter.size.get().to_logical(1.),
            });
            adapter
        })
    }

    fn resize(&self, new_size: PhysicalSize) {
        self.size.set(new_size);
        self.slint_window
            .dispatch_event(slint::platform::WindowEvent::Resized {
                size: self.size.get().to_logical(1.),
            })
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
        SLINT_WINDOWS.with(|windows| windows.borrow_mut().push(Rc::downgrade(&adapter)));
        Ok(adapter)
    }
}



fn main() {
    slint::platform::set_platform(Box::new(SlintBevyPlatform {})).unwrap();
    App::new()
        .init_non_send_resource::<DemoResource>()
        .add_plugins(DefaultPlugins)
        .add_systems(Update, render_slint)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>, mut images: ResMut<Assets<Image>>) {
    commands.spawn((
        SceneRoot(assets.load("Monkey.gltf#Scene0")),
        Transform::default(),
    ));

    commands.spawn((
        PointLight::default(),
        Transform::from_xyz(4.0, 5.0, 4.0),
    ));

    let size = Extent3d {
        width: 128,
        height: 128,
        ..default()
    };

    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: None,
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        },
        ..default()
    };

    image.resize(size);

    if let Some(data) = image.data.as_mut() {
        data.fill(128);
    }

    let image_handle = images.add(image);

    commands.spawn(SlintScene(image_handle.clone()));

    commands.spawn((
        Sprite::from_image(image_handle),
    ));

    commands.spawn((
        Camera2d,
        Camera {
            order: 2,
            ..default()
        },
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn render_slint(
    mut images: ResMut<Assets<Image>>,
    slint_scenes: Query<&SlintScene>,
    _res: NonSend<DemoResource>,
) {
    for scene in slint_scenes.iter() {
        let image = images.get_mut(&scene.0).unwrap();

        if let Some(adapter) = SLINT_WINDOWS.with(|windows| {
            windows
                .borrow()
                .first()
                .and_then(|adapter_weak| adapter_weak.upgrade())
        }) {
            let requested_size = slint::PhysicalSize::new(
                image.texture_descriptor.size.width,
                image.texture_descriptor.size.height,
            );

            if requested_size != adapter.size.get() {
                adapter.resize(requested_size);
            }

            let width = requested_size.width;
            let height = requested_size.height;

            if adapter.needs_redraw.take() {
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
    }
}
