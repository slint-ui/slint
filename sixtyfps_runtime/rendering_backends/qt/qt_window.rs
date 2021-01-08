/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use cpp::*;
use items::{ImageFit, TextHorizontalAlignment, TextVerticalAlignment};
use sixtyfps_corelib::component::{ComponentRc, ComponentWeak};
use sixtyfps_corelib::graphics::{FontRequest, GraphicsBackend, Point};
use sixtyfps_corelib::input::{MouseEventType, MouseInputState, TextCursorBlinker};
use sixtyfps_corelib::item_rendering::ItemRenderer;
use sixtyfps_corelib::items::{self, ItemRef};
use sixtyfps_corelib::items::{Item, ItemWeak};
use sixtyfps_corelib::properties::PropertyTracker;
use sixtyfps_corelib::window::{ComponentWindow, GenericWindow};
use sixtyfps_corelib::Resource;

use std::cell::{Cell, RefCell};
use std::pin::Pin;
use std::ptr::NonNull;
use std::rc::{Rc, Weak};

use super::qttypes;

cpp! {{
    #include <QtWidgets/QWidget>
    #include <QtGui/QPainter>
    #include <QtGui/QPaintEngine>
    #include <QtGui/QWindow>
    #include <QtGui/QResizeEvent>
    #include <QtGui/QTextLayout>
    #include <QtCore/QBasicTimer>
    void ensure_initialized();

    struct TimerHandler : QObject {
        QBasicTimer timer;
        static TimerHandler& instance() {
            static TimerHandler instance;
            return instance;
        }

        void timerEvent(QTimerEvent *event) override {
            if (event->timerId() != timer.timerId()) {
                QObject::timerEvent(event);
                return;
            }
            timer.stop();
            rust!(SFPS_timerEvent [] { timer_event() });
        }

    };

    struct SixtyFPSWidget : QWidget {
        void *rust_window;
        void paintEvent(QPaintEvent *) override {
            QPainter painter(this);
            auto painter_ptr = &painter;
            rust!(SFPS_paintEvent [rust_window: &QtWindow as "void*", painter_ptr: &mut QPainter as "QPainter*"] {
                rust_window.paint_event(painter_ptr)
            });
        }

        void resizeEvent(QResizeEvent *event) override {
            QSize size = event->size();
            rust!(SFPS_resizeEvent [rust_window: &QtWindow as "void*", size: qttypes::QSize as "QSize"] {
                rust_window.resize_event(size)
            });
        }

        void mousePressEvent(QMouseEvent *event) override {
            QPoint pos = event->pos();
            rust!(SFPS_mousePressEvent [rust_window: &QtWindow as "void*", pos: qttypes::QPoint as "QPoint"] {
                rust_window.mouse_event(MouseEventType::MousePressed, pos)
            });
        }
        void mouseReleaseEvent(QMouseEvent *event) override {
            QPoint pos = event->pos();
            rust!(SFPS_mouseReleaseEvent [rust_window: &QtWindow as "void*", pos: qttypes::QPoint as "QPoint"] {
                rust_window.mouse_event(MouseEventType::MouseReleased, pos)
            });
        }
        void mouseMoveEvent(QMouseEvent *event) override {
            QPoint pos = event->pos();
            rust!(SFPS_mouseMoveEvent [rust_window: &QtWindow as "void*", pos: qttypes::QPoint as "QPoint"] {
                rust_window.mouse_event(MouseEventType::MouseMoved, pos)
            });
        }
    };

}}

cpp_class! {pub unsafe struct QPainter as "QPainter"}

/// Given a position offset and an object of a given type that has x,y,width,height properties,
/// create a QRectF that fits it.
macro_rules! get_geometry {
    ($pos:expr, $ty:ty, $obj:expr) => {{
        type Ty = $ty;
        let width = Ty::FIELD_OFFSETS.width.apply_pin($obj).get();
        let height = Ty::FIELD_OFFSETS.height.apply_pin($obj).get();
        let x = Ty::FIELD_OFFSETS.x.apply_pin($obj).get();
        let y = Ty::FIELD_OFFSETS.y.apply_pin($obj).get();
        if width < 1. || height < 1. {
            return Default::default();
        };
        qttypes::QRectF {
            x: (x + $pos.x as f32) as _,
            y: (y + $pos.y as f32) as _,
            width: width as _,
            height: height as _,
        }
    }};
}

