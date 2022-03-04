// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use crate::fonts::FontMetrics;
use crate::{
    profiler, Devices, LogicalItemGeometry, LogicalLength, LogicalPoint, LogicalRect,
    PhysicalLength, PhysicalPoint, PhysicalRect, PhysicalSize, PointLengths, RectLengths,
    ScaleFactor, SizeLengths,
};
use alloc::collections::VecDeque;
use alloc::rc::Rc;
use alloc::{vec, vec::Vec};
use core::pin::Pin;
use derive_more::{Add, Mul, Sub};
use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::*;
use i_slint_core::graphics::{FontRequest, IntRect, PixelFormat, Rect as RectF};
use i_slint_core::item_rendering::PartialRenderingCache;
use i_slint_core::{Color, ImageInner, StaticTextures};
use integer_sqrt::IntegerSquareRoot;

use euclid::num::Zero;

type DirtyRegion = PhysicalRect;

pub fn render_window_frame(
    runtime_window: Rc<i_slint_core::window::Window>,
    background: Rgb888,
    devices: &mut dyn Devices,
    cache: &mut PartialRenderingCache,
) {
    let size = devices.screen_size();
    let mut scene = prepare_scene(runtime_window, size, devices, cache);

    /*for item in scene.future_items {
        match item.command {
            SceneCommand::Rectangle { color } => {
                embedded_graphics::primitives::Rectangle {
                    top_left: Point { x: item.x as _, y: item.y as _ },
                    size: Size { width: item.width as _, height: item.height as _ },
                }
                .into_styled(
                    embedded_graphics::primitives::PrimitiveStyleBuilder::new()
                        .fill_color(Rgb888::new(color.red(), color.green(), color.blue()))
                        .build(),
                )
                .draw(display)
                .unwrap();
            }
            SceneCommand::Texture { data, format, stride, source_width, source_height, color } => {
                let sx = item.width as f32 / source_width as f32;
                let sy = item.height as f32 / source_height as f32;
                let bpp = bpp(format) as usize;
                for y in 0..item.height {
                    let pixel_iter = (0..item.width).into_iter().map(|x| {
                        let pos = ((y as f32 / sy) as usize * stride as usize)
                            + (x as f32 / sx) as usize * bpp;
                        to_color(&data[pos..], format, color)
                    });

                    display
                        .fill_contiguous(
                            &embedded_graphics::primitives::Rectangle::new(
                                Point::new(item.x as i32, (item.y + y) as i32),
                                Size::new(item.width as u32, 1),
                            ),
                            pixel_iter,
                        )
                        .unwrap()
                }
            }
        }
    }*/

    let mut line_processing_profiler = profiler::Timer::new_stopped();
    let mut span_drawing_profiler = profiler::Timer::new_stopped();
    let mut screen_fill_profiler = profiler::Timer::new_stopped();

    let mut line_buffer = vec![background; size.width as usize];

    let dirty_region = scene
        .dirty_region
        .intersection(&PhysicalRect { origin: euclid::point2(0, 0), size })
        .unwrap_or_default();

    while scene.current_line < dirty_region.origin.y_length() + dirty_region.size.height_length() {
        line_buffer.fill(background);
        line_processing_profiler.start(devices);
        let line = scene.process_line();
        line_processing_profiler.stop(devices);
        if scene.current_line < dirty_region.origin.y_length() {
            // FIXME: ideally we should start with that coordinate and not call process_line for all the lines before
            continue;
        }
        span_drawing_profiler.start(devices);
        for span in line.spans.iter().rev() {
            match span.command {
                SceneCommand::Rectangle { color } => {
                    blend_buffer(
                        &mut line_buffer[span.pos.x as usize
                            ..(span.pos.x_length() + span.size.width_length()).get() as usize],
                        color,
                    );
                }
                SceneCommand::Texture { texture_index } => {
                    let SceneTexture { data, format, stride, source_size, color } =
                        scene.textures[texture_index as usize];
                    let source_size = source_size.cast::<usize>();
                    let span_size = span.size.cast::<usize>();
                    let bpp = bpp(format) as usize;
                    let y = (line.line - span.pos.y_length()).cast::<usize>();
                    let y_pos = (y.get() * source_size.height / span_size.height) * stride as usize;
                    for (x, pix) in line_buffer[span.pos.x as usize
                        ..(span.pos.x_length() + span.size.width_length()).get() as usize]
                        .iter_mut()
                        .enumerate()
                    {
                        let pos = y_pos + (x * source_size.width / span_size.width) * bpp;
                        *pix = match format {
                            PixelFormat::Rgb => {
                                Rgb888::new(data[pos + 0], data[pos + 1], data[pos + 2])
                            }
                            PixelFormat::Rgba => {
                                if color.alpha() == 0 {
                                    let a = (u8::MAX - data[pos + 3]) as u16;
                                    let b = data[pos + 3] as u16;
                                    Rgb888::new(
                                        ((pix.r() as u16 * a + data[pos + 0] as u16 * b) >> 8)
                                            as u8,
                                        ((pix.g() as u16 * a + data[pos + 1] as u16 * b) >> 8)
                                            as u8,
                                        ((pix.b() as u16 * a + data[pos + 2] as u16 * b) >> 8)
                                            as u8,
                                    )
                                } else {
                                    let a = (u8::MAX - data[pos + 3]) as u16;
                                    let b = data[pos + 3] as u16;
                                    Rgb888::new(
                                        ((pix.r() as u16 * a + color.red() as u16 * b) >> 8) as u8,
                                        ((pix.g() as u16 * a + color.green() as u16 * b) >> 8)
                                            as u8,
                                        ((pix.b() as u16 * a + color.blue() as u16 * b) >> 8) as u8,
                                    )
                                }
                            }
                            PixelFormat::AlphaMap => {
                                let a = (u8::MAX - data[pos]) as u16;
                                let b = data[pos] as u16;
                                Rgb888::new(
                                    ((pix.r() as u16 * a + color.red() as u16 * b) >> 8) as u8,
                                    ((pix.g() as u16 * a + color.green() as u16 * b) >> 8) as u8,
                                    ((pix.b() as u16 * a + color.blue() as u16 * b) >> 8) as u8,
                                )
                            }
                        }
                    }
                }
                SceneCommand::RoundedRectangle { rectangle_index } => {
                    /// This is an integer shifted by 4 bits.
                    /// Note: this is not a "fixed point" because multiplication and sqrt operation operate to
                    /// the shifted integer
                    #[derive(Clone, Copy, PartialEq, Ord, PartialOrd, Eq, Add, Sub, Mul)]
                    struct Shifted(u32);
                    impl Shifted {
                        const ONE: Self = Shifted(1 << 4);
                        pub fn new(value: impl TryInto<u32>) -> Self {
                            Self(value.try_into().map_err(|_| ()).unwrap() << 4)
                        }
                        pub fn floor(self) -> u32 {
                            self.0 >> 4
                        }
                        pub fn ceil(self) -> u32 {
                            (self.0 + Self::ONE.0 - 1) >> 4
                        }
                        pub fn saturating_sub(self, other: Self) -> Self {
                            Self(self.0.saturating_sub(other.0))
                        }
                        pub fn sqrt(self) -> Self {
                            Self(self.0.integer_sqrt())
                        }
                    }
                    impl core::ops::Mul for Shifted {
                        type Output = Shifted;
                        fn mul(self, rhs: Self) -> Self::Output {
                            Self(self.0 * rhs.0)
                        }
                    }

                    let pos_x = span.pos.x as usize;
                    let rr = &scene.rounded_rectangles[rectangle_index as usize];
                    let y1 = (line.line - span.pos.y_length()) + rr.top_clip;
                    let y2 = (span.pos.y_length() + span.size.height_length() - line.line)
                        + rr.bottom_clip
                        - PhysicalLength::new(1);
                    let y = y1.min(y2);
                    debug_assert!(y.get() >= 0,);
                    let border = Shifted::new(rr.width.get());
                    const ONE: Shifted = Shifted::ONE;
                    let anti_alias =
                        |x1: Shifted, x2: Shifted, process_pixel: &mut dyn FnMut(usize, u32)| {
                            // x1 and x2 are the coordinate on the top and bottom of the intersection of the pixel
                            // line and the curve.
                            // `process_pixel` be called for the coordinate in the array and a coverage between 0..255
                            // This algorithm just go linearly which is not perfect, but good enough.
                            for x in x1.floor()..x2.ceil() {
                                // the coverage is basically how much of the pixel should be used
                                let cov = ((ONE + Shifted::new(x) - x1).0 << 8) / (ONE + x2 - x1).0;
                                process_pixel(x as usize, cov);
                            }
                        };
                    let rev = |x: Shifted| {
                        (Shifted::new(span.size.width) + Shifted::new(rr.right_clip.get()))
                            .saturating_sub(x)
                    };
                    // x1 and x2 are the lower and upper x coordinate of the beginning of the border.
                    // x3 and x4 are the lower and upper x coordinate between the border and the inner part.
                    // The upper part is the intersection with the top of the pixel line, and lower is the
                    // intersection with the bottom of the pixel.  (or the opposite for the bottom corner)
                    // The coordinate are bit-shifted so we keep a fractional part to know how much of the pixel
                    // is covered for anti-aliasing. The area between x1 and x2, (and between x3 and x4) is partially covered
                    // and the `anti_alias` function will draw the pixel in there
                    let (x1, x2, x3, x4) = if y < rr.radius {
                        let r = Shifted::new(rr.radius.get());
                        // `y` is how far away from the center of the circle the current line is.
                        let y = r - Shifted::new(y.get());
                        // Circle equation: x = √(r² - y²)
                        // Coordinate from the left edge: x' = r - x
                        let x2 = r - (r * r).saturating_sub(y * y).sqrt();
                        let x1 = r - (r * r).saturating_sub((y - ONE) * (y - ONE)).sqrt();
                        let r2 = r.saturating_sub(border);
                        let x4 = r - (r2 * r2).saturating_sub(y * y).sqrt();
                        let x3 = r - (r2 * r2).saturating_sub((y - ONE) * (y - ONE)).sqrt();
                        (x1, x2, x3, x4)
                    } else {
                        (Shifted(0), Shifted(0), border, border)
                    };
                    // 1. The part between 0 and x1 (exclusive) is transparent
                    // 2. The part between x1 (inclusive) and x2 (rounded up) is anti aliased border
                    anti_alias(
                        x1.saturating_sub(Shifted::new(rr.left_clip.get())),
                        x2.saturating_sub(Shifted::new(rr.left_clip.get())),
                        &mut |x, cov| {
                            if x >= span.size.width as usize {
                                return;
                            }
                            let c =
                                if border == Shifted(0) { rr.inner_color } else { rr.border_color };
                            let alpha = ((c.alpha() as u32) * cov as u32) / 255;
                            let col =
                                Color::from_argb_u8(alpha as u8, c.red(), c.green(), c.blue());
                            blend_pixel(&mut line_buffer[pos_x + x], col)
                        },
                    );
                    if y < rr.width {
                        // up or down border (x2 .. x2)
                        if (x2 * 2).floor() as i16 + rr.right_clip.get()
                            < span.size.width + rr.left_clip.get()
                        {
                            blend_buffer(
                                &mut line_buffer[pos_x
                                    + x2.ceil()
                                        .saturating_sub(rr.left_clip.get() as u32)
                                        .min(span.size.width as u32)
                                        as usize
                                    ..pos_x + rev(x2).floor().min(span.size.width as u32) as usize],
                                rr.border_color,
                            )
                        }
                    } else {
                        if border > Shifted(0) {
                            // 3. draw the border (between x2 and x3)
                            if ONE + x2 <= x3 {
                                blend_buffer(
                                    &mut line_buffer[pos_x
                                        + x2.ceil()
                                            .saturating_sub(rr.left_clip.get() as u32)
                                            .min(span.size.width as u32)
                                            as usize
                                        ..pos_x
                                            + x3.floor()
                                                .saturating_sub(rr.left_clip.get() as u32)
                                                .min(span.size.width as u32)
                                                as usize],
                                    rr.border_color,
                                )
                            }
                            // 4. anti-aliasing for the contents (x3 .. x4)
                            anti_alias(
                                x3.saturating_sub(Shifted::new(rr.left_clip.get())),
                                x4.saturating_sub(Shifted::new(rr.left_clip.get())),
                                &mut |x, cov| {
                                    if x >= span.size.width as usize {
                                        return;
                                    }
                                    let col =
                                        interpolate_color(cov, rr.border_color, rr.inner_color);
                                    blend_pixel(&mut line_buffer[pos_x + x], col)
                                },
                            );
                        }
                        // 5. inside (x4 .. x4)
                        if (x4 * 2).ceil() as i16 + rr.right_clip.get()
                            <= span.size.width + rr.left_clip.get()
                        {
                            blend_buffer(
                                &mut line_buffer[pos_x
                                    + x4.ceil()
                                        .saturating_sub(rr.left_clip.get() as u32)
                                        .min(span.size.width as u32)
                                        as usize
                                    ..pos_x + rev(x4).floor().min(span.size.width as u32) as usize],
                                rr.inner_color,
                            )
                        }
                        if border > Shifted(0) {
                            // 6. border anti-aliasing: x4..x3
                            anti_alias(rev(x4), rev(x3), &mut |x, cov| {
                                if x >= span.size.width as usize {
                                    return;
                                }
                                let col = interpolate_color(cov, rr.inner_color, rr.border_color);
                                blend_pixel(&mut line_buffer[pos_x + x], col)
                            });
                            // 7. border x3 .. x2
                            if ONE + x2 <= x3 {
                                blend_buffer(
                                    &mut line_buffer[pos_x
                                        + rev(x3).ceil().min(span.size.width as u32) as usize
                                        ..pos_x
                                            + rev(x2).floor().min(span.size.width as u32) as usize
                                                as usize],
                                    rr.border_color,
                                )
                            }
                        }
                    }
                    // 8. anti-alias x2 .. x1
                    anti_alias(rev(x2), rev(x1), &mut |x, cov| {
                        if x >= span.size.width as usize {
                            return;
                        }
                        let c = if border == Shifted(0) { rr.inner_color } else { rr.border_color };
                        let alpha = ((c.alpha() as u32) * (255 - cov) as u32) / 255;
                        let col = Color::from_argb_u8(alpha as u8, c.red(), c.green(), c.blue());
                        blend_pixel(&mut line_buffer[pos_x + x], col)
                    });
                }
            }
        }
        span_drawing_profiler.stop(devices);
        screen_fill_profiler.start(devices);
        devices.fill_region(
            euclid::rect(dirty_region.origin.x, line.line.get() as i16, dirty_region.size.width, 1),
            &line_buffer[dirty_region.origin.x as usize
                ..(dirty_region.origin.x + dirty_region.size.width) as usize],
        );
        screen_fill_profiler.stop(devices);
    }

    line_processing_profiler.stop_profiling(devices, "line processing");
    span_drawing_profiler.stop_profiling(devices, "span drawing");
    screen_fill_profiler.stop_profiling(devices, "screen fill");
}

