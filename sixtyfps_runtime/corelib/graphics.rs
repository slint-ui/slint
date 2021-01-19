/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#![warn(missing_docs)]
/*!
    Graphics Abstractions.

    This module contains the abstractions and convenience types used for rendering.

    The run-time library also makes use of [RenderingCache] to store the rendering primitives
    created by the backend in a type-erased manner.
*/
extern crate alloc;
use crate::properties::InterpolatedPropertyValue;
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::SharedString;
use auto_enums::auto_enum;
use const_field_offset::FieldOffsets;
use sixtyfps_corelib_macros::*;

/// 2D Rectangle
pub type Rect = euclid::default::Rect<f32>;
/// 2D Rectangle with integer coordinates
pub type IntRect = euclid::default::Rect<i32>;
/// 2D Point
pub type Point = euclid::default::Point2D<f32>;
/// 2D Size
pub type Size = euclid::default::Size2D<f32>;

/// RgbaColor stores the red, green, blue and alpha components of a color
/// with the precision of the generic parameter T. For example if T is f32,
/// the values are normalized between 0 and 1. If T is u8, they values range
/// is 0 to 255.
/// This is merely a helper class for use with [`Color`].
#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub struct RgbaColor<T> {
    /// The alpha component.
    pub alpha: T,
    /// The red channel.
    pub red: T,
    /// The green channel.
    pub green: T,
    /// The blue channel.
    pub blue: T,
}

/// Color represents a color in the SixtyFPS run-time, represented using 8-bit channels for
/// red, green, blue and the alpha (opacity).
/// It can be conveniently constructed and destructured using the to_ and from_ (a)rgb helper functions:
/// ```
/// # fn do_something_with_red_and_green(_:f32, _:f32) {}
/// # fn do_something_with_red(_:u8) {}
/// # use sixtyfps_corelib::graphics::{Color, RgbaColor};
/// # let some_color = Color::from_rgb_u8(0, 0, 0);
/// let col = some_color.to_argb_f32();
/// do_something_with_red_and_green(col.red, col.green);
///
/// let RgbaColor { red, blue, green, .. } = some_color.to_argb_u8();
/// do_something_with_red(red);
///
/// let new_col = Color::from(RgbaColor{ red: 0.5, green: 0.65, blue: 0.32, alpha: 1.});
/// ```
#[derive(Copy, Clone, PartialEq, Debug, Default)]
#[repr(C)]
pub struct Color {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl From<RgbaColor<u8>> for Color {
    fn from(col: RgbaColor<u8>) -> Self {
        Self { red: col.red, green: col.green, blue: col.blue, alpha: col.alpha }
    }
}

impl From<Color> for RgbaColor<u8> {
    fn from(col: Color) -> Self {
        RgbaColor { red: col.red, green: col.green, blue: col.blue, alpha: col.alpha }
    }
}

impl From<RgbaColor<u8>> for RgbaColor<f32> {
    fn from(col: RgbaColor<u8>) -> Self {
        Self {
            red: (col.red as f32) / 255.0,
            green: (col.green as f32) / 255.0,
            blue: (col.blue as f32) / 255.0,
            alpha: (col.alpha as f32) / 255.0,
        }
    }
}

impl From<Color> for RgbaColor<f32> {
    fn from(col: Color) -> Self {
        let u8col: RgbaColor<u8> = col.into();
        u8col.into()
    }
}

impl From<RgbaColor<f32>> for Color {
    fn from(col: RgbaColor<f32>) -> Self {
        Self {
            red: (col.red * 255.) as u8,
            green: (col.green * 255.) as u8,
            blue: (col.blue * 255.) as u8,
            alpha: (col.alpha * 255.) as u8,
        }
    }
}

impl Color {
    /// Construct a color from an integer encoded as `0xAARRGGBB`
    pub const fn from_argb_encoded(encoded: u32) -> Color {
        Self {
            red: (encoded >> 16) as u8,
            green: (encoded >> 8) as u8,
            blue: encoded as u8,
            alpha: (encoded >> 24) as u8,
        }
    }

    /// Returns `(alpha, red, green, blue)` encoded as u32
    pub fn as_argb_encoded(&self) -> u32 {
        ((self.red as u32) << 16)
            | ((self.green as u32) << 8)
            | (self.blue as u32)
            | ((self.alpha as u32) << 24)
    }

    /// Construct a color from the alpha, red, green and blue color channel parameters.
    pub fn from_argb_u8(alpha: u8, red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue, alpha }
    }

