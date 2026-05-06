// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

pub fn decode_data_uri(data_uri: &str) -> Result<(Vec<u8>, String), String> {
    let data_url =
        data_url::DataUrl::process(data_uri).map_err(|e| format!("Invalid data URI: {e}"))?;

    let media_type = data_url.mime_type();
    if media_type.type_ != "image" {
        return Err(format!(
            "Unsupported media type: {media_type}. Only image/* data URLs are supported",
        ));
    }

    let extension = if media_type.subtype.starts_with("svg") {
        "svg".to_string()
    } else {
        media_type.subtype.clone()
    };

    let (decoded_data, _) =
        data_url.decode_to_vec().map_err(|e| format!("Invalid data URI payload: {e}"))?;

    Ok((decoded_data, extension))
}
