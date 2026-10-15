// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#[derive(Clone, Copy)]
pub(super) struct VialAssets {
    pub empty: &'static [u16],
    pub liquid: &'static [u16],
    pub mask: &'static [u8],
}

#[cfg(all(feature = "ink-native-assets", target_endian = "little"))]
mod data {
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
    pub(super) static MASK1: Aligned<6720> = Aligned(*include_bytes!("assets/vial-1-mask.alpha"));
    pub(super) static EMPTY2: Aligned<13440> =
        Aligned(*include_bytes!("assets/vial-2-empty.rgb565"));
    pub(super) static LIQUID2: Aligned<13440> =
        Aligned(*include_bytes!("assets/vial-2-liquid.rgb565"));
    pub(super) static MASK2: Aligned<6720> = Aligned(*include_bytes!("assets/vial-2-mask.alpha"));
    pub(super) static EMPTY3: Aligned<13440> =
        Aligned(*include_bytes!("assets/vial-3-empty.rgb565"));
    pub(super) static LIQUID3: Aligned<13440> =
        Aligned(*include_bytes!("assets/vial-3-liquid.rgb565"));
    pub(super) static MASK3: Aligned<6720> = Aligned(*include_bytes!("assets/vial-3-mask.alpha"));
}

pub(super) fn vial(width: usize, height: usize, index: usize) -> Option<VialAssets> {
    #[cfg(all(feature = "ink-native-assets", target_endian = "little"))]
    if (width, height) == (48, 140) {
        let (empty, liquid, mask) = match index {
            0 => (&data::EMPTY0.0, &data::LIQUID0.0, &data::MASK0.0),
            1 => (&data::EMPTY1.0, &data::LIQUID1.0, &data::MASK1.0),
            2 => (&data::EMPTY2.0, &data::LIQUID2.0, &data::MASK2.0),
            3 => (&data::EMPTY3.0, &data::LIQUID3.0, &data::MASK3.0),
            _ => return None,
        };
        return Some(VialAssets {
            empty: bytemuck::cast_slice(empty),
            liquid: bytemuck::cast_slice(liquid),
            mask,
        });
    }
    let _ = (width, height, index);
    None
}
