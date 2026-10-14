// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#[cfg(target_arch = "xtensa")]
core::arch::global_asm!(include_str!("pie_fill.S"));

#[cfg(target_arch = "xtensa")]
unsafe extern "C" {
    fn box_fill_blocks(destination: *mut u16, pattern: *const u16, blocks: usize);
}

pub(super) fn fill(pixels: &mut [u16], color: u16) {
    #[cfg(target_arch = "xtensa")]
    if pixels.len() >= 32 {
        #[repr(align(16))]
        struct Pattern([u16; 8]);
        let pattern = Pattern([color; 8]);
        let head = ((16 - (pixels.as_ptr() as usize & 15)) & 15) / 2;
        pixels[..head].fill(color);
        let blocks = (pixels.len() - head) / 8;
        // Aligned complete vectors remain inside the original mutable slice.
        // The assembly preserves q0, CPENABLE, and interrupt state.
        unsafe {
            box_fill_blocks(pixels.as_mut_ptr().add(head), pattern.0.as_ptr(), blocks);
        }
        pixels[head + blocks * 8..].fill(color);
        return;
    }
    pixels.fill(color);
}

#[cfg(any(test, feature = "box-profile"))]
pub(super) fn verify() -> usize {
    #[repr(align(16))]
    struct Pixels([u16; 344]);
    let mut cases = 0;
    let mut check = |color: u16, offset: usize, length: usize| {
        let mut actual = Pixels([0xa123; 344]);
        let mut expected = actual.0;
        fill(&mut actual.0[offset..offset + length], color);
        expected[offset..offset + length].fill(color);
        assert_eq!(actual.0, expected, "offset={offset},length={length}");
        cases += 1;
    };
    for color in 0..=u16::MAX {
        check(color, color as usize % 8, 48);
    }
    for color in [0, 1, 0x7e0, 0xf81f, 0xffff] {
        for offset in 0..8 {
            for length in [0, 1, 7, 8, 15, 31, 32, 33, 63, 64, 295, 296, 320] {
                check(color, offset, length);
            }
        }
    }
    cases
}

#[cfg(test)]
mod tests {
    #[test]
    fn exhaustive_colors_preserve_boundary_canaries() {
        assert_eq!(super::verify(), 66056);
    }
    #[test]
    fn fills_preserve_every_alignment_and_boundary_canary() {
        #[repr(align(16))]
        struct Pixels([u16; 344]);
        for color in [0, 1, 0x7e0, 0xf81f, 0xffff] {
            for offset in 0..8 {
                for length in [0, 1, 7, 8, 15, 31, 32, 33, 63, 64, 295, 296, 320] {
                    let mut actual = Pixels([0xa123; 344]);
                    let mut expected = actual.0;
                    super::fill(&mut actual.0[offset..offset + length], color);
                    expected[offset..offset + length].fill(color);
                    assert_eq!(actual.0, expected, "offset={offset},length={length}");
                }
            }
        }
    }
}