// a is between 0 and 255. When 0, we get color1, when 2 we get color2
fn interpolate_color(a: u32, color1: Color, color2: Color) -> Color {
    let b = 255 - a;

    let al1 = color1.alpha() as u32;
    let al2 = color2.alpha() as u32;

    let a_ = a * al2;
    let b_ = b * al1;
    let m = a_ + b_;

    if m == 0 {
        return Color::default();
    }

    let col = Color::from_argb_u8(
        (m / 255) as u8,
        ((b_ * color1.red() as u32 + a_ * color2.red() as u32) / m) as u8,
        ((b_ * color1.green() as u32 + a_ * color2.green() as u32) / m) as u8,
        ((b_ * color1.blue() as u32 + a_ * color2.blue() as u32) / m) as u8,
    );
    col
}

fn blend_buffer(to_fill: &mut [Rgb888], color: Color) {
    if color.alpha() == u8::MAX {
        to_fill.fill(to_rgb888_color_discard_alpha(color))
    } else {
        for pix in to_fill {
            blend_pixel(pix, color);
        }
    }
}

fn blend_pixel(pix: &mut Rgb888, color: Color) {
    let a = (u8::MAX - color.alpha()) as u16;
    let b = color.alpha() as u16;
    *pix = Rgb888::new(
        ((pix.r() as u16 * a + color.red() as u16 * b) / 255) as u8,
        ((pix.g() as u16 * a + color.green() as u16 * b) / 255) as u8,
        ((pix.b() as u16 * a + color.blue() as u16 * b) / 255) as u8,
    );
}

