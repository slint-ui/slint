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
use sixtyfps_corelib::graphics::{Brush, FontRequest, Point, Rect, RenderingCache};
use sixtyfps_corelib::input::{InternalKeyCode, KeyEvent, KeyEventType, MouseEventType};
use sixtyfps_corelib::item_rendering::{CachedRenderingData, ItemRenderer};
use sixtyfps_corelib::items::{self, FillRule, ItemRef, TextOverflow, TextWrap};
use sixtyfps_corelib::properties::PropertyTracker;
use sixtyfps_corelib::slice::Slice;
use sixtyfps_corelib::window::PlatformWindow;
use sixtyfps_corelib::{component::ComponentRc, SharedString};
use sixtyfps_corelib::{PathData, Property, Resource};

use std::cell::RefCell;
use std::pin::Pin;
use std::ptr::NonNull;
use std::rc::{Rc, Weak};

use crate::key_generated;

use super::qttypes;

cpp! {{
    #include <QtWidgets/QWidget>
    #include <QtGui/QPainter>
    #include <QtGui/QPaintEngine>
    #include <QtGui/QPainterPath>
    #include <QtGui/QWindow>
    #include <QtGui/QResizeEvent>
    #include <QtGui/QTextLayout>
    #include <QtCore/QBasicTimer>
    #include <QtCore/QTimer>
    #include <QtCore/QPointer>
    #include <memory>
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

        SixtyFPSWidget() {
            setMouseTracking(true);
        }

        void paintEvent(QPaintEvent *) override {
            QPainter painter(this);
            painter.setRenderHints(QPainter::Antialiasing | QPainter::SmoothPixmapTransform);
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
            if (auto p = dynamic_cast<const SixtyFPSWidget*>(parent())) {
                // FIXME: better way to close the popup
                void *parent_window = p->rust_window;
                rust!(SFPS_mouseReleaseEventPopup [parent_window: &QtWindow as "void*", pos: qttypes::QPoint as "QPoint"] {
                    parent_window.close_popup();
                });
            }
        }
        void mouseMoveEvent(QMouseEvent *event) override {
            QPoint pos = event->pos();
            rust!(SFPS_mouseMoveEvent [rust_window: &QtWindow as "void*", pos: qttypes::QPoint as "QPoint"] {
                rust_window.mouse_event(MouseEventType::MouseMoved, pos)
            });
        }

        void keyPressEvent(QKeyEvent *event) override {
            uint modif = uint(event->modifiers());
            QString text =  event->text();
            int key = event->key();
            rust!(SFPS_keyPress [rust_window: &QtWindow as "void*", key: i32 as "int", text: qttypes::QString as "QString", modif: u32 as "uint"] {
                rust_window.key_event(key, text.clone(), modif, false);
            });
        }
        void keyReleaseEvent(QKeyEvent *event) override {
            uint modif = uint(event->modifiers());
            QString text =  event->text();
            int key = event->key();
            rust!(SFPS_keyRelease [rust_window: &QtWindow as "void*", key: i32 as "int", text: qttypes::QString as "QString", modif: u32 as "uint"] {
                rust_window.key_event(key, text.clone(), modif, true);
            });
        }
    };

}}

cpp_class! {pub unsafe struct QPainter as "QPainter"}

impl QPainter {
    pub fn save_state(&mut self) {
        cpp! { unsafe [self as "QPainter*"] {
            self->save();
        }}
    }

    pub fn restore_state(&mut self) {
        cpp! { unsafe [self as "QPainter*"] {
            self->restore();
        }}
    }
}

cpp_class! {pub unsafe struct QPainterPath as "QPainterPath"}

impl QPainterPath {
    /*
    pub fn reserve(&mut self, size: usize) {
        cpp! { unsafe [self as "QPainterPath*", size as "long long"] {
            self->reserve(size);
        }}
    }*/

    pub fn move_to(&mut self, to: qttypes::QPointF) {
        cpp! { unsafe [self as "QPainterPath*", to as "QPointF"] {
            self->moveTo(to);
        }}
    }
    pub fn line_to(&mut self, to: qttypes::QPointF) {
        cpp! { unsafe [self as "QPainterPath*", to as "QPointF"] {
            self->lineTo(to);
        }}
    }
    pub fn quad_to(&mut self, ctrl: qttypes::QPointF, to: qttypes::QPointF) {
        cpp! { unsafe [self as "QPainterPath*", ctrl as "QPointF", to as "QPointF"] {
            self->quadTo(ctrl, to);
        }}
    }
    pub fn cubic_to(
        &mut self,
        ctrl1: qttypes::QPointF,
        ctrl2: qttypes::QPointF,
        to: qttypes::QPointF,
    ) {
        cpp! { unsafe [self as "QPainterPath*", ctrl1 as "QPointF", ctrl2 as "QPointF", to as "QPointF"] {
            self->cubicTo(ctrl1, ctrl2, to);
        }}
    }

