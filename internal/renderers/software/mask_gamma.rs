// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Coverage ("mask gamma") correction for text rendering.
//!
//! Glyph coverage produced by the rasterizer is linear: a pixel that is half
//! covered by an outline gets the value 128. The blitters in
//! [`super::draw_functions`] however blend directly on gamma-encoded (sRGB)
//! channel values, treating them as if they were linear light. The result is
//! that anti-aliased edges come out too dark against a lighter background and
//! too light against a darker background; most visibly, light text on a dark
//! background renders noticeably thinner and dimmer than dark text on a light
//! background at the same size, and thinner than other renderers (Skia,
//! FreeType with its default settings, ...) draw the same glyphs.
//!
//! Instead of making the blend gamma-correct (which would be expensive per
//! pixel and would change the rendering of everything, not just text), do
//! what Skia does for its A8 glyph masks: pre-distort the coverage values
//! with a lookup table such that the gamma-incorrect blend produces the
//! gamma-correct result. The table depends on the text color and on the
//! background color behind it; the background is unknown at this point, so
//! like Skia we assume it is the perceptual inverse of the text color. That
//! guess is exact for the two most important cases (dark-on-light and
//! light-on-dark full-contrast text) and the correction smoothly approaches
//! the identity as text and assumed background converge, so it degrades
//! gracefully for mid-tones. An artificial contrast boost (Skia's
//! SK_GAMMA_CONTRAST = 0.5) is folded into the same tables; it tapers off to
//! zero for light text.
//!
//! The tables below are bit-identical to the ones Skia builds at runtime in
//! `SkTMaskGamma_build_correcting_lut()` (SkMaskGamma.cpp) with its default
//! configuration for 8-bit sRGB surfaces: sRGB device gamma, contrast 0.5
//! (quantized to 128/255 by SkScalerContextRec), and 3 bits of text luminance
//! (`SkTMaskGamma<3, 3, 3>`), i.e. 8 tables of 256 entries selected by the
//! top 3 bits of the text color's luminance. They are precomputed because
//! they never change and computing them requires floating point `powf`,
//! which keeps this usable in `no_std` builds; the unit test below rebuilds
//! them from the formula and verifies them.