struct Scene {
    /// the next line to be processed
    current_line: PhysicalLength,

    /// Element that have `y > current_line`
    /// They must be sorted by `y` in reverse order (bottom to top)
    /// then by `z` top to bottom
    future_items: Vec<SceneItem>,

    /// The items that overlap with the current line, sorted by z top to bottom
    current_items: VecDeque<SceneItem>,

    /// Some staging buffer of scene item
    next_items: VecDeque<SceneItem>,

    textures: Vec<SceneTexture>,
    rounded_rectangles: Vec<RoundedRectangle>,

    dirty_region: DirtyRegion,
}

impl Scene {
    fn new(
        mut items: Vec<SceneItem>,
        textures: Vec<SceneTexture>,
        rounded_rectangles: Vec<RoundedRectangle>,
        dirty_region: DirtyRegion,
    ) -> Self {
        items.sort_unstable_by(|a, b| compare_scene_item(a, b).reverse());
        Self {
            future_items: items,
            current_line: PhysicalLength::zero(),
            current_items: Default::default(),
            next_items: Default::default(),
            textures,
            rounded_rectangles,
            dirty_region,
        }
    }

    /// Will generate a LineCommand for the current_line, remove all items that are done from the items
    fn process_line(&mut self) -> LineCommand {
        let mut command = vec![];
        // Take the next element from current_items or future_items
        loop {
            let a_next_z = self
                .future_items
                .last()
                .filter(|i| i.pos.y_length() == self.current_line)
                .map(|i| i.z);
            let b_next_z = self.current_items.front().map(|i| i.z);
            let item = match (a_next_z, b_next_z) {
                (Some(a), Some(b)) => {
                    if a > b {
                        self.future_items.pop()
                    } else {
                        self.current_items.pop_front()
                    }
                }
                (Some(_), None) => self.future_items.pop(),
                (None, Some(_)) => self.current_items.pop_front(),
                _ => break,
            };
            let item = item.unwrap();
            if item.pos.y_length() + item.size.height_length()
                > self.current_line + PhysicalLength::new(1)
            {
                self.next_items.push_back(item.clone());
            }
            command.push(item);
        }
        core::mem::swap(&mut self.next_items, &mut self.current_items);
        let line = self.current_line;
        self.current_line += PhysicalLength::new(1);
        LineCommand { spans: command, line }
    }
}

