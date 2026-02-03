// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use servo::ServoUrl;

pub fn convert_input_string_to_servo_url(url: &str) -> ServoUrl {
    let parsed_url = input_to_url(url, "https://google.com/search?q=%s");
    let blank_url = ServoUrl::parse("about:blank").ok();
    parsed_url.or(blank_url).unwrap()
}

/// Interpret an input URL.
///
/// If this is not a valid URL, try to "fix" it by adding a scheme or if all else fails,
/// interpret the string as a search term.
fn input_to_url(request: &str, searchpage: &str) -> Option<ServoUrl> {
    let request = request.trim();
    ServoUrl::parse(request)
        .ok()
        .or_else(|| try_as_domain(request))
        .or_else(|| try_as_search_page(request, searchpage))
}

fn try_as_search_page(request: &str, searchpage: &str) -> Option<ServoUrl> {
    if request.is_empty() {
        return None;
    }
    ServoUrl::parse(&searchpage.replace("%s", &request)).ok()
}

fn try_as_domain(request: &str) -> Option<ServoUrl> {
    if is_domain_like(request) {
        return ServoUrl::parse(&format!("https://{}", request)).ok();
    }
    None
}

fn is_domain_like(s: &str) -> bool {
    if s.starts_with('/') {
        return false;
    }
    if s.contains('/') {
        return true;
    }
    let has_space = s.contains(' ');
    let starts_with_dot = s.starts_with('.');
    let has_dots = s.split('.').count() > 1;
    let is_localhost = s == "localhost";

    !has_space && !starts_with_dot && (has_dots || is_localhost)
}
