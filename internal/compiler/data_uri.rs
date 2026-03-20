// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

fn extract_extension_from_media_type(media_type: &str) -> String {
    let subtype = media_type.strip_prefix("image/").unwrap_or(media_type);
    let subtype = subtype.split(';').next().unwrap_or(subtype);
    if subtype.starts_with("svg") { "svg".to_string() } else { subtype.to_string() }
}

pub fn decode_data_uri(data_uri: &str) -> Result<(Vec<u8>, String), String> {
    let data_url =
        data_url::DataUrl::process(data_uri).map_err(|e| format!("Invalid data URI: {e}"))?;

    let media_type = data_url.mime_type();
    if !media_type.matches("image", &media_type.subtype) {
        return Err(format!(
            "Unsupported media type: {}. Only image/* data URLs are supported",
            media_type
        ));
    }

    let extension = extract_extension_from_media_type(&media_type.to_string());

    let (decoded_data, _) =
        data_url.decode_to_vec().map_err(|e| format!("Invalid data URI payload: {e}"))?;

    Ok((decoded_data, extension))
}
