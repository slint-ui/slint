// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]
#![cfg_attr(not(feature = "shared-fontdb"), no_std)]

pub mod builtin_structs;
pub mod enums;
pub mod key_codes;

#[cfg(feature = "shared-fontdb")]
pub mod sharedfontdb;