// Keep 16 entries per row so an entry's index can be read off its position.
#[rustfmt::skip]
static GAMMA_TABLES: [[u8; 256]; 8] = [
    // luminance bucket 0 (text luminance 0)
    [
        0, 1, 1, 2, 3, 3, 4, 5, 5, 6, 7, 7, 8, 9, 9, 10, 
        11, 11, 12, 13, 13, 14, 15, 15, 16, 17, 17, 18, 19, 19, 20, 21, 
        21, 22, 23, 23, 24, 25, 26, 26, 27, 28, 28, 29, 30, 30, 31, 32, 
        32, 33, 34, 34, 35, 36, 37, 37, 38, 39, 39, 40, 41, 41, 42, 43, 
        44, 44, 45, 46, 46, 47, 48, 48, 49, 50, 51, 51, 52, 53, 53, 54, 
        55, 56, 56, 57, 58, 59, 59, 60, 61, 61, 62, 63, 64, 64, 65, 66, 
        67, 67, 68, 69, 69, 70, 71, 72, 72, 73, 74, 75, 75, 76, 77, 78, 
        78, 79, 80, 81, 81, 82, 83, 84, 85, 85, 86, 87, 88, 88, 89, 90, 
        91, 92, 92, 93, 94, 95, 95, 96, 97, 98, 99, 99, 100, 101, 102, 103, 
        103, 104, 105, 106, 107, 108, 108, 109, 110, 111, 112, 113, 113, 114, 115, 116, 
        117, 118, 118, 119, 120, 121, 122, 123, 124, 125, 125, 126, 127, 128, 129, 130, 
        131, 132, 133, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 
        146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 
        162, 163, 164, 166, 167, 168, 169, 170, 171, 173, 174, 175, 176, 177, 179, 180, 
        181, 183, 184, 185, 187, 188, 190, 191, 192, 194, 196, 197, 199, 200, 202, 204, 
        206, 208, 210, 212, 214, 216, 218, 221, 224, 226, 230, 233, 237, 242, 249, 255, 
    ],
    // luminance bucket 1 (text luminance 36)
    [
        0, 1, 1, 2, 3, 4, 4, 5, 6, 6, 7, 8, 8, 9, 10, 11, 
        11, 12, 13, 13, 14, 15, 16, 16, 17, 18, 18, 19, 20, 21, 21, 22, 
        23, 24, 24, 25, 26, 26, 27, 28, 29, 29, 30, 31, 32, 32, 33, 34, 
        35, 35, 36, 37, 38, 38, 39, 40, 40, 41, 42, 43, 43, 44, 45, 46, 
        46, 47, 48, 49, 50, 50, 51, 52, 53, 53, 54, 55, 56, 56, 57, 58, 
        59, 60, 60, 61, 62, 63, 63, 64, 65, 66, 67, 67, 68, 69, 70, 71, 
        71, 72, 73, 74, 75, 75, 76, 77, 78, 79, 79, 80, 81, 82, 83, 83, 
        84, 85, 86, 87, 88, 88, 89, 90, 91, 92, 93, 93, 94, 95, 96, 97, 
        98, 98, 99, 100, 101, 102, 103, 104, 105, 105, 106, 107, 108, 109, 110, 111, 
        112, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 122, 123, 124, 125, 
        126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 
        142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 
        158, 159, 160, 161, 163, 164, 165, 166, 167, 168, 169, 170, 172, 173, 174, 175, 
        176, 178, 179, 180, 181, 183, 184, 185, 186, 188, 189, 190, 192, 193, 194, 196, 
        197, 198, 200, 201, 203, 204, 206, 207, 209, 210, 212, 214, 215, 217, 219, 220, 
        222, 224, 226, 228, 229, 231, 233, 236, 238, 240, 242, 245, 247, 250, 252, 255, 
    ],
    // luminance bucket 2 (text luminance 73)
    [
        0, 1, 2, 2, 3, 4, 5, 6, 6, 7, 8, 9, 10, 10, 11, 12, 
        13, 14, 14, 15, 16, 17, 18, 18, 19, 20, 21, 22, 23, 23, 24, 25, 
        26, 27, 27, 28, 29, 30, 31, 32, 32, 33, 34, 35, 36, 37, 37, 38, 
        39, 40, 41, 42, 42, 43, 44, 45, 46, 47, 47, 48, 49, 50, 51, 52, 
        53, 53, 54, 55, 56, 57, 58, 58, 59, 60, 61, 62, 63, 64, 65, 65, 
        66, 67, 68, 69, 70, 71, 72, 72, 73, 74, 75, 76, 77, 78, 79, 79, 
        80, 81, 82, 83, 84, 85, 86, 87, 88, 88, 89, 90, 91, 92, 93, 94, 
        95, 96, 97, 98, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 
        110, 111, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 
        125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 
        141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 
        157, 158, 159, 160, 161, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 
        175, 176, 177, 178, 179, 180, 181, 182, 183, 185, 186, 187, 188, 189, 190, 192, 
        193, 194, 195, 196, 197, 199, 200, 201, 202, 204, 205, 206, 207, 208, 210, 211, 
        212, 213, 215, 216, 217, 219, 220, 221, 222, 224, 225, 226, 228, 229, 231, 232, 
        233, 235, 236, 237, 239, 240, 242, 243, 245, 246, 247, 249, 250, 252, 253, 255, 
    ],
    // luminance bucket 3 (text luminance 109)
    [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 
        15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 
        31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 
        47, 48, 49, 50, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 
        62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 
        78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 
        94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 
        110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 
        126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 
        142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 
        158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 
        174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 
        190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 
        206, 207, 208, 210, 211, 212, 213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 
        223, 224, 225, 226, 227, 228, 229, 230, 231, 232, 233, 234, 235, 236, 237, 238, 
        239, 240, 241, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252, 253, 254, 255, 
    ],
    // luminance bucket 4 (text luminance 146)
    [
        0, 1, 3, 4, 5, 7, 8, 9, 10, 12, 13, 14, 15, 17, 18, 19, 
        21, 22, 23, 24, 26, 27, 28, 29, 31, 32, 33, 34, 35, 37, 38, 39, 
        40, 41, 43, 44, 45, 46, 47, 49, 50, 51, 52, 53, 55, 56, 57, 58, 
        59, 60, 62, 63, 64, 65, 66, 67, 68, 70, 71, 72, 73, 74, 75, 76, 
        77, 79, 80, 81, 82, 83, 84, 85, 86, 87, 89, 90, 91, 92, 93, 94, 
        95, 96, 97, 98, 99, 100, 101, 103, 104, 105, 106, 107, 108, 109, 110, 111, 
        112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 
        128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 
        144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 
        160, 161, 162, 163, 164, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 
        175, 176, 177, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 186, 187, 188, 
        189, 190, 191, 192, 193, 194, 195, 195, 196, 197, 198, 199, 200, 201, 202, 202, 
        203, 204, 205, 206, 207, 208, 208, 209, 210, 211, 212, 213, 214, 214, 215, 216, 
        217, 218, 219, 219, 220, 221, 222, 223, 224, 224, 225, 226, 227, 228, 229, 229, 
        230, 231, 232, 233, 234, 234, 235, 236, 237, 238, 238, 239, 240, 241, 242, 242, 
        243, 244, 245, 246, 246, 247, 248, 249, 250, 250, 251, 252, 253, 253, 254, 255, 
    ],
    // luminance bucket 5 (text luminance 182)
    [
        0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 19, 21, 23, 25, 26, 28, 
        30, 32, 33, 35, 37, 38, 40, 41, 43, 45, 46, 48, 49, 51, 52, 54, 
        55, 57, 58, 59, 61, 62, 64, 65, 67, 68, 69, 71, 72, 73, 75, 76, 
        77, 79, 80, 81, 82, 84, 85, 86, 87, 89, 90, 91, 92, 94, 95, 96, 
        97, 98, 99, 101, 102, 103, 104, 105, 106, 107, 109, 110, 111, 112, 113, 114, 
        115, 116, 117, 118, 119, 120, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 
        132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 145, 146, 
        147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 157, 158, 159, 160, 161, 
        162, 163, 164, 165, 165, 166, 167, 168, 169, 170, 171, 171, 172, 173, 174, 175, 
        176, 176, 177, 178, 179, 180, 181, 181, 182, 183, 184, 185, 185, 186, 187, 188, 
        189, 189, 190, 191, 192, 193, 193, 194, 195, 196, 196, 197, 198, 199, 200, 200, 
        201, 202, 203, 203, 204, 205, 206, 206, 207, 208, 208, 209, 210, 211, 211, 212, 
        213, 214, 214, 215, 216, 216, 217, 218, 219, 219, 220, 221, 221, 222, 223, 223, 
        224, 225, 226, 226, 227, 228, 228, 229, 230, 230, 231, 232, 232, 233, 234, 234, 
        235, 236, 236, 237, 238, 238, 239, 240, 240, 241, 242, 242, 243, 244, 244, 245, 
        246, 246, 247, 247, 248, 249, 249, 250, 251, 251, 252, 253, 253, 254, 254, 255, 
    ],
    // luminance bucket 6 (text luminance 219)
    [
        0, 4, 8, 12, 16, 19, 22, 25, 28, 31, 33, 36, 38, 41, 43, 45, 
        47, 50, 52, 54, 56, 58, 60, 61, 63, 65, 67, 69, 70, 72, 74, 75, 
        77, 78, 80, 82, 83, 85, 86, 87, 89, 90, 92, 93, 94, 96, 97, 98, 
        100, 101, 102, 104, 105, 106, 107, 108, 110, 111, 112, 113, 114, 115, 117, 118, 
        119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 130, 131, 132, 133, 134, 135, 
        136, 137, 138, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 
        151, 151, 152, 153, 154, 155, 156, 157, 158, 158, 159, 160, 161, 162, 163, 163, 
        164, 165, 166, 167, 167, 168, 169, 170, 171, 171, 172, 173, 174, 175, 175, 176, 
        177, 178, 178, 179, 180, 181, 181, 182, 183, 184, 184, 185, 186, 186, 187, 188, 
        189, 189, 190, 191, 191, 192, 193, 194, 194, 195, 196, 196, 197, 198, 198, 199, 
        200, 200, 201, 202, 202, 203, 204, 204, 205, 206, 206, 207, 208, 208, 209, 209, 
        210, 211, 211, 212, 213, 213, 214, 215, 215, 216, 216, 217, 218, 218, 219, 219, 
        220, 221, 221, 222, 222, 223, 224, 224, 225, 225, 226, 227, 227, 228, 228, 229, 
        229, 230, 231, 231, 232, 232, 233, 233, 234, 235, 235, 236, 236, 237, 237, 238, 
        239, 239, 240, 240, 241, 241, 242, 242, 243, 243, 244, 245, 245, 246, 246, 247, 
        247, 248, 248, 249, 249, 250, 250, 251, 251, 252, 252, 253, 253, 254, 254, 255, 
    ],
    // luminance bucket 7 (text luminance 255)
    [
        0, 13, 22, 28, 34, 38, 42, 46, 50, 53, 56, 59, 61, 64, 66, 69, 
        71, 73, 75, 77, 79, 81, 83, 85, 86, 88, 90, 92, 93, 95, 96, 98, 
        99, 101, 102, 104, 105, 106, 108, 109, 110, 112, 113, 114, 115, 117, 118, 119, 
        120, 121, 122, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 
        137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 148, 149, 150, 151, 
        152, 153, 154, 155, 155, 156, 157, 158, 159, 159, 160, 161, 162, 163, 163, 164, 
        165, 166, 167, 167, 168, 169, 170, 170, 171, 172, 173, 173, 174, 175, 175, 176, 
        177, 178, 178, 179, 180, 180, 181, 182, 182, 183, 184, 185, 185, 186, 187, 187, 
        188, 189, 189, 190, 190, 191, 192, 192, 193, 194, 194, 195, 196, 196, 197, 197, 
        198, 199, 199, 200, 200, 201, 202, 202, 203, 203, 204, 205, 205, 206, 206, 207, 
        208, 208, 209, 209, 210, 210, 211, 212, 212, 213, 213, 214, 214, 215, 215, 216, 
        216, 217, 218, 218, 219, 219, 220, 220, 221, 221, 222, 222, 223, 223, 224, 224, 
        225, 226, 226, 227, 227, 228, 228, 229, 229, 230, 230, 231, 231, 232, 232, 233, 
        233, 234, 234, 235, 235, 236, 236, 237, 237, 238, 238, 238, 239, 239, 240, 240, 
        241, 241, 242, 242, 243, 243, 244, 244, 245, 245, 246, 246, 246, 247, 247, 248, 
        248, 249, 249, 250, 250, 251, 251, 251, 252, 252, 253, 253, 254, 254, 255, 255, 
    ],
];

