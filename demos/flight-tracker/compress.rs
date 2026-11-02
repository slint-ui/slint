// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore texel texels

//! Encoders for the GPU's block-compressed texture formats, BC1 for desktop GPUs and ETC2 for
//! mobile and embedded ones. Both store a block of 4 by 4 texels in 8 bytes.

/// The 16 texels of the block at `bx`, `by` of an RGBA image, repeating the last row or column
/// where the image ends inside the block.
fn block(rgba: &[u8], width: usize, height: usize, bx: usize, by: usize) -> [[f32; 3]; 16] {
    let mut texels = [[0.0; 3]; 16];
    for y in 0..4 {
        for x in 0..4 {
            let (sx, sy) = ((bx * 4 + x).min(width - 1), (by * 4 + y).min(height - 1));
            let i = (sy * width + sx) * 4;
            texels[y * 4 + x] = [rgba[i] as f32, rgba[i + 1] as f32, rgba[i + 2] as f32];
        }
    }
    texels
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

/// Encodes each 4 by 4 block of an RGBA image with `encode`, in rows of blocks, on all CPU
/// cores where there are threads.
fn encode_blocks(
    rgba: &[u8],
    width: usize,
    height: usize,
    encode: fn(&[[f32; 3]; 16]) -> [u8; 8],
) -> Vec<u8> {
    let (columns, rows) = (width.div_ceil(4), height.div_ceil(4));
    let mut out = vec![0u8; columns * rows * 8];
    let encode_rows = |first_row: usize, out: &mut [u8]| {
        for (k, block_out) in out.chunks_exact_mut(8).enumerate() {
            let (bx, by) = (k % columns, first_row + k / columns);
            block_out.copy_from_slice(&encode(&block(rgba, width, height, bx, by)));
        }
    };
    let threads = if cfg!(target_arch = "wasm32") {
        1
    } else {
        std::thread::available_parallelism().map_or(1, |n| n.get())
    };
    let rows_per_thread = rows.div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        for (index, chunk) in out.chunks_mut(rows_per_thread * columns * 8).enumerate() {
            let encode_rows = &encode_rows;
            if threads == 1 {
                encode_rows(index * rows_per_thread, chunk);
            } else {
                scope.spawn(move || encode_rows(index * rows_per_thread, chunk));
            }
        }
    });
    out
}

/// BC1: two RGB565 end colors per block, and 2 bits per texel that pick one of them or one of
/// the two colors a third and two thirds of the way between.
pub fn bc1(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
    encode_blocks(rgba, width, height, bc1_block)
}

fn to_565(color: [f32; 3]) -> u16 {
    let r = (color[0].clamp(0.0, 255.0) * 31.0 / 255.0).round() as u16;
    let g = (color[1].clamp(0.0, 255.0) * 63.0 / 255.0).round() as u16;
    let b = (color[2].clamp(0.0, 255.0) * 31.0 / 255.0).round() as u16;
    (r << 11) | (g << 5) | b
}

fn from_565(color: u16) -> [f32; 3] {
    let (r, g, b) = ((color >> 11) & 31, (color >> 5) & 63, color & 31);
    [((r << 3) | (r >> 2)) as f32, ((g << 2) | (g >> 4)) as f32, ((b << 3) | (b >> 2)) as f32]
}

fn bc1_block(texels: &[[f32; 3]; 16]) -> [u8; 8] {
    // The end colors are the extremes of the texels along the axis they spread most along.
    let mut mean = [0.0f32; 3];
    for texel in texels {
        for c in 0..3 {
            mean[c] += texel[c] / 16.0;
        }
    }
    let mut covariance = [[0.0f32; 3]; 3];
    for texel in texels {
        for i in 0..3 {
            for j in 0..3 {
                covariance[i][j] += (texel[i] - mean[i]) * (texel[j] - mean[j]);
            }
        }
    }
    let mut axis = [1.0f32, 1.0, 1.0];
    for _ in 0..8 {
        let next: [f32; 3] =
            std::array::from_fn(|i| (0..3).map(|j| covariance[i][j] * axis[j]).sum());
        let length = next.iter().map(|v| v * v).sum::<f32>().sqrt();
        if length < 1e-6 {
            break;
        }
        axis = next.map(|v| v / length);
    }
    let along = |texel: &[f32; 3]| (0..3).map(|c| (texel[c] - mean[c]) * axis[c]).sum::<f32>();
    let (low, high) = texels.iter().fold((f32::MAX, f32::MIN), |(low, high), texel| {
        (low.min(along(texel)), high.max(along(texel)))
    });
    let mut end0 = to_565(std::array::from_fn(|c| mean[c] + axis[c] * high));
    let mut end1 = to_565(std::array::from_fn(|c| mean[c] + axis[c] * low));
    // The first end color must be the larger one, or the block has only three colors.
    if end0 < end1 {
        std::mem::swap(&mut end0, &mut end1);
    }
    let mut indices = 0u32;
    if end0 != end1 {
        let (a, b) = (from_565(end0), from_565(end1));
        let palette: [[f32; 3]; 4] = [
            a,
            b,
            std::array::from_fn(|c| (2.0 * a[c] + b[c]) / 3.0),
            std::array::from_fn(|c| (a[c] + 2.0 * b[c]) / 3.0),
        ];
        for (i, texel) in texels.iter().enumerate() {
            let best = (0..4)
                .min_by(|&x, &y| {
                    distance(*texel, palette[x]).total_cmp(&distance(*texel, palette[y]))
                })
                .unwrap_or(0);
            indices |= (best as u32) << (2 * i);
        }
    }
    let mut out = [0u8; 8];
    out[0..2].copy_from_slice(&end0.to_le_bytes());
    out[2..4].copy_from_slice(&end1.to_le_bytes());
    out[4..8].copy_from_slice(&indices.to_le_bytes());
    out
}