    /// Construct a color from the red, green and blue color channel parameters. The alpha
    /// channel will have the value 255.
    pub fn from_rgb_u8(red: u8, green: u8, blue: u8) -> Self {
        Self::from_argb_u8(255, red, green, blue)
    }

    /// Construct a color from the alpha, red, green and blue color channel parameters.
    pub fn from_argb_f32(alpha: f32, red: f32, green: f32, blue: f32) -> Self {
        RgbaColor { alpha, red, green, blue }.into()
    }

    /// Construct a color from the red, green and blue color channel parameters. The alpha
    /// channel will have the value 255.
    pub fn from_rgb_f32(red: f32, green: f32, blue: f32) -> Self {
        Self::from_argb_f32(1.0, red, green, blue)
    }

    /// Converts this color to an RgbaColor struct for easy destructuring.
    pub fn to_argb_u8(&self) -> RgbaColor<u8> {
        RgbaColor::from(*self)
    }

    /// Converts this color to an RgbaColor struct for easy destructuring.
    pub fn to_argb_f32(&self) -> RgbaColor<f32> {
        RgbaColor::from(*self)
    }

    /// Returns the red channel of the color as u8 in the range 0..255.
    pub fn red(self) -> u8 {
        self.red
    }

    /// Returns the green channel of the color as u8 in the range 0..255.
    pub fn green(self) -> u8 {
        self.green
    }

    /// Returns the blue channel of the color as u8 in the range 0..255.
    pub fn blue(self) -> u8 {
        self.blue
    }

    /// Returns the alpha channel of the color as u8 in the range 0..255.
    pub fn alpha(self) -> u8 {
        self.alpha
    }
}

impl InterpolatedPropertyValue for Color {
    fn interpolate(self, target_value: Self, t: f32) -> Self {
        Self {
            red: self.red.interpolate(target_value.red, t),
            green: self.green.interpolate(target_value.green, t),
            blue: self.blue.interpolate(target_value.blue, t),
            alpha: self.alpha.interpolate(target_value.alpha, t),
        }
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "argb({}, {}, {}, {})", self.alpha, self.red, self.green, self.blue)
    }
}

#[cfg(feature = "femtovg_backend")]
impl From<&Color> for femtovg::Color {
    fn from(col: &Color) -> Self {
        Self::rgba(col.red, col.green, col.blue, col.alpha)
    }
}

#[cfg(feature = "femtovg_backend")]
impl From<Color> for femtovg::Color {
    fn from(col: Color) -> Self {
        Self::rgba(col.red, col.green, col.blue, col.alpha)
    }
}

/// A resource is a reference to binary data, for example images. They can be accessible on the file
/// system or embedded in the resulting binary. Or they might be URLs to a web server and a downloaded
/// is necessary before they can be used.
#[derive(Clone, PartialEq, Debug)]
#[repr(u8)]
pub enum Resource {
    /// A resource that does not represent any data.
    None,
    /// A resource that points to a file in the file system
    AbsoluteFilePath(crate::SharedString),
    /// A resource that is embedded in the program and accessible via pointer
    /// The format is the same as in a file
    EmbeddedData(super::slice::Slice<'static, u8>),
    /// Raw ARGB
    #[allow(missing_docs)]
    EmbeddedRgbaImage { width: u32, height: u32, data: super::sharedvector::SharedVector<u32> },
}

impl Default for Resource {
    fn default() -> Self {
        Resource::None
    }
}

/// CachedGraphicsData allows the graphics backend to store an arbitrary piece of data associated with
/// an item, which is typically computed by accessing properties. The dependency_tracker is used to allow
/// for a lazy computation. Typically backends store either compute intensive data or handles that refer to
/// data that's stored in GPU memory.
pub struct CachedGraphicsData<T> {
    /// The backend specific data.
    pub data: T,
    /// The property tracker that should be used to evaluate whether the primitive needs to be re-created
    /// or not.
    pub dependency_tracker: core::pin::Pin<Box<crate::properties::PropertyTracker>>,
}

impl<T> CachedGraphicsData<T> {
    /// Creates a new TrackingRenderingPrimitive by evaluating the provided update_fn once, storing the returned
    /// rendering primitive and initializing the dependency tracker.
    pub fn new(update_fn: impl FnOnce() -> T) -> Self {
        let dependency_tracker = Box::pin(crate::properties::PropertyTracker::default());
        let data = dependency_tracker.as_ref().evaluate(update_fn);
        Self { data, dependency_tracker }
    }
}

/// The RenderingCache, in combination with CachedGraphicsData, allows backends to store data that's either
/// intensive to compute or has bad CPU locality. Backends typically keep a RenderingCache instance and use
/// the item's cached_rendering_data() integer as index in the vec_arena::Arena.
pub type RenderingCache<T> = vec_arena::Arena<CachedGraphicsData<T>>;

/// FontRequest collects all the developer-configurable properties for fonts, such as family, weight, etc.
/// It is submitted as a request to the platform font system (i.e. CoreText on macOS) and in exchange the
/// backend returns a Box<dyn Font>.
#[derive(Debug, Clone, PartialEq)]
#[repr(C)]
pub struct FontRequest {
    /// The name of the font family to be used, such as "Helvetica". An empty family name means the system
    /// default font family should be used.
    pub family: SharedString,
    /// If the weight is None, the the system default font weight should be used.
    pub weight: Option<i32>,
    /// If the pixel size is None, the system default font size should be used.
    pub pixel_size: Option<f32>,
}

/// The FontMetrics trait is constructed from a FontRequest by the graphics backend and supplied to text related
/// items in order to measure text.
pub trait FontMetrics {
    /// Returns the size of the given string in physical pixels.
    fn text_size(&self, text: &str) -> Size;
    /// Returns the (UTF-8) byte offset in the given text that refers to the character that contributed to
    /// the glyph cluster that's visually nearest to the given x coordinate. This is used for hit-testing,
    /// for example when receiving a mouse click into a text field. Then this function returns the "cursor"
    /// position.
    fn text_offset_for_x_position<'a>(&self, text: &'a str, x: f32) -> usize;
    /// Returns the height of the font. This is typically the sum of the ascent and the descent, resulting
    /// in the height that can fit the talltest glyphs of the font. Note that it is possible though that
    /// the font may include glyphs that exceed this.
    fn height(&self) -> f32;
}