macro_rules! get_pos {
    ($ty:ty, $obj:expr) => {{
        type Ty = $ty;
        let x = Ty::FIELD_OFFSETS.x.apply_pin($obj).get();
        let y = Ty::FIELD_OFFSETS.y.apply_pin($obj).get();
        qttypes::QPoint { x: x as _, y: y as _ }
    }};
}

impl ItemRenderer for QPainter {
    fn draw_rectangle(&mut self, pos: Point, rect: Pin<&items::Rectangle>) {
        let pos = qttypes::QPoint { x: pos.x as _, y: pos.y as _ };
        let color: u32 = rect.color().as_argb_encoded();
        let rect: qttypes::QRectF = get_geometry!(pos, items::Rectangle, rect);
        cpp! { unsafe [self as "QPainter*", color as "QRgb", rect as "QRectF"] {
            self->fillRect(rect, QColor::fromRgba(color));
        }}
    }

    fn draw_border_rectangle(&mut self, pos: Point, rect: std::pin::Pin<&items::BorderRectangle>) {
        let color: u32 = rect.color().as_argb_encoded();
        let border_color: u32 = rect.border_color().as_argb_encoded();
        let border_width: f32 = rect.border_width();
        let radius: f32 = rect.border_radius();
        let rect: qttypes::QRectF = get_geometry!(pos, items::BorderRectangle, rect);
        cpp! { unsafe [self as "QPainter*", color as "QRgb",  border_color as "QRgb", border_width as "float", radius as "float", rect as "QRectF"] {
            self->setPen(border_width > 0 ? QPen(QColor::fromRgba(border_color), border_width) : Qt::NoPen);
            self->setBrush(QColor::fromRgba(color));
            if (radius > 0) {
                self->drawRoundedRect(rect, radius, radius);
            } else {
                self->drawRect(rect);
            }
        }}
    }

    fn draw_image(&mut self, pos: Point, image: Pin<&items::Image>) {
        let dest_rect: qttypes::QRectF = get_geometry!(pos, items::Image, image);
        self.draw_image_impl(image.source(), dest_rect, None, image.image_fit());
    }

    fn draw_clipped_image(&mut self, pos: Point, image: Pin<&items::ClippedImage>) {
        let dest_rect: qttypes::QRectF = get_geometry!(pos, items::ClippedImage, image);
        let source_rect = qttypes::QRectF {
            x: image.source_clip_x() as _,
            y: image.source_clip_y() as _,
            width: image.source_clip_width() as _,
            height: image.source_clip_height() as _,
        };
        self.draw_image_impl(image.source(), dest_rect, Some(source_rect), image.image_fit());
    }

    fn draw_text(&mut self, pos: Point, text: std::pin::Pin<&items::Text>) {
        let rect: qttypes::QRectF = get_geometry!(pos, items::Text, text);
        let color: u32 = text.color().as_argb_encoded();
        let string: qttypes::QString = text.text().as_str().into();
        let font: QFont = get_font(text.font_request());

        let flags = match text.horizontal_alignment() {
            TextHorizontalAlignment::align_left => {
                cpp!(unsafe [] -> i32 as "int" { return Qt::AlignLeft; })
            }
            TextHorizontalAlignment::align_center => {
                cpp!(unsafe [] -> i32 as "int" { return Qt::AlignHCenter; })
            }
            TextHorizontalAlignment::align_right => {
                cpp!(unsafe [] -> i32 as "int" { return Qt::AlignRight; })
            }
        } | match text.vertical_alignment() {
            TextVerticalAlignment::align_top => {
                cpp!(unsafe [] -> i32 as "int" { return Qt::AlignTop; })
            }
            TextVerticalAlignment::align_center => {
                cpp!(unsafe [] -> i32 as "int" { return Qt::AlignVCenter; })
            }
            TextVerticalAlignment::align_bottom => {
                cpp!(unsafe [] -> i32 as "int" { return Qt::AlignBottom; })
            }
        };
        cpp! { unsafe [self as "QPainter*", rect as "QRectF", color as "QRgb", string as "QString", flags as "int", font as "QFont"] {
            self->setFont(font);
            self->setPen(QColor{color});
            self->setBrush(Qt::NoBrush);
            self->drawText(rect, flags, string);
        }}
    }