    pub fn close(&mut self) {
        cpp! { unsafe [self as "QPainterPath*"] {
            self->closeSubpath();
        }}
    }

    pub fn set_fill_rule(&mut self, rule: key_generated::Qt_FillRule) {
        cpp! { unsafe [self as "QPainterPath*", rule as "Qt::FillRule" ] {
            self->setFillRule(rule);
        }}
    }
}

/// Given a position offset and an object of a given type that has x,y,width,height properties,
/// create a QRectF that fits it.
macro_rules! get_geometry {
    ($pos:expr, $ty:ty, $obj:expr) => {{
        type Ty = $ty;
        let width = Ty::FIELD_OFFSETS.width.apply_pin($obj).get();
        let height = Ty::FIELD_OFFSETS.height.apply_pin($obj).get();
        let x = Ty::FIELD_OFFSETS.x.apply_pin($obj).get();
        let y = Ty::FIELD_OFFSETS.y.apply_pin($obj).get();
        if width <= 0. || height <= 0. {
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

#[derive(Clone)]
enum QtRenderingCacheItem {
    Pixmap(qttypes::QPixmap),
    Invalid,
}

type QtRenderingCache = Rc<RefCell<RenderingCache<QtRenderingCacheItem>>>;

struct QtItemRenderer<'a> {
    painter: &'a mut QPainter,
    cache: QtRenderingCache,
}

impl ItemRenderer for QtItemRenderer<'_> {
    fn draw_rectangle(&mut self, pos: Point, rect: Pin<&items::Rectangle>) {
        let pos = qttypes::QPoint { x: pos.x as _, y: pos.y as _ };
        let brush: qttypes::QBrush = rect.background().into();
        let rect: qttypes::QRectF = get_geometry!(pos, items::Rectangle, rect);
        let painter: &mut QPainter = &mut *self.painter;
        cpp! { unsafe [painter as "QPainter*", brush as "QBrush", rect as "QRectF"] {
            painter->fillRect(rect, brush);
        }}
    }

    fn draw_border_rectangle(&mut self, pos: Point, rect: std::pin::Pin<&items::BorderRectangle>) {
        self.draw_rectangle_impl(
            get_geometry!(pos, items::BorderRectangle, rect),
            rect.background(),
            rect.border_color(),
            rect.border_width(),
            rect.border_radius(),
        );
    }

    fn draw_image(&mut self, pos: Point, image: Pin<&items::Image>) {
        let dest_rect: qttypes::QRectF = get_geometry!(pos, items::Image, image);
        self.draw_image_impl(
            &image.cached_rendering_data,
            items::Image::FIELD_OFFSETS.source.apply_pin(image),
            dest_rect,
            None,
            image.image_fit(),
            None,
        );
    }

    fn draw_clipped_image(&mut self, pos: Point, image: Pin<&items::ClippedImage>) {
        let dest_rect: qttypes::QRectF = get_geometry!(pos, items::ClippedImage, image);
        let source_rect = qttypes::QRectF {
            x: image.source_clip_x() as _,
            y: image.source_clip_y() as _,
            width: image.source_clip_width() as _,
            height: image.source_clip_height() as _,
        };
        self.draw_image_impl(
            &image.cached_rendering_data,
            items::ClippedImage::FIELD_OFFSETS.source.apply_pin(image),
            dest_rect,
            Some(source_rect),
            image.image_fit(),
            Some(items::ClippedImage::FIELD_OFFSETS.colorize.apply_pin(image)),
        );
    }

    fn draw_text(&mut self, pos: Point, text: std::pin::Pin<&items::Text>) {
        let rect: qttypes::QRectF = get_geometry!(pos, items::Text, text);
        let fill_brush: qttypes::QBrush = text.color().into();
        let string: qttypes::QString = text.text().as_str().into();
        let font: QFont = get_font(text.font_request());
        let flags = match text.horizontal_alignment() {
            TextHorizontalAlignment::left => key_generated::Qt_AlignmentFlag_AlignLeft,
            TextHorizontalAlignment::center => key_generated::Qt_AlignmentFlag_AlignHCenter,
            TextHorizontalAlignment::right => key_generated::Qt_AlignmentFlag_AlignRight,
        } | match text.vertical_alignment() {
            TextVerticalAlignment::top => key_generated::Qt_AlignmentFlag_AlignTop,
            TextVerticalAlignment::center => key_generated::Qt_AlignmentFlag_AlignVCenter,
            TextVerticalAlignment::bottom => key_generated::Qt_AlignmentFlag_AlignBottom,
        } | match text.wrap() {
            TextWrap::no_wrap => 0,
            TextWrap::word_wrap => key_generated::Qt_TextFlag_TextWordWrap,
        };
        let elide = text.overflow() == TextOverflow::elide && text.wrap() == TextWrap::no_wrap;
        let painter: &mut QPainter = &mut *self.painter;
        cpp! { unsafe [painter as "QPainter*", rect as "QRectF", fill_brush as "QBrush", string as "QString", flags as "int", font as "QFont", elide as "bool"] {
            painter->setFont(font);
            painter->setPen(QPen(fill_brush, 0));
            painter->setBrush(Qt::NoBrush);
            if (!elide) {
                painter->drawText(rect, flags, string);
            } else {
                auto elided = QFontMetrics(font).elidedText(string, Qt::ElideRight, rect.width());
                painter->drawText(rect, flags, elided);
            }
        }}
    }

    fn draw_text_input(&mut self, pos: Point, text_input: std::pin::Pin<&items::TextInput>) {
        let rect: qttypes::QRectF = get_geometry!(pos, items::TextInput, text_input);
        let fill_brush: qttypes::QBrush = text_input.color().into();
        let selection_foreground_color: u32 =
            text_input.selection_foreground_color().as_argb_encoded();
        let selection_background_color: u32 =
            text_input.selection_background_color().as_argb_encoded();

        let string: qttypes::QString = text_input.text().as_str().into();
        let font: QFont = get_font(text_input.font_request());
        let flags = match text_input.horizontal_alignment() {
            TextHorizontalAlignment::left => key_generated::Qt_AlignmentFlag_AlignLeft,
            TextHorizontalAlignment::center => key_generated::Qt_AlignmentFlag_AlignHCenter,
            TextHorizontalAlignment::right => key_generated::Qt_AlignmentFlag_AlignRight,
        } | match text_input.vertical_alignment() {
            TextVerticalAlignment::top => key_generated::Qt_AlignmentFlag_AlignTop,
            TextVerticalAlignment::center => key_generated::Qt_AlignmentFlag_AlignVCenter,
            TextVerticalAlignment::bottom => key_generated::Qt_AlignmentFlag_AlignBottom,
        };
        let cursor_position: i32 = text_input.cursor_position();
        let anchor_position: i32 = text_input.anchor_position();
        let text_cursor_width: f32 =
            if text_input.cursor_visible() { text_input.text_cursor_width() } else { 0. };

        let painter: &mut QPainter = &mut *self.painter;
        cpp! { unsafe [
                painter as "QPainter*",
                rect as "QRectF",
                fill_brush as "QBrush",
                selection_foreground_color as "QRgb",
                selection_background_color as "QRgb",
                string as "QString",
                flags as "int",
                font as "QFont",
                cursor_position as "int",
                anchor_position as "int",
                text_cursor_width as "float"] {
            Q_UNUSED(flags); // FIXME
            QTextLayout layout(string, font);
            layout.beginLayout();
            layout.createLine();
            layout.endLayout();
            painter->setPen(QPen(fill_brush, 0));
            QVector<QTextLayout::FormatRange> selections;
            if (anchor_position != cursor_position) {
                QTextCharFormat fmt;
                fmt.setBackground(QColor(selection_background_color));
                fmt.setForeground(QColor(selection_foreground_color));
                selections << QTextLayout::FormatRange{
                    std::min(anchor_position, cursor_position),
                    std::abs(anchor_position - cursor_position),
                    fmt
                };
            }
            layout.draw(painter, rect.topLeft(), selections);
            if (text_cursor_width > 0) {
                layout.drawCursor(painter, rect.topLeft(), cursor_position, text_cursor_width);
            }
        }}
    }

    fn draw_path(&mut self, pos: Point, path: Pin<&items::Path>) {
        let elements = path.elements();
        if matches!(elements, PathData::None) {
            return;
        }
        // FIXME: handle width/height
        //let rect: qttypes::QRectF = get_geometry!(pos, items::Path, path);
        let fill_brush: qttypes::QBrush = path.fill().into();
        let stroke_brush: qttypes::QBrush = path.stroke().into();
        let stroke_width: f32 = path.stroke_width();
        let (offset, path_events) = path.fitted_path_events();
        let pos = qttypes::QPoint {
            x: (pos.x + path.x() + offset.x) as _,
            y: (pos.y + path.y() + offset.y) as _,
        };
        let mut painter_path = QPainterPath::default();

        painter_path.set_fill_rule(match path.fill_rule() {
            FillRule::nonzero => key_generated::Qt_FillRule_WindingFill,
            FillRule::evenodd => key_generated::Qt_FillRule_OddEvenFill,
        });

        for x in path_events.iter() {
            impl From<Point> for qttypes::QPointF {
                fn from(p: Point) -> Self {
                    qttypes::QPointF { x: p.x as _, y: p.y as _ }
                }
            }
            match x {
                lyon_path::Event::Begin { at } => {
                    painter_path.move_to(at.into());
                }
                lyon_path::Event::Line { from: _, to } => {
                    painter_path.line_to(to.into());
                }
                lyon_path::Event::Quadratic { from: _, ctrl, to } => {
                    painter_path.quad_to(ctrl.into(), to.into());
                }

                lyon_path::Event::Cubic { from: _, ctrl1, ctrl2, to } => {
                    painter_path.cubic_to(ctrl1.into(), ctrl2.into(), to.into());
                }
                lyon_path::Event::End { last: _, first: _, close } => {
                    // FIXME: are we supposed to do something with last and first?
                    if close {
                        painter_path.close()
                    }
                }
            }
        }

        let painter: &mut QPainter = &mut *self.painter;
        cpp! { unsafe [
                painter as "QPainter*",
                pos as "QPoint",
                mut painter_path as "QPainterPath",
                fill_brush as "QBrush",
                stroke_brush as "QBrush",
                stroke_width as "float"] {
            painter->save();
            auto cleanup = qScopeGuard([&] { painter->restore(); });
            painter->translate(pos);
            painter->setPen(stroke_width > 0 ? QPen(stroke_brush, stroke_width) : Qt::NoPen);
            painter->setBrush(fill_brush);
            painter->drawPath(painter_path);
        }}
    }

    fn draw_box_shadow(&mut self, pos: Point, box_shadow: Pin<&items::BoxShadow>) {
        // This could be improved to use a guassian blur.

        let mut shadow_rect = get_geometry!(pos, items::BoxShadow, box_shadow);
        shadow_rect.x += box_shadow.offset_x() as f64;
        shadow_rect.y += box_shadow.offset_y() as f64;

        self.draw_rectangle_impl(
            shadow_rect,
            Brush::SolidColor(box_shadow.color()),
            Brush::default(),
            0.,
            box_shadow.border_radius(),
        );
    }

    fn combine_clip(&mut self, pos: Point, rect: Rect) {
        let clip_rect = qttypes::QRectF {
            x: (rect.min_x() + pos.x as f32) as _,
            y: (rect.min_y() + pos.y as f32) as _,
            width: rect.width() as _,
            height: rect.height() as _,
        };
        let painter: &mut QPainter = &mut *self.painter;
        cpp! { unsafe [painter as "QPainter*", clip_rect as "QRectF"] {
            painter->setClipRect(clip_rect, Qt::IntersectClip);
        }}
    }

    fn save_state(&mut self) {
        self.painter.save_state()
    }

    fn restore_state(&mut self) {
        self.painter.restore_state()
    }

    fn scale_factor(&self) -> f32 {
        return 1.;
        /* cpp! { unsafe [painter as "QPainter*"] -> f32 as "float" {
            return painter->paintEngine()->paintDevice()->devicePixelRatioF();
        }} */
    }

    fn draw_cached_pixmap(
        &mut self,
        _item_cache: &sixtyfps_corelib::item_rendering::CachedRenderingData,
        pos: Point,
        update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    ) {
        update_fn(&mut |width: u32, height: u32, data: &[u8]| {
            let pos = qttypes::QPoint { x: pos.x as _, y: pos.y as _ };
            let data = data.as_ptr();
            let painter: &mut QPainter = &mut *self.painter;
            cpp! { unsafe [painter as "QPainter*", pos as "QPoint", width as "int", height as "int", data as "const unsigned char *"] {
                QImage img(data, width, height, width * 4, QImage::Format_ARGB32_Premultiplied);
                painter->drawImage(pos, img);
            }}
        })
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self.painter
    }
}

fn load_image_from_resource(resource: Resource) -> Option<qttypes::QPixmap> {
    let (is_path, data) = match resource {
        Resource::None => return None,
        Resource::AbsoluteFilePath(path) => (true, qttypes::QByteArray::from(path.as_str())),
        Resource::EmbeddedData(data) => (false, qttypes::QByteArray::from(data.as_slice())),
        Resource::EmbeddedRgbaImage { .. } => todo!(),
    };
    Some(cpp! { unsafe [data as "QByteArray", is_path as "bool"] -> qttypes::QPixmap as "QPixmap" {
        QPixmap img;
        is_path ? img.load(QString::fromUtf8(data)) : img.loadFromData(data);
        return img;
    }})
}

impl QtItemRenderer<'_> {
    fn draw_image_impl(
        &mut self,
        item_cache: &CachedRenderingData,
        source_property: Pin<&Property<Resource>>,
        dest_rect: qttypes::QRectF,
        source_rect: Option<qttypes::QRectF>,
        image_fit: ImageFit,
        colorize_property: Option<Pin<&Property<Brush>>>,
    ) {
        let cached = item_cache.ensure_up_to_date(&mut self.cache.borrow_mut(), || {
            load_image_from_resource(source_property.get()).map_or(
                QtRenderingCacheItem::Invalid,
                |mut pixmap: qttypes::QPixmap| {
                    let colorize = colorize_property.map_or(Brush::default(), |c| c.get());
                    if colorize != Brush::NoBrush {
                        let brush: qttypes::QBrush = colorize.into();
                        cpp!(unsafe [mut pixmap as "QPixmap", brush as "QBrush"] {
                            QPainter p(&pixmap);
                            p.setCompositionMode(QPainter::CompositionMode_SourceIn);
                            p.fillRect(QRect(QPoint(), pixmap.size()), brush);
                        });
                    }
                    QtRenderingCacheItem::Pixmap(pixmap)
                },
            )
        });
        let pixmap: &qttypes::QPixmap = match &cached {
            QtRenderingCacheItem::Pixmap(pixmap) => pixmap,
            _ => return,
        };
        let image_size = pixmap.size();
        let mut source_rect = source_rect.unwrap_or_else(|| qttypes::QRectF {
            x: 0.,
            y: 0.,
            width: image_size.width as _,
            height: image_size.height as _,
        });
        let mut dest_rect = dest_rect;
        match image_fit {
            sixtyfps_corelib::items::ImageFit::fill => (),
            sixtyfps_corelib::items::ImageFit::cover => {
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
            sixtyfps_corelib::items::ImageFit::contain => {
                let ratio = qttypes::qreal::min(
                    dest_rect.width / source_rect.width,
                    dest_rect.height / source_rect.height,
                );
                if dest_rect.width > source_rect.width * ratio {
                    dest_rect.x += (dest_rect.width - source_rect.width * ratio) / 2.;
                    dest_rect.width = source_rect.width * ratio;
                }
                if dest_rect.height > source_rect.height * ratio {
                    dest_rect.y += (dest_rect.height - source_rect.height * ratio) / 2.;
                    dest_rect.height = source_rect.height * ratio;
                }
            }
        };
        let painter: &mut QPainter = &mut *self.painter;
        cpp! { unsafe [painter as "QPainter*", pixmap as "QPixmap*", source_rect as "QRectF", dest_rect as "QRectF"] {
            painter->drawPixmap(dest_rect, *pixmap, source_rect);
        }};
    }

    fn draw_rectangle_impl(
        &mut self,
        mut rect: qttypes::QRectF,
        brush: Brush,
        border_color: Brush,
        border_width: f32,
        border_radius: f32,
    ) {
        let brush: qttypes::QBrush = brush.into();
        let border_color: qttypes::QBrush = border_color.into();
        let border_width: f32 = border_width.min((rect.width as f32) / 2.);
        // adjust the size so that the border is drawn within the geometry
        rect.x += border_width as f64 / 2.;
        rect.y += border_width as f64 / 2.;
        rect.width -= border_width as f64;
        rect.height -= border_width as f64;
        let painter: &mut QPainter = &mut *self.painter;
        cpp! { unsafe [painter as "QPainter*", brush as "QBrush",  border_color as "QBrush", border_width as "float", border_radius as "float", rect as "QRectF"] {
            painter->setPen(border_width > 0 ? QPen(border_color, border_width) : Qt::NoPen);
            painter->setBrush(brush);
            if (border_radius > 0) {
                painter->drawRoundedRect(rect, border_radius, border_radius);
            } else {
                painter->drawRect(rect);
            }
        }}
    }
}

cpp_class!(unsafe struct QWidgetPtr as "std::unique_ptr<QWidget>");

pub struct QtWindow {
    widget_ptr: QWidgetPtr,
    pub(crate) self_weak: once_cell::unsync::OnceCell<Weak<sixtyfps_corelib::window::Window>>,

    /// Gets dirty when the layout restrictions, or some other property of the windows change
    meta_property_listener: Pin<Rc<PropertyTracker>>,
    /// Gets dirty if something needs to be painted
    redraw_listener: Pin<Rc<PropertyTracker>>,

    popup_window: RefCell<Option<(Rc<sixtyfps_corelib::window::Window>, ComponentRc)>>,

    cache: QtRenderingCache,

    scale_factor: Pin<Box<Property<f32>>>,
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
            meta_property_listener: Rc::pin(Default::default()),
            redraw_listener: Rc::pin(Default::default()),
            popup_window: Default::default(),
            cache: Default::default(),
            scale_factor: Box::pin(Property::new(1.)),
        });
        let self_weak = Rc::downgrade(&rc);
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

        let component_rc = self.self_weak.get().unwrap().upgrade().unwrap().component();
        let component = ComponentRc::borrow_pin(&component_rc);

        self.meta_property_listener.as_ref().evaluate_if_dirty(|| {
            self.apply_geometry_constraint(component.as_ref().layout_info());
            component.as_ref().apply_layout(Default::default());

            let root_item = component.as_ref().get_item_ref(0);
            if let Some(window_item) = ItemRef::downcast_pin(root_item) {
                self.apply_window_properties(window_item);
            }
        });

        let cache = self.cache.clone();
        self.redraw_listener.as_ref().evaluate(|| {
            let mut renderer = QtItemRenderer { painter, cache };
            sixtyfps_corelib::item_rendering::render_component_items(
                &component_rc,
                &mut renderer,
                Point::default(),
            );
        });

        sixtyfps_corelib::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            if !driver.has_active_animations() {
                return;
            }
            let widget_ptr = self.widget_ptr();
            cpp! {unsafe [widget_ptr as "QWidget*"] {
                // FIXME: using QTimer -::singleShot is not optimal. We should use Qt animation timer
                QTimer::singleShot(16, [widget_ptr = QPointer<QWidget>(widget_ptr)] {
                    if (widget_ptr)
                        widget_ptr->update();
                });
                //return widget_ptr->update();
            }}
        });
    }

    fn resize_event(&self, size: qttypes::QSize) {
        let component_rc = self.self_weak.get().unwrap().upgrade().unwrap().component();
        let component = ComponentRc::borrow_pin(&component_rc);
        let root_item = component.as_ref().get_item_ref(0);
        if let Some(window_item) = ItemRef::downcast_pin::<items::Window>(root_item) {
            window_item.width.set(size.width as _);
            window_item.height.set(size.height as _);
        }
    }

    fn mouse_event(&self, what: MouseEventType, pos: qttypes::QPoint) {
        let pos = Point::new(pos.x as _, pos.y as _);
        self.self_weak.get().unwrap().upgrade().unwrap().process_mouse_input(pos, what);
        timer_event();
    }

    fn key_event(&self, key: i32, text: qttypes::QString, modif: u32, released: bool) {
        sixtyfps_corelib::animations::update_animations();
        let text: String = text.into();
        let modifiers = sixtyfps_corelib::input::KeyboardModifiers {
            control: (modif & key_generated::Qt_KeyboardModifier_ControlModifier) != 0,
            alt: (modif & key_generated::Qt_KeyboardModifier_AltModifier) != 0,
            shift: (modif & key_generated::Qt_KeyboardModifier_ShiftModifier) != 0,
            meta: (modif & key_generated::Qt_KeyboardModifier_MetaModifier) != 0,
        };

        let text = qt_key_to_string(key as key_generated::Qt_Key, text);

        let event = KeyEvent {
            event_type: if released { KeyEventType::KeyReleased } else { KeyEventType::KeyPressed },
            text,
            modifiers,
        };
        self.self_weak.get().unwrap().upgrade().unwrap().process_key_input(&event);

        timer_event();
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
        let title: qttypes::QString = window_item.title().as_str().into();
        let size = qttypes::QSize {
            width: window_item.width().ceil() as _,
            height: window_item.height().ceil() as _,
        };
        cpp! {unsafe [widget_ptr as "QWidget*",  title as "QString", size as "QSize"] {
            if (!size.isEmpty())
                widget_ptr->resize(size);
            widget_ptr->setWindowTitle(title);
        }};
    }
}