#[repr(C)]
#[derive(FieldOffsets, Default, SixtyFPSElement, Clone, Debug, PartialEq)]
#[pin]
/// PathLineTo describes the event of moving the cursor on the path to the specified location
/// along a straight line.
pub struct PathLineTo {
    #[rtti_field]
    /// The x coordinate where the line should go to.
    pub x: f32,
    #[rtti_field]
    /// The y coordinate where the line should go to.
    pub y: f32,
}

#[repr(C)]
#[derive(FieldOffsets, Default, SixtyFPSElement, Clone, Debug, PartialEq)]
#[pin]
/// PathArcTo describes the event of moving the cursor on the path across an arc to the specified
/// x/y coordinates, with the specified x/y radius and additional properties.
pub struct PathArcTo {
    #[rtti_field]
    /// The x coordinate where the arc should end up.
    pub x: f32,
    #[rtti_field]
    /// The y coordinate where the arc should end up.
    pub y: f32,
    #[rtti_field]
    /// The radius on the x-axis of the arc.
    pub radius_x: f32,
    #[rtti_field]
    /// The radius on the y-axis of the arc.
    pub radius_y: f32,
    #[rtti_field]
    /// The rotation along the x-axis of the arc in degress.
    pub x_rotation: f32,
    #[rtti_field]
    /// large_arc indicates whether to take the long or the shorter path to complete the arc.
    pub large_arc: bool,
    #[rtti_field]
    /// sweep indicates the direction of the arc. If true, a clockwise direction is chosen,
    /// otherwise counter-clockwise.
    pub sweep: bool,
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
/// PathElement describes a single element on a path, such as move-to, line-to, etc.
pub enum PathElement {
    /// The LineTo variant describes a line.
    LineTo(PathLineTo),
    /// The PathArcTo variant describes an arc.
    ArcTo(PathArcTo),
    /// Indicates that the path should be closed now by connecting to the starting point.
    Close,
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
/// PathEvent is a low-level data structure describing the composition of a path. Typically it is
/// generated at compile time from a higher-level description, such as SVG commands.
pub enum PathEvent {
    /// The beginning of the path.
    Begin,
    /// A straight line on the path.
    Line,
    /// A quadratic bezier curve on the path.
    Quadratic,
    /// A cubic bezier curve on the path.
    Cubic,
    /// The end of the path that remains open.
    EndOpen,
    /// The end of a path that is closed.
    EndClosed,
}

struct ToLyonPathEventIterator<'a> {
    events_it: std::slice::Iter<'a, PathEvent>,
    coordinates_it: std::slice::Iter<'a, Point>,
    first: Option<&'a Point>,
    last: Option<&'a Point>,
}

impl<'a> Iterator for ToLyonPathEventIterator<'a> {
    type Item = lyon::path::Event<lyon::math::Point, lyon::math::Point>;
    fn next(&mut self) -> Option<Self::Item> {
        use lyon::path::Event;

        self.events_it.next().map(|event| match event {
            PathEvent::Begin => Event::Begin { at: self.coordinates_it.next().unwrap().clone() },
            PathEvent::Line => Event::Line {
                from: self.coordinates_it.next().unwrap().clone(),
                to: self.coordinates_it.next().unwrap().clone(),
            },
            PathEvent::Quadratic => Event::Quadratic {
                from: self.coordinates_it.next().unwrap().clone(),
                ctrl: self.coordinates_it.next().unwrap().clone(),
                to: self.coordinates_it.next().unwrap().clone(),
            },
            PathEvent::Cubic => Event::Cubic {
                from: self.coordinates_it.next().unwrap().clone(),
                ctrl1: self.coordinates_it.next().unwrap().clone(),
                ctrl2: self.coordinates_it.next().unwrap().clone(),
                to: self.coordinates_it.next().unwrap().clone(),
            },
            PathEvent::EndOpen => Event::End {
                first: self.first.unwrap().clone(),
                last: self.last.unwrap().clone(),
                close: false,
            },
            PathEvent::EndClosed => Event::End {
                first: self.first.unwrap().clone(),
                last: self.last.unwrap().clone(),
                close: true,
            },
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.events_it.size_hint()
    }
}

impl<'a> ExactSizeIterator for ToLyonPathEventIterator<'a> {}

struct TransformedLyonPathIterator<EventIt> {
    it: EventIt,
    transform: lyon::math::Transform,
}

impl<EventIt: Iterator<Item = lyon::path::Event<lyon::math::Point, lyon::math::Point>>> Iterator
    for TransformedLyonPathIterator<EventIt>
{
    type Item = lyon::path::Event<lyon::math::Point, lyon::math::Point>;
    fn next(&mut self) -> Option<Self::Item> {
        self.it.next().map(|ev| ev.transformed(&self.transform))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }
}

impl<EventIt: Iterator<Item = lyon::path::Event<lyon::math::Point, lyon::math::Point>>>
    ExactSizeIterator for TransformedLyonPathIterator<EventIt>
{
}

/// PathDataIterator is a data structure that acts as starting point for iterating
/// through the low-level events of a path. If the path was constructed from said
/// events, then it is a very thin abstraction. If the path was created from higher-level
/// elements, then an intermediate lyon path is required/built.
pub struct PathDataIterator<'a> {
    it: LyonPathIteratorVariant<'a>,
    transform: Option<lyon::math::Transform>,
}

enum LyonPathIteratorVariant<'a> {
    FromPath(lyon::path::Path),
    FromEvents(&'a crate::SharedVector<PathEvent>, &'a crate::SharedVector<Point>),
}

impl<'a> PathDataIterator<'a> {
    /// Create a new iterator for path traversal.
    #[auto_enum(Iterator)]
    pub fn iter(
        &'a self,
    ) -> impl Iterator<Item = lyon::path::Event<lyon::math::Point, lyon::math::Point>> + 'a {
        match &self.it {
            LyonPathIteratorVariant::FromPath(path) => self.apply_transform(path.iter()),
            LyonPathIteratorVariant::FromEvents(events, coordinates) => {
                self.apply_transform(ToLyonPathEventIterator {
                    events_it: events.iter(),
                    coordinates_it: coordinates.iter(),
                    first: coordinates.first(),
                    last: coordinates.last(),
                })
            }
        }
    }