    fn draw_text_input(&mut self, pos: Point, text_input: std::pin::Pin<&items::TextInput>) {
        let pos1 = qttypes::QPoint { x: pos.x as _, y: pos.y as _ };
        let pos2: qttypes::QPoint = get_pos!(items::TextInput, text_input);
        let color: u32 =
            items::TextInput::FIELD_OFFSETS.color.apply_pin(text_input).get().as_argb_encoded();
        let string: qttypes::QString =
            items::TextInput::FIELD_OFFSETS.text.apply_pin(text_input).get().as_str().into();
        cpp! { unsafe [self as "QPainter*", pos1 as "QPoint", pos2 as "QPoint", color as "QRgb", string as "QString"] {
            self->setPen(QColor{color});
            self->setBrush(Qt::NoBrush);
            self->drawText(pos1 + pos2, string);
        }}
    }

    fn draw_path(&mut self, pos: Point, path: std::pin::Pin<&items::Path>) {
        todo!()
    }

    fn combine_clip(&mut self, pos: Point, clip: &std::pin::Pin<&items::Clip>) {
        let clip_rect: qttypes::QRectF = get_geometry!(pos, items::Clip, *clip);
        cpp! { unsafe [self as "QPainter*", clip_rect as "QRectF"] {
            self->setClipRect(clip_rect, Qt::IntersectClip);
        }}
    }

    fn scale_factor(&self) -> f32 {
        cpp! { unsafe [self as "QPainter*"] -> f32 as "float" {
            return self->paintEngine()->paintDevice()->devicePixelRatioF();
        }}
    }

    fn draw_cached_pixmap(
        &mut self,
        item_cache: &sixtyfps_corelib::item_rendering::CachedRenderingData,
        pos: Point,
        update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    ) {
        // FIXME! draw_cached_pixmap is the wrong abstraction now
        update_fn(&mut |width: u32, height: u32, data: &[u8]| {
            let pos = qttypes::QPoint { x: pos.x as _, y: pos.y as _ };
            let data = data.as_ptr();
            cpp! { unsafe [self as "QPainter*", pos as "QPoint", width as "int", height as "int", data as "const unsigned char *"] {
                QImage img(data, width, height, width * 4, QImage::Format_ARGB32_Premultiplied);
                self->drawImage(pos, img);
            }}
        })
    }

    fn save_state(&mut self) {
        cpp! { unsafe [self as "QPainter*"] {
            self->save();
        }}
    }

    fn restore_state(&mut self) {
        cpp! { unsafe [self as "QPainter*"] {
            self->restore();
        }}
    }
}

impl QPainter {
    fn draw_image_impl(
        &mut self,
        resource: Resource,
        dest_rect: qttypes::QRectF,
        source_rect: Option<qttypes::QRectF>,
        image_fit: ImageFit,
    ) {
        // FIXME: the loaded png image could be cached.
        let (is_path, data) = match resource {
            Resource::None => return,
            Resource::AbsoluteFilePath(path) => (true, qttypes::QByteArray::from(path.as_str())),
            Resource::EmbeddedData(data) => (false, qttypes::QByteArray::from(data.as_slice())),
            Resource::EmbeddedRgbaImage { .. } => todo!(),
        };
        let img = cpp! { unsafe [data as "QByteArray", is_path as "bool"] -> qttypes::QImage as "QImage" {
            QImage img;
            is_path ? img.load(QString::fromUtf8(data)) : img.loadFromData(data);
            return img;
        }};
        let mut source_rect = source_rect.unwrap_or_else(|| {
            let s = img.size();
            qttypes::QRectF { x: 0., y: 0., width: s.width as _, height: s.height as _ }
        });
        match image_fit {
            sixtyfps_corelib::items::ImageFit::fill => (),
            sixtyfps_corelib::items::ImageFit::contain => {
                let ratio = qttypes::qreal::max(
                    dest_rect.width / source_rect.width,
                    dest_rect.height / source_rect.height,
                );
                if source_rect.width > dest_rect.width / ratio {
                    source_rect.x += (source_rect.width - dest_rect.width / ratio) / 2.;
                    source_rect.width = dest_rect.width / ratio;
                }
                if source_rect.height > dest_rect.height / ratio {
                    source_rect.y += (source_rect.height - dest_rect.height / ratio) / 2.;
                    source_rect.height = dest_rect.height / ratio;
                }
            }
        };
        cpp! { unsafe [self as "QPainter*", img as "QImage", source_rect as "QRectF", dest_rect as "QRectF"] {
            self->drawImage(dest_rect, img, source_rect);
        }};
    }
}

cpp_class!(unsafe struct QWidgetPtr as "std::unique_ptr<QWidget>");