#[derive(Clone, Copy)]
struct SceneItem {
    pos: PhysicalPoint,
    size: PhysicalSize,
    // this is the order of the item from which it is in the item tree
    z: u16,
    command: SceneCommand,
}

struct LineCommand {
    line: PhysicalLength,
    // Fixme: we need to process these so we do not draw items under opaque regions
    spans: Vec<SceneItem>,
}

fn compare_scene_item(a: &SceneItem, b: &SceneItem) -> core::cmp::Ordering {
    // First, order by line (top to bottom)
    match a.pos.y.partial_cmp(&b.pos.y) {
        None | Some(core::cmp::Ordering::Equal) => {}
        Some(ord) => return ord,
    }
    // Then by the reverse z (front to back)
    match a.z.partial_cmp(&b.z) {
        None | Some(core::cmp::Ordering::Equal) => {}
        Some(ord) => return ord.reverse(),
    }

    // anything else, we don't care
    core::cmp::Ordering::Equal
}

#[derive(Clone, Copy)]
#[repr(u8)]
enum SceneCommand {
    Rectangle {
        color: Color,
    },
    /// texture_index is an index in the Scene::textures array
    Texture {
        texture_index: u16,
    },
    /// rectangle_index is an index in the Scene::rounded_rectangle array
    RoundedRectangle {
        rectangle_index: u16,
    },
}