/// ETC2 RGB, using its ETC1 modes: each half of a block has a base color, and each texel adds
/// one of four brightness steps from one of eight tables to it.
pub fn etc2(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
    encode_blocks(rgba, width, height, etc2_block)
}

const ETC_TABLES: [[i32; 2]; 8] =
    [[2, 8], [5, 17], [9, 29], [13, 42], [18, 60], [24, 80], [33, 106], [47, 183]];

/// The brightness steps of `table`, in the order of the 2-bit texel values.
fn etc_steps(table: usize) -> [i32; 4] {
    let [small, large] = ETC_TABLES[table];
    [small, large, -small, -large]
}

/// The texels of each half of a block: the left and right halves, or with `flip`, the top and
/// bottom ones.
fn etc_halves(flip: bool) -> [[usize; 8]; 2] {
    std::array::from_fn(|half| {
        std::array::from_fn(|k| {
            let (x, y) = if flip { (k % 4, half * 2 + k / 4) } else { (half * 2 + k / 4, k % 4) };
            y * 4 + x
        })
    })
}

/// The best table for the texels of one half around `base`, its error, and each texel's step.
/// Each texel takes the step closest to how much brighter it is than the base, which is the
/// best one unless the step clamps a channel.
fn etc_fit(texels: &[[f32; 3]; 16], half: &[usize; 8], base: [i32; 3]) -> (usize, f32, [u8; 8]) {
    let base_f = base.map(|c| c as f32);
    let offsets: [f32; 8] =
        half.map(|i| (0..3).map(|c| texels[i][c] - base_f[c]).sum::<f32>() / 3.0);
    let mut best = (0, f32::MAX, [0u8; 8]);
    for table in 0..8 {
        let steps = etc_steps(table);
        let mut error = 0.0;
        let mut picks = [0u8; 8];
        for (k, &i) in half.iter().enumerate() {
            let mut pick = 0;
            for s in 1..4 {
                if (offsets[k] - steps[s] as f32).abs() < (offsets[k] - steps[pick] as f32).abs() {
                    pick = s;
                }
            }
            picks[k] = pick as u8;
            error += distance(texels[i], base.map(|c| (c + steps[pick]).clamp(0, 255) as f32));
            if error >= best.1 {
                break;
            }
        }
        if error < best.1 {
            best = (table, error, picks);
        }
    }
    best
}

fn etc2_block(texels: &[[f32; 3]; 16]) -> [u8; 8] {
    let expand4 = |c: i32| (c << 4) | c;
    let expand5 = |c: i32| (c << 3) | (c >> 2);
    let mut best: Option<(f32, [u8; 8])> = None;
    for flip in [false, true] {
        let halves = etc_halves(flip);
        let means: [[f32; 3]; 2] = halves.map(|half| {
            std::array::from_fn(|c| half.iter().map(|&i| texels[i][c]).sum::<f32>() / 8.0)
        });
        // Differential mode: 5-bit base colors, the second one within -4..3 of the first.
        // Beyond that range, the bits mean one of ETC2's other modes, so it can't be used.
        let five = means.map(|m| m.map(|c| (c * 31.0 / 255.0).round().clamp(0.0, 31.0) as i32));
        let delta: [i32; 3] = std::array::from_fn(|c| five[1][c] - five[0][c]);
        let mut candidates = Vec::with_capacity(2);
        if delta.iter().all(|d| (-4..=3).contains(d)) {
            let bases = five.map(|color| color.map(expand5));
            let mut header = [0u8; 4];
            for c in 0..3 {
                header[c] = ((five[0][c] << 3) | (delta[c] & 7)) as u8;
            }
            header[3] = 2 | flip as u8;
            candidates.push((bases, header));
        }
        // Individual mode: two 4-bit base colors, less precise, so only when the base colors
        // differ too much for the differential mode.
        if candidates.is_empty() {
            let four = means.map(|m| m.map(|c| (c * 15.0 / 255.0).round().clamp(0.0, 15.0) as i32));
            let mut header = [0u8; 4];
            for c in 0..3 {
                header[c] = ((four[0][c] << 4) | four[1][c]) as u8;
            }
            header[3] = flip as u8;
            candidates.push((four.map(|color| color.map(expand4)), header));
        }

        for (bases, mut header) in candidates {
            let fits = [0, 1].map(|h| etc_fit(texels, &halves[h], bases[h]));
            let error = fits[0].1 + fits[1].1;
            if best.as_ref().is_some_and(|(best_error, _)| *best_error <= error) {
                continue;
            }
            header[3] |= ((fits[0].0 << 5) | (fits[1].0 << 2)) as u8;
            // The texel values, high bits first, bit `x * 4 + y` for the texel at `x`, `y`.
            let (mut high, mut low) = (0u16, 0u16);
            for (h, fit) in fits.iter().enumerate() {
                for (k, &i) in halves[h].iter().enumerate() {
                    let bit = (i % 4) * 4 + i / 4;
                    high |= ((fit.2[k] as u16 >> 1) & 1) << bit;
                    low |= (fit.2[k] as u16 & 1) << bit;
                }
            }
            let mut out = [0u8; 8];
            out[0..4].copy_from_slice(&header);
            out[4..6].copy_from_slice(&high.to_be_bytes());
            out[6..8].copy_from_slice(&low.to_be_bytes());
            best = Some((error, out));
        }
    }
    best.map_or([0; 8], |(_, out)| out)
}