pub struct QtWindow {
    widget_ptr: QWidgetPtr,
    self_weak: once_cell::unsync::OnceCell<Weak<QtWindow>>,
    component: RefCell<ComponentWeak>,
    /// Gets dirty when the layout restrictions, or some other property of the windows change
    meta_property_listener: Pin<Rc<PropertyTracker>>,
    /// Gets dirty if something needs to be painted
    redraw_listener: Pin<Rc<PropertyTracker>>,
    mouse_input_state: Cell<MouseInputState>,
    focus_item: std::cell::RefCell<ItemWeak>,
    cursor_blinker: RefCell<pin_weak::rc::PinWeak<TextCursorBlinker>>,
}

impl QtWindow {
    pub fn new() -> Rc<Self> {
        let widget_ptr = cpp! {unsafe [] -> QWidgetPtr as "std::unique_ptr<QWidget>" {
            ensure_initialized();
            return std::make_unique<SixtyFPSWidget>();
        }};
        let rc = Rc::new(QtWindow {
            widget_ptr,
            self_weak: Default::default(),
            component: Default::default(),
            meta_property_listener: Rc::pin(Default::default()),
            redraw_listener: Rc::pin(Default::default()),
            mouse_input_state: Default::default(),
            focus_item: Default::default(),
            cursor_blinker: Default::default(),
        });
        let self_weak = Rc::downgrade(&rc);
        rc.self_weak.set(self_weak.clone()).ok().unwrap();
        let widget_ptr = rc.widget_ptr();
        let rust_window = Rc::as_ptr(&rc);
        cpp! {unsafe [widget_ptr as "SixtyFPSWidget*", rust_window as "void*"]  {
            widget_ptr->rust_window = rust_window;
        }};
        ALL_WINDOWS.with(|aw| aw.borrow_mut().push(self_weak));
        rc
    }

    /// Return the QWidget*
    fn widget_ptr(&self) -> NonNull<()> {
        unsafe { std::mem::transmute_copy::<QWidgetPtr, NonNull<_>>(&self.widget_ptr) }
    }

    /// ### Candidate to be moved in corelib as this kind of duplicate GraphicsWindow::draw
    fn paint_event(&self, painter: &mut QPainter) {
        sixtyfps_corelib::animations::update_animations();

        let component_rc = self.component.borrow().upgrade().unwrap();
        let component = ComponentRc::borrow_pin(&component_rc);

        if self.meta_property_listener.as_ref().is_dirty() {
            self.meta_property_listener.as_ref().evaluate(|| {
                self.apply_geometry_constraint(component.as_ref().layout_info());
                component.as_ref().apply_layout(Default::default());

                let root_item = component.as_ref().get_item_ref(0);
                if let Some(window_item) = ItemRef::downcast_pin(root_item) {
                    self.apply_window_properties(window_item);
                }
            })
        }

        self.redraw_listener.as_ref().evaluate(|| {
            sixtyfps_corelib::item_rendering::render_component_items::<QtBackend>(
                &component_rc,
                painter,
                Point::default(),
            );
        });
    }

    fn resize_event(&self, size: qttypes::QSize) {
        let component = self.component.borrow().upgrade().unwrap();
        let component = ComponentRc::borrow_pin(&component);
        let root_item = component.as_ref().get_item_ref(0);
        if let Some(window_item) = ItemRef::downcast_pin::<items::Window>(root_item) {
            window_item.width.set(size.width as _);
            window_item.height.set(size.height as _);
        }
    }

    /// ### Candidate to be moved in corelib as this kind of duplicate GraphicsWindow::process_mouse_input
    fn mouse_event(&self, what: MouseEventType, pos: qttypes::QPoint) {
        sixtyfps_corelib::animations::update_animations();
        let component = self.component.borrow().upgrade().unwrap();
        let pos = Point::new(pos.x as _, pos.y as _);
        self.mouse_input_state.set(sixtyfps_corelib::input::process_mouse_input(
            component,
            sixtyfps_corelib::input::MouseEvent { pos, what },
            &ComponentWindow::new(self.self_weak.get().unwrap().upgrade().unwrap()),
            self.mouse_input_state.take(),
        ));
        timer_event();
        //self.request_redraw();
    }