struct SceneTexture {
    data: &'static [u8],
    format: PixelFormat,
    /// bytes between two lines in the source
    stride: u16,
    source_size: PhysicalSize,
    color: Color,
}

struct RoundedRectangle {
    radius: PhysicalLength,
    /// the border's width
    width: PhysicalLength,
    border_color: Color,
    inner_color: Color,
    /// The clips is the amount of pixels of the rounded rectangle that is clipped away.
    /// For example, if left_clip > width, then the left border will not be visible, and
    /// if left_clip > radius, then no radius will be seen in the left side
    left_clip: PhysicalLength,
    right_clip: PhysicalLength,
    top_clip: PhysicalLength,
    bottom_clip: PhysicalLength,
}

fn prepare_scene(
    runtime_window: Rc<i_slint_core::window::Window>,
    size: PhysicalSize,
    devices: &dyn Devices,
    cache: &mut PartialRenderingCache,
) -> Scene {
    let prepare_scene_profiler = profiler::Timer::new(devices);
    let factor = ScaleFactor::new(runtime_window.scale_factor());
    let prepare_scene = PrepareScene::new(size, factor, runtime_window.default_font_properties());
    let mut renderer = i_slint_core::item_rendering::PartialRenderer::new(cache, prepare_scene);

    runtime_window.draw_contents(|components| {
        for (component, origin) in components {
            renderer.compute_dirty_regions(component, *origin);
        }
        for (component, origin) in components {
            i_slint_core::item_rendering::render_component_items(component, &mut renderer, *origin);
        }
    });
    prepare_scene_profiler.stop_profiling(devices, "prepare_scene");
    let dirty_region =
        (euclid::Rect::from_untyped(&renderer.dirty_region.to_rect()) * factor).round_out().cast();
    let prepare_scene = renderer.into_inner();
    Scene::new(
        prepare_scene.items,
        prepare_scene.textures,
        prepare_scene.rounded_rectangles,
        dirty_region,
    )
}

