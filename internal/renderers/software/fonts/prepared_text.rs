// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{RenderableGlyph, pixelfont::PixelFont};
use crate::{PhysicalLength, PhysicalPoint, PhysicalRect};
use alloc::{collections::VecDeque, rc::Rc, string::String, vec::Vec};
use core::cell::RefCell;
use i_slint_core::textlayout::{FontMetrics, TextParagraphLayout};

const MAX_ENTRIES: usize = 16;
const MAX_GLYPHS: usize = 1024;

pub(crate) struct PreparedGlyph {
    pub glyph: RenderableGlyph,
    pub rect: PhysicalRect,
}

#[derive(PartialEq)]
struct LayoutKey {
    font: usize,
    glyph_set: usize,
    pixel_size: i16,
    size: [i16; 2],
    line_metrics: [Option<i16>; 2],
    modes: [u8; 4],
    single_line: bool,
    max_lines: Option<usize>,
}

impl LayoutKey {
    fn new(p: &TextParagraphLayout<'_, PixelFont>) -> Self {
        Self {
            font: p.layout.font.bitmap_font as *const _ as usize,
            glyph_set: p.layout.font.glyphs as *const _ as usize,
            pixel_size: p.layout.font.pixel_size.get(),
            size: [p.max_width.get(), p.max_height.get()],
            line_metrics: [
                p.layout.letter_spacing.map(|v| v.get()),
                p.layout.line_height.map(|v| v.get()),
            ],
            modes: [
                p.horizontal_alignment as u8,
                p.vertical_alignment as u8,
                p.wrap as u8,
                p.overflow as u8,
            ],
            single_line: p.single_line,
            max_lines: p.max_lines,
        }
    }
}

struct Entry {
    key: LayoutKey,
    text: String,
    glyphs: Rc<[PreparedGlyph]>,
}

#[derive(Default)]
struct Cache {
    entries: VecDeque<Entry>,
    glyph_count: usize,
}

fn prepare(p: &TextParagraphLayout<'_, PixelFont>) -> Rc<[PreparedGlyph]> {
    let mut result = Vec::new();
    p.layout_lines(
        &mut |glyphs, line_x, line_y, _, _| {
            let baseline = line_y + p.layout.half_leading() + p.layout.font.ascent();
            for positioned in glyphs {
                let bitmap = &p.layout.font.glyphs.glyph_data
                    [PixelFont::glyph_id_to_glyph_index(positioned.glyph_id)];
                let Some(glyph) = p.layout.font.render_bitmap_glyph(bitmap) else { continue };
                let gl_x = PhysicalLength::new((-glyph.x).truncate() as i16);
                let gl_y = PhysicalLength::new(glyph.y.truncate() as i16);
                let rect = PhysicalRect::new(
                    PhysicalPoint::from_lengths(
                        line_x + positioned.x - gl_x,
                        baseline - gl_y - glyph.height,
                    ),
                    glyph.size(),
                );
                result.push(PreparedGlyph { glyph, rect });
            }
            core::ops::ControlFlow::Continue(())
        },
        None,
    );
    result.into()
}

impl Cache {
    #[cfg_attr(all(feature = "embedded-ram-functions", target_arch = "arm"), allow(unsafe_code))]
    #[cfg_attr(
        all(feature = "embedded-ram-functions", target_arch = "arm"),
        unsafe(link_section = ".data.ram_func")
    )]
    #[cfg_attr(all(feature = "embedded-ram-functions", target_arch = "arm"), inline(never))]
    fn get(&mut self, p: &TextParagraphLayout<'_, PixelFont>) -> Option<Rc<[PreparedGlyph]>> {
        if p.string.len() > 256 {
            return None;
        }
        let key = LayoutKey::new(p);
        if let Some(index) = self.entries.iter().position(|e| e.key == key && e.text == p.string) {
            #[cfg(feature = "embedded-profile")]
            {
                crate::embedded_profile::text_cache_event(crate::embedded_profile::TextCacheEvent::Hit);
                crate::embedded_profile::text_cache_size(self.entries.len(), self.glyph_count);
            }
            let entry = self.entries.remove(index).unwrap();
            let result = entry.glyphs.clone();
            self.entries.push_back(entry);
            return Some(result);
        }
        #[cfg(feature = "embedded-profile")]
        crate::embedded_profile::text_cache_event(crate::embedded_profile::TextCacheEvent::Miss);
        let glyphs = prepare(p);
        if glyphs.len() > MAX_GLYPHS {
            return Some(glyphs);
        }
        while self.entries.len() >= MAX_ENTRIES || self.glyph_count + glyphs.len() > MAX_GLYPHS {
            #[cfg(feature = "embedded-profile")]
            crate::embedded_profile::text_cache_event(crate::embedded_profile::TextCacheEvent::Evict);
            self.glyph_count -= self.entries.pop_front().unwrap().glyphs.len();
        }
        self.glyph_count += glyphs.len();
        self.entries.push_back(Entry { key, text: p.string.into(), glyphs: glyphs.clone() });
        #[cfg(feature = "embedded-profile")]
        crate::embedded_profile::text_cache_size(self.entries.len(), self.glyph_count);
        Some(glyphs)
    }
}

