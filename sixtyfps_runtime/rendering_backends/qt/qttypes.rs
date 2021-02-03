/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

//! Wrapper around some most important types
//! Some of it is actually copied from the qmetaobject crate

#![allow(dead_code)]
use cpp::{cpp, cpp_class};
use std::fmt::Display;
use std::os::raw::c_char;

cpp! {{
    #include <QtGui/QImage>
    #include <QtGui/QPixmap>
}}

cpp_class!(
    pub unsafe struct QImage as "QImage"
);

impl QImage {
    pub fn data(&self) -> &[u8] {
        unsafe {
            let len = cpp!([self as "const QImage*"] -> usize as "std::size_t" { return self->sizeInBytes(); });
            let data = cpp!([self as "const QImage*"] -> *const u8 as "const unsigned char*" { return self->bits(); });
            return core::slice::from_raw_parts(data, len);
        }
    }
    pub fn size(&self) -> QSize {
        cpp!(unsafe [self as "const QImage*"] -> QSize as "QSize" { return self->size(); })
    }
}

cpp_class!(
    pub unsafe struct QPixmap as "QPixmap"
);

impl QPixmap {
    pub fn size(&self) -> QSize {
        cpp!(unsafe [self as "const QPixmap*"] -> QSize as "QSize" { return self->size(); })
    }
}

cpp_class!(
    /// Wrapper around [`QByteArray`][class] class.
    ///
    /// [class]: https://doc.qt.io/qt-5/qbytearray.html
    #[derive(PartialEq, PartialOrd, Eq, Ord)]
    pub unsafe struct QByteArray as "QByteArray"
);
impl QByteArray {
    pub fn to_slice(&self) -> &[u8] {
        unsafe {
            let mut size: usize = 0;
            let c_ptr = cpp!([self as "const QByteArray*", mut size as "size_t"] -> *const u8 as "const char*" {
                size = self->size();
                return self->constData();
            });
            std::slice::from_raw_parts(c_ptr, size)
        }
    }
    pub fn to_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(self.to_slice())
    }
}
impl<'a> From<&'a [u8]> for QByteArray {
    /// Constructs a `QByteArray` from a slice. (Copy the slice.)
    fn from(s: &'a [u8]) -> QByteArray {
        let len = s.len();
        let ptr = s.as_ptr();
        cpp!(unsafe [len as "size_t", ptr as "char*"] -> QByteArray as "QByteArray" {
            return QByteArray(ptr, len);
        })
    }
}
impl<'a> From<&'a str> for QByteArray {
    /// Constructs a `QByteArray` from a `&str`. (Copy the string.)
    fn from(s: &'a str) -> QByteArray {
        s.as_bytes().into()
    }
}
impl From<String> for QByteArray {
    /// Constructs a `QByteArray` from a `String`. (Copy the string.)
    fn from(s: String) -> QByteArray {
        QByteArray::from(&*s)
    }
}
impl From<QString> for QByteArray {
    /// Converts a `QString` to a `QByteArray`
    fn from(s: QString) -> QByteArray {
        cpp!(unsafe [s as "QString"] -> QByteArray as "QByteArray" {
            return std::move(s).toUtf8();
        })
    }
}
impl Display for QByteArray {
    /// Prints the contents of the `QByteArray` if it contains UTF-8, do nothing otherwise.
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        unsafe {
            let c_ptr = cpp!([self as "const QByteArray*"] -> *const c_char as "const char*" {
                return self->constData();
            });
            f.write_str(std::ffi::CStr::from_ptr(c_ptr).to_str().map_err(|_| Default::default())?)
        }
    }
}
impl std::fmt::Debug for QByteArray {
    /// Prints the contents of the `QByteArray` if it contains UTF-8,  nothing otherwise
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

cpp_class!(
    /// Wrapper around [`QString`][class] class.
    ///
    /// [class]: https://doc.qt.io/qt-5/qstring.html
    #[derive(PartialEq, PartialOrd, Eq, Ord)]
    pub unsafe struct QString as "QString"
);
impl QString {
    /// Return a slice containing the UTF-16 data.
    pub fn to_slice(&self) -> &[u16] {
        unsafe {
            let mut size: usize = 0;
            let c_ptr = cpp!([self as "const QString*", mut size as "size_t"] -> *const u16 as "const QChar*" {
                size = self->size();
                return self->constData();
            });
            std::slice::from_raw_parts(c_ptr, size)
        }
    }
}

impl<'a> From<&'a str> for QString {
    /// Copy the data from a `&str`.
    fn from(s: &'a str) -> QString {
        let len = s.len();
        let ptr = s.as_ptr();
        cpp!(unsafe [len as "size_t", ptr as "char*"] -> QString as "QString" {
            return QString::fromUtf8(ptr, len);
        })
    }
}
impl From<String> for QString {
    fn from(s: String) -> QString {
        QString::from(&*s)
    }
}
impl Into<String> for QString {
    fn into(self) -> String {
        String::from_utf16_lossy(self.to_slice())
    }
}

/// Bindings for [`QSize`][class] class.
///
/// [class]: https://doc.qt.io/qt-5/qsize.html
#[repr(C)]
#[derive(Default, Clone, Copy, PartialEq, Debug)]
pub struct QSize {
    pub width: u32,
    pub height: u32,
}

/// Bindings for [`QSizeF`][class] class.
///
/// [class]: https://doc.qt.io/qt-5/qsizef.html
#[repr(C)]
#[derive(Default, Clone, Copy, PartialEq, Debug)]
pub struct QSizeF {
    pub width: qreal,
    pub height: qreal,
}

#[repr(C)]
#[derive(Default, Clone, Copy, PartialEq, Debug)]
pub struct QPoint {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Default, Clone, Copy, PartialEq, Debug)]
pub struct QPointF {
    pub x: qreal,
    pub y: qreal,
}