#[allow(unused)]
impl PlatformWindow for QtWindow {
    fn show(self: Rc<Self>) {
        let component_rc = self.self_weak.get().unwrap().upgrade().unwrap().component();
        let component = ComponentRc::borrow_pin(&component_rc);
        component.as_ref().apply_layout(Default::default());
        let root_item = component.as_ref().get_item_ref(0);
        if let Some(window_item) = ItemRef::downcast_pin(root_item) {
            self.apply_window_properties(window_item);
        }

        let widget_ptr = self.widget_ptr();
        cpp! {unsafe [widget_ptr as "QWidget*"] {
            widget_ptr->show();
        }};
    }

    fn hide(self: Rc<Self>) {
        let widget_ptr = self.widget_ptr();
        cpp! {unsafe [widget_ptr as "QWidget*"] {
            widget_ptr->hide();
        }};
    }

    fn request_redraw(&self) {
        // We should check that redraw_listener.is_dirty, but that does not take in account the repeater
        //if self.redraw_listener.is_dirty() {
        let widget_ptr = self.widget_ptr();
        cpp! {unsafe [widget_ptr as "QWidget*"] {
            return widget_ptr->update();
        }}
        //}
    }

    fn scale_factor(&self) -> f32 {
        self.scale_factor.as_ref().get()
        /* let widget_ptr = self.widget_ptr();
        cpp! {unsafe [widget_ptr as "QWidget*"] -> f32 as "float" {
            return widget_ptr->windowHandle()->devicePixelRatio();
        }} */
    }