    fn fit(&mut self, width: f32, height: f32) {
        if width > 0. || height > 0. {
            let br = lyon::algorithms::aabb::bounding_rect(self.iter());
            self.transform = Some(lyon::algorithms::fit::fit_rectangle(
                &br,
                &Rect::from_size(Size::new(width, height)),
                lyon::algorithms::fit::FitStyle::Min,
            ));
        }
    }
    #[auto_enum(Iterator)]
    fn apply_transform(
        &'a self,
        event_it: impl Iterator<Item = lyon::path::Event<lyon::math::Point, lyon::math::Point>> + 'a,
    ) -> impl Iterator<Item = lyon::path::Event<lyon::math::Point, lyon::math::Point>> + 'a {
        match self.transform {
            Some(transform) => TransformedLyonPathIterator { it: event_it, transform },
            None => event_it,
        }
    }
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
/// PathData represents a path described by either high-level elements or low-level
/// events and coordinates.
pub enum PathData {
    /// None is the variant when the path is empty.
    None,
    /// The Elements variant is used to make a Path from shared arrays of elements.
    Elements(crate::SharedVector<PathElement>),
    /// The Events variant describes the path as a series of low-level events and
    /// associated coordinates.
    Events(crate::SharedVector<PathEvent>, crate::SharedVector<Point>),
}

