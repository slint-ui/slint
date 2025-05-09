// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use bevy::asset::io::{AssetReader, AssetSource, AssetSourceId};
use bevy::prelude::*;
use slint::SharedString;

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

pub struct WebAssetReaderPlugin(pub smol::channel::Sender<(SharedString, f32)>);

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