    /// Set the min/max sizes on the QWidget
    fn apply_geometry_constraint(&self, constraints: sixtyfps_corelib::layout::LayoutInfo) {
        let widget_ptr = self.widget_ptr();
        let min_width: f32 = constraints.min_width.min(constraints.max_width);
        let min_height: f32 = constraints.min_height.min(constraints.max_height);
        let mut max_width: f32 = constraints.max_width.max(constraints.min_width);
        let mut max_height: f32 = constraints.max_height.max(constraints.min_height);
        cpp! {unsafe [widget_ptr as "QWidget*",  min_width as "float", min_height as "float", mut max_width as "float", mut max_height as "float"] {
            widget_ptr->setMinimumSize(QSize(min_width, min_height));
            if (max_width > QWIDGETSIZE_MAX)
                max_width = QWIDGETSIZE_MAX;
            if (max_height > QWIDGETSIZE_MAX)
                max_height = QWIDGETSIZE_MAX;
            widget_ptr->setMaximumSize(QSize(max_width, max_height));
        }};
    }

    /// Apply windows property such as title to the QWidget*
    fn apply_window_properties(&self, window_item: Pin<&items::Window>) {
        let widget_ptr = self.widget_ptr();
        let title: qttypes::QString =
            items::Window::FIELD_OFFSETS.title.apply_pin(window_item).get().as_str().into();
        cpp! {unsafe [widget_ptr as "QWidget*",  title as "QString"] {
            widget_ptr->setWindowTitle(title);
        }};
    }
}

#[allow(unused)]
impl GenericWindow for QtWindow {
    fn set_component(self: Rc<Self>, component: &sixtyfps_corelib::component::ComponentRc) {
        *self.component.borrow_mut() = vtable::VRc::downgrade(&component)
    }

    fn draw(self: Rc<Self>) {
        todo!()
    }

    fn process_mouse_input(
        self: Rc<Self>,
        pos: Point,
        what: sixtyfps_corelib::input::MouseEventType,
    ) {
        todo!()
    }

    fn process_key_input(self: Rc<Self>, event: &sixtyfps_corelib::input::KeyEvent) {
        todo!()
    }

    fn run(self: Rc<Self>) {
        let widget_ptr = self.widget_ptr();
        cpp! {unsafe [widget_ptr as "QWidget*"] {
            widget_ptr->show();
            qApp->exec();
        }};
    }

    fn request_redraw(&self) {
        if self.redraw_listener.is_dirty() {
            let widget_ptr = self.widget_ptr();
            cpp! {unsafe [widget_ptr as "QWidget*"] {
                return widget_ptr->update();
            }}
        }
    }

    fn scale_factor(&self) -> f32 {
        let widget_ptr = self.widget_ptr();
        cpp! {unsafe [widget_ptr as "QWidget*"] -> f32 as "float" {
            return widget_ptr->windowHandle()->devicePixelRatio();
        }}
    }

    fn set_scale_factor(&self, factor: f32) {
        todo!()
    }

    fn refresh_window_scale_factor(&self) {
        todo!()
    }

    fn set_width(&self, width: f32) {
        todo!()
    }

    fn set_height(&self, height: f32) {
        todo!()
    }

    fn get_geometry(&self) -> sixtyfps_corelib::graphics::Rect {
        todo!()
    }

    fn free_graphics_resources<'a>(
        self: Rc<Self>,
        items: &sixtyfps_corelib::slice::Slice<'a, std::pin::Pin<items::ItemRef<'a>>>,
    ) {
    }

    fn set_cursor_blink_binding(&self, prop: &sixtyfps_corelib::Property<bool>) {
        let existing_blinker = self.cursor_blinker.borrow().clone();

        let blinker = existing_blinker.upgrade().unwrap_or_else(|| {
            let new_blinker = TextCursorBlinker::new();
            *self.cursor_blinker.borrow_mut() =
                pin_weak::rc::PinWeak::downgrade(new_blinker.clone());
            new_blinker
        });

        TextCursorBlinker::set_binding(blinker, prop);
    }

    fn current_keyboard_modifiers(&self) -> sixtyfps_corelib::input::KeyboardModifiers {
        todo!()
    }

    fn set_current_keyboard_modifiers(
        &self,
        modifiers: sixtyfps_corelib::input::KeyboardModifiers,
    ) {
        todo!()
    }

    /// ### Candidate to be moved in corelib as this kind of duplicate GraphicsWindow::set_focus_item
    fn set_focus_item(self: Rc<Self>, focus_item: &items::ItemRc) {
        let window = ComponentWindow::new(self.clone());

        if let Some(old_focus_item) = self.as_ref().focus_item.borrow().upgrade() {
            old_focus_item
                .borrow()
                .as_ref()
                .focus_event(&sixtyfps_corelib::input::FocusEvent::FocusOut, &window);
        }

        *self.as_ref().focus_item.borrow_mut() = focus_item.downgrade();

        focus_item
            .borrow()
            .as_ref()
            .focus_event(&sixtyfps_corelib::input::FocusEvent::FocusIn, &window);
    }

    /// ### Candidate to be moved in corelib as this kind of duplicate GraphicsWindow::set_focussixtyfps_
    fn set_focus(self: Rc<Self>, have_focus: bool) {
        let window = ComponentWindow::new(self.clone());
        let event = if have_focus {
            sixtyfps_corelib::input::FocusEvent::WindowReceivedFocus
        } else {
            sixtyfps_corelib::input::FocusEvent::WindowLostFocus
        };

        if let Some(focus_item) = self.as_ref().focus_item.borrow().upgrade() {
            focus_item.borrow().as_ref().focus_event(&event, &window);
        }
    }

    fn show_popup(&self, popup: &sixtyfps_corelib::component::ComponentRc, position: Point) {
        todo!()
    }

    fn close_popup(&self) {
        todo!()
    }

    fn font(&self, request: FontRequest) -> Option<Box<dyn sixtyfps_corelib::graphics::Font>> {
        Some(Box::new(get_font(request)))
    }
}