struct PrepareScene {
    items: Vec<SceneItem>,
    textures: Vec<SceneTexture>,
    rounded_rectangles: Vec<RoundedRectangle>,
    state_stack: Vec<RenderState>,
    current_state: RenderState,
    scale_factor: ScaleFactor,
    default_font: FontRequest,
}

impl PrepareScene {
    fn new(size: PhysicalSize, scale_factor: ScaleFactor, default_font: FontRequest) -> Self {
        Self {
            items: vec![],
            rounded_rectangles: vec![],
            textures: vec![],
            state_stack: vec![],
            current_state: RenderState {
                alpha: 1.,
                offset: LogicalPoint::default(),
                clip: LogicalRect::new(LogicalPoint::default(), size.cast() / scale_factor),
            },
            scale_factor,
            default_font,
        }
    }

    fn should_draw(&self, rect: &LogicalRect) -> bool {
        !rect.size.is_empty()
            && self.current_state.alpha > 0.01
            && self.current_state.clip.intersects(rect)
    }

    fn new_scene_rectangle(&mut self, geometry: LogicalRect, color: Color) {
        self.new_scene_item(geometry, SceneCommand::Rectangle { color });
    }

    fn new_scene_texture(&mut self, geometry: LogicalRect, texture: SceneTexture) {
        let texture_index = self.textures.len() as u16;
        self.textures.push(texture);
        self.new_scene_item(geometry, SceneCommand::Texture { texture_index });
    }

    fn new_scene_item(&mut self, geometry: LogicalRect, command: SceneCommand) {
        let size = (geometry.size * self.scale_factor).cast();
        if !size.is_empty() {
            let z = self.items.len() as u16;
            let pos = ((geometry.origin + self.current_state.offset.to_vector())
                * self.scale_factor)
                .cast();
            self.items.push(SceneItem { pos, size, z, command });
        }
    }

    fn draw_image_impl(
        &mut self,
        geom: LogicalRect,
        source: &i_slint_core::graphics::Image,
        source_clip: IntRect,
        colorize: Color,
    ) {
        let image_inner: &ImageInner = source.into();
        match image_inner {
            ImageInner::None => (),
            ImageInner::AbsoluteFilePath(_) | ImageInner::EmbeddedData { .. } => {
                unimplemented!()
            }
            ImageInner::EmbeddedImage(_) => todo!(),
            ImageInner::StaticTextures(StaticTextures { size, data, textures, .. }) => {
                let sx = geom.width() / (size.width as f32);
                let sy = geom.height() / (size.height as f32);
                for t in textures.as_slice() {
                    if let Some(dest_rect) = t.rect.intersection(&source_clip).and_then(|r| {
                        r.intersection(
                            &self
                                .current_state
                                .clip
                                .to_untyped()
                                .scale(1. / sx, 1. / sy)
                                .round_in()
                                .cast(),
                        )
                    }) {
                        let actual_x = dest_rect.origin.x - t.rect.origin.x;
                        let actual_y = dest_rect.origin.y - t.rect.origin.y;
                        let stride = t.rect.width() as u16 * bpp(t.format);
                        self.new_scene_texture(
                            LogicalRect::from_untyped(&dest_rect.cast().scale(sx, sy)),
                            SceneTexture {
                                data: &data.as_slice()[(t.index
                                    + (stride as usize) * (actual_y as usize)
                                    + (bpp(t.format) as usize) * (actual_x as usize))..],
                                stride,
                                source_size: PhysicalSize::from_untyped(dest_rect.size.cast()),
                                format: t.format,
                                color: if colorize.alpha() > 0 { colorize } else { t.color },
                            },
                        );
                    }
                }
            }
        };
    }
}

