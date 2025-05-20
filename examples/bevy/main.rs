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
    let (model_selector_sender, model_selector_receiver) = smol::channel::bounded::<GLTFModel>(1);

    let (download_progress_sender, download_progress_receiver) =
        smol::channel::bounded::<(SharedString, f32)>(5);

    let (new_texture_receiver, control_message_sender) =
        spin_on::spin_on(slint_bevy_adapter::run_bevy_app_with_slint(
            |app| {
                app.add_plugins(web_asset::WebAssetReaderPlugin(download_progress_sender));
            },
            |mut app| {
                app.insert_resource(CameraPos(Vec3::new(3., 4.0, 4.0)))
                    //                    .insert_resource(ModelBasePath("".into()))
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
        ))?;

    let app_window = AppWindow::new().unwrap();
    let app2 = app_window.as_weak();

    app_window.window().set_rendering_notifier(move |state, _| {
        let slint::RenderingState::BeforeRendering = state else { return };
        let Some(app) = app2.upgrade() else { return };
        app.window().request_redraw();
        let Ok(new_texture) = new_texture_receiver.try_recv() else { return };
        if let Some(old_texture) = app.get_texture().to_wgpu_24_texture() {
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

fn setup(mut commands: Commands, camera: Res<CameraPos>) {
    commands.spawn(DirectionalLight { illuminance: 100_000.0, ..default() });
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(camera.0).looking_at(Vec3::new(0.0, -0.5, 0.0), Vec3::Y),
        PointLight { color: Color::linear_rgb(0.5, 0., 0.), ..default() },
    ));

    /*
    commands.spawn(SceneRoot(
        //asset_server.load(GltfAssetLabel::Scene(0).from_asset("DamagedHelmet.glb")),
        asset_server.load(
            GltfAssetLabel::Scene(0)
                .from_asset("Models/DamagedHelmet/glTF-Binary/DamagedHelmet.glb"), //  GltfAssetLabel::Scene(0)
                                                                                   //      .from_asset("https://github.com/KhronosGroup/glTF-Sample-Assets/raw/refs/heads/main/Models/DamagedHelmet/glTF-Binary/DamagedHelmet.glb"),
        ),
    ));
    */
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
        commands.spawn(SceneRoot(
            //asset_server.load(GltfAssetLabel::Scene(0).from_asset("DamagedHelmet.glb")),
            asset_server.load(
                GltfAssetLabel::Scene(0).from_asset(format!("{}{}", base_path.0, new_model.path)),
            ),
        ));
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
        // transform.translation = vec3(ops::cos(now), 0.0, ops::sin(now)) * vec3(3.0, 4.0, 4.0);
        transform.translation = vec3(ops::cos(now), 0.0, ops::sin(now)) * camera.0;
        transform.look_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y);
    }
}