#[repr(C)]
#[derive(Default, Clone, Copy, PartialEq, Debug)]
pub struct QMargins {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// FIXME: qreal is not always f64
#[allow(non_camel_case_types)]
pub type qreal = f64;

#[repr(C)]
#[derive(Default, Clone, Copy, PartialEq, Debug)]
pub struct QRectF {
    pub x: qreal,
    pub y: qreal,
    pub width: qreal,
    pub height: qreal,
}

cpp_class!(
    pub unsafe struct QBrush as "QBrush"
);

impl std::convert::From<sixtyfps_corelib::Brush> for QBrush {
    fn from(brush: sixtyfps_corelib::Brush) -> Self {
        match brush {
            sixtyfps_corelib::Brush::NoBrush => QBrush::default(),
            sixtyfps_corelib::Brush::SolidColor(color) => {
                let color: u32 = color.as_argb_encoded();
                cpp!(unsafe [color as "QRgb"] -> QBrush as "QBrush" {
                    return QBrush(QColor::fromRgba(color));
                })
            }
            sixtyfps_corelib::Brush::LinearGradient(g) => {
                let angle = g.angle().to_radians();
                let r = (angle.sin().abs() + angle.cos().abs()) / 2.;
                let (y, x) = (angle - std::f32::consts::PI / 2.).sin_cos();
                let (y, x) = (y * r, x * r);
                let p1 = QPointF { x: (0.5 - x) as _, y: (0.5 - y) as _ };
                let p2 = QPointF { x: (0.5 + x) as _, y: (0.5 + y) as _ };
                cpp_class!(unsafe struct QLinearGradient as "QLinearGradient");
                let mut qlg = cpp! {
                    unsafe [p1 as "QPointF", p2 as "QPointF"] -> QLinearGradient as "QLinearGradient" {
                        return QLinearGradient(p1, p2);
                    }
                };
                for s in g.stops() {
                    let pos: f32 = s.position;
                    let color: u32 = s.color.as_argb_encoded();
                    cpp! {unsafe [mut qlg as "QLinearGradient", pos as "float", color as "QRgb"] {
                        qlg.setColorAt(pos, QColor::fromRgba(color));
                    }};
                }
                cpp! {unsafe [qlg as "QLinearGradient"] -> QBrush as "QBrush" {
                    return QBrush(qlg);
                }}
            }
        }
    }
}