/// Returns the coverage correction table for text of the given color.
///
/// The returned table maps linear glyph coverage to the alpha value that the
/// (gamma-incorrect) blitter blend must use to produce a gamma-correct
/// result, assuming the background is the inverse of the text color.
pub(crate) fn coverage_table(text_color: i_slint_core::Color) -> &'static [u8; 256] {
    // BT.709 luminance on gamma-encoded values, same fixed-point
    // coefficients as Skia's SkComputeLuminance (SkColorData.h).
    let lum = (text_color.red() as u32 * 54
        + text_color.green() as u32 * 183
        + text_color.blue() as u32 * 19)
        >> 8;
    &GAMMA_TABLES[(lum >> 5) as usize]
}

#[cfg(test)]
mod tests {
    /// Replicates Skia's SkTMaskGamma_build_correcting_lut() (SkMaskGamma.cpp)
    /// in f32 for the sRGB device gamma configuration.
    fn build_table(src_i: u8) -> [u8; 256] {
        fn srgb_to_linear(l: f32) -> f32 {
            if l <= 0.04045 { l / 12.92 } else { ((l + 0.055) / 1.055).powf(2.4) }
        }
        fn linear_to_srgb(l: f32) -> f32 {
            if l <= 0.0031308 { l * 12.92 } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 }
        }
        fn apply_contrast(srca: f32, contrast: f32) -> f32 {
            srca + ((1.0 - srca) * contrast * srca)
        }
        // Skia stores its 0.5 default contrast quantized to 8 bits
        // (SkScalerContextRec::InternalContrastFromExternal).
        const CONTRAST: f32 = 128.0 / 255.0;
        let mut table = [0u8; 256];
        let src = src_i as f32 / 255.0;
        let lin_src = srgb_to_linear(src);
        // The assumed background: the perceptual inverse of the text color.
        let dst = 1.0 - src;
        let lin_dst = srgb_to_linear(dst);
        // The contrast boost tapers off to 0 as the text becomes white.
        let adjusted_contrast = CONTRAST * lin_dst;
        let round = |x: f32| (x + 0.5).floor() as i32;
        if (src - dst).abs() < 1.0 / 256.0 {
            // Near-equal src/dst: the correction degenerates to contrast only.
            for (i, t) in table.iter_mut().enumerate() {
                let srca = apply_contrast(i as f32 / 255.0, adjusted_contrast);
                *t = round(255.0 * srca).clamp(0, 255) as u8;
            }
        } else {
            for (i, t) in table.iter_mut().enumerate() {
                let srca = apply_contrast(i as f32 / 255.0, adjusted_contrast);
                let dsta = 1.0 - srca;
                // The blend result we want, computed in linear space...
                let lin_out = lin_src * srca + dsta * lin_dst;
                let out = linear_to_srgb(lin_out);
                // ...then undo what the gamma-incorrect blend will do to it.
                let result = (out - dst) / (src - dst);
                *t = round(255.0 * result).clamp(0, 255) as u8;
            }
        }
        table
    }

    #[test]
    fn tables_match_formula() {
        for bucket in 0..8u32 {
            // sk_t_scale255<3>: representative 8-bit luminance of the bucket.
            let src_i = ((bucket << 5) | (bucket << 2) | (bucket >> 1)) as u8;
            assert_eq!(super::GAMMA_TABLES[bucket as usize], build_table(src_i), "bucket {bucket}");
        }
    }

    #[test]
    fn known_values() {
        // White text: strong gamma expansion, no contrast boost.
        let white = super::coverage_table(i_slint_core::Color::from_rgb_u8(255, 255, 255));
        assert_eq!(white[0], 0);
        assert_eq!(white[128], 188);
        assert_eq!(white[255], 255);
        // Black text: coverage is compressed instead.
        let black = super::coverage_table(i_slint_core::Color::from_rgb_u8(0, 0, 0));
        assert_eq!(black[128], 91);
        // Identity endpoints hold for every table.
        for t in super::GAMMA_TABLES.iter() {
            assert_eq!(t[0], 0);
            assert_eq!(t[255], 255);
        }
    }
}
