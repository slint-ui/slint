// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use bevy::{
    asset::io::{AssetReader, AssetSource, AssetSourceId},
    prelude::*,
};
use slint::SharedString;

mod slint_bevy_adapter;

slint::slint! {
import { Button, ComboBox, GroupBox, GridBox, Slider, VerticalBox, ProgressIndicator } from "std-widgets.slint";
export component AppWindow inherits Window {
    in property <image> texture <=> i.source;
    out property <length> requested-texture-width: i.width;
    out property <length> requested-texture-height: i.height;
    out property <float> selected-red <=> red.value;
    out property <float> selected-green <=> green.value;
    out property <float> selected-blue <=> blue.value;

    in property <bool> show-loading-screen: true;
    in property <string> download-url;
    in property <percent> download-progress;

    title: @tr("Slint & Bevy");
    preferred-width: 500px;
    preferred-height: 600px;

    VerticalLayout {
        alignment: start;
        GroupBox {
            title: "Light Color Controls";

            GridBox {
                Row {
                    Text {
                        text: "Red:";
                        vertical-alignment: center;
                    }

                    red := Slider {
                        minimum: 0.1;
                        maximum: 1.0;
                        value: 0.2;
                    }
                }

                Row {
                    Text {
                        text: "Green:";
                        vertical-alignment: center;
                    }

                    green := Slider {
                        minimum: 0.1;
                        maximum: 1.0;
                        value: 0.5;
                    }
                }

                Row {
                    Text {
                        text: "Blue:";
                        vertical-alignment: center;
                    }

                    blue := Slider {
                        minimum: 0.1;
                        maximum: 1.0;
                        value: 0.9;
                    }
                }
            }
        }

        i := Image {
            image-fit: fill;
            width: 100%;
            height: 100%;
            preferred-width: self.source.width * 1px;
            preferred-height: self.source.height * 1px;

            if show-loading-screen: VerticalBox {
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (download_progress_sender, download_progress_receiver) =
        smol::channel::bounded::<(SharedString, f32)>(5);

    let (light_color_sender, light_color_receiver) =
        smol::channel::bounded::<bevy::color::LinearRgba>(1);

    let (new_texture_receiver, control_message_sender) =
        spin_on::spin_on(slint_bevy_adapter::run_bevy_app_with_slint(
            |app| {
                app.add_plugins(WebAssetReaderPlugin(download_progress_sender));
            },
            |mut app| {
                app.insert_resource::<LightColorSource>(LightColorSource(light_color_receiver))
                    .add_systems(Startup, setup)
                    .add_systems(Update, animate_camera)
                    .add_systems(Update, update_light_color)
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

        let red = app.get_selected_red();
        let green = app.get_selected_green();
        let blue = app.get_selected_blue();
        let light_color_sender = light_color_sender.clone();
        slint::spawn_local(async move {
            light_color_sender
                .send(bevy::color::LinearRgba { red, green, blue, alpha: 1. })
                .await
                .unwrap();
        })
        .unwrap();
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
            if progress >= 1.0 {
                app.set_show_loading_screen(false);
            }
        }
    })
    .unwrap();

    app_window.run()?;

    Ok(())
}

#[derive(Resource)]
struct LightColorSource(smol::channel::Receiver<bevy::color::LinearRgba>);

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(DirectionalLight { illuminance: 100_000.0, ..default() });
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.7, 0.0, 4.0).looking_at(Vec3::new(0.0, -0.5, 0.0), Vec3::Y),
        PointLight { color: Color::linear_rgb(0.5, 0., 0.), ..default() },
    ));

    commands.spawn(SceneRoot(
        //asset_server.load(GltfAssetLabel::Scene(0).from_asset("DamagedHelmet.glb")),
        asset_server.load(
            GltfAssetLabel::Scene(0)
                .from_asset("https://github.com/KhronosGroup/glTF-Sample-Assets/raw/refs/heads/main/Models/DamagedHelmet/glTF-Binary/DamagedHelmet.glb"),
        ),
    ));
}

fn update_light_color(mut lights: Query<&mut PointLight>, color_source: Res<LightColorSource>) {
    if let Ok(new_color) = color_source.0.try_recv() {
        for mut light in lights.iter_mut() {
            light.color = bevy::color::Color::LinearRgba(new_color);
        }
    }
}