    /// Only used for testing
    fn set_scale_factor(&self, factor: f32) {
        self.scale_factor.as_ref().set(factor)
    }

    fn get_geometry(&self) -> sixtyfps_corelib::graphics::Rect {
        // FIXME
        Default::default()
    }

    fn free_graphics_resources<'a>(self: Rc<Self>, items: &Slice<'a, Pin<items::ItemRef<'a>>>) {
        for item in items.iter() {
            let cached_rendering_data = item.cached_rendering_data_offset();
            cached_rendering_data.release(&mut self.cache.borrow_mut());
        }
    }

    fn show_popup(&self, popup: &sixtyfps_corelib::component::ComponentRc, position: Point) {
        let popup_window = QtWindow::new();
        let window = Rc::new(sixtyfps_corelib::window::Window::new(popup_window.clone()));
        popup_window.self_weak.set(Rc::downgrade(&window)).ok().unwrap();
        window.set_component(popup);
        let popup_ptr = popup_window.widget_ptr();
        let pos = qttypes::QPoint { x: position.x as _, y: position.y as _ };
        let widget_ptr = self.widget_ptr();
        cpp! {unsafe [widget_ptr as "QWidget*", popup_ptr as "QWidget*", pos as "QPoint"] {
            popup_ptr->setParent(widget_ptr, Qt::Popup);
            popup_ptr->move(pos + widget_ptr->pos());
            popup_ptr->show();
        }};
        self.popup_window.replace(Some((window, popup.clone())));
    }

    fn close_popup(&self) {
        self.popup_window.replace(None);
    }

    fn font_metrics(
        &self,
        request: FontRequest,
    ) -> Option<Box<dyn sixtyfps_corelib::graphics::FontMetrics>> {
        Some(Box::new(get_font(request)))
    }

    fn image_size(
        &self,
        _item_graphics_cache: &sixtyfps_corelib::item_rendering::CachedRenderingData,
        source: Pin<&sixtyfps_corelib::properties::Property<Resource>>,
    ) -> sixtyfps_corelib::graphics::Size {
        load_image_from_resource(source.get())
            .map(|img| {
                let qsize = img.size();
                euclid::size2(qsize.width as f32, qsize.height as f32)
            })
            .unwrap_or_default()
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
            f.setWeight((weight-100)/8);
        return f;
    })
}