#[derive(Clone, Copy)]
struct RenderState {
    alpha: f32,
    offset: LogicalPoint,
    clip: LogicalRect,
}

impl i_slint_core::item_rendering::ItemRenderer for PrepareScene {
    fn draw_rectangle(&mut self, rect: Pin<&i_slint_core::items::Rectangle>) {
        let geom = LogicalRect::new(LogicalPoint::default(), rect.logical_geometry().size_length());
        if self.should_draw(&geom) {
            let geom = match geom.intersection(&self.current_state.clip) {
                Some(geom) => geom,
                None => return,
            };

            // FIXME: gradients
            let color = rect.background().color();
            if color.alpha() == 0 {
                return;
            }
            self.new_scene_rectangle(geom, color);
        }
    }

    fn draw_border_rectangle(&mut self, rect: Pin<&i_slint_core::items::BorderRectangle>) {
        let geom = LogicalRect::new(LogicalPoint::default(), rect.logical_geometry().size_length());
        if self.should_draw(&geom) {
            let border = rect.border_width();
            let radius = rect.border_radius();
            // FIXME: gradients
            let color = rect.background().color();
            if radius > 0. {
                if let Some(clipped) = geom.intersection(&self.current_state.clip) {
                    let geom2 = (geom * self.scale_factor).cast::<i16>();
                    let clipped2 = (clipped * self.scale_factor).cast::<i16>();
                    let rectangle_index = self.rounded_rectangles.len() as u16;
                    self.rounded_rectangles.push(RoundedRectangle {
                        radius: (LogicalLength::new(radius) * self.scale_factor).cast(),
                        width: (LogicalLength::new(border) * self.scale_factor).cast(),
                        border_color: rect.border_color().color(),
                        inner_color: color,
                        top_clip: PhysicalLength::new(clipped2.min_y() - geom2.min_y()),
                        bottom_clip: PhysicalLength::new(geom2.max_y() - clipped2.max_y()),
                        left_clip: PhysicalLength::new(clipped2.min_x() - geom2.min_x()),
                        right_clip: PhysicalLength::new(geom2.max_x() - clipped2.max_x()),
                    });
                    self.new_scene_item(
                        clipped,
                        SceneCommand::RoundedRectangle { rectangle_index },
                    );
                }
                return;
            }

            if color.alpha() > 0 {
                if let Some(r) =
                    geom.inflate(-border, -border).intersection(&self.current_state.clip)
                {
                    self.new_scene_rectangle(r, color);
                }
            }
            if border > 0.01 {
                // FIXME: radius
                // FIXME: gradients
                let border_color = rect.border_color().color();
                if border_color.alpha() > 0 {
                    let mut add_border = |r: LogicalRect| {
                        if let Some(r) = r.intersection(&self.current_state.clip) {
                            self.new_scene_rectangle(r, border_color);
                        }
                    };
                    add_border(euclid::rect(0., 0., geom.width(), border));
                    add_border(euclid::rect(0., geom.height() - border, geom.width(), border));
                    add_border(euclid::rect(0., border, border, geom.height() - border - border));
                    add_border(euclid::rect(
                        geom.width() - border,
                        border,
                        border,
                        geom.height() - border - border,
                    ));
                }
            }
        }
    }

    fn draw_image(&mut self, image: Pin<&i_slint_core::items::ImageItem>) {
        let geom =
            LogicalRect::new(LogicalPoint::default(), image.logical_geometry().size_length());
        if self.should_draw(&geom) {
            self.draw_image_impl(
                geom,
                &image.source(),
                euclid::rect(0, 0, i32::MAX, i32::MAX),
                Default::default(),
            );
        }
    }

    fn draw_clipped_image(&mut self, image: Pin<&i_slint_core::items::ClippedImage>) {
        // when the source_clip size is empty, make it full
        let a = |v| if v == 0 { i32::MAX } else { v };

        let geom =
            LogicalRect::new(LogicalPoint::default(), image.logical_geometry().size_length());
        if self.should_draw(&geom) {
            self.draw_image_impl(
                geom,
                &image.source(),
                euclid::rect(
                    image.source_clip_x(),
                    image.source_clip_y(),
                    a(image.source_clip_width()),
                    a(image.source_clip_height()),
                ),
                image.colorize().color(),
            );
        }
    }

