
        mod __gl_imports {
            pub use std::mem;
            pub use std::os::raw;
        }
    

        pub mod types {
            #![allow(non_camel_case_types, non_snake_case, dead_code, missing_copy_implementations)]
    
// Common types from OpenGL 1.1
pub type GLenum = super::__gl_imports::raw::c_uint;
pub type GLboolean = super::__gl_imports::raw::c_uchar;
pub type GLbitfield = super::__gl_imports::raw::c_uint;
pub type GLvoid = super::__gl_imports::raw::c_void;
pub type GLbyte = super::__gl_imports::raw::c_char;
pub type GLshort = super::__gl_imports::raw::c_short;
pub type GLint = super::__gl_imports::raw::c_int;
pub type GLclampx = super::__gl_imports::raw::c_int;
pub type GLubyte = super::__gl_imports::raw::c_uchar;
pub type GLushort = super::__gl_imports::raw::c_ushort;
pub type GLuint = super::__gl_imports::raw::c_uint;
pub type GLsizei = super::__gl_imports::raw::c_int;
pub type GLfloat = super::__gl_imports::raw::c_float;
pub type GLclampf = super::__gl_imports::raw::c_float;
pub type GLdouble = super::__gl_imports::raw::c_double;
pub type GLclampd = super::__gl_imports::raw::c_double;
pub type GLeglImageOES = *const super::__gl_imports::raw::c_void;
pub type GLchar = super::__gl_imports::raw::c_char;
pub type GLcharARB = super::__gl_imports::raw::c_char;

#[cfg(target_os = "macos")]
pub type GLhandleARB = *const super::__gl_imports::raw::c_void;
#[cfg(not(target_os = "macos"))]
pub type GLhandleARB = super::__gl_imports::raw::c_uint;

pub type GLhalfARB = super::__gl_imports::raw::c_ushort;
pub type GLhalf = super::__gl_imports::raw::c_ushort;

// Must be 32 bits
pub type GLfixed = GLint;

pub type GLintptr = isize;
pub type GLsizeiptr = isize;
pub type GLint64 = i64;
pub type GLuint64 = u64;
pub type GLintptrARB = isize;
pub type GLsizeiptrARB = isize;
pub type GLint64EXT = i64;
pub type GLuint64EXT = u64;

pub enum __GLsync {}
pub type GLsync = *const __GLsync;

// compatible with OpenCL cl_context
pub enum _cl_context {}
pub enum _cl_event {}

pub type GLDEBUGPROC = extern "system" fn(source: GLenum,
                                          gltype: GLenum,
                                          id: GLuint,
                                          severity: GLenum,
                                          length: GLsizei,
                                          message: *const GLchar,
                                          userParam: *mut super::__gl_imports::raw::c_void);
pub type GLDEBUGPROCARB = extern "system" fn(source: GLenum,
                                             gltype: GLenum,
                                             id: GLuint,
                                             severity: GLenum,
                                             length: GLsizei,
                                             message: *const GLchar,
                                             userParam: *mut super::__gl_imports::raw::c_void);
pub type GLDEBUGPROCKHR = extern "system" fn(source: GLenum,
                                             gltype: GLenum,
                                             id: GLuint,
                                             severity: GLenum,
                                             length: GLsizei,
                                             message: *const GLchar,
                                             userParam: *mut super::__gl_imports::raw::c_void);

// GLES 1 types
// "pub type GLclampx = i32;",

// GLES 1/2 types (tagged for GLES 1)
// "pub type GLbyte = i8;",
// "pub type GLubyte = u8;",
// "pub type GLfloat = GLfloat;",
// "pub type GLclampf = GLfloat;",
// "pub type GLfixed = i32;",
// "pub type GLint64 = i64;",
// "pub type GLuint64 = u64;",
// "pub type GLintptr = intptr_t;",
// "pub type GLsizeiptr = ssize_t;",

// GLES 1/2 types (tagged for GLES 2 - attribute syntax is limited)
// "pub type GLbyte = i8;",
// "pub type GLubyte = u8;",
// "pub type GLfloat = GLfloat;",
// "pub type GLclampf = GLfloat;",
// "pub type GLfixed = i32;",
// "pub type GLint64 = i64;",
// "pub type GLuint64 = u64;",
// "pub type GLint64EXT = i64;",
// "pub type GLuint64EXT = u64;",
// "pub type GLintptr = intptr_t;",
// "pub type GLsizeiptr = ssize_t;",

// GLES 2 types (none currently)

// Vendor extension types
pub type GLDEBUGPROCAMD = extern "system" fn(id: GLuint,
                                             category: GLenum,
                                             severity: GLenum,
                                             length: GLsizei,
                                             message: *const GLchar,
                                             userParam: *mut super::__gl_imports::raw::c_void);
pub type GLhalfNV = super::__gl_imports::raw::c_ushort;
pub type GLvdpauSurfaceNV = GLintptr;

// From WinNT.h

pub type CHAR = super::__gl_imports::raw::c_char;
pub type HANDLE = PVOID;
pub type LONG = super::__gl_imports::raw::c_long;
pub type LPCSTR = *const super::__gl_imports::raw::c_char;
pub type VOID = ();
// #define DECLARE_HANDLE(name) struct name##__{int unused;}; typedef struct name##__ *name
pub type HPBUFFERARB = *const super::__gl_imports::raw::c_void;
pub type HPBUFFEREXT = *const super::__gl_imports::raw::c_void;
pub type HVIDEOOUTPUTDEVICENV = *const super::__gl_imports::raw::c_void;
pub type HPVIDEODEV = *const super::__gl_imports::raw::c_void;
pub type HPGPUNV = *const super::__gl_imports::raw::c_void;
pub type HGPUNV = *const super::__gl_imports::raw::c_void;
pub type HVIDEOINPUTDEVICENV = *const super::__gl_imports::raw::c_void;

// From Windef.h

pub type BOOL = super::__gl_imports::raw::c_int;
pub type BYTE = super::__gl_imports::raw::c_uchar;
pub type COLORREF = DWORD;
pub type FLOAT = super::__gl_imports::raw::c_float;
pub type HDC = HANDLE;
pub type HENHMETAFILE = HANDLE;
pub type HGLRC = *const super::__gl_imports::raw::c_void;
pub type INT = super::__gl_imports::raw::c_int;
pub type PVOID = *const super::__gl_imports::raw::c_void;
pub type LPVOID = *const super::__gl_imports::raw::c_void;
pub enum __PROC_fn {}
pub type PROC = *mut __PROC_fn;

#[repr(C)]
pub struct RECT {
    left: LONG,
    top: LONG,
    right: LONG,
    bottom: LONG,
}

pub type UINT = super::__gl_imports::raw::c_uint;
pub type USHORT = super::__gl_imports::raw::c_ushort;
pub type WORD = super::__gl_imports::raw::c_ushort;

// From BaseTsd.h

pub type INT32 = i32;
pub type INT64 = i64;

// From IntSafe.h

pub type DWORD = super::__gl_imports::raw::c_ulong;

// From Wingdi.h

#[repr(C)]
pub struct POINTFLOAT {
    pub x: FLOAT,
    pub y: FLOAT,
}

#[repr(C)]
pub struct GLYPHMETRICSFLOAT {
    pub gmfBlackBoxX: FLOAT,
    pub gmfBlackBoxY: FLOAT,
    pub gmfptGlyphOrigin: POINTFLOAT,
    pub gmfCellIncX: FLOAT,
    pub gmfCellIncY: FLOAT,
}
pub type LPGLYPHMETRICSFLOAT = *const GLYPHMETRICSFLOAT;

#[repr(C)]
pub struct LAYERPLANEDESCRIPTOR {
    pub nSize: WORD,
    pub nVersion: WORD,
    pub dwFlags: DWORD,
    pub iPixelType: BYTE,
    pub cColorBits: BYTE,
    pub cRedBits: BYTE,
    pub cRedShift: BYTE,
    pub cGreenBits: BYTE,
    pub cGreenShift: BYTE,
    pub cBlueBits: BYTE,
    pub cBlueShift: BYTE,
    pub cAlphaBits: BYTE,
    pub cAlphaShift: BYTE,
    pub cAccumBits: BYTE,
    pub cAccumRedBits: BYTE,
    pub cAccumGreenBits: BYTE,
    pub cAccumBlueBits: BYTE,
    pub cAccumAlphaBits: BYTE,
    pub cDepthBits: BYTE,
    pub cStencilBits: BYTE,
    pub cAuxBuffers: BYTE,
    pub iLayerType: BYTE,
    pub bReserved: BYTE,
    pub crTransparent: COLORREF,
}

#[repr(C)]
pub struct PIXELFORMATDESCRIPTOR {
    pub nSize: WORD,
    pub nVersion: WORD,
    pub dwFlags: DWORD,
    pub iPixelType: BYTE,
    pub cColorBits: BYTE,
    pub cRedBits: BYTE,
    pub cRedShift: BYTE,
    pub cGreenBits: BYTE,
    pub cGreenShift: BYTE,
    pub cBlueBits: BYTE,
    pub cBlueShift: BYTE,
    pub cAlphaBits: BYTE,
    pub cAlphaShift: BYTE,
    pub cAccumBits: BYTE,
    pub cAccumRedBits: BYTE,
    pub cAccumGreenBits: BYTE,
    pub cAccumBlueBits: BYTE,
    pub cAccumAlphaBits: BYTE,
    pub cDepthBits: BYTE,
    pub cStencilBits: BYTE,
    pub cAuxBuffers: BYTE,
    pub iLayerType: BYTE,
    pub bReserved: BYTE,
    pub dwLayerMask: DWORD,
    pub dwVisibleMask: DWORD,
    pub dwDamageMask: DWORD,
}

#[repr(C)]
pub struct _GPU_DEVICE {
    cb: DWORD,
    DeviceName: [CHAR; 32],
    DeviceString: [CHAR; 128],
    Flags: DWORD,
    rcVirtualScreen: RECT,
}

pub struct GPU_DEVICE(_GPU_DEVICE);
pub struct PGPU_DEVICE(*const _GPU_DEVICE);


        }
    