i_slint_core::thread_local! {
    static CACHE: RefCell<Cache> = Default::default();
}

pub(crate) fn get(p: &TextParagraphLayout<'_, PixelFont>) -> Option<Rc<[PreparedGlyph]>> {
    CACHE.with(|cache| cache.borrow_mut().get(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_core::graphics::{BitmapFont, BitmapGlyph, BitmapGlyphs};
    use i_slint_core::items::{
        TextHorizontalAlignment, TextOverflow, TextVerticalAlignment, TextWrap,
    };
    use i_slint_core::slice::Slice;
    use i_slint_core::textlayout::TextLayout;

    static GLYPHS: BitmapGlyphs = BitmapGlyphs {
        pixel_size: 12,
        glyph_data: Slice::from_slice(&[
            BitmapGlyph {
                x: -32,
                y: -64,
                width: 2,
                height: 2,
                x_advance: 4 * 64,
                data: Slice::from_slice(&[0, 81, 173, 255]),
            },
            BitmapGlyph {
                x: -32,
                y: -64,
                width: 2,
                height: 2,
                x_advance: 4 * 64,
                data: Slice::from_slice(&[0, 81, 173, 255]),
            },
        ]),
    };
    static FONT: BitmapFont = BitmapFont {
        family_name: Slice::from_slice(b"Prepared text test"),
        character_map: Slice::from_slice(&['A', 'é']),
        units_per_em: 1000.,
        ascent: 800.,
        descent: -200.,
        x_height: 500.,
        cap_height: 700.,
        glyphs: Slice::from_slice(&[]),
        weight: 400,
        italic: false,
        sdf: false,
    };

    fn compare(a: &[PreparedGlyph], b: &[PreparedGlyph]) {
        assert_eq!(a.len(), b.len());
        for (a, b) in a.iter().zip(b) {
            assert_eq!(a.rect, b.rect);
            assert_eq!((a.glyph.x.0, a.glyph.y.0), (b.glyph.x.0, b.glyph.y.0));
            assert_eq!(a.glyph.pixel_stride, b.glyph.pixel_stride);
        }
    }

    #[test]
    fn prepared_text_invalidates_layout_inputs_and_evicts() {
        let font =
            PixelFont { bitmap_font: &FONT, glyphs: &GLYPHS, pixel_size: PhysicalLength::new(12) };
        let mut p = TextParagraphLayout {
            string: "Aé Aé Aé",
            layout: TextLayout { font: &font, letter_spacing: None, line_height: None },
            max_width: PhysicalLength::new(80),
            max_height: PhysicalLength::new(80),
            horizontal_alignment: TextHorizontalAlignment::Left,
            vertical_alignment: TextVerticalAlignment::Top,
            wrap: TextWrap::NoWrap,
            overflow: TextOverflow::Clip,
            single_line: false,
            max_lines: None,
        };
        let mut cache = Cache::default();
        let initial = cache.get(&p).unwrap();
        assert!(!initial.is_empty());
        assert!(Rc::ptr_eq(&initial, &cache.get(&p).unwrap()));
        for width in [12, 24, 80] {
            for alignment in [
                TextHorizontalAlignment::Left,
                TextHorizontalAlignment::Center,
                TextHorizontalAlignment::Right,
            ] {
                for wrap in [TextWrap::NoWrap, TextWrap::WordWrap] {
                    p.max_width = PhysicalLength::new(width);
                    p.horizontal_alignment = alignment;
                    p.wrap = wrap;
                    compare(&cache.get(&p).unwrap(), &prepare(&p));
                    assert!(cache.entries.len() <= MAX_ENTRIES);
                    assert!(cache.glyph_count <= MAX_GLYPHS);
                }
            }
        }
        p.layout.letter_spacing = Some(PhysicalLength::new(2));
        p.layout.line_height = Some(PhysicalLength::new(20));
        p.max_lines = Some(1);
        p.overflow = TextOverflow::Elide;
        p.vertical_alignment = TextVerticalAlignment::Center;
        p.max_height = PhysicalLength::new(24);
        compare(&cache.get(&p).unwrap(), &prepare(&p));
        p.string = "éA";
        compare(&cache.get(&p).unwrap(), &prepare(&p));
        let long = "A".repeat(257);
        p.string = &long;
        assert!(cache.get(&p).is_none());
    }
}