    fn draw_text(&mut self, text: Pin<&i_slint_core::items::Text>) {
        let font_request = text.unresolved_font_request().merge(&self.default_font);
        let (font, glyphs) = crate::fonts::match_font(&font_request, self.scale_factor);

        let color = text.color().color();

        let baseline_y = glyphs.ascent(font);

        for (glyph_baseline_x, glyph) in crate::fonts::glyphs_for_text(font, glyphs, &text.text()) {
            if let Some(dest_rect) = (PhysicalRect::new(
                PhysicalPoint::from_lengths(
                    glyph_baseline_x + glyph.x(),
                    baseline_y - glyph.y() - glyph.height(),
                ),
                glyph.size(),
            )
            .cast()
                / self.scale_factor)
                .intersection(&self.current_state.clip)
            {
                let stride = glyph.width().get() as u16;

                self.new_scene_texture(
                    dest_rect,
                    SceneTexture {
                        data: glyph.data().as_slice(),
                        stride,
                        source_size: glyph.size(),
                        format: PixelFormat::AlphaMap,
                        color,
                    },
                );
            }
        }
    }

    fn draw_text_input(&mut self, _text_input: Pin<&i_slint_core::items::TextInput>) {
        // TODO
    }

    #[cfg(feature = "std")]
    fn draw_path(&mut self, _path: Pin<&i_slint_core::items::Path>) {
        // TODO
    }

    fn draw_box_shadow(&mut self, _box_shadow: Pin<&i_slint_core::items::BoxShadow>) {
        // TODO
    }

    fn combine_clip(&mut self, other: RectF, _radius: f32, _border_width: f32) {
        match self.current_state.clip.intersection(&LogicalRect::from_untyped(&other)) {
            Some(r) => {
                self.current_state.clip = r;
            }
            None => {
                self.current_state.clip = LogicalRect::default();
            }
        };
        // TODO: handle radius and border
    }

    fn get_current_clip(&self) -> i_slint_core::graphics::Rect {
        self.current_state.clip.to_untyped()
    }

    fn translate(&mut self, x: f32, y: f32) {
        self.current_state.offset.x += x;
        self.current_state.offset.y += y;
        self.current_state.clip = self.current_state.clip.translate((-x, -y).into())
    }

    fn rotate(&mut self, _angle_in_degrees: f32) {
        todo!()
    }

    fn apply_opacity(&mut self, opacity: f32) {
        self.current_state.alpha *= opacity;
    }

    fn save_state(&mut self) {
        self.state_stack.push(self.current_state);
    }

    fn restore_state(&mut self) {
        self.current_state = self.state_stack.pop().unwrap();
    }

    fn scale_factor(&self) -> f32 {
        self.scale_factor.0
    }

    fn draw_cached_pixmap(
        &mut self,
        _item_cache: &i_slint_core::item_rendering::CachedRenderingData,
        _update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    ) {
        todo!()
    }

    fn draw_string(&mut self, _string: &str, _color: Color) {
        todo!()
    }

    fn window(&self) -> i_slint_core::window::WindowRc {
        unreachable!("this backend don't query the window")
    }

    fn as_any(&mut self) -> &mut dyn core::any::Any {
        self
    }
}

/// bytes per pixels
fn bpp(format: PixelFormat) -> u16 {
    match format {
        PixelFormat::Rgb => 3,
        PixelFormat::Rgba => 4,
        PixelFormat::AlphaMap => 1,
    }
}
/*
fn to_color(data: &[u8], format: PixelFormat, color: Color) -> Rgb888 {
    match format {
        PixelFormat::Rgba if color.alpha() > 0 => Rgb888::new(
            ((color.red() as u16 * data[3] as u16) >> 8) as u8,
            ((color.green() as u16 * data[3] as u16) >> 8) as u8,
            ((color.blue() as u16 * data[3] as u16) >> 8) as u8,
        ),
        PixelFormat::Rgb => Rgb888::new(data[0], data[1], data[2]),
        PixelFormat::Rgba => Rgb888::new(data[0], data[1], data[2]),
        PixelFormat::AlphaMap => Rgb888::new(
            ((color.red() as u16 * data[0] as u16) >> 8) as u8,
            ((color.green() as u16 * data[0] as u16) >> 8) as u8,
            ((color.blue() as u16 * data[0] as u16) >> 8) as u8,
        ),
    }
}*/

pub fn to_rgb888_color_discard_alpha(col: Color) -> Rgb888 {
    Rgb888::new(col.red(), col.green(), col.blue())
}
