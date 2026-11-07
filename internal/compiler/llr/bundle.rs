// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! A compiled unit together with the files it needs at run time, in one byte buffer,
//! for an application that runs its `.slint` files without compiling them at startup.
//!
//! Both ends must be built from the same Slint version: [`from_bytes()`] rejects another one,
//! but can't tell two builds of the same version apart.

use super::CompilationUnit;
use super::serialize;
use alloc::string::String;
use alloc::vec::Vec;
use i_slint_common::pre_rendered::PreRenderedResource;
use serde::{Deserialize, Serialize};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Serialize, Deserialize)]
struct Bundle {
    version: String,
    unit: Vec<u8>,
    resources: Vec<Vec<u8>>,
    /// The custom fonts by the path that the unit registers them with.
    fonts: Vec<(String, Vec<u8>)>,
}

/// What [`from_bytes()`] reads.
pub struct LoadedBundle {
    pub unit: CompilationUnit,
    pub resources: Vec<Option<PreRenderedResource>>,
    pub fonts: Vec<(String, Vec<u8>)>,
}

/// Writes `unit`, the resources it refers to by index, and the font files it registers by path.
pub fn to_bytes(
    unit: &CompilationUnit,
    resources: &[Option<PreRenderedResource>],
    fonts: Vec<(String, Vec<u8>)>,
) -> postcard::Result<Vec<u8>> {
    let bundle = Bundle {
        version: VERSION.into(),
        unit: serialize::to_bytes(unit)?,
        resources: resources.iter().map(serialize::resource_to_bytes).collect::<Result<_, _>>()?,
        fonts,
    };
    postcard::to_allocvec(&bundle)
}

/// Reads what [`to_bytes()`] wrote, with the same Slint version.
pub fn from_bytes(bytes: &[u8]) -> Result<LoadedBundle, String> {
    let bundle: Bundle = postcard::from_bytes(bytes).map_err(|e| alloc::format!("{e}"))?;
    if bundle.version != VERSION {
        return Err(alloc::format!(
            "the unit is from Slint {}, not from Slint {VERSION}",
            bundle.version
        ));
    }
    Ok(LoadedBundle {
        unit: serialize::from_bytes(&bundle.unit).map_err(|e| alloc::format!("{e}"))?,
        resources: bundle
            .resources
            .iter()
            .map(|r| serialize::resource_from_bytes(r))
            .collect::<Result<_, _>>()
            .map_err(|e| alloc::format!("{e}"))?,
        fonts: bundle.fonts,
    })
}
