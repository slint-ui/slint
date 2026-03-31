// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use bevy::prelude::*;
use slint::{Model, SharedString};

mod slint_bevy_adapter;
mod web_asset;

slint::slint! {
import { Palette, Button, ComboBox, GroupBox, GridBox, Slider, HorizontalBox, VerticalBox, ProgressIndicator } from "std-widgets.slint";

export component AppWindow inherits Window {
    in property <image> texture <=> i.source;
    out property <length> requested-texture-width: i.width;
    out property <length> requested-texture-height: i.height;

    in property <bool> show-loading-screen: false;
    in property <string> download-url;
    in property <percent> download-progress;

    in property <[string]> available-models;
    callback load-model(index: int);

    title: @tr("Slint & Bevy");
    preferred-width: 500px;
    preferred-height: 600px;

    VerticalBox {
        alignment: start;
        Rectangle {
            background: Palette.alternate-background;

            VerticalBox {
                Text {
                    text: "This text is rendered using Slint. The animation below is rendered using Bevy code.";
                    wrap: word-wrap;
                }

                HorizontalBox {
                    Text {
                        text: "Select Model:";
                        vertical-alignment: center;
                    }
                    ComboBox {
                        model: root.available-models;
                        selected(current-value) => { root.load-model(self.current-index) }
                    }
                }
            }
        }

        Rectangle {
            width: 100%;
            height: 100%;
            if !show-loading-screen: Text {
                y: 80px;
                width: 450px;
                font-size: 14px;
                text: "This text is also rendered using Slint. It can be seen because Bevy is rendering with a transparent background.";
                wrap: word-wrap;
            }
            i := Image {
                image-fit: fill;
                width: 100%;
                height: 100%;
                preferred-width: self.source.width * 1px;
                preferred-height: self.source.height * 1px;

                if show-loading-screen: Rectangle {
                    VerticalBox {
                        alignment: start;
                        Text {
                            horizontal-alignment: center;
                            text: "Downloading Assets";
                        }
                        Text {
                            text: download-url;
                            overflow: elide;
                        }
                        ProgressIndicator {
                            indeterminate: download-url.is-empty;
                            progress: root.download-progress;
                        }
                    }
                }
            }

        }
    }
}
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::cell::RefCell;
    use std::rc::Rc;

    let (model_selector_sender, model_selector_receiver) = smol::channel::bounded::<GLTFModel>(1);

    let (download_progress_sender, download_progress_receiver) =
        smol::channel::bounded::<(SharedString, f32)>(5);

    // Slint initializes first with wgpu — on linuxkms it creates its own wgpu instance
    // with DRM surface support. The wgpu resources are then passed to Bevy via the
    // rendering notifier callback.
    let mut wgpu_settings = slint::wgpu_27::WGPUSettings::default();
    wgpu_settings.device_required_limits = slint::wgpu_27::wgpu::Limits::default()
        .using_resolution(slint::wgpu_27::wgpu::Limits::downlevel_defaults());
    slint::BackendSelector::new()
        .require_wgpu_27(slint::wgpu_27::WGPUConfiguration::Automatic(wgpu_settings))
        .select()?;
    let app_window = AppWindow::new().unwrap();

    // These will be filled once Bevy is initialized from the rendering notifier.
    let bevy_channels: Rc<
        RefCell<
            Option<(
                smol::channel::Receiver<slint::wgpu_27::wgpu::Texture>,
                smol::channel::Sender<slint_bevy_adapter::ControlMessage>,
            )>,
        >,
    > = Rc::new(RefCell::new(None));

    let app_weak = app_window.as_weak();
    let bevy_channels_setup = bevy_channels.clone();

    // Wrap in RefCell+Option so we can move out of it exactly once in the FnMut closure
    let model_selector_receiver = RefCell::new(Some(model_selector_receiver));
    let download_progress_sender = RefCell::new(Some(download_progress_sender));

    app_window.window().set_rendering_notifier(move |state, graphics_api| {
        match state {
            slint::RenderingState::RenderingSetup => {
                // Extract wgpu resources from Slint and initialize Bevy
                let slint::GraphicsAPI::WGPU27 { instance, device, queue, .. } = graphics_api
                else {
                    return;
                };

                let model_selector_receiver =
                    model_selector_receiver.borrow_mut().take().unwrap();
                let download_progress_sender =
                    download_progress_sender.borrow_mut().take().unwrap();

                let channels = slint_bevy_adapter::run_bevy_app_with_slint(
                    instance.clone(),
                    device.clone(),
                    queue.clone(),
                    move |app| {
                        app.add_plugins(web_asset::WebAssetReaderPlugin(
                            download_progress_sender,
                        ));
                    },
                    move |mut app| {
                        app.insert_resource(CameraPos(Vec3::new(3., 4.0, 4.0)))
                            .insert_resource(ModelBasePath(
                                "https://github.com/KhronosGroup/glTF-Sample-Assets/raw/refs/heads/main/"
                                    .into(),
                            ))
                            .add_systems(Startup, setup)
                            .add_systems(Update, reload_model_from_channel(model_selector_receiver))
                            .add_systems(Update, animate_camera)
                            .insert_resource(ClearColor(Color::NONE))
                            .run();
                    },
                );

                *bevy_channels_setup.borrow_mut() = Some(channels);
            }
            slint::RenderingState::BeforeRendering => {
                let Some(app) = app_weak.upgrade() else { return };
                app.window().request_redraw();

                let channels = bevy_channels_setup.borrow();
                let Some((new_texture_receiver, control_message_sender)) = channels.as_ref()
                else {
                    return;
                };

                let Ok(new_texture) = new_texture_receiver.try_recv() else { return };
                if let Some(old_texture) = app.get_texture().to_wgpu_27_texture() {
                    let control_message_sender = control_message_sender.clone();
                    slint::spawn_local(async move {
                        control_message_sender
                            .send(slint_bevy_adapter::ControlMessage::ReleaseFrontBufferTexture {
                                texture: old_texture,
                            })
                            .await
                            .unwrap();
                    })
                    .unwrap();
                }

                let requested_width = app.get_requested_texture_width().round() as u32;
                let requested_height = app.get_requested_texture_height().round() as u32;
                if requested_width > 0 && requested_height > 0 {
                    let control_message_sender = control_message_sender.clone();
                    slint::spawn_local(async move {
                        control_message_sender
                            .send(slint_bevy_adapter::ControlMessage::ResizeBuffers {
                                width: requested_width,
                                height: requested_height,
                            })
                            .await
                            .unwrap();
                    })
                    .unwrap();
                }

                if let Ok(image) = new_texture.try_into() {
                    app.set_texture(image);
                }
            }
            _ => {}
        }
    })?;

    let app_weak = app_window.as_weak();

    slint::spawn_local(async move {
        loop {
            let Ok((url, progress)) = download_progress_receiver.recv().await else {
                break;
            };
            let Some(app) = app_weak.upgrade() else { return };
            app.set_download_url(url);
            app.set_download_progress(progress * 100.);
            app.set_show_loading_screen(progress < 1.0);
        }
    })
    .unwrap();

    let models = slint::VecModel::from_slice(&[
        GLTFModel {
            name: "Damaged Helmet".into(),
            path: "Models/DamagedHelmet/glTF-Binary/DamagedHelmet.glb".into(),
            center: Vec3::new(3.0, 4.0, 4.0),
        },
        GLTFModel {
            name: "Fish".into(),
            path: "Models/BarramundiFish/glTF-Binary/BarramundiFish.glb".into(),
            center: Vec3::new(3.0, 2.0, 1.0),
        },
        GLTFModel {
            name: "Box".into(),
            path: "Models/Box/glTF-Binary/Box.glb".into(),
            center: Vec3::new(3.0, 4.0, 4.0),
        },
    ]);

    app_window
        .set_available_models(slint::ModelRc::new(models.clone().map(|model| model.name.clone())));

    model_selector_sender.send_blocking(models.row_data(0).unwrap()).unwrap();

    app_window.on_load_model(move |index| {
        let model = models.row_data(index as usize).unwrap();
        let model_selector_sender = model_selector_sender.clone();
        slint::spawn_local(async move {
            model_selector_sender.send(model).await.ok();
        })
        .unwrap();
    });

    app_window.run()?;

    Ok(())
}

