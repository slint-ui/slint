use core::ptr::NonNull;
use vtable::*;

pub type Rect = euclid::default::Rect<f32>;
pub type Point = euclid::default::Point2D<f32>;
pub type Size = euclid::default::Size2D<f32>;

/// Expand Rect so that cbindgen can see it. ( is in fact euclid::default::Rect<f32>)
#[cfg(cbindgen)]
#[repr(C)]
struct Rect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

/// Expand Point so that cbindgen can see it. ( is in fact euclid::default::PointD2<f32>)
#[cfg(cbindgen)]
#[repr(C)]
struct Point {
    x: f32,
    y: f32,
}

#[vtable]
#[repr(C)]
pub struct ComponentVTable {
    /// Allocate an instance of this component
    pub create: extern "C" fn(&ComponentVTable) -> VBox<ComponentVTable>,

    /// Destruct this component.
    pub drop: extern "C" fn(VRefMut<ComponentVTable>),

    /// Returns an array that represent the item tree
    pub item_tree: extern "C" fn(VRef<ComponentVTable>) -> *const ItemTreeNode,
}

#[derive(Default)]
#[repr(C)]
pub struct CachedRenderingData {
    /// Used and modified by the backend, should be initialized to 0 by the user code
    pub(crate) cache_index: usize,
    /// Set to false initially and when changes happen that require updating the cache
    pub(crate) cache_ok: bool,
}

impl CachedRenderingData {
    pub(crate) fn low_level_rendering_primitive<
        'a,
        GraphicsBackend: crate::graphics::GraphicsBackend,
    >(
        &self,
        cache: &'a crate::graphics::RenderingCache<GraphicsBackend>,
    ) -> Option<&'a GraphicsBackend::LowLevelRenderingPrimitive> {
        if !self.cache_ok {
            return None;
        }
        Some(cache.entry_at(self.cache_index))
    }
}

/// The item tree is an array of ItemTreeNode representing a static tree of items
/// within a component.
#[repr(C)]
pub enum ItemTreeNode {
    /// Static item
    Item {
        /// byte offset where we can find the item (from the *ComponentImpl)
        offset: isize,
        /// virtual table of the item
        vtable: *const ItemVTable,

        /// number of children
        chilren_count: u32,

        /// index of the first children within the item tree
        children_index: u32,
    },
    /// A placeholder for many instance of item in their own component which
    /// are instentiated according to a model.
    DynamicTree {
        /// Component vtable.
        /// This component is going to be instantiated as many time as the model tells
        component_type: *const ComponentVTable,

        /// vtable of the model
        model_type: *const super::model::ModelType,

        /// byte offset of the ModelImpl within the component.
        /// The model is an instance of the model described by model_type and must be
        /// stored within the component
        model_offset: isize,

        /// byte offset of the vector of components within the parent component
        /// (ComponentVec)
        /// a ComponentVec must be stored within the component to represent this tree
        components_holder_offset: isize,
    },
}

/// It is supposed to be in static array
unsafe impl Sync for ItemTreeNode {}

