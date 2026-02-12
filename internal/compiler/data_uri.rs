// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

fn extract_extension_from_media_type(media_type: &str) -> String {
    let subtype = media_type.strip_prefix("image/").unwrap_or(media_type);
    let subtype = subtype.split(';').next().unwrap_or(subtype);
    if subtype.starts_with("svg") { "svg".to_string() } else { subtype.to_string() }
}

pub fn decode_data_uri(data_uri: &str) -> Result<(Vec<u8>, String), String> {
    let data_url =
        dataurl::DataUrl::parse(data_uri).map_err(|e| format!("Invalid data URI: {:?}", e))?;

    let media_type = data_url.get_media_type();
    if !media_type.starts_with("image/") {
        return Err(format!(
            "Unsupported media type: {}. Only image/* data URLs are supported",
            media_type
        ));
    }

    let extension = extract_extension_from_media_type(media_type);

    let decoded_data = if data_url.get_is_base64_encoded() {
        data_url.get_data().to_vec()
    } else {
        data_url.get_text().as_bytes().to_vec()
    };

    Ok((decoded_data, extension))
}
