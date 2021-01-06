/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use cpp::*;
use sixtyfps_corelib::component::{ComponentRc, ComponentWeak};
use sixtyfps_corelib::graphics::{GraphicsBackend, Point};
use sixtyfps_corelib::item_rendering::ItemRenderer;
use sixtyfps_corelib::items;
use sixtyfps_corelib::window::GenericWindow;
use std::pin::Pin;
use std::ptr::NonNull;
use std::rc::Rc;

use super::qttypes;

cpp! {{
    #include <QtWidgets/QWidget>
    #include <QtGui/QPainter>
    #include <QtGui/QPaintEngine>
    #include <QtGui/QWindow>
    void ensure_initialized();

    struct SixtyFPSWidget : QWidget {
        void *rust_window;
        void paintEvent(QPaintEvent *) override {
            QPainter painter(this);
            auto painter_ptr = &painter;
            rust!(SFPS_paintEvent [rust_window: &QtWindow as "void*", painter_ptr: &mut QPainter as "QPainter*"] {
                rust_window.paint_event(painter_ptr)
            });
        }

    };
}}

cpp_class! {pub unsafe struct QPainter as "QPainter"}

macro_rules! get_geometry {
    ($ty:ty, $obj:expr) => {{
        type Ty = $ty;
        let width = Ty::FIELD_OFFSETS.width.apply_pin($obj).get();
        let height = Ty::FIELD_OFFSETS.height.apply_pin($obj).get();
        let x = Ty::FIELD_OFFSETS.x.apply_pin($obj).get();
        let y = Ty::FIELD_OFFSETS.y.apply_pin($obj).get();
        if width < 1. || height < 1. {
            return Default::default();
        };
        qttypes::QRectF { x: x as _, y: y as _, width: width as _, height: height as _ }
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
        let color: u32 =
            items::Rectangle::FIELD_OFFSETS.color.apply_pin(rect).get().as_argb_encoded();
        let rect: qttypes::QRectF = get_geometry!(items::Rectangle, rect);
        cpp! { unsafe [self as "QPainter*", pos as "QPoint", color as "QRgb", rect as "QRectF"] {
            self->fillRect(rect.translated(pos), color);
        }}
    }

    fn draw_border_rectangle(&mut self, pos: Point, rect: std::pin::Pin<&items::BorderRectangle>) {
        todo!()
    }

    fn draw_image(&mut self, pos: Point, image: std::pin::Pin<&items::Image>) {
        todo!()
    }

    fn draw_clipped_image(&mut self, pos: Point, image: std::pin::Pin<&items::ClippedImage>) {
        todo!()
    }

    fn draw_text(&mut self, pos: Point, text: std::pin::Pin<&items::Text>) {
        let pos1 = qttypes::QPoint { x: pos.x as _, y: pos.y as _ };
        let pos2: qttypes::QPoint = get_pos!(items::Text, text);
        let color: u32 = items::Text::FIELD_OFFSETS.color.apply_pin(text).get().as_argb_encoded();
        let string: qttypes::QString =
            items::Text::FIELD_OFFSETS.text.apply_pin(text).get().as_str().into();
        cpp! { unsafe [self as "QPainter*", pos1 as "QPoint", pos2 as "QPoint", color as "QRgb", string as "QString"] {
            self->setPen(QColor{color});
            self->drawText(pos1 + pos2, string);
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
            self->drawText(pos1 + pos2, string);
        }}
    }

    fn draw_path(&mut self, pos: Point, path: std::pin::Pin<&items::Path>) {
        todo!()
    }

    fn combine_clip(&mut self, pos: Point, clip: &std::pin::Pin<&items::Clip>) {
        todo!()
    }

    fn clip_rects(&self) -> sixtyfps_corelib::SharedVector<sixtyfps_corelib::graphics::Rect> {
        // FIXME
        return Default::default();
    }

    fn reset_clip(
        &mut self,
        rects: sixtyfps_corelib::SharedVector<sixtyfps_corelib::graphics::Rect>,
    ) {
        let mut iter = rects.iter();
        if let Some(r) =
            iter.next().and_then(|first| iter.try_fold(*first, |acc, r| acc.intersection(r)))
        {
            let rect = qttypes::QRectF {
                x: r.origin.x as _,
                y: r.origin.y as _,
                width: r.size.width as _,
                height: r.size.height as _,
            };
            cpp! { unsafe [self as "QPainter*", rect as "QRectF"] {
                self->setClipRect(rect, Qt::ReplaceClip);
            }}
        } else {
            cpp! { unsafe [self as "QPainter*"] {
                self->setClipRect(QRect(), Qt::NoClip);
            }}
        }
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
}

cpp_class!(unsafe struct QWidgetPtr as "std::unique_ptr<QWidget>");

pub struct QtWindow {
    widget_ptr: QWidgetPtr,
    component: std::cell::RefCell<ComponentWeak>,
}

impl QtWindow {
    pub fn new() -> Rc<Self> {
        let widget_ptr = cpp! {unsafe [] -> QWidgetPtr as "std::unique_ptr<QWidget>" {
            ensure_initialized();
            return std::make_unique<SixtyFPSWidget>();
        }};
        let rc = Rc::new(QtWindow { widget_ptr, component: Default::default() });
        let widget_ptr = rc.widget_ptr();
        let rust_window = Rc::as_ptr(&rc);
        cpp! {unsafe [widget_ptr as "SixtyFPSWidget*", rust_window as "void*"]  {
            widget_ptr->rust_window = rust_window;
        }};
        rc
    }

    /// Return the QWidget*
    fn widget_ptr(&self) -> NonNull<()> {
        unsafe { std::mem::transmute_copy::<QWidgetPtr, NonNull<_>>(&self.widget_ptr) }
    }

    fn paint_event(&self, painter: &mut QPainter) {
        sixtyfps_corelib::animations::update_animations();

        let component_rc = self.component.borrow().upgrade().unwrap();
        let component = ComponentRc::borrow_pin(&component_rc);

        // FIXME: not every frame
        component.as_ref().apply_layout(Default::default());

        sixtyfps_corelib::item_rendering::render_component_items::<QtBackend>(
            &component_rc,
            painter,
            Point::default(),
        );
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
        todo!()
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
        todo!()
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

    fn set_focus_item(self: Rc<Self>, focus_item: &items::ItemRc) {
        todo!()
    }

    fn set_focus(self: Rc<Self>, have_focus: bool) {
        todo!()
    }

    fn show_popup(&self, popup: &sixtyfps_corelib::component::ComponentRc, position: Point) {
        todo!()
    }

    fn close_popup(&self) {
        todo!()
    }

    fn font(
        &self,
        request: sixtyfps_corelib::graphics::FontRequest,
    ) -> Option<Rc<dyn sixtyfps_corelib::graphics::Font>> {
        // FIXME
        None
    }
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

    fn font(
        &mut self,
        request: sixtyfps_corelib::graphics::FontRequest,
    ) -> Rc<dyn sixtyfps_corelib::graphics::Font> {
        todo!()
    }

    fn window(&self) -> &winit::window::Window {
        todo!()
    }
}
