// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#[derive(Clone, Copy)]
pub(super) struct VialAssets {
    pub empty: &'static [u16],
    pub liquid: &'static [u16],
    pub mask: &'static [u8],
    pub meniscus: &'static [[u8; 2]],
    pub opaque_rows: &'static [[u16; 2]],
}

pub(super) const fn meniscus(width: usize, x: usize, y: usize) -> [u8; 2] {
    let (mut outer, mut inner) = (0, 0);
    let rx = width as i64 * 8 - 16;
    let ry = 48i64;
    let mut sy = 2;
    while sy <= 14 {
        let mut sx = 2;
        while sx <= 14 {
            let dx = x as i64 * 16 + sx - width as i64 * 8;
            let dy = y as i64 * 16 + sy - 56;
            if dx * dx * ry * ry + dy * dy * rx * rx <= rx * rx * ry * ry {
                outer += 1;
            }
            let (ix, iy) = (rx - 12, ry - 12);
            if dx * dx * iy * iy + dy * dy * ix * ix <= ix * ix * iy * iy {
                inner += 1;
            }
            sx += 4;
        }
        sy += 4;
    }
    [(outer * 255 / 16) as u8, ((outer - inner) * 255 / 16) as u8]
}

#[cfg(all(feature = "ink-native-assets", target_endian = "little"))]
mod data {
    pub(super) static MENISCUS: [[u8; 2]; 48 * 7] = {
        let mut pixels = [[0; 2]; 48 * 7];
        let mut i = 0;
        while i < pixels.len() {
            pixels[i] = super::meniscus(48, i % 48, i / 48);
            i += 1;
        }
        pixels
    };
    pub(super) static OPAQUE_ROWS: [[u16; 2]; 140] = {
        let mask = include_bytes!("assets/vial-0-mask.alpha");
        let mut rows = [[0; 2]; 140];
        let mut y = 0;
        while y < 140 {
            let mut left = 0;
            while left < 48 && mask[y * 48 + left] != 255 {
                left += 1;
            }
            let mut right = left;
            while right < 48 && mask[y * 48 + right] == 255 {
                right += 1;
            }
            rows[y] = [left as u16, right as u16];
            y += 1;
        }
        rows
    };
    #[repr(align(4))]
    pub(super) struct Aligned<const N: usize>(pub [u8; N]);
    pub(super) static EMPTY0: Aligned<13440> =
        Aligned(*include_bytes!("assets/vial-0-empty.rgb565"));
    pub(super) static LIQUID0: Aligned<13440> =
        Aligned(*include_bytes!("assets/vial-0-liquid.rgb565"));
    pub(super) static MASK0: Aligned<6720> = Aligned(*include_bytes!("assets/vial-0-mask.alpha"));
    pub(super) static EMPTY1: Aligned<13440> =
        Aligned(*include_bytes!("assets/vial-1-empty.rgb565"));
    pub(super) static LIQUID1: Aligned<13440> =
        Aligned(*include_bytes!("assets/vial-1-liquid.rgb565"));
    pub(super) static EMPTY2: Aligned<13440> =
        Aligned(*include_bytes!("assets/vial-2-empty.rgb565"));
    pub(super) static LIQUID2: Aligned<13440> =
        Aligned(*include_bytes!("assets/vial-2-liquid.rgb565"));
    pub(super) static EMPTY3: Aligned<13440> =
        Aligned(*include_bytes!("assets/vial-3-empty.rgb565"));
    pub(super) static LIQUID3: Aligned<13440> =
        Aligned(*include_bytes!("assets/vial-3-liquid.rgb565"));
}

pub(super) fn vial(width: usize, height: usize, index: usize) -> Option<VialAssets> {
    #[cfg(all(feature = "ink-native-assets", target_endian = "little"))]
    if (width, height) == (48, 140) {
        let (empty, liquid) = match index {
            0 => (&data::EMPTY0.0, &data::LIQUID0.0),
            1 => (&data::EMPTY1.0, &data::LIQUID1.0),
            2 => (&data::EMPTY2.0, &data::LIQUID2.0),
            3 => (&data::EMPTY3.0, &data::LIQUID3.0),
            _ => return None,
        };
        return Some(VialAssets {
            empty: bytemuck::cast_slice(empty),
            liquid: bytemuck::cast_slice(liquid),
            mask: &data::MASK0.0,
            meniscus: &data::MENISCUS,
            opaque_rows: &data::OPAQUE_ROWS,
        });
    }
    let _ = (width, height, index);
    None
}
