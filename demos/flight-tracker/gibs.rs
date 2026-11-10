// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore VIIRS

//! The map tiles of NASA's GIBS that the demo shows, for `tiles.rs`, and for `build.rs`, which
//! can embed them.

/// The zoom levels that the demo shows.
pub const LEVELS: usize = 3;
/// The first, coarsest of them. Its 0.035 degrees per pixel are 2.5 times as fine as the
/// globe's own imagery.
pub const FIRST_ZOOM: u32 = 4;

const GIBS: &str = "https://gibs.earthdata.nasa.gov/wmts/epsg4326/best";

/// The columns of tiles that cover the world at `zoom`, in half as many rows.
pub fn columns(zoom: u32) -> u32 {
    // GIBS covers the world with 2.5 by 1.25 tiles at zoom 0, rounded up, and at zoom 3 and
    // beyond with a whole number of them.
    10 << (zoom - 3)
}

/// The imagery of a tile.
#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Day,
    Night,
}

impl Kind {
    /// The address of a tile at GIBS, which has the globe's own imagery as tiles: the Blue
    /// Marble with relief and bathymetry, and the lights of the Black Marble.
    #[cfg_attr(not(any(target_arch = "wasm32", feature = "online-imagery")), allow(dead_code))]
    pub fn url(self, zoom: u32, row: u32, column: u32) -> String {
        match self {
            Kind::Day => format!(
                "{GIBS}/BlueMarble_ShadedRelief_Bathymetry/default/2004-08-01/500m/{zoom}/{row}/{column}.jpeg"
            ),
            Kind::Night => {
                format!(
                    "{GIBS}/VIIRS_Night_Lights/default/2016-01-01/500m/{zoom}/{row}/{column}.png"
                )
            }
        }
    }

    /// The path of a tile in a directory of tiles.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub fn path(self, zoom: u32, row: u32, column: u32) -> String {
        match self {
            Kind::Day => format!("{zoom}/{row}/{column}.jpg"),
            Kind::Night => format!("night/{zoom}/{row}/{column}.png"),
        }
    }
}
