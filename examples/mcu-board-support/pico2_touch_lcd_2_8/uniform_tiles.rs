// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

pub const COLUMNS: usize = 40;
pub const ROWS: usize = 30;
pub const TILE: usize = 8;
pub const CACHE_LEN: usize = COLUMNS * ROWS;
const UNIFORM: u32 = 1 << 16;

pub fn plan(
    pixels: &[u16],
    cache: &mut [u32],
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) -> [u32; COLUMNS] {
    let mut columns = [0; COLUMNS];
    // u32 accepts every bit pattern; an unaligned u16 buffer retains the scalar path.
    let (head, words, tail) = unsafe { pixels.align_to::<u32>() };
    let aligned = head.is_empty() && tail.is_empty();
    for column in x / TILE..(x + width).div_ceil(TILE) {
        for row in y / TILE..(y + height).div_ceil(TILE) {
            let origin = row * TILE * 320 + column * TILE;
            let color = pixels[origin];
            let uniform = if aligned {
                let pattern = u32::from(color) * 0x10001;
                (0..TILE).all(|dy| {
                    words[origin / 2 + dy * 160..origin / 2 + dy * 160 + 4]
                        .iter()
                        .all(|word| *word == pattern)
                })
            } else {
                (0..TILE).all(|dy| {
                    pixels[origin + dy * 320..origin + dy * 320 + TILE]
                        .iter()
                        .all(|pixel| *pixel == color)
                })
            };
            let state = if uniform { UNIFORM | u32::from(color) } else { 0 };
            let previous = core::mem::replace(&mut cache[row * COLUMNS + column], state);
            if state == 0 || state != previous {
                columns[column] |= 1 << row;
            }
        }
    }
    columns
}

pub fn rectangles(columns: &[u32; COLUMNS]) -> impl Iterator<Item = (u16, u16, u16)> + '_ {
    columns.iter().enumerate().flat_map(|(column, &mask)| {
        let mut mask = mask;
        core::iter::from_fn(move || {
            if mask == 0 {
                return None;
            }
            let start = mask.trailing_zeros();
            let mut end = start + (mask >> start).trailing_ones();
            loop {
                let remaining = mask & !((1u32 << end) - 1);
                if remaining == 0 {
                    break;
                }
                let next = remaining.trailing_zeros();
                if next - end > 4 {
                    break;
                }
                end = next + (mask >> next).trailing_ones();
            }
            mask &= !(((1u32 << end) - 1) & !((1u32 << start) - 1));
            Some((
                (column * TILE) as u16,
                (start as usize * TILE) as u16,
                ((end - start) as usize * TILE) as u16,
            ))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_and_halfword_aligned_sources_produce_identical_plans() {
        let mut source = vec![0u16; 320 * 240];
        let mut shifted = vec![0u16; source.len() + 1];
        let mut first_cache = vec![0; CACHE_LEN];
        let mut second_cache = vec![0; CACHE_LEN];
        for color in [0, 1, 0x1234, 0xffff] {
            source.fill(color);
            for frame in 0..20 {
                let index = frame * 641;
                source[index] ^= 0x55;
                shifted[1..].copy_from_slice(&source);
                assert_eq!(
                    plan(&source, &mut first_cache, 0, 0, 320, 240),
                    plan(&shifted[1..], &mut second_cache, 0, 0, 320, 240)
                );
                assert_eq!(first_cache, second_cache);
            }
        }
    }

    #[test]
    fn only_identical_uniform_tiles_are_skipped() {
        let mut pixels = vec![17u16; 320 * 240];
        let mut cache = vec![0; CACHE_LEN];
        assert_eq!(plan(&pixels, &mut cache, 0, 0, 320, 240), [(1 << ROWS) - 1; COLUMNS]);
        assert_eq!(plan(&pixels, &mut cache, 0, 0, 320, 240), [0; COLUMNS]);
        pixels[12 * 320 + 9] = 99;
        let changed = plan(&pixels, &mut cache, 9, 12, 1, 1);
        assert_eq!(rectangles(&changed).collect::<Vec<_>>(), [(8, 8, 8)]);
        assert_eq!(plan(&pixels, &mut cache, 9, 12, 1, 1), changed);
        pixels[12 * 320 + 9] = 17;
        assert_eq!(plan(&pixels, &mut cache, 9, 12, 1, 1), changed);
        assert_eq!(plan(&pixels, &mut cache, 9, 12, 1, 1), [0; COLUMNS]);
    }

    #[test]
    fn planned_uploads_preserve_every_pixel_across_partial_changes() {
        let mut source = vec![0u16; 320 * 240];
        let mut delivered = vec![99u16; source.len()];
        let mut cache = vec![0; CACHE_LEN];
        for frame in 0..80 {
            let (x, y, width, height) = if frame == 0 {
                (0, 0, 320, 240)
            } else {
                ((frame * 37) % 270, (frame * 19) % 210, 49, 29)
            };
            for dy in y..y + height {
                for dx in x..x + width {
                    source[dy * 320 + dx] =
                        if frame % 3 == 0 { 0 } else { ((dx * dy + frame) % 65536) as u16 };
                }
            }
            let columns = plan(&source, &mut cache, x, y, width, height);
            for (x, y, height) in rectangles(&columns) {
                for dy in usize::from(y)..usize::from(y + height) {
                    let start = dy * 320 + usize::from(x);
                    delivered[start..start + TILE].copy_from_slice(&source[start..start + TILE]);
                }
            }
            assert_eq!(source, delivered, "frame {frame}");
        }
    }
}