cpp_class! {pub unsafe struct QFont as "QFont"}

impl sixtyfps_corelib::graphics::FontMetrics for QFont {
    fn text_size(&self, text: &str) -> sixtyfps_corelib::graphics::Size {
        let string = qttypes::QString::from(text);
        let size = cpp! { unsafe [self as "const QFont*",  string as "QString"]
                -> qttypes::QSizeF as "QSizeF"{
            return QFontMetricsF(*self).boundingRect(QRectF(), 0, string).size();
        }};
        sixtyfps_corelib::graphics::Size::new(size.width as _, size.height as _)
    }

    fn text_offset_for_x_position<'a>(&self, text: &'a str, x: f32) -> usize {
        let string = qttypes::QString::from(text);
        cpp! { unsafe [self as "const QFont*", string as "QString", x as "float"] -> usize as "long long" {
            QTextLayout layout(string, *self);
            layout.beginLayout();
            layout.createLine();
            layout.endLayout();
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

    let mut timeout = sixtyfps_corelib::timers::TimerList::next_timeout().map(|instant| {
        let now = std::time::Instant::now();
        if instant > now {
            instant.duration_since(now).as_millis() as i32
        } else {
            0
        }
    });
    if sixtyfps_corelib::animations::CURRENT_ANIMATION_DRIVER
        .with(|driver| driver.has_active_animations())
    {
        timeout = timeout.map(|x| x.max(16)).or(Some(16));
    };
    if let Some(timeout) = timeout {
        cpp! { unsafe [timeout as "int"] {
            TimerHandler::instance().timer.start(timeout, &TimerHandler::instance());
        }}
    }
}

fn qt_key_to_string(key: key_generated::Qt_Key, event_text: String) -> SharedString {
    // First try to see if we received one of the non-ascii keys that we have
    // a special representation for. If that fails, try to use the provided
    // text. If that's empty, then try to see if the provided key has an ascii
    // representation. The last step is needed because modifiers may result in
    // the text to be empty otherwise, for example Ctrl+C.
    if let Some(special_key_code) = match key as key_generated::Qt_Key {
        key_generated::Qt_Key_Key_Left => Some(InternalKeyCode::Left),
        key_generated::Qt_Key_Key_Right => Some(InternalKeyCode::Right),
        key_generated::Qt_Key_Key_Backspace => Some(InternalKeyCode::Back),
        key_generated::Qt_Key_Key_Delete => Some(InternalKeyCode::Delete),
        key_generated::Qt_Key_Key_End => Some(InternalKeyCode::End),
        key_generated::Qt_Key_Key_Home => Some(InternalKeyCode::Home),
        key_generated::Qt_Key_Key_Return => Some(InternalKeyCode::Return),
        _ => None,
    } {
        return special_key_code.encode_to_string();
    };

    if !event_text.is_empty() {
        return event_text.into();
    }

    match key {
        key_generated::Qt_Key_Key_Space => " ",
        key_generated::Qt_Key_Key_Exclam => "!",
        key_generated::Qt_Key_Key_QuoteDbl => "\"",
        key_generated::Qt_Key_Key_NumberSign => "#",
        key_generated::Qt_Key_Key_Dollar => "$",
        key_generated::Qt_Key_Key_Percent => "%",
        key_generated::Qt_Key_Key_Ampersand => "&",
        key_generated::Qt_Key_Key_Apostrophe => "'",
        key_generated::Qt_Key_Key_ParenLeft => "(",
        key_generated::Qt_Key_Key_ParenRight => ")",
        key_generated::Qt_Key_Key_Asterisk => "*",
        key_generated::Qt_Key_Key_Plus => "+",
        key_generated::Qt_Key_Key_Comma => ",",
        key_generated::Qt_Key_Key_Minus => "-",
        key_generated::Qt_Key_Key_Period => ".",
        key_generated::Qt_Key_Key_Slash => "/",
        key_generated::Qt_Key_Key_0 => "0",
        key_generated::Qt_Key_Key_1 => "1",
        key_generated::Qt_Key_Key_2 => "2",
        key_generated::Qt_Key_Key_3 => "3",
        key_generated::Qt_Key_Key_4 => "4",
        key_generated::Qt_Key_Key_5 => "5",
        key_generated::Qt_Key_Key_6 => "6",
        key_generated::Qt_Key_Key_7 => "7",
        key_generated::Qt_Key_Key_8 => "8",
        key_generated::Qt_Key_Key_9 => "9",
        key_generated::Qt_Key_Key_Colon => ":",
        key_generated::Qt_Key_Key_Semicolon => ";",
        key_generated::Qt_Key_Key_Less => "<",
        key_generated::Qt_Key_Key_Equal => "=",
        key_generated::Qt_Key_Key_Greater => ">",
        key_generated::Qt_Key_Key_Question => "?",
        key_generated::Qt_Key_Key_At => "@",
        key_generated::Qt_Key_Key_A => "a",
        key_generated::Qt_Key_Key_B => "b",
        key_generated::Qt_Key_Key_C => "c",
        key_generated::Qt_Key_Key_D => "d",
        key_generated::Qt_Key_Key_E => "e",
        key_generated::Qt_Key_Key_F => "f",
        key_generated::Qt_Key_Key_G => "g",
        key_generated::Qt_Key_Key_H => "h",
        key_generated::Qt_Key_Key_I => "i",
        key_generated::Qt_Key_Key_J => "j",
        key_generated::Qt_Key_Key_K => "k",
        key_generated::Qt_Key_Key_L => "l",
        key_generated::Qt_Key_Key_M => "m",
        key_generated::Qt_Key_Key_N => "n",
        key_generated::Qt_Key_Key_O => "o",
        key_generated::Qt_Key_Key_P => "p",
        key_generated::Qt_Key_Key_Q => "q",
        key_generated::Qt_Key_Key_R => "r",
        key_generated::Qt_Key_Key_S => "s",
        key_generated::Qt_Key_Key_T => "t",
        key_generated::Qt_Key_Key_U => "u",
        key_generated::Qt_Key_Key_V => "v",
        key_generated::Qt_Key_Key_W => "w",
        key_generated::Qt_Key_Key_X => "x",
        key_generated::Qt_Key_Key_Y => "y",
        key_generated::Qt_Key_Key_Z => "z",
        key_generated::Qt_Key_Key_BracketLeft => "[",
        key_generated::Qt_Key_Key_Backslash => "\\",
        key_generated::Qt_Key_Key_BracketRight => "]",
        key_generated::Qt_Key_Key_AsciiCircum => "^",
        key_generated::Qt_Key_Key_Underscore => "_",
        key_generated::Qt_Key_Key_QuoteLeft => "`",
        key_generated::Qt_Key_Key_BraceLeft => "{",
        key_generated::Qt_Key_Key_Bar => "|",
        key_generated::Qt_Key_Key_BraceRight => "}",
        key_generated::Qt_Key_Key_AsciiTilde => "~",
        _ => "",
    }
    .into()
}
