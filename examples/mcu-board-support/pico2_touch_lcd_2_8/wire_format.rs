// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#[repr(align(4))]
pub struct PacketBytes<const N: usize>(pub [u8; N]);

impl<const N: usize> PacketBytes<N> {
    pub fn words(&self, offset: usize, length: usize) -> &[u16] {
        assert_eq!(offset % 2, 0);
        assert_eq!(length % 2, 0);
        let bytes = &self.0[offset..offset + length];
        unsafe { core::slice::from_raw_parts(bytes.as_ptr().cast::<u16>(), length / 2) }
    }
}

#[cfg_attr(target_arch = "arm", unsafe(link_section = ".data.ram_func"))]
#[inline(never)]
pub fn pack(
    frame: &[u8],
    stride: usize,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    out: &mut [u8],
) {
    assert_eq!(out.len(), width * height * 2);
    if width == 8 && stride % 2 == 0 && x % 2 == 0 {
        // Every u32 bit pattern is valid; align_to preserves disjoint slice ownership.
        let (head, source, tail) = unsafe { frame.align_to::<u32>() };
        let (out_head, dest, out_tail) = unsafe { out.align_to_mut::<u32>() };
        if head.is_empty() && tail.is_empty() && out_head.is_empty() && out_tail.is_empty() {
            for (row, dest) in dest.chunks_exact_mut(4).enumerate() {
                let start = (y + row) * (stride / 2) + x / 2;
                dest.copy_from_slice(&source[start..start + 4]);
            }
            return;
        }
    }
    for row in 0..height {
        let source = &frame[((y + row) * stride + x) * 2..((y + row) * stride + x + width) * 2];
        let dest = &mut out[row * width * 2..(row + 1) * width * 2];
        if width == 8 {
            dest[..16].copy_from_slice(&source[..16]);
        } else {
            dest.copy_from_slice(source);
        }
    }
}

#[cfg(test)]
fn apply(
    frame: &mut [u8],
    stride: usize,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    data: &[u8],
) -> bool {
    assert_eq!(data.len(), width * height * 2);
    let mut changed = false;
    for row in 0..height {
        let dest = &mut frame[((y + row) * stride + x) * 2..((y + row) * stride + x + width) * 2];
        let source = &data[row * width * 2..(row + 1) * width * 2];
        changed |= source != dest;
        if width == 8 {
            dest[..16].copy_from_slice(&source[..16]);
        } else {
            dest.copy_from_slice(source);
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halfword_dma_byte_swap_preserves_every_rgb565_word() {
        let mut packet = PacketBytes([0; 65536 * 2]);
        for (value, bytes) in packet.0.chunks_exact_mut(2).enumerate() {
            bytes.copy_from_slice(&(value as u16).to_be_bytes());
        }
        for (value, &word) in packet.words(0, packet.0.len()).iter().enumerate() {
            assert_eq!(word.to_be(), value as u16);
        }
        for (offset, length) in [(0, 0), (2, 2), (6, 42), (65534 * 2, 4)] {
            assert_eq!(packet.words(offset, length).len(), length / 2);
            assert_eq!(packet.words(offset, length).as_ptr() as usize % 2, 0);
        }
        for (offset, length) in [(1, 2), (0, 1), (packet.0.len(), 2)] {
            assert!(std::panic::catch_unwind(|| packet.words(offset, length)).is_err());
        }
    }

    #[test]
    fn packing_preserves_unaligned_sources_destinations_and_odd_strides() {
        let data: Vec<u8> = (0..320 * 240 * 2 + 4).map(|i| (i * 37 + i / 641) as u8).collect();
        for source_offset in 0..4 {
            let frame = &data[source_offset..source_offset + 320 * 240 * 2];
            for destination_offset in 0..4 {
                for stride in [319, 320] {
                    for x in [0, 1, 8, 151] {
                        for height in [0, 1, 191] {
                            let mut output = vec![0xa5; height * 16 + 8];
                            pack(
                                frame,
                                stride,
                                x,
                                44,
                                8,
                                height,
                                &mut output[destination_offset..destination_offset + height * 16],
                            );
                            for row in 0..height {
                                let start = ((44 + row) * stride + x) * 2;
                                assert_eq!(
                                    &output[destination_offset + row * 16
                                        ..destination_offset + (row + 1) * 16],
                                    &frame[start..start + 16]
                                );
                            }
                            assert!(output[..destination_offset].iter().all(|b| *b == 0xa5));
                            assert!(
                                output[destination_offset + height * 16..]
                                    .iter()
                                    .all(|b| *b == 0xa5)
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn strips_reconstruct_native_frame_and_detect_real_pixel_changes() {
        let source: Vec<u8> = (0..320 * 240 * 2).map(|i| (i * 37 + i / 641) as u8).collect();
        let mut delivered = vec![0u8; source.len()];
        for x in (0..320).step_by(8) {
            let mut strip = vec![0; 8 * 240 * 2];
            pack(&source, 320, x, 0, 8, 240, &mut strip);
            assert!(apply(&mut delivered, 320, x, 0, 8, 240, &strip));
        }
        assert_eq!(delivered, source);
        for (x, y, width, height) in [(0, 0, 1, 1), (151, 44, 8, 191), (317, 237, 3, 3)] {
            let mut strip = vec![0; width * height * 2];
            pack(&source, 320, x, y, width, height, &mut strip);
            assert!(!apply(&mut delivered, 320, x, y, width, height, &strip));
            let last = strip.len() - 1;
            strip[last] ^= 1;
            assert!(apply(&mut delivered, 320, x, y, width, height, &strip));
            assert_ne!(delivered, source);
            pack(&source, 320, x, y, width, height, &mut strip);
            assert!(apply(&mut delivered, 320, x, y, width, height, &strip));
            assert_eq!(delivered, source);
        }
    }
}