fn animate_camera(mut cameras: Query<&mut Transform, With<Camera3d>>, time: Res<Time>) {
    let now = time.elapsed_secs();
    for mut transform in cameras.iter_mut() {
        transform.translation = vec3(ops::cos(now), 0.0, ops::sin(now)) * vec3(3.0, 4.0, 4.0);
        transform.look_at(Vec3::new(0.0, -0.5, 0.0), Vec3::Y);
    }
}

fn map_err(err: reqwest::Error) -> bevy::asset::io::AssetReaderError {
    match err.status().map(|s| s.as_u16()) {
        Some(404) => bevy::asset::io::AssetReaderError::NotFound(
            err.url().map(|u| u.path()).unwrap_or_default().into(),
        ),
        Some(code) => bevy::asset::io::AssetReaderError::HttpError(code),
        _ => bevy::asset::io::AssetReaderError::Io(
            std::io::Error::new(std::io::ErrorKind::Unsupported, "Unknown error").into(),
        ),
    }
}

async fn get(
    url: impl reqwest::IntoUrl,
    progress_channel: smol::channel::Sender<(SharedString, f32)>,
) -> Result<bevy::asset::io::VecReader, bevy::asset::io::AssetReaderError> {
    use smol::stream::StreamExt;

    let url = url.into_url().unwrap();

    let response = reqwest::get(url.clone()).await.map_err(map_err)?;

    let content_length = response.content_length();

    let mut stream = response.bytes_stream();

    let mut data = Vec::new();

    let progress_url_str = SharedString::from(url.as_str());

    let _ = progress_channel.send((progress_url_str.clone(), 0.)).await.ok();

    while let Some(chunk) = stream.next().await {
        let chunk_bytes = chunk.map_err(map_err)?;
        data.extend(chunk_bytes);
        let progress_percent = content_length
            .map(|total_length| data.len() as f32 / total_length as f32)
            .unwrap_or_default();
        let _ = progress_channel.send((progress_url_str.clone(), progress_percent)).await.ok();
    }

    Ok(bevy::asset::io::VecReader::new(data))
}

struct WebAssetLoader(smol::channel::Sender<(SharedString, f32)>);

impl AssetReader for WebAssetLoader {
    fn read<'a>(
        &'a self,
        path: &'a std::path::Path,
    ) -> impl bevy::asset::io::AssetReaderFuture<Value: bevy::asset::io::Reader + 'a> {
        let url = reqwest::Url::parse(&format!("https://{}", path.to_string_lossy())).unwrap();
        async_compat::Compat::new(get(url, self.0.clone()))
    }

    fn read_meta<'a>(
        &'a self,
        path: &'a std::path::Path,
    ) -> impl bevy::asset::io::AssetReaderFuture<Value: bevy::asset::io::Reader + 'a> {
        std::future::ready(Result::<bevy::asset::io::VecReader, _>::Err(
            bevy::asset::io::AssetReaderError::NotFound(path.into()),
        ))
    }

    fn read_directory<'a>(
        &'a self,
        path: &'a std::path::Path,
    ) -> impl bevy::tasks::ConditionalSendFuture<
        Output = std::result::Result<
            Box<bevy::asset::io::PathStream>,
            bevy::asset::io::AssetReaderError,
        >,
    > {
        return std::future::ready(Err(bevy::asset::io::AssetReaderError::NotFound(path.into())));
    }

    fn is_directory<'a>(
        &'a self,
        _path: &'a std::path::Path,
    ) -> impl bevy::tasks::ConditionalSendFuture<
        Output = std::result::Result<bool, bevy::asset::io::AssetReaderError>,
    > {
        std::future::ready(Ok(false))
    }
}

struct WebAssetReaderPlugin(smol::channel::Sender<(SharedString, f32)>);

impl Plugin for WebAssetReaderPlugin {
    fn build(&self, app: &mut App) {
        let progress_channel = self.0.clone();
        app.register_asset_source(
            AssetSourceId::Name("https".into()),
            AssetSource::build()
                .with_reader(move || Box::new(WebAssetLoader(progress_channel.clone()))),
        );
    }
}