/// Items are the nodes in the render tree.
#[vtable]
#[repr(C)]
pub struct ItemVTable {
    ///
    pub geometry: extern "C" fn(VRef<'_, ItemVTable>) -> Rect,

    /// offset in bytes fromthe *const ItemImpl.
    /// isize::MAX  means None
    #[allow(non_upper_case_globals)]
    #[offset(CachedRenderingData)]
    pub cached_rendering_data_offset: usize,

    /// Return a rendering info
    pub rendering_info: extern "C" fn(VRef<'_, ItemVTable>) -> RenderingInfo,

    /// We would need max/min/preferred size, and all layout info
    pub layouting_info: extern "C" fn(VRef<'_, ItemVTable>) -> LayoutInfo,

    /// input event
    pub input_event: extern "C" fn(VRef<'_, ItemVTable>, MouseEvent, VRef<'_, ComponentVTable>),
}

// given an ItemImpl & ItemVTable
// (1) Identify that the item *is* a rectangle or has everything a rectangle would have
// (2) change the width

#[repr(C)]
pub struct LayoutInfo {
    min_size: f32,
    //...
    width_offset: isize,
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub enum RenderingInfo {
    NoContents,
    Rectangle(f32, f32, f32, f32, u32), // Should be a beret structure
    Image(f32, f32, crate::SharedString),
    Text(f32, f32, crate::SharedString, crate::SharedString, f32, u32),
    /*Path(Vec<PathElement>),
    Image(OpaqueImageHandle, AspectRatio)*/
}

impl Default for RenderingInfo {
    fn default() -> Self {
        RenderingInfo::NoContents
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
#[repr(C)]
pub struct Color {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl Color {
    pub const fn from_argb_encoded(encoded: u32) -> Color {
        Color {
            red: (encoded >> 16) as u8,
            green: (encoded >> 8) as u8,
            blue: encoded as u8,
            alpha: (encoded >> 24) as u8,
        }
    }

    pub const fn from_rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Color {
        Color { red, green, blue, alpha }
    }
    pub const fn from_rgb(red: u8, green: u8, blue: u8) -> Color {
        Color::from_rgba(red, green, blue, 0xff)
    }

    pub fn as_rgba_f32(&self) -> (f32, f32, f32, f32) {
        (
            (self.red as f32) / 255.0,
            (self.green as f32) / 255.0,
            (self.blue as f32) / 255.0,
            (self.alpha as f32) / 255.0,
        )
    }

    pub const BLACK: Color = Color::from_rgb(0, 0, 0);
    pub const RED: Color = Color::from_rgb(255, 0, 0);
    pub const GREEN: Color = Color::from_rgb(0, 255, 0);
    pub const BLUE: Color = Color::from_rgb(0, 0, 255);
    pub const WHITE: Color = Color::from_rgb(255, 255, 255);
}

#[derive(PartialEq, Debug)]
#[repr(C)]
pub enum RenderingPrimitive {
    NoContents,
    Rectangle {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: Color,
    },
    Image {
        x: f32,
        y: f32,
        source: crate::SharedString,
    },
    Text {
        x: f32,
        y: f32,
        text: crate::SharedString,
        font_family: crate::SharedString,
        font_pixel_size: f32,
        color: Color,
    },
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum MouseEventType {
    MousePressed,
    MouseReleased,
    MouseMoved,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MouseEvent {
    pub pos: Point,
    pub what: MouseEventType,
}

/* -- Safe wrappers*/

/// Visit each items recursively
///
/// The state parametter returned by the visitor is passed to each children.
pub fn visit_items<State>(
    component: VRef<'_, ComponentVTable>,
    mut visitor: impl FnMut(ItemRef<'_>, &State) -> State,
    state: State,
) {
    visit_internal(component, &mut visitor, 0, &state)
}

fn visit_internal<State>(
    component: VRef<'_, ComponentVTable>,
    visitor: &mut impl FnMut(ItemRef<'_>, &State) -> State,
    index: isize,
    state: &State,
) {
    let item_tree = component.item_tree();
    match unsafe { &*item_tree.offset(index) } {
        ItemTreeNode::Item { vtable, offset, children_index, chilren_count } => {
            let item = unsafe {
                ItemRef::from_raw(
                    NonNull::new_unchecked(*vtable as *mut _),
                    NonNull::new_unchecked(component.as_ptr().offset(*offset) as *mut _),
                )
            };
            let state = visitor(item, state);
            for c in *children_index..(*children_index + *chilren_count) {
                visit_internal(component, visitor, c as isize, &state)
            }
        }
        ItemTreeNode::DynamicTree { .. } => todo!(),
    }
}

pub fn visit_items_mut<State>(
    component: VRefMut<'_, ComponentVTable>,
    mut visitor: impl FnMut(ItemRefMut<'_>, &State) -> State,
    state: State,
) {
    visit_internal_mut(component, &mut visitor, 0, &state)
}

fn visit_internal_mut<State>(
    mut component: VRefMut<'_, ComponentVTable>,
    visitor: &mut impl FnMut(ItemRefMut<'_>, &State) -> State,
    index: isize,
    state: &State,
) {
    let item_tree = component.item_tree();
    match unsafe { &*item_tree.offset(index) } {
        ItemTreeNode::Item { vtable, offset, children_index, chilren_count } => {
            let mut item = unsafe {
                ItemRefMut::from_raw(
                    NonNull::new_unchecked(*vtable as *mut _),
                    NonNull::new_unchecked(
                        (component.as_ptr() as *mut u8).offset(*offset) as *mut _
                    ),
                )
            };
            let state = visitor(item.borrow_mut(), state);
            for c in *children_index..(*children_index + *chilren_count) {
                visit_internal_mut(component.borrow_mut(), visitor, c as isize, &state)
            }
        }
        ItemTreeNode::DynamicTree { .. } => todo!(),
    }
}

/*

/*
Button { visible: false; text: "foo"}

 -> ProxyWithVisibility<NativeItem>

// Qt style selected:
fn render_button(item: *const ItemImpl) -> RenderingInfo {
    let button = reinterpret_cast<&Button>(button)
    let text = b.text();
    let isPressed = b.isPressed();
    // ...
    let image = qt_render_button(width, height, isPressed, text)

    return RenderingInfo::Image(image)
}

// Basic style selected:

 -> Rectangle / Text

 fn render_rectangle(item: *const ItemImpl) -> RenderingInfo {
     let rect = reinterpret_cast<&Rectangle>(item)
     ...
     return RenderingInfo::Path(rect_path)
 }

 fn render_text(item: *const ItemImpl) -> RenderInfo {

 }

*/

// in corelib/primitives.rs

bitflags! {
    enum ItemExtensions {
        HasVisibility,
        HasOpacity
    }
}

struct ItemBase {
extensions: ItemExtensions,
x: Property<f32>,
y: Property<f32>,
// visible, opacity, ?
extraData: Vec<...>
}

impl ItemBase {
    pub fn is_visible(&self) -> bool {
        if self.extensions & HasVisibility {
            return self.extraData
        } else {
            return true;
        }
    }
}

#[derive(SixtyFpsItem)]
/// ```
/// width: f32
/// height: f32
/// ```
fn render_rectangle(item: *const ItemImpl) -> RenderingInfo {
    //let width = property_at_offset(item, 1);
    let rect : &Rectnalge = unsafe { std::mem::transmute(item) };
    let width = rect.width.get()
}


pub static RECTANGLE_VTABLE: ItemVTable = ItemVTable {
    rendering_info: render_rectangle,
}

// in styles/qt.rs

//#[derive(SixtyFpsItem)]
struct QtButton {
    text: String,
    is_pressed: bool,
}


pub static QT_BUTTON_VTABLE: ItemVTable = ItemVTable {
    rendering_info: render_qt_button,
};
*/

// This is here because for some reason (rust bug?) the ItemVTable_static is not accessible in the other modules
ItemVTable_static!(crate::abi::primitives::Image);
ItemVTable_static!(crate::abi::primitives::Rectangle);
ItemVTable_static!(crate::abi::primitives::Text);
ItemVTable_static!(crate::abi::primitives::TouchArea);