#[allow(dead_code, non_upper_case_globals)] pub const FONT_LINES: types::GLenum = 0;
#[allow(dead_code, non_upper_case_globals)] pub const FONT_POLYGONS: types::GLenum = 1;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_MAIN_PLANE: types::GLenum = 0x00000001;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_OVERLAY1: types::GLenum = 0x00000002;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_OVERLAY10: types::GLenum = 0x00000400;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_OVERLAY11: types::GLenum = 0x00000800;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_OVERLAY12: types::GLenum = 0x00001000;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_OVERLAY13: types::GLenum = 0x00002000;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_OVERLAY14: types::GLenum = 0x00004000;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_OVERLAY15: types::GLenum = 0x00008000;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_OVERLAY2: types::GLenum = 0x00000004;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_OVERLAY3: types::GLenum = 0x00000008;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_OVERLAY4: types::GLenum = 0x00000010;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_OVERLAY5: types::GLenum = 0x00000020;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_OVERLAY6: types::GLenum = 0x00000040;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_OVERLAY7: types::GLenum = 0x00000080;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_OVERLAY8: types::GLenum = 0x00000100;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_OVERLAY9: types::GLenum = 0x00000200;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_UNDERLAY1: types::GLenum = 0x00010000;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_UNDERLAY10: types::GLenum = 0x02000000;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_UNDERLAY11: types::GLenum = 0x04000000;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_UNDERLAY12: types::GLenum = 0x08000000;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_UNDERLAY13: types::GLenum = 0x10000000;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_UNDERLAY14: types::GLenum = 0x20000000;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_UNDERLAY15: types::GLenum = 0x40000000;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_UNDERLAY2: types::GLenum = 0x00020000;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_UNDERLAY3: types::GLenum = 0x00040000;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_UNDERLAY4: types::GLenum = 0x00080000;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_UNDERLAY5: types::GLenum = 0x00100000;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_UNDERLAY6: types::GLenum = 0x00200000;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_UNDERLAY7: types::GLenum = 0x00400000;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_UNDERLAY8: types::GLenum = 0x00800000;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_UNDERLAY9: types::GLenum = 0x01000000;

        #[allow(non_snake_case, unused_variables, dead_code)]
        extern "system" {
#[link_name="wglCopyContext"]
            pub fn CopyContext(hglrcSrc: types::HGLRC, hglrcDst: types::HGLRC, mask: types::UINT) -> types::BOOL;
#[link_name="wglCreateContext"]
            pub fn CreateContext(hDc: types::HDC) -> types::HGLRC;
#[link_name="wglCreateLayerContext"]
            pub fn CreateLayerContext(hDc: types::HDC, level: __gl_imports::raw::c_int) -> types::HGLRC;
#[link_name="wglDeleteContext"]
            pub fn DeleteContext(oldContext: types::HGLRC) -> types::BOOL;
#[link_name="wglDescribeLayerPlane"]
            pub fn DescribeLayerPlane(hDc: types::HDC, pixelFormat: __gl_imports::raw::c_int, layerPlane: __gl_imports::raw::c_int, nBytes: types::UINT, plpd: *const types::LAYERPLANEDESCRIPTOR) -> types::BOOL;
#[link_name="wglGetCurrentContext"]
            pub fn GetCurrentContext() -> types::HGLRC;
#[link_name="wglGetCurrentDC"]
            pub fn GetCurrentDC() -> types::HDC;
#[link_name="wglGetLayerPaletteEntries"]
            pub fn GetLayerPaletteEntries(hdc: types::HDC, iLayerPlane: __gl_imports::raw::c_int, iStart: __gl_imports::raw::c_int, cEntries: __gl_imports::raw::c_int, pcr: *const types::COLORREF) -> __gl_imports::raw::c_int;
#[link_name="wglGetProcAddress"]
            pub fn GetProcAddress(lpszProc: types::LPCSTR) -> types::PROC;
#[link_name="wglMakeCurrent"]
            pub fn MakeCurrent(hDc: types::HDC, newContext: types::HGLRC) -> types::BOOL;
#[link_name="wglRealizeLayerPalette"]
            pub fn RealizeLayerPalette(hdc: types::HDC, iLayerPlane: __gl_imports::raw::c_int, bRealize: types::BOOL) -> types::BOOL;
#[link_name="wglSetLayerPaletteEntries"]
            pub fn SetLayerPaletteEntries(hdc: types::HDC, iLayerPlane: __gl_imports::raw::c_int, iStart: __gl_imports::raw::c_int, cEntries: __gl_imports::raw::c_int, pcr: *const types::COLORREF) -> __gl_imports::raw::c_int;
#[link_name="wglShareLists"]
            pub fn ShareLists(hrcSrvShare: types::HGLRC, hrcSrvSource: types::HGLRC) -> types::BOOL;
#[link_name="wglSwapLayerBuffers"]
            pub fn SwapLayerBuffers(hdc: types::HDC, fuFlags: types::UINT) -> types::BOOL;
#[link_name="wglUseFontBitmaps"]
            pub fn UseFontBitmaps(hDC: types::HDC, first: types::DWORD, count: types::DWORD, listBase: types::DWORD) -> types::BOOL;
#[link_name="wglUseFontBitmapsA"]
            pub fn UseFontBitmapsA(hDC: types::HDC, first: types::DWORD, count: types::DWORD, listBase: types::DWORD) -> types::BOOL;
#[link_name="wglUseFontBitmapsW"]
            pub fn UseFontBitmapsW(hDC: types::HDC, first: types::DWORD, count: types::DWORD, listBase: types::DWORD) -> types::BOOL;
#[link_name="wglUseFontOutlines"]
            pub fn UseFontOutlines(hDC: types::HDC, first: types::DWORD, count: types::DWORD, listBase: types::DWORD, deviation: types::FLOAT, extrusion: types::FLOAT, format: __gl_imports::raw::c_int, lpgmf: types::LPGLYPHMETRICSFLOAT) -> types::BOOL;
#[link_name="wglUseFontOutlinesA"]
            pub fn UseFontOutlinesA(hDC: types::HDC, first: types::DWORD, count: types::DWORD, listBase: types::DWORD, deviation: types::FLOAT, extrusion: types::FLOAT, format: __gl_imports::raw::c_int, lpgmf: types::LPGLYPHMETRICSFLOAT) -> types::BOOL;
#[link_name="wglUseFontOutlinesW"]
            pub fn UseFontOutlinesW(hDC: types::HDC, first: types::DWORD, count: types::DWORD, listBase: types::DWORD, deviation: types::FLOAT, extrusion: types::FLOAT, format: __gl_imports::raw::c_int, lpgmf: types::LPGLYPHMETRICSFLOAT) -> types::BOOL;
}