fn get_font(request: FontRequest) -> QFont {
    let family: qttypes::QString = request.family.as_str().into();
    let pixel_size: f32 = request.pixel_size.unwrap_or(0.);
    let weight: i32 = request.weight.unwrap_or(0);
    cpp!(unsafe [family as "QString", pixel_size as "float", weight as "int"] -> QFont as "QFont" {
        QFont f;
        if (!family.isEmpty())
            f.setFamily(family);
        if (pixel_size > 0)
            f.setPixelSize(pixel_size);
        if (weight > 0)
            f.setWeight(weight);
        return f;
    })
}

struct QtBackend;
impl GraphicsBackend for QtBackend {
    type ItemRenderer = QPainter;

    fn new_renderer(&mut self, clear_color: &sixtyfps_corelib::Color) -> Self::ItemRenderer {
        todo!()
    }

    fn flush_renderer(&mut self, renderer: Self::ItemRenderer) {
        todo!()
    }

    fn release_item_graphics_cache(
        &self,
        data: &sixtyfps_corelib::item_rendering::CachedRenderingData,
    ) {
        todo!()
    }

    fn font(&mut self, request: FontRequest) -> Box<dyn sixtyfps_corelib::graphics::Font> {
        todo!()
    }

    fn window(&self) -> &winit::window::Window {
        todo!()
    }
}

cpp_class! {pub unsafe struct QFont as "QFont"}

impl sixtyfps_corelib::graphics::Font for QFont {
    fn text_width(&self, text: &str) -> f32 {
        let string = qttypes::QString::from(text);
        cpp! { unsafe [self as "const QFont*",  string as "QString"] -> f32 as "float"{
            return QFontMetricsF(*self).boundingRect(string).width();
        }}
    }

    fn text_offset_for_x_position<'a>(&self, text: &'a str, x: f32) -> usize {
        let string = qttypes::QString::from(text);
        cpp! { unsafe [self as "const QFont*", string as "QString", x as "float"] -> usize as "long long" {
            QTextLayout layout(string, *self);
            if (layout.lineCount() == 0)
                return 0;
            auto cur = layout.lineAt(0).xToCursor(x);
            // convert to an utf8 pos;
            return QStringView(string).left(cur).toUtf8().size();
        }}
    }

    fn height(&self) -> f32 {
        cpp! { unsafe [self as "const QFont*"] -> f32 as "float"{
            return QFontMetricsF(*self).height();
        }}
    }
}

thread_local! {
    // FIXME: currently the window are never removed
    static ALL_WINDOWS: RefCell<Vec<Weak<QtWindow>>> = Default::default();
}

/// Called by C++'s TimerHandler::timerEvent, or everytime a timer might have been started
fn timer_event() {
    sixtyfps_corelib::animations::update_animations();
    sixtyfps_corelib::timers::TimerList::maybe_activate_timers();

    ALL_WINDOWS.with(|windows| {
        for x in windows.borrow().iter() {
            if let Some(x) = x.upgrade() {
                x.request_redraw();
            }
        }
    });

    if let Some(instant) = sixtyfps_corelib::timers::TimerList::next_timeout() {
        let timeout = instant.duration_since(std::time::Instant::now()).as_millis() as i32;
        cpp! { unsafe [timeout as "int"] {
            TimerHandler::instance().timer.start(timeout, &TimerHandler::instance());
        }}
    }
}
