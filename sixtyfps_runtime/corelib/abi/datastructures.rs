use core::ptr::NonNull;

/// The opaque component type
pub struct ComponentImpl;

#[repr(C)]
pub struct ComponentType {
    /// Allocate an instance of this component
    create: Option<fn(*const ComponentType) -> *mut ComponentImpl>,

    /// Destruct this component.
    destroy: fn(*const ComponentType, *mut ComponentImpl),

    /// Returns an array that represent the item tree
    /// FIXME: dynamic items
    item_tree: fn(*const ComponentType) -> *const ItemTreeNode,
}

/// From the ItemTreeNode and a ComponentImpl, you can get a pointer to the instance data
/// ItemImpl via the offset field.
type ItemImpl = ();
// Example memory representation:
// offset| type | value
// 0     | f32 | x
// 4     | f32 | y
// ...
// 64    | RenderNode | render node index

#[repr(C)]
pub struct RenderNode {
    /// Used and modified by the backend, should be initialized to 0 by the user code
    cache_index: core::cell::Cell<usize>,
    /// Set to true by the user code, and reset to false by the backend
    dirty_bit: core::cell::Cell<bool>,
}

#[repr(C)]
pub struct ItemTreeNode {
    /// byte offset where we can find the item (from the *ComponentImpl)
    offset: isize,
    /// virtual table of the item
    vtable: *const ItemVTable,

    /// number of children
    chilren_count: u32,

    /// index of the first children
    children_index: u32,
}

#[repr(C)]
#[derive(Default)]
pub struct ItemVTable {
    /// Rectangle: x/y/width/height ==> (path -> vertices/indicies(triangle))
    pub geometry: Option<fn(*const ItemImpl) -> ()>, // like kurbo::Rect

    /// offset in bytes fromthe *const ItemImpl.
    /// isize::MAX  means None
    pub render_node_index_offset: isize,
    // fn(*const ItemImpl) -> usize,
    /// ???
    pub rendering_info: Option<fn(*const ItemImpl) -> RenderingInfo>,

    /// We would need max/min/preferred size, and all layout info
    pub layouting_info: Option<fn(*const ItemImpl) -> LayoutInfo>,

    /// input event
    pub input_event: Option<fn(*const ItemImpl, MouseEvent)>,
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
pub enum RenderingInfo {
    NoContents,
    /*Path(Vec<PathElement>),
    Image(OpaqueImageHandle, AspectRatio),
    Text(String)*/
}

type MouseEvent = ();

/* -- Safe wrappers*/

/*trait Item {
    fn geometry(&self) -> ();
    fn render_node(&self) -> &RenderNode;
    fn render_node_mut(&mut self) -> &mut RenderNode;
    fn rendering_info(&self) -> RenderNode;
}*/

// To be used as &'x Item
pub struct Item {
    vtable: NonNull<ItemVTable>,
    inner: NonNull<ItemImpl>,
}

impl Item {
    /// One should create only one instance of item at the time to keep the &mut invariant
    pub unsafe fn new(vtable: NonNull<ItemVTable>, inner: NonNull<ItemImpl>) -> Self {
        Self { vtable, inner }
    }

    pub fn render_node(&self) -> &RenderNode {
        unsafe {
            &*((self.inner.as_ptr() as *const u8)
                .offset(self.vtable.as_ref().render_node_index_offset)
                as *const RenderNode)
        }
    }

    pub fn render_node_mut(&mut self) -> &mut RenderNode {
        unsafe {
            &mut *((self.inner.as_ptr() as *mut u8)
                .offset(self.vtable.as_ref().render_node_index_offset)
                as *mut RenderNode)
        }
    }

    pub fn rendering_info(&self) -> Option<RenderingInfo> {
        unsafe { self.vtable.as_ref().rendering_info.map(|x| x(self.inner.as_ptr())) }
    }
}

pub struct ComponentUniquePtr {
    vtable: NonNull<ComponentType>,
    inner: NonNull<ComponentImpl>,
}
impl Drop for ComponentUniquePtr {
    fn drop(&mut self) {
        unsafe {
            let destroy = self.vtable.as_ref().destroy;
            destroy(self.vtable.as_ptr(), self.inner.as_ptr());
        }
    }
}
impl ComponentUniquePtr {
    /// Both pointer must be valid until the call to vtable.destroy
    /// vtable will is a *const, and inner like a *mut
    pub unsafe fn new(vtable: NonNull<ComponentType>, inner: NonNull<ComponentImpl>) -> Self {
        Self { vtable, inner }
    }

    pub fn visit_items(&self, mut visitor: impl FnMut(&Item)) {
        self.visit_internal(&mut visitor, 0)
    }

    fn visit_internal(&self, visitor: &mut impl FnMut(&Item), index: isize) {
        let item_tree = unsafe { (self.vtable.as_ref().item_tree)(self.vtable.as_ptr()) };
        let current_item = unsafe { &*item_tree.offset(index) };

        let item = unsafe {
            Item::new(
                NonNull::new_unchecked(current_item.vtable as *mut _),
                NonNull::new_unchecked(
                    (self.inner.as_ptr() as *mut u8).offset(current_item.offset) as *mut _,
                ),
            )
        };
        visitor(&item);
        for c in
            current_item.children_index..(current_item.children_index + current_item.chilren_count)
        {
            self.visit_internal(visitor, c as isize)
        }
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

//#[derive(SixtyFpsItem)]
struct Rectangle {
    base: ItemBase,
    width: Property<f32>,
    height: Property<f32>,
    radius: Property<f32>,
}

/// fn rect_from_item(item: *const ItemImpl) -> *const Rectangle {
///  reinterpet_cast<const Rectangle *>(reinterpret_cast<char *>(item) + ... some offset))
///}

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

/// Run the given component
/// Both pointer must be valid until the call to vtable.destroy
/// vtable will is a *const, and inner like a *mut
#[no_mangle]
pub extern "C" fn sixtyfps_runtime_run_component(
    component_type: *const ComponentType,
    component: NonNull<ComponentImpl>,
) {
    let component = unsafe {
        ComponentUniquePtr::new(NonNull::new_unchecked(component_type as *mut _), component)
    };
    let mut count = 0;
    component.visit_items(|_item| count += 1);
    println!("FOUND {} items", count);
    todo!();
}