#[derive(Clone)]
struct GLTFModel {
    name: SharedString,
    path: SharedString,
    center: Vec3,
}

#[derive(Resource)]
struct CameraPos(Vec3);

#[derive(Resource)]
struct ModelBasePath(String);

fn setup(mut commands: Commands) {
    commands.spawn(DirectionalLight { illuminance: 100_000.0, ..default() });
    commands.spawn((Camera3d::default(), PointLight::default()));
}

fn reload_model_from_channel(
    receiver: smol::channel::Receiver<GLTFModel>,
) -> impl FnMut(
    Commands,
    Res<AssetServer>,
    Query<Entity, With<SceneRoot>>,
    ResMut<CameraPos>,
    Res<ModelBasePath>,
) {
    move |mut commands, asset_server, loaded_bundles, mut camera, base_path| {
        let Ok(new_model) = receiver.try_recv() else {
            return;
        };
        for loaded_bundle in loaded_bundles.iter() {
            commands.entity(loaded_bundle).despawn();
        }
        commands.spawn(SceneRoot(asset_server.load(
            GltfAssetLabel::Scene(0).from_asset(format!("{}{}", base_path.0, new_model.path)),
        )));
        camera.0 = new_model.center;
    }
}

fn animate_camera(
    mut cameras: Query<&mut Transform, With<Camera3d>>,
    time: Res<Time>,
    camera: Res<CameraPos>,
) {
    let now = time.elapsed_secs();
    for mut transform in cameras.iter_mut() {
        transform.translation = vec3(ops::cos(now), 0.0, ops::sin(now)) * camera.0;
        transform.look_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y);
    }
}