impl Default for PathData {
    fn default() -> Self {
        Self::None
    }
}

impl PathData {
    /// This function returns an iterator that allows traversing the path by means of lyon events.
    pub fn iter(&self) -> PathDataIterator {
        PathDataIterator {
            it: match self {
                PathData::None => LyonPathIteratorVariant::FromPath(lyon::path::Path::new()),
                PathData::Elements(elements) => LyonPathIteratorVariant::FromPath(
                    PathData::build_path(elements.as_slice().iter()),
                ),
                PathData::Events(events, coordinates) => {
                    LyonPathIteratorVariant::FromEvents(events, coordinates)
                }
            },
            transform: None,
        }
    }

    /// This function returns an iterator that allows traversing the path by means of lyon events.
    pub fn iter_fitted(&self, width: f32, height: f32) -> PathDataIterator {
        let mut it = self.iter();
        it.fit(width, height);
        it
    }

    fn build_path(element_it: std::slice::Iter<PathElement>) -> lyon::path::Path {
        use lyon::geom::SvgArc;
        use lyon::math::{Angle, Point, Vector};
        use lyon::path::traits::SvgPathBuilder;
        use lyon::path::ArcFlags;

        let mut path_builder = lyon::path::Path::builder().with_svg();
        for element in element_it {
            match element {
                PathElement::LineTo(PathLineTo { x, y }) => {
                    path_builder.line_to(Point::new(*x, *y));
                }
                PathElement::ArcTo(PathArcTo {
                    x,
                    y,
                    radius_x,
                    radius_y,
                    x_rotation,
                    large_arc,
                    sweep,
                }) => {
                    let radii = Vector::new(*radius_x, *radius_y);
                    let x_rotation = Angle::degrees(*x_rotation);
                    let flags = ArcFlags { large_arc: *large_arc, sweep: *sweep };
                    let to = Point::new(*x, *y);

                    let svg_arc = SvgArc {
                        from: path_builder.current_position(),
                        radii,
                        x_rotation,
                        flags,
                        to,
                    };

                    if svg_arc.is_straight_line() {
                        path_builder.line_to(to);
                    } else {
                        path_builder.arc_to(radii, x_rotation, flags, to)
                    }
                }
                PathElement::Close => path_builder.close(),
            }
        }

        path_builder.build()
    }
}

pub(crate) mod ffi {
    #![allow(unsafe_code)]

    use super::*;

    #[allow(non_camel_case_types)]
    type c_void = ();

    /// Expand Rect so that cbindgen can see it. ( is in fact euclid::default::Rect<f32>)
    #[cfg(cbindgen)]
    #[repr(C)]
    struct Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    }

    /// Expand IntRect so that cbindgen can see it. ( is in fact euclid::default::Rect<i32>)
    #[cfg(cbindgen)]
    #[repr(C)]
    struct IntRect {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    }

    /// Expand Point so that cbindgen can see it. ( is in fact euclid::default::Point2D<f32>)
    #[cfg(cbindgen)]
    #[repr(C)]
    struct Point {
        x: f32,
        y: f32,
    }

    /// Expand Size so that cbindgen can see it. ( is in fact euclid::default::Size2D<f32>)
    #[cfg(cbindgen)]
    #[repr(C)]
    struct Size {
        width: f32,
        height: f32,
    }

    #[no_mangle]
    /// This function is used for the low-level C++ interface to allocate the backing vector for a shared path element array.
    pub unsafe extern "C" fn sixtyfps_new_path_elements(
        out: *mut c_void,
        first_element: *const PathElement,
        count: usize,
    ) {
        let arr = crate::SharedVector::from(std::slice::from_raw_parts(first_element, count));
        core::ptr::write(out as *mut crate::SharedVector<PathElement>, arr.clone());
    }

    #[no_mangle]
    /// This function is used for the low-level C++ interface to allocate the backing vector for a shared path event array.
    pub unsafe extern "C" fn sixtyfps_new_path_events(
        out_events: *mut c_void,
        out_coordinates: *mut c_void,
        first_event: *const PathEvent,
        event_count: usize,
        first_coordinate: *const Point,
        coordinate_count: usize,
    ) {
        let events =
            crate::SharedVector::from(std::slice::from_raw_parts(first_event, event_count));
        core::ptr::write(out_events as *mut crate::SharedVector<PathEvent>, events.clone());
        let coordinates = crate::SharedVector::from(std::slice::from_raw_parts(
            first_coordinate,
            coordinate_count,
        ));
        core::ptr::write(out_coordinates as *mut crate::SharedVector<Point>, coordinates.clone());
    }
}
