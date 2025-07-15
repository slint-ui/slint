
        mod __gl_imports {
            pub use std::mem;
            pub use std::marker::Send;
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

pub type GLDEBUGPROC = Option<extern "system" fn(source: GLenum,
                                                 gltype: GLenum,
                                                 id: GLuint,
                                                 severity: GLenum,
                                                 length: GLsizei,
                                                 message: *const GLchar,
                                                 userParam: *mut super::__gl_imports::raw::c_void)>;
pub type GLDEBUGPROCARB = Option<extern "system" fn(source: GLenum,
                                                    gltype: GLenum,
                                                    id: GLuint,
                                                    severity: GLenum,
                                                    length: GLsizei,
                                                    message: *const GLchar,
                                                    userParam: *mut super::__gl_imports::raw::c_void)>;
pub type GLDEBUGPROCKHR = Option<extern "system" fn(source: GLenum,
                                                    gltype: GLenum,
                                                    id: GLuint,
                                                    severity: GLenum,
                                                    length: GLsizei,
                                                    message: *const GLchar,
                                                    userParam: *mut super::__gl_imports::raw::c_void)>;

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
pub type GLDEBUGPROCAMD = Option<extern "system" fn(id: GLuint,
                                                    category: GLenum,
                                                    severity: GLenum,
                                                    length: GLsizei,
                                                    message: *const GLchar,
                                                    userParam: *mut super::__gl_imports::raw::c_void)>;
pub type GLhalfNV = super::__gl_imports::raw::c_ushort;
pub type GLvdpauSurfaceNV = GLintptr;

}
#[allow(dead_code, non_upper_case_globals)] pub const ACCUM: types::GLenum = 0x0100;
#[allow(dead_code, non_upper_case_globals)] pub const ACCUM_ALPHA_BITS: types::GLenum = 0x0D5B;
#[allow(dead_code, non_upper_case_globals)] pub const ACCUM_BLUE_BITS: types::GLenum = 0x0D5A;
#[allow(dead_code, non_upper_case_globals)] pub const ACCUM_BUFFER_BIT: types::GLenum = 0x00000200;
#[allow(dead_code, non_upper_case_globals)] pub const ACCUM_CLEAR_VALUE: types::GLenum = 0x0B80;
#[allow(dead_code, non_upper_case_globals)] pub const ACCUM_GREEN_BITS: types::GLenum = 0x0D59;
#[allow(dead_code, non_upper_case_globals)] pub const ACCUM_RED_BITS: types::GLenum = 0x0D58;
#[allow(dead_code, non_upper_case_globals)] pub const ACTIVE_ATOMIC_COUNTER_BUFFERS: types::GLenum = 0x92D9;
#[allow(dead_code, non_upper_case_globals)] pub const ACTIVE_ATTRIBUTES: types::GLenum = 0x8B89;
#[allow(dead_code, non_upper_case_globals)] pub const ACTIVE_ATTRIBUTE_MAX_LENGTH: types::GLenum = 0x8B8A;
#[allow(dead_code, non_upper_case_globals)] pub const ACTIVE_PROGRAM: types::GLenum = 0x8259;
#[allow(dead_code, non_upper_case_globals)] pub const ACTIVE_RESOURCES: types::GLenum = 0x92F5;
#[allow(dead_code, non_upper_case_globals)] pub const ACTIVE_TEXTURE: types::GLenum = 0x84E0;
#[allow(dead_code, non_upper_case_globals)] pub const ACTIVE_UNIFORMS: types::GLenum = 0x8B86;
#[allow(dead_code, non_upper_case_globals)] pub const ACTIVE_UNIFORM_BLOCKS: types::GLenum = 0x8A36;
#[allow(dead_code, non_upper_case_globals)] pub const ACTIVE_UNIFORM_BLOCK_MAX_NAME_LENGTH: types::GLenum = 0x8A35;
#[allow(dead_code, non_upper_case_globals)] pub const ACTIVE_UNIFORM_MAX_LENGTH: types::GLenum = 0x8B87;
#[allow(dead_code, non_upper_case_globals)] pub const ACTIVE_VARIABLES: types::GLenum = 0x9305;
#[allow(dead_code, non_upper_case_globals)] pub const ADD: types::GLenum = 0x0104;
#[allow(dead_code, non_upper_case_globals)] pub const ADD_SIGNED: types::GLenum = 0x8574;
#[allow(dead_code, non_upper_case_globals)] pub const ALIASED_LINE_WIDTH_RANGE: types::GLenum = 0x846E;
#[allow(dead_code, non_upper_case_globals)] pub const ALIASED_POINT_SIZE_RANGE: types::GLenum = 0x846D;
#[allow(dead_code, non_upper_case_globals)] pub const ALL_ATTRIB_BITS: types::GLenum = 0xFFFFFFFF;
#[allow(dead_code, non_upper_case_globals)] pub const ALL_BARRIER_BITS: types::GLenum = 0xFFFFFFFF;
#[allow(dead_code, non_upper_case_globals)] pub const ALL_SHADER_BITS: types::GLenum = 0xFFFFFFFF;
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA: types::GLenum = 0x1906;
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA12: types::GLenum = 0x803D;
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA16: types::GLenum = 0x803E;
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA16F_EXT: types::GLenum = 0x881C;
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA32F_EXT: types::GLenum = 0x8816;
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA4: types::GLenum = 0x803B;
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA8: types::GLenum = 0x803C;
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA8_EXT: types::GLenum = 0x803C;
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA_BIAS: types::GLenum = 0x0D1D;
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA_BITS: types::GLenum = 0x0D55;
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA_INTEGER: types::GLenum = 0x8D97;
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA_SCALE: types::GLenum = 0x0D1C;
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA_TEST: types::GLenum = 0x0BC0;
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA_TEST_FUNC: types::GLenum = 0x0BC1;
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA_TEST_REF: types::GLenum = 0x0BC2;
#[allow(dead_code, non_upper_case_globals)] pub const ALREADY_SIGNALED: types::GLenum = 0x911A;
#[allow(dead_code, non_upper_case_globals)] pub const ALWAYS: types::GLenum = 0x0207;
#[allow(dead_code, non_upper_case_globals)] pub const AMBIENT: types::GLenum = 0x1200;
#[allow(dead_code, non_upper_case_globals)] pub const AMBIENT_AND_DIFFUSE: types::GLenum = 0x1602;
#[allow(dead_code, non_upper_case_globals)] pub const AND: types::GLenum = 0x1501;
#[allow(dead_code, non_upper_case_globals)] pub const AND_INVERTED: types::GLenum = 0x1504;
#[allow(dead_code, non_upper_case_globals)] pub const AND_REVERSE: types::GLenum = 0x1502;
#[allow(dead_code, non_upper_case_globals)] pub const ANY_SAMPLES_PASSED: types::GLenum = 0x8C2F;
#[allow(dead_code, non_upper_case_globals)] pub const ANY_SAMPLES_PASSED_CONSERVATIVE: types::GLenum = 0x8D6A;
#[allow(dead_code, non_upper_case_globals)] pub const ARRAY_BUFFER: types::GLenum = 0x8892;
#[allow(dead_code, non_upper_case_globals)] pub const ARRAY_BUFFER_BINDING: types::GLenum = 0x8894;
#[allow(dead_code, non_upper_case_globals)] pub const ARRAY_SIZE: types::GLenum = 0x92FB;
#[allow(dead_code, non_upper_case_globals)] pub const ARRAY_STRIDE: types::GLenum = 0x92FE;
#[allow(dead_code, non_upper_case_globals)] pub const ATOMIC_COUNTER_BARRIER_BIT: types::GLenum = 0x00001000;
#[allow(dead_code, non_upper_case_globals)] pub const ATOMIC_COUNTER_BUFFER: types::GLenum = 0x92C0;
#[allow(dead_code, non_upper_case_globals)] pub const ATOMIC_COUNTER_BUFFER_BINDING: types::GLenum = 0x92C1;
#[allow(dead_code, non_upper_case_globals)] pub const ATOMIC_COUNTER_BUFFER_INDEX: types::GLenum = 0x9301;
#[allow(dead_code, non_upper_case_globals)] pub const ATOMIC_COUNTER_BUFFER_SIZE: types::GLenum = 0x92C3;
#[allow(dead_code, non_upper_case_globals)] pub const ATOMIC_COUNTER_BUFFER_START: types::GLenum = 0x92C2;
#[allow(dead_code, non_upper_case_globals)] pub const ATTACHED_SHADERS: types::GLenum = 0x8B85;
#[allow(dead_code, non_upper_case_globals)] pub const ATTRIB_STACK_DEPTH: types::GLenum = 0x0BB0;
#[allow(dead_code, non_upper_case_globals)] pub const AUTO_NORMAL: types::GLenum = 0x0D80;
#[allow(dead_code, non_upper_case_globals)] pub const AUX0: types::GLenum = 0x0409;
#[allow(dead_code, non_upper_case_globals)] pub const AUX1: types::GLenum = 0x040A;
#[allow(dead_code, non_upper_case_globals)] pub const AUX2: types::GLenum = 0x040B;
#[allow(dead_code, non_upper_case_globals)] pub const AUX3: types::GLenum = 0x040C;
#[allow(dead_code, non_upper_case_globals)] pub const AUX_BUFFERS: types::GLenum = 0x0C00;
#[allow(dead_code, non_upper_case_globals)] pub const BACK: types::GLenum = 0x0405;
#[allow(dead_code, non_upper_case_globals)] pub const BACK_LEFT: types::GLenum = 0x0402;
#[allow(dead_code, non_upper_case_globals)] pub const BACK_RIGHT: types::GLenum = 0x0403;
#[allow(dead_code, non_upper_case_globals)] pub const BGR: types::GLenum = 0x80E0;
#[allow(dead_code, non_upper_case_globals)] pub const BGRA: types::GLenum = 0x80E1;
#[allow(dead_code, non_upper_case_globals)] pub const BGRA8_EXT: types::GLenum = 0x93A1;
#[allow(dead_code, non_upper_case_globals)] pub const BGRA_EXT: types::GLenum = 0x80E1;
#[allow(dead_code, non_upper_case_globals)] pub const BGRA_INTEGER: types::GLenum = 0x8D9B;
#[allow(dead_code, non_upper_case_globals)] pub const BGR_INTEGER: types::GLenum = 0x8D9A;
#[allow(dead_code, non_upper_case_globals)] pub const BITMAP: types::GLenum = 0x1A00;
#[allow(dead_code, non_upper_case_globals)] pub const BITMAP_TOKEN: types::GLenum = 0x0704;
#[allow(dead_code, non_upper_case_globals)] pub const BLEND: types::GLenum = 0x0BE2;
#[allow(dead_code, non_upper_case_globals)] pub const BLEND_ADVANCED_COHERENT_KHR: types::GLenum = 0x9285;
#[allow(dead_code, non_upper_case_globals)] pub const BLEND_COLOR: types::GLenum = 0x8005;
#[allow(dead_code, non_upper_case_globals)] pub const BLEND_DST: types::GLenum = 0x0BE0;
#[allow(dead_code, non_upper_case_globals)] pub const BLEND_DST_ALPHA: types::GLenum = 0x80CA;
#[allow(dead_code, non_upper_case_globals)] pub const BLEND_DST_RGB: types::GLenum = 0x80C8;
#[allow(dead_code, non_upper_case_globals)] pub const BLEND_EQUATION: types::GLenum = 0x8009;
#[allow(dead_code, non_upper_case_globals)] pub const BLEND_EQUATION_ALPHA: types::GLenum = 0x883D;
#[allow(dead_code, non_upper_case_globals)] pub const BLEND_EQUATION_RGB: types::GLenum = 0x8009;
#[allow(dead_code, non_upper_case_globals)] pub const BLEND_SRC: types::GLenum = 0x0BE1;
#[allow(dead_code, non_upper_case_globals)] pub const BLEND_SRC_ALPHA: types::GLenum = 0x80CB;
#[allow(dead_code, non_upper_case_globals)] pub const BLEND_SRC_RGB: types::GLenum = 0x80C9;
#[allow(dead_code, non_upper_case_globals)] pub const BLOCK_INDEX: types::GLenum = 0x92FD;
#[allow(dead_code, non_upper_case_globals)] pub const BLUE: types::GLenum = 0x1905;
#[allow(dead_code, non_upper_case_globals)] pub const BLUE_BIAS: types::GLenum = 0x0D1B;
#[allow(dead_code, non_upper_case_globals)] pub const BLUE_BITS: types::GLenum = 0x0D54;
#[allow(dead_code, non_upper_case_globals)] pub const BLUE_INTEGER: types::GLenum = 0x8D96;
#[allow(dead_code, non_upper_case_globals)] pub const BLUE_SCALE: types::GLenum = 0x0D1A;
#[allow(dead_code, non_upper_case_globals)] pub const BOOL: types::GLenum = 0x8B56;
#[allow(dead_code, non_upper_case_globals)] pub const BOOL_VEC2: types::GLenum = 0x8B57;
#[allow(dead_code, non_upper_case_globals)] pub const BOOL_VEC3: types::GLenum = 0x8B58;
#[allow(dead_code, non_upper_case_globals)] pub const BOOL_VEC4: types::GLenum = 0x8B59;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER: types::GLenum = 0x82E0;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_ACCESS: types::GLenum = 0x88BB;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_ACCESS_FLAGS: types::GLenum = 0x911F;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_BINDING: types::GLenum = 0x9302;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_DATA_SIZE: types::GLenum = 0x9303;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_IMMUTABLE_STORAGE: types::GLenum = 0x821F;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_IMMUTABLE_STORAGE_EXT: types::GLenum = 0x821F;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_KHR: types::GLenum = 0x82E0;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_MAPPED: types::GLenum = 0x88BC;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_MAP_LENGTH: types::GLenum = 0x9120;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_MAP_OFFSET: types::GLenum = 0x9121;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_MAP_POINTER: types::GLenum = 0x88BD;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_SIZE: types::GLenum = 0x8764;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_STORAGE_FLAGS: types::GLenum = 0x8220;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_STORAGE_FLAGS_EXT: types::GLenum = 0x8220;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_UPDATE_BARRIER_BIT: types::GLenum = 0x00000200;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_USAGE: types::GLenum = 0x8765;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_VARIABLE: types::GLenum = 0x92E5;
#[allow(dead_code, non_upper_case_globals)] pub const BYTE: types::GLenum = 0x1400;
#[allow(dead_code, non_upper_case_globals)] pub const C3F_V3F: types::GLenum = 0x2A24;
#[allow(dead_code, non_upper_case_globals)] pub const C4F_N3F_V3F: types::GLenum = 0x2A26;
#[allow(dead_code, non_upper_case_globals)] pub const C4UB_V2F: types::GLenum = 0x2A22;
#[allow(dead_code, non_upper_case_globals)] pub const C4UB_V3F: types::GLenum = 0x2A23;
#[allow(dead_code, non_upper_case_globals)] pub const CCW: types::GLenum = 0x0901;
#[allow(dead_code, non_upper_case_globals)] pub const CLAMP: types::GLenum = 0x2900;
#[allow(dead_code, non_upper_case_globals)] pub const CLAMP_FRAGMENT_COLOR: types::GLenum = 0x891B;
#[allow(dead_code, non_upper_case_globals)] pub const CLAMP_READ_COLOR: types::GLenum = 0x891C;
#[allow(dead_code, non_upper_case_globals)] pub const CLAMP_TO_BORDER: types::GLenum = 0x812D;
#[allow(dead_code, non_upper_case_globals)] pub const CLAMP_TO_EDGE: types::GLenum = 0x812F;
#[allow(dead_code, non_upper_case_globals)] pub const CLAMP_VERTEX_COLOR: types::GLenum = 0x891A;
#[allow(dead_code, non_upper_case_globals)] pub const CLEAR: types::GLenum = 0x1500;
#[allow(dead_code, non_upper_case_globals)] pub const CLIENT_ACTIVE_TEXTURE: types::GLenum = 0x84E1;
#[allow(dead_code, non_upper_case_globals)] pub const CLIENT_ALL_ATTRIB_BITS: types::GLenum = 0xFFFFFFFF;
#[allow(dead_code, non_upper_case_globals)] pub const CLIENT_ATTRIB_STACK_DEPTH: types::GLenum = 0x0BB1;
#[allow(dead_code, non_upper_case_globals)] pub const CLIENT_MAPPED_BUFFER_BARRIER_BIT: types::GLenum = 0x00004000;
#[allow(dead_code, non_upper_case_globals)] pub const CLIENT_MAPPED_BUFFER_BARRIER_BIT_EXT: types::GLenum = 0x00004000;
#[allow(dead_code, non_upper_case_globals)] pub const CLIENT_PIXEL_STORE_BIT: types::GLenum = 0x00000001;
#[allow(dead_code, non_upper_case_globals)] pub const CLIENT_STORAGE_BIT: types::GLenum = 0x0200;
#[allow(dead_code, non_upper_case_globals)] pub const CLIENT_STORAGE_BIT_EXT: types::GLenum = 0x0200;
#[allow(dead_code, non_upper_case_globals)] pub const CLIENT_VERTEX_ARRAY_BIT: types::GLenum = 0x00000002;
#[allow(dead_code, non_upper_case_globals)] pub const CLIP_DISTANCE0: types::GLenum = 0x3000;
#[allow(dead_code, non_upper_case_globals)] pub const CLIP_DISTANCE1: types::GLenum = 0x3001;
#[allow(dead_code, non_upper_case_globals)] pub const CLIP_DISTANCE2: types::GLenum = 0x3002;
#[allow(dead_code, non_upper_case_globals)] pub const CLIP_DISTANCE3: types::GLenum = 0x3003;
#[allow(dead_code, non_upper_case_globals)] pub const CLIP_DISTANCE4: types::GLenum = 0x3004;
#[allow(dead_code, non_upper_case_globals)] pub const CLIP_DISTANCE5: types::GLenum = 0x3005;
#[allow(dead_code, non_upper_case_globals)] pub const CLIP_DISTANCE6: types::GLenum = 0x3006;
#[allow(dead_code, non_upper_case_globals)] pub const CLIP_DISTANCE7: types::GLenum = 0x3007;
#[allow(dead_code, non_upper_case_globals)] pub const CLIP_PLANE0: types::GLenum = 0x3000;
#[allow(dead_code, non_upper_case_globals)] pub const CLIP_PLANE1: types::GLenum = 0x3001;
#[allow(dead_code, non_upper_case_globals)] pub const CLIP_PLANE2: types::GLenum = 0x3002;
#[allow(dead_code, non_upper_case_globals)] pub const CLIP_PLANE3: types::GLenum = 0x3003;
#[allow(dead_code, non_upper_case_globals)] pub const CLIP_PLANE4: types::GLenum = 0x3004;
#[allow(dead_code, non_upper_case_globals)] pub const CLIP_PLANE5: types::GLenum = 0x3005;
#[allow(dead_code, non_upper_case_globals)] pub const COEFF: types::GLenum = 0x0A00;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR: types::GLenum = 0x1800;
#[allow(dead_code, non_upper_case_globals)] pub const COLORBURN_KHR: types::GLenum = 0x929A;
#[allow(dead_code, non_upper_case_globals)] pub const COLORDODGE_KHR: types::GLenum = 0x9299;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ARRAY: types::GLenum = 0x8076;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ARRAY_BUFFER_BINDING: types::GLenum = 0x8898;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ARRAY_POINTER: types::GLenum = 0x8090;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ARRAY_SIZE: types::GLenum = 0x8081;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ARRAY_STRIDE: types::GLenum = 0x8083;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ARRAY_TYPE: types::GLenum = 0x8082;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT0: types::GLenum = 0x8CE0;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT1: types::GLenum = 0x8CE1;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT10: types::GLenum = 0x8CEA;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT11: types::GLenum = 0x8CEB;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT12: types::GLenum = 0x8CEC;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT13: types::GLenum = 0x8CED;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT14: types::GLenum = 0x8CEE;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT15: types::GLenum = 0x8CEF;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT16: types::GLenum = 0x8CF0;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT17: types::GLenum = 0x8CF1;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT18: types::GLenum = 0x8CF2;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT19: types::GLenum = 0x8CF3;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT2: types::GLenum = 0x8CE2;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT20: types::GLenum = 0x8CF4;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT21: types::GLenum = 0x8CF5;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT22: types::GLenum = 0x8CF6;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT23: types::GLenum = 0x8CF7;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT24: types::GLenum = 0x8CF8;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT25: types::GLenum = 0x8CF9;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT26: types::GLenum = 0x8CFA;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT27: types::GLenum = 0x8CFB;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT28: types::GLenum = 0x8CFC;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT29: types::GLenum = 0x8CFD;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT3: types::GLenum = 0x8CE3;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT30: types::GLenum = 0x8CFE;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT31: types::GLenum = 0x8CFF;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT4: types::GLenum = 0x8CE4;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT5: types::GLenum = 0x8CE5;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT6: types::GLenum = 0x8CE6;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT7: types::GLenum = 0x8CE7;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT8: types::GLenum = 0x8CE8;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_ATTACHMENT9: types::GLenum = 0x8CE9;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_BUFFER_BIT: types::GLenum = 0x00004000;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_CLEAR_VALUE: types::GLenum = 0x0C22;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_INDEX: types::GLenum = 0x1900;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_INDEXES: types::GLenum = 0x1603;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_LOGIC_OP: types::GLenum = 0x0BF2;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_MATERIAL: types::GLenum = 0x0B57;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_MATERIAL_FACE: types::GLenum = 0x0B55;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_MATERIAL_PARAMETER: types::GLenum = 0x0B56;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_SUM: types::GLenum = 0x8458;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_WRITEMASK: types::GLenum = 0x0C23;
#[allow(dead_code, non_upper_case_globals)] pub const COMBINE: types::GLenum = 0x8570;
#[allow(dead_code, non_upper_case_globals)] pub const COMBINE_ALPHA: types::GLenum = 0x8572;
#[allow(dead_code, non_upper_case_globals)] pub const COMBINE_RGB: types::GLenum = 0x8571;
#[allow(dead_code, non_upper_case_globals)] pub const COMMAND_BARRIER_BIT: types::GLenum = 0x00000040;
#[allow(dead_code, non_upper_case_globals)] pub const COMPARE_REF_TO_TEXTURE: types::GLenum = 0x884E;
#[allow(dead_code, non_upper_case_globals)] pub const COMPARE_R_TO_TEXTURE: types::GLenum = 0x884E;
#[allow(dead_code, non_upper_case_globals)] pub const COMPILE: types::GLenum = 0x1300;
#[allow(dead_code, non_upper_case_globals)] pub const COMPILE_AND_EXECUTE: types::GLenum = 0x1301;
#[allow(dead_code, non_upper_case_globals)] pub const COMPILE_STATUS: types::GLenum = 0x8B81;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_ALPHA: types::GLenum = 0x84E9;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_INTENSITY: types::GLenum = 0x84EC;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_LUMINANCE: types::GLenum = 0x84EA;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_LUMINANCE_ALPHA: types::GLenum = 0x84EB;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_R11_EAC: types::GLenum = 0x9270;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_RED: types::GLenum = 0x8225;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_RED_RGTC1: types::GLenum = 0x8DBB;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_RG: types::GLenum = 0x8226;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_RG11_EAC: types::GLenum = 0x9272;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_RGB: types::GLenum = 0x84ED;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_RGB8_ETC2: types::GLenum = 0x9274;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_RGB8_PUNCHTHROUGH_ALPHA1_ETC2: types::GLenum = 0x9276;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_RGBA: types::GLenum = 0x84EE;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_RGBA8_ETC2_EAC: types::GLenum = 0x9278;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_RG_RGTC2: types::GLenum = 0x8DBD;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_SIGNED_R11_EAC: types::GLenum = 0x9271;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_SIGNED_RED_RGTC1: types::GLenum = 0x8DBC;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_SIGNED_RG11_EAC: types::GLenum = 0x9273;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_SIGNED_RG_RGTC2: types::GLenum = 0x8DBE;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_SLUMINANCE: types::GLenum = 0x8C4A;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_SLUMINANCE_ALPHA: types::GLenum = 0x8C4B;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_SRGB: types::GLenum = 0x8C48;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_SRGB8_ALPHA8_ETC2_EAC: types::GLenum = 0x9279;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_SRGB8_ETC2: types::GLenum = 0x9275;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_SRGB8_PUNCHTHROUGH_ALPHA1_ETC2: types::GLenum = 0x9277;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_SRGB_ALPHA: types::GLenum = 0x8C49;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_TEXTURE_FORMATS: types::GLenum = 0x86A3;
#[allow(dead_code, non_upper_case_globals)] pub const COMPUTE_SHADER: types::GLenum = 0x91B9;
#[allow(dead_code, non_upper_case_globals)] pub const COMPUTE_SHADER_BIT: types::GLenum = 0x00000020;
#[allow(dead_code, non_upper_case_globals)] pub const COMPUTE_WORK_GROUP_SIZE: types::GLenum = 0x8267;
#[allow(dead_code, non_upper_case_globals)] pub const CONDITION_SATISFIED: types::GLenum = 0x911C;
#[allow(dead_code, non_upper_case_globals)] pub const CONSTANT: types::GLenum = 0x8576;
#[allow(dead_code, non_upper_case_globals)] pub const CONSTANT_ALPHA: types::GLenum = 0x8003;
#[allow(dead_code, non_upper_case_globals)] pub const CONSTANT_ATTENUATION: types::GLenum = 0x1207;
#[allow(dead_code, non_upper_case_globals)] pub const CONSTANT_COLOR: types::GLenum = 0x8001;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_COMPATIBILITY_PROFILE_BIT: types::GLenum = 0x00000002;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_CORE_PROFILE_BIT: types::GLenum = 0x00000001;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_FLAGS: types::GLenum = 0x821E;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_FLAG_DEBUG_BIT: types::GLenum = 0x00000002;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_FLAG_DEBUG_BIT_KHR: types::GLenum = 0x00000002;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_FLAG_FORWARD_COMPATIBLE_BIT: types::GLenum = 0x00000001;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_PROFILE_MASK: types::GLenum = 0x9126;
#[allow(dead_code, non_upper_case_globals)] pub const COORD_REPLACE: types::GLenum = 0x8862;
#[allow(dead_code, non_upper_case_globals)] pub const COPY: types::GLenum = 0x1503;
#[allow(dead_code, non_upper_case_globals)] pub const COPY_INVERTED: types::GLenum = 0x150C;
#[allow(dead_code, non_upper_case_globals)] pub const COPY_PIXEL_TOKEN: types::GLenum = 0x0706;
#[allow(dead_code, non_upper_case_globals)] pub const COPY_READ_BUFFER: types::GLenum = 0x8F36;
#[allow(dead_code, non_upper_case_globals)] pub const COPY_READ_BUFFER_BINDING: types::GLenum = 0x8F36;
#[allow(dead_code, non_upper_case_globals)] pub const COPY_WRITE_BUFFER: types::GLenum = 0x8F37;
#[allow(dead_code, non_upper_case_globals)] pub const COPY_WRITE_BUFFER_BINDING: types::GLenum = 0x8F37;
#[allow(dead_code, non_upper_case_globals)] pub const CULL_FACE: types::GLenum = 0x0B44;
#[allow(dead_code, non_upper_case_globals)] pub const CULL_FACE_MODE: types::GLenum = 0x0B45;
#[allow(dead_code, non_upper_case_globals)] pub const CURRENT_BIT: types::GLenum = 0x00000001;
#[allow(dead_code, non_upper_case_globals)] pub const CURRENT_COLOR: types::GLenum = 0x0B00;
#[allow(dead_code, non_upper_case_globals)] pub const CURRENT_FOG_COORD: types::GLenum = 0x8453;
#[allow(dead_code, non_upper_case_globals)] pub const CURRENT_FOG_COORDINATE: types::GLenum = 0x8453;
#[allow(dead_code, non_upper_case_globals)] pub const CURRENT_INDEX: types::GLenum = 0x0B01;
#[allow(dead_code, non_upper_case_globals)] pub const CURRENT_NORMAL: types::GLenum = 0x0B02;
#[allow(dead_code, non_upper_case_globals)] pub const CURRENT_PROGRAM: types::GLenum = 0x8B8D;
#[allow(dead_code, non_upper_case_globals)] pub const CURRENT_QUERY: types::GLenum = 0x8865;
#[allow(dead_code, non_upper_case_globals)] pub const CURRENT_QUERY_EXT: types::GLenum = 0x8865;
#[allow(dead_code, non_upper_case_globals)] pub const CURRENT_RASTER_COLOR: types::GLenum = 0x0B04;
#[allow(dead_code, non_upper_case_globals)] pub const CURRENT_RASTER_DISTANCE: types::GLenum = 0x0B09;
#[allow(dead_code, non_upper_case_globals)] pub const CURRENT_RASTER_INDEX: types::GLenum = 0x0B05;
#[allow(dead_code, non_upper_case_globals)] pub const CURRENT_RASTER_POSITION: types::GLenum = 0x0B07;
#[allow(dead_code, non_upper_case_globals)] pub const CURRENT_RASTER_POSITION_VALID: types::GLenum = 0x0B08;
#[allow(dead_code, non_upper_case_globals)] pub const CURRENT_RASTER_SECONDARY_COLOR: types::GLenum = 0x845F;
#[allow(dead_code, non_upper_case_globals)] pub const CURRENT_RASTER_TEXTURE_COORDS: types::GLenum = 0x0B06;
#[allow(dead_code, non_upper_case_globals)] pub const CURRENT_SECONDARY_COLOR: types::GLenum = 0x8459;
#[allow(dead_code, non_upper_case_globals)] pub const CURRENT_TEXTURE_COORDS: types::GLenum = 0x0B03;
#[allow(dead_code, non_upper_case_globals)] pub const CURRENT_VERTEX_ATTRIB: types::GLenum = 0x8626;
#[allow(dead_code, non_upper_case_globals)] pub const CW: types::GLenum = 0x0900;
#[allow(dead_code, non_upper_case_globals)] pub const DARKEN_KHR: types::GLenum = 0x9297;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_CALLBACK_FUNCTION: types::GLenum = 0x8244;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_CALLBACK_FUNCTION_KHR: types::GLenum = 0x8244;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_CALLBACK_USER_PARAM: types::GLenum = 0x8245;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_CALLBACK_USER_PARAM_KHR: types::GLenum = 0x8245;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_GROUP_STACK_DEPTH: types::GLenum = 0x826D;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_GROUP_STACK_DEPTH_KHR: types::GLenum = 0x826D;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_LOGGED_MESSAGES: types::GLenum = 0x9145;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_LOGGED_MESSAGES_KHR: types::GLenum = 0x9145;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_NEXT_LOGGED_MESSAGE_LENGTH: types::GLenum = 0x8243;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_NEXT_LOGGED_MESSAGE_LENGTH_KHR: types::GLenum = 0x8243;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_OUTPUT: types::GLenum = 0x92E0;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_OUTPUT_KHR: types::GLenum = 0x92E0;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_OUTPUT_SYNCHRONOUS: types::GLenum = 0x8242;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_OUTPUT_SYNCHRONOUS_KHR: types::GLenum = 0x8242;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_SEVERITY_HIGH: types::GLenum = 0x9146;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_SEVERITY_HIGH_KHR: types::GLenum = 0x9146;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_SEVERITY_LOW: types::GLenum = 0x9148;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_SEVERITY_LOW_KHR: types::GLenum = 0x9148;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_SEVERITY_MEDIUM: types::GLenum = 0x9147;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_SEVERITY_MEDIUM_KHR: types::GLenum = 0x9147;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_SEVERITY_NOTIFICATION: types::GLenum = 0x826B;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_SEVERITY_NOTIFICATION_KHR: types::GLenum = 0x826B;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_SOURCE_API: types::GLenum = 0x8246;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_SOURCE_API_KHR: types::GLenum = 0x8246;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_SOURCE_APPLICATION: types::GLenum = 0x824A;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_SOURCE_APPLICATION_KHR: types::GLenum = 0x824A;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_SOURCE_OTHER: types::GLenum = 0x824B;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_SOURCE_OTHER_KHR: types::GLenum = 0x824B;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_SOURCE_SHADER_COMPILER: types::GLenum = 0x8248;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_SOURCE_SHADER_COMPILER_KHR: types::GLenum = 0x8248;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_SOURCE_THIRD_PARTY: types::GLenum = 0x8249;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_SOURCE_THIRD_PARTY_KHR: types::GLenum = 0x8249;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_SOURCE_WINDOW_SYSTEM: types::GLenum = 0x8247;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_SOURCE_WINDOW_SYSTEM_KHR: types::GLenum = 0x8247;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_TYPE_DEPRECATED_BEHAVIOR: types::GLenum = 0x824D;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_TYPE_DEPRECATED_BEHAVIOR_KHR: types::GLenum = 0x824D;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_TYPE_ERROR: types::GLenum = 0x824C;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_TYPE_ERROR_KHR: types::GLenum = 0x824C;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_TYPE_MARKER: types::GLenum = 0x8268;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_TYPE_MARKER_KHR: types::GLenum = 0x8268;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_TYPE_OTHER: types::GLenum = 0x8251;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_TYPE_OTHER_KHR: types::GLenum = 0x8251;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_TYPE_PERFORMANCE: types::GLenum = 0x8250;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_TYPE_PERFORMANCE_KHR: types::GLenum = 0x8250;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_TYPE_POP_GROUP: types::GLenum = 0x826A;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_TYPE_POP_GROUP_KHR: types::GLenum = 0x826A;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_TYPE_PORTABILITY: types::GLenum = 0x824F;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_TYPE_PORTABILITY_KHR: types::GLenum = 0x824F;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_TYPE_PUSH_GROUP: types::GLenum = 0x8269;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_TYPE_PUSH_GROUP_KHR: types::GLenum = 0x8269;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_TYPE_UNDEFINED_BEHAVIOR: types::GLenum = 0x824E;
#[allow(dead_code, non_upper_case_globals)] pub const DEBUG_TYPE_UNDEFINED_BEHAVIOR_KHR: types::GLenum = 0x824E;
#[allow(dead_code, non_upper_case_globals)] pub const DECAL: types::GLenum = 0x2101;
#[allow(dead_code, non_upper_case_globals)] pub const DECR: types::GLenum = 0x1E03;
#[allow(dead_code, non_upper_case_globals)] pub const DECR_WRAP: types::GLenum = 0x8508;
#[allow(dead_code, non_upper_case_globals)] pub const DELETE_STATUS: types::GLenum = 0x8B80;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH: types::GLenum = 0x1801;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH24_STENCIL8: types::GLenum = 0x88F0;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH32F_STENCIL8: types::GLenum = 0x8CAD;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_ATTACHMENT: types::GLenum = 0x8D00;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_BIAS: types::GLenum = 0x0D1F;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_BITS: types::GLenum = 0x0D56;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_BUFFER_BIT: types::GLenum = 0x00000100;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_CLAMP: types::GLenum = 0x864F;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_CLEAR_VALUE: types::GLenum = 0x0B73;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_COMPONENT: types::GLenum = 0x1902;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_COMPONENT16: types::GLenum = 0x81A5;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_COMPONENT24: types::GLenum = 0x81A6;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_COMPONENT32: types::GLenum = 0x81A7;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_COMPONENT32F: types::GLenum = 0x8CAC;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_FUNC: types::GLenum = 0x0B74;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_RANGE: types::GLenum = 0x0B70;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_SCALE: types::GLenum = 0x0D1E;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_STENCIL: types::GLenum = 0x84F9;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_STENCIL_ATTACHMENT: types::GLenum = 0x821A;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_STENCIL_TEXTURE_MODE: types::GLenum = 0x90EA;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_TEST: types::GLenum = 0x0B71;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_TEXTURE_MODE: types::GLenum = 0x884B;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_WRITEMASK: types::GLenum = 0x0B72;
#[allow(dead_code, non_upper_case_globals)] pub const DIFFERENCE_KHR: types::GLenum = 0x929E;
#[allow(dead_code, non_upper_case_globals)] pub const DIFFUSE: types::GLenum = 0x1201;
#[allow(dead_code, non_upper_case_globals)] pub const DISPATCH_INDIRECT_BUFFER: types::GLenum = 0x90EE;
#[allow(dead_code, non_upper_case_globals)] pub const DISPATCH_INDIRECT_BUFFER_BINDING: types::GLenum = 0x90EF;
#[allow(dead_code, non_upper_case_globals)] pub const DISPLAY_LIST: types::GLenum = 0x82E7;
#[allow(dead_code, non_upper_case_globals)] pub const DITHER: types::GLenum = 0x0BD0;
#[allow(dead_code, non_upper_case_globals)] pub const DOMAIN: types::GLenum = 0x0A02;
#[allow(dead_code, non_upper_case_globals)] pub const DONT_CARE: types::GLenum = 0x1100;
#[allow(dead_code, non_upper_case_globals)] pub const DOT3_RGB: types::GLenum = 0x86AE;
#[allow(dead_code, non_upper_case_globals)] pub const DOT3_RGBA: types::GLenum = 0x86AF;
#[allow(dead_code, non_upper_case_globals)] pub const DOUBLE: types::GLenum = 0x140A;
#[allow(dead_code, non_upper_case_globals)] pub const DOUBLEBUFFER: types::GLenum = 0x0C32;
#[allow(dead_code, non_upper_case_globals)] pub const DRAW_BUFFER: types::GLenum = 0x0C01;
#[allow(dead_code, non_upper_case_globals)] pub const DRAW_BUFFER0: types::GLenum = 0x8825;
#[allow(dead_code, non_upper_case_globals)] pub const DRAW_BUFFER1: types::GLenum = 0x8826;
#[allow(dead_code, non_upper_case_globals)] pub const DRAW_BUFFER10: types::GLenum = 0x882F;
#[allow(dead_code, non_upper_case_globals)] pub const DRAW_BUFFER11: types::GLenum = 0x8830;
#[allow(dead_code, non_upper_case_globals)] pub const DRAW_BUFFER12: types::GLenum = 0x8831;
#[allow(dead_code, non_upper_case_globals)] pub const DRAW_BUFFER13: types::GLenum = 0x8832;
#[allow(dead_code, non_upper_case_globals)] pub const DRAW_BUFFER14: types::GLenum = 0x8833;
#[allow(dead_code, non_upper_case_globals)] pub const DRAW_BUFFER15: types::GLenum = 0x8834;
#[allow(dead_code, non_upper_case_globals)] pub const DRAW_BUFFER2: types::GLenum = 0x8827;
#[allow(dead_code, non_upper_case_globals)] pub const DRAW_BUFFER3: types::GLenum = 0x8828;
#[allow(dead_code, non_upper_case_globals)] pub const DRAW_BUFFER4: types::GLenum = 0x8829;
#[allow(dead_code, non_upper_case_globals)] pub const DRAW_BUFFER5: types::GLenum = 0x882A;
#[allow(dead_code, non_upper_case_globals)] pub const DRAW_BUFFER6: types::GLenum = 0x882B;
#[allow(dead_code, non_upper_case_globals)] pub const DRAW_BUFFER7: types::GLenum = 0x882C;
#[allow(dead_code, non_upper_case_globals)] pub const DRAW_BUFFER8: types::GLenum = 0x882D;
#[allow(dead_code, non_upper_case_globals)] pub const DRAW_BUFFER9: types::GLenum = 0x882E;
#[allow(dead_code, non_upper_case_globals)] pub const DRAW_FRAMEBUFFER: types::GLenum = 0x8CA9;
#[allow(dead_code, non_upper_case_globals)] pub const DRAW_FRAMEBUFFER_BINDING: types::GLenum = 0x8CA6;
#[allow(dead_code, non_upper_case_globals)] pub const DRAW_INDIRECT_BUFFER: types::GLenum = 0x8F3F;
#[allow(dead_code, non_upper_case_globals)] pub const DRAW_INDIRECT_BUFFER_BINDING: types::GLenum = 0x8F43;
#[allow(dead_code, non_upper_case_globals)] pub const DRAW_PIXELS_APPLE: types::GLenum = 0x8A0A;
#[allow(dead_code, non_upper_case_globals)] pub const DRAW_PIXEL_TOKEN: types::GLenum = 0x0705;
#[allow(dead_code, non_upper_case_globals)] pub const DST_ALPHA: types::GLenum = 0x0304;
#[allow(dead_code, non_upper_case_globals)] pub const DST_COLOR: types::GLenum = 0x0306;
#[allow(dead_code, non_upper_case_globals)] pub const DYNAMIC_COPY: types::GLenum = 0x88EA;
#[allow(dead_code, non_upper_case_globals)] pub const DYNAMIC_DRAW: types::GLenum = 0x88E8;
#[allow(dead_code, non_upper_case_globals)] pub const DYNAMIC_READ: types::GLenum = 0x88E9;
#[allow(dead_code, non_upper_case_globals)] pub const DYNAMIC_STORAGE_BIT: types::GLenum = 0x0100;
#[allow(dead_code, non_upper_case_globals)] pub const DYNAMIC_STORAGE_BIT_EXT: types::GLenum = 0x0100;
#[allow(dead_code, non_upper_case_globals)] pub const EDGE_FLAG: types::GLenum = 0x0B43;
#[allow(dead_code, non_upper_case_globals)] pub const EDGE_FLAG_ARRAY: types::GLenum = 0x8079;
#[allow(dead_code, non_upper_case_globals)] pub const EDGE_FLAG_ARRAY_BUFFER_BINDING: types::GLenum = 0x889B;
#[allow(dead_code, non_upper_case_globals)] pub const EDGE_FLAG_ARRAY_POINTER: types::GLenum = 0x8093;
#[allow(dead_code, non_upper_case_globals)] pub const EDGE_FLAG_ARRAY_STRIDE: types::GLenum = 0x808C;
#[allow(dead_code, non_upper_case_globals)] pub const ELEMENT_ARRAY_BARRIER_BIT: types::GLenum = 0x00000002;
#[allow(dead_code, non_upper_case_globals)] pub const ELEMENT_ARRAY_BUFFER: types::GLenum = 0x8893;
#[allow(dead_code, non_upper_case_globals)] pub const ELEMENT_ARRAY_BUFFER_BINDING: types::GLenum = 0x8895;
#[allow(dead_code, non_upper_case_globals)] pub const EMISSION: types::GLenum = 0x1600;
#[allow(dead_code, non_upper_case_globals)] pub const ENABLE_BIT: types::GLenum = 0x00002000;
#[allow(dead_code, non_upper_case_globals)] pub const EQUAL: types::GLenum = 0x0202;
#[allow(dead_code, non_upper_case_globals)] pub const EQUIV: types::GLenum = 0x1509;
#[allow(dead_code, non_upper_case_globals)] pub const EVAL_BIT: types::GLenum = 0x00010000;
#[allow(dead_code, non_upper_case_globals)] pub const EXCLUSION_KHR: types::GLenum = 0x92A0;
#[allow(dead_code, non_upper_case_globals)] pub const EXP: types::GLenum = 0x0800;
#[allow(dead_code, non_upper_case_globals)] pub const EXP2: types::GLenum = 0x0801;
#[allow(dead_code, non_upper_case_globals)] pub const EXTENSIONS: types::GLenum = 0x1F03;
#[allow(dead_code, non_upper_case_globals)] pub const EYE_LINEAR: types::GLenum = 0x2400;
#[allow(dead_code, non_upper_case_globals)] pub const EYE_PLANE: types::GLenum = 0x2502;
#[allow(dead_code, non_upper_case_globals)] pub const FALSE: types::GLboolean = 0;
#[allow(dead_code, non_upper_case_globals)] pub const FASTEST: types::GLenum = 0x1101;
#[allow(dead_code, non_upper_case_globals)] pub const FEEDBACK: types::GLenum = 0x1C01;
#[allow(dead_code, non_upper_case_globals)] pub const FEEDBACK_BUFFER_POINTER: types::GLenum = 0x0DF0;
#[allow(dead_code, non_upper_case_globals)] pub const FEEDBACK_BUFFER_SIZE: types::GLenum = 0x0DF1;
#[allow(dead_code, non_upper_case_globals)] pub const FEEDBACK_BUFFER_TYPE: types::GLenum = 0x0DF2;
#[allow(dead_code, non_upper_case_globals)] pub const FENCE_APPLE: types::GLenum = 0x8A0B;
#[allow(dead_code, non_upper_case_globals)] pub const FILL: types::GLenum = 0x1B02;
#[allow(dead_code, non_upper_case_globals)] pub const FIRST_VERTEX_CONVENTION: types::GLenum = 0x8E4D;
#[allow(dead_code, non_upper_case_globals)] pub const FIXED: types::GLenum = 0x140C;
#[allow(dead_code, non_upper_case_globals)] pub const FIXED_ONLY: types::GLenum = 0x891D;
#[allow(dead_code, non_upper_case_globals)] pub const FLAT: types::GLenum = 0x1D00;
#[allow(dead_code, non_upper_case_globals)] pub const FLOAT: types::GLenum = 0x1406;
#[allow(dead_code, non_upper_case_globals)] pub const FLOAT_32_UNSIGNED_INT_24_8_REV: types::GLenum = 0x8DAD;
#[allow(dead_code, non_upper_case_globals)] pub const FLOAT_MAT2: types::GLenum = 0x8B5A;
#[allow(dead_code, non_upper_case_globals)] pub const FLOAT_MAT2x3: types::GLenum = 0x8B65;
#[allow(dead_code, non_upper_case_globals)] pub const FLOAT_MAT2x4: types::GLenum = 0x8B66;
#[allow(dead_code, non_upper_case_globals)] pub const FLOAT_MAT3: types::GLenum = 0x8B5B;
#[allow(dead_code, non_upper_case_globals)] pub const FLOAT_MAT3x2: types::GLenum = 0x8B67;
#[allow(dead_code, non_upper_case_globals)] pub const FLOAT_MAT3x4: types::GLenum = 0x8B68;
#[allow(dead_code, non_upper_case_globals)] pub const FLOAT_MAT4: types::GLenum = 0x8B5C;
#[allow(dead_code, non_upper_case_globals)] pub const FLOAT_MAT4x2: types::GLenum = 0x8B69;
#[allow(dead_code, non_upper_case_globals)] pub const FLOAT_MAT4x3: types::GLenum = 0x8B6A;
#[allow(dead_code, non_upper_case_globals)] pub const FLOAT_VEC2: types::GLenum = 0x8B50;
#[allow(dead_code, non_upper_case_globals)] pub const FLOAT_VEC3: types::GLenum = 0x8B51;
#[allow(dead_code, non_upper_case_globals)] pub const FLOAT_VEC4: types::GLenum = 0x8B52;
#[allow(dead_code, non_upper_case_globals)] pub const FOG: types::GLenum = 0x0B60;
#[allow(dead_code, non_upper_case_globals)] pub const FOG_BIT: types::GLenum = 0x00000080;
#[allow(dead_code, non_upper_case_globals)] pub const FOG_COLOR: types::GLenum = 0x0B66;
#[allow(dead_code, non_upper_case_globals)] pub const FOG_COORD: types::GLenum = 0x8451;
#[allow(dead_code, non_upper_case_globals)] pub const FOG_COORDINATE: types::GLenum = 0x8451;
#[allow(dead_code, non_upper_case_globals)] pub const FOG_COORDINATE_ARRAY: types::GLenum = 0x8457;
#[allow(dead_code, non_upper_case_globals)] pub const FOG_COORDINATE_ARRAY_BUFFER_BINDING: types::GLenum = 0x889D;
#[allow(dead_code, non_upper_case_globals)] pub const FOG_COORDINATE_ARRAY_POINTER: types::GLenum = 0x8456;
#[allow(dead_code, non_upper_case_globals)] pub const FOG_COORDINATE_ARRAY_STRIDE: types::GLenum = 0x8455;
#[allow(dead_code, non_upper_case_globals)] pub const FOG_COORDINATE_ARRAY_TYPE: types::GLenum = 0x8454;
#[allow(dead_code, non_upper_case_globals)] pub const FOG_COORDINATE_SOURCE: types::GLenum = 0x8450;
#[allow(dead_code, non_upper_case_globals)] pub const FOG_COORD_ARRAY: types::GLenum = 0x8457;
#[allow(dead_code, non_upper_case_globals)] pub const FOG_COORD_ARRAY_BUFFER_BINDING: types::GLenum = 0x889D;
#[allow(dead_code, non_upper_case_globals)] pub const FOG_COORD_ARRAY_POINTER: types::GLenum = 0x8456;
#[allow(dead_code, non_upper_case_globals)] pub const FOG_COORD_ARRAY_STRIDE: types::GLenum = 0x8455;
#[allow(dead_code, non_upper_case_globals)] pub const FOG_COORD_ARRAY_TYPE: types::GLenum = 0x8454;
#[allow(dead_code, non_upper_case_globals)] pub const FOG_COORD_SRC: types::GLenum = 0x8450;
#[allow(dead_code, non_upper_case_globals)] pub const FOG_DENSITY: types::GLenum = 0x0B62;
#[allow(dead_code, non_upper_case_globals)] pub const FOG_END: types::GLenum = 0x0B64;
#[allow(dead_code, non_upper_case_globals)] pub const FOG_HINT: types::GLenum = 0x0C54;
#[allow(dead_code, non_upper_case_globals)] pub const FOG_INDEX: types::GLenum = 0x0B61;
#[allow(dead_code, non_upper_case_globals)] pub const FOG_MODE: types::GLenum = 0x0B65;
#[allow(dead_code, non_upper_case_globals)] pub const FOG_START: types::GLenum = 0x0B63;
#[allow(dead_code, non_upper_case_globals)] pub const FRAGMENT_DEPTH: types::GLenum = 0x8452;
#[allow(dead_code, non_upper_case_globals)] pub const FRAGMENT_SHADER: types::GLenum = 0x8B30;
#[allow(dead_code, non_upper_case_globals)] pub const FRAGMENT_SHADER_BIT: types::GLenum = 0x00000002;
#[allow(dead_code, non_upper_case_globals)] pub const FRAGMENT_SHADER_DERIVATIVE_HINT: types::GLenum = 0x8B8B;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER: types::GLenum = 0x8D40;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_ATTACHMENT_ALPHA_SIZE: types::GLenum = 0x8215;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_ATTACHMENT_ANGLE: types::GLenum = 0x93A3;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_ATTACHMENT_BLUE_SIZE: types::GLenum = 0x8214;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_ATTACHMENT_COLOR_ENCODING: types::GLenum = 0x8210;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_ATTACHMENT_COMPONENT_TYPE: types::GLenum = 0x8211;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_ATTACHMENT_DEPTH_SIZE: types::GLenum = 0x8216;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_ATTACHMENT_GREEN_SIZE: types::GLenum = 0x8213;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_ATTACHMENT_LAYERED: types::GLenum = 0x8DA7;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_ATTACHMENT_OBJECT_NAME: types::GLenum = 0x8CD1;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_ATTACHMENT_OBJECT_TYPE: types::GLenum = 0x8CD0;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_ATTACHMENT_RED_SIZE: types::GLenum = 0x8212;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_ATTACHMENT_STENCIL_SIZE: types::GLenum = 0x8217;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_ATTACHMENT_TEXTURE_CUBE_MAP_FACE: types::GLenum = 0x8CD3;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_ATTACHMENT_TEXTURE_LAYER: types::GLenum = 0x8CD4;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_ATTACHMENT_TEXTURE_LEVEL: types::GLenum = 0x8CD2;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_BARRIER_BIT: types::GLenum = 0x00000400;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_BINDING: types::GLenum = 0x8CA6;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_COMPLETE: types::GLenum = 0x8CD5;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_DEFAULT: types::GLenum = 0x8218;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_DEFAULT_FIXED_SAMPLE_LOCATIONS: types::GLenum = 0x9314;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_DEFAULT_HEIGHT: types::GLenum = 0x9311;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_DEFAULT_SAMPLES: types::GLenum = 0x9313;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_DEFAULT_WIDTH: types::GLenum = 0x9310;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_INCOMPLETE_ATTACHMENT: types::GLenum = 0x8CD6;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_INCOMPLETE_DIMENSIONS: types::GLenum = 0x8CD9;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_INCOMPLETE_DRAW_BUFFER: types::GLenum = 0x8CDB;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_INCOMPLETE_LAYER_TARGETS: types::GLenum = 0x8DA8;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_INCOMPLETE_MISSING_ATTACHMENT: types::GLenum = 0x8CD7;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_INCOMPLETE_MULTISAMPLE: types::GLenum = 0x8D56;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_INCOMPLETE_READ_BUFFER: types::GLenum = 0x8CDC;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_SRGB: types::GLenum = 0x8DB9;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_UNDEFINED: types::GLenum = 0x8219;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_UNSUPPORTED: types::GLenum = 0x8CDD;
#[allow(dead_code, non_upper_case_globals)] pub const FRONT: types::GLenum = 0x0404;
#[allow(dead_code, non_upper_case_globals)] pub const FRONT_AND_BACK: types::GLenum = 0x0408;
#[allow(dead_code, non_upper_case_globals)] pub const FRONT_FACE: types::GLenum = 0x0B46;
#[allow(dead_code, non_upper_case_globals)] pub const FRONT_LEFT: types::GLenum = 0x0400;
#[allow(dead_code, non_upper_case_globals)] pub const FRONT_RIGHT: types::GLenum = 0x0401;
#[allow(dead_code, non_upper_case_globals)] pub const FUNC_ADD: types::GLenum = 0x8006;
#[allow(dead_code, non_upper_case_globals)] pub const FUNC_REVERSE_SUBTRACT: types::GLenum = 0x800B;
#[allow(dead_code, non_upper_case_globals)] pub const FUNC_SUBTRACT: types::GLenum = 0x800A;
#[allow(dead_code, non_upper_case_globals)] pub const GENERATE_MIPMAP: types::GLenum = 0x8191;
#[allow(dead_code, non_upper_case_globals)] pub const GENERATE_MIPMAP_HINT: types::GLenum = 0x8192;
#[allow(dead_code, non_upper_case_globals)] pub const GEOMETRY_INPUT_TYPE: types::GLenum = 0x8917;
#[allow(dead_code, non_upper_case_globals)] pub const GEOMETRY_OUTPUT_TYPE: types::GLenum = 0x8918;
#[allow(dead_code, non_upper_case_globals)] pub const GEOMETRY_SHADER: types::GLenum = 0x8DD9;
#[allow(dead_code, non_upper_case_globals)] pub const GEOMETRY_VERTICES_OUT: types::GLenum = 0x8916;
#[allow(dead_code, non_upper_case_globals)] pub const GEQUAL: types::GLenum = 0x0206;
#[allow(dead_code, non_upper_case_globals)] pub const GPU_DISJOINT_EXT: types::GLenum = 0x8FBB;
#[allow(dead_code, non_upper_case_globals)] pub const GREATER: types::GLenum = 0x0204;
#[allow(dead_code, non_upper_case_globals)] pub const GREEN: types::GLenum = 0x1904;
#[allow(dead_code, non_upper_case_globals)] pub const GREEN_BIAS: types::GLenum = 0x0D19;
#[allow(dead_code, non_upper_case_globals)] pub const GREEN_BITS: types::GLenum = 0x0D53;
#[allow(dead_code, non_upper_case_globals)] pub const GREEN_INTEGER: types::GLenum = 0x8D95;
#[allow(dead_code, non_upper_case_globals)] pub const GREEN_SCALE: types::GLenum = 0x0D18;
#[allow(dead_code, non_upper_case_globals)] pub const HALF_FLOAT: types::GLenum = 0x140B;
#[allow(dead_code, non_upper_case_globals)] pub const HALF_FLOAT_OES: types::GLenum = 0x8D61;
#[allow(dead_code, non_upper_case_globals)] pub const HARDLIGHT_KHR: types::GLenum = 0x929B;
#[allow(dead_code, non_upper_case_globals)] pub const HIGH_FLOAT: types::GLenum = 0x8DF2;
#[allow(dead_code, non_upper_case_globals)] pub const HIGH_INT: types::GLenum = 0x8DF5;
#[allow(dead_code, non_upper_case_globals)] pub const HINT_BIT: types::GLenum = 0x00008000;
#[allow(dead_code, non_upper_case_globals)] pub const HSL_COLOR_KHR: types::GLenum = 0x92AF;
#[allow(dead_code, non_upper_case_globals)] pub const HSL_HUE_KHR: types::GLenum = 0x92AD;
#[allow(dead_code, non_upper_case_globals)] pub const HSL_LUMINOSITY_KHR: types::GLenum = 0x92B0;
#[allow(dead_code, non_upper_case_globals)] pub const HSL_SATURATION_KHR: types::GLenum = 0x92AE;
#[allow(dead_code, non_upper_case_globals)] pub const IMAGE_2D: types::GLenum = 0x904D;
#[allow(dead_code, non_upper_case_globals)] pub const IMAGE_2D_ARRAY: types::GLenum = 0x9053;
#[allow(dead_code, non_upper_case_globals)] pub const IMAGE_3D: types::GLenum = 0x904E;
#[allow(dead_code, non_upper_case_globals)] pub const IMAGE_BINDING_ACCESS: types::GLenum = 0x8F3E;
#[allow(dead_code, non_upper_case_globals)] pub const IMAGE_BINDING_FORMAT: types::GLenum = 0x906E;
#[allow(dead_code, non_upper_case_globals)] pub const IMAGE_BINDING_LAYER: types::GLenum = 0x8F3D;
#[allow(dead_code, non_upper_case_globals)] pub const IMAGE_BINDING_LAYERED: types::GLenum = 0x8F3C;
#[allow(dead_code, non_upper_case_globals)] pub const IMAGE_BINDING_LEVEL: types::GLenum = 0x8F3B;
#[allow(dead_code, non_upper_case_globals)] pub const IMAGE_BINDING_NAME: types::GLenum = 0x8F3A;
#[allow(dead_code, non_upper_case_globals)] pub const IMAGE_CUBE: types::GLenum = 0x9050;
#[allow(dead_code, non_upper_case_globals)] pub const IMAGE_FORMAT_COMPATIBILITY_BY_CLASS: types::GLenum = 0x90C9;
#[allow(dead_code, non_upper_case_globals)] pub const IMAGE_FORMAT_COMPATIBILITY_BY_SIZE: types::GLenum = 0x90C8;
#[allow(dead_code, non_upper_case_globals)] pub const IMAGE_FORMAT_COMPATIBILITY_TYPE: types::GLenum = 0x90C7;
#[allow(dead_code, non_upper_case_globals)] pub const IMPLEMENTATION_COLOR_READ_FORMAT: types::GLenum = 0x8B9B;
#[allow(dead_code, non_upper_case_globals)] pub const IMPLEMENTATION_COLOR_READ_TYPE: types::GLenum = 0x8B9A;
#[allow(dead_code, non_upper_case_globals)] pub const INCR: types::GLenum = 0x1E02;
#[allow(dead_code, non_upper_case_globals)] pub const INCR_WRAP: types::GLenum = 0x8507;
#[allow(dead_code, non_upper_case_globals)] pub const INDEX: types::GLenum = 0x8222;
#[allow(dead_code, non_upper_case_globals)] pub const INDEX_ARRAY: types::GLenum = 0x8077;
#[allow(dead_code, non_upper_case_globals)] pub const INDEX_ARRAY_BUFFER_BINDING: types::GLenum = 0x8899;
#[allow(dead_code, non_upper_case_globals)] pub const INDEX_ARRAY_POINTER: types::GLenum = 0x8091;
#[allow(dead_code, non_upper_case_globals)] pub const INDEX_ARRAY_STRIDE: types::GLenum = 0x8086;
#[allow(dead_code, non_upper_case_globals)] pub const INDEX_ARRAY_TYPE: types::GLenum = 0x8085;
#[allow(dead_code, non_upper_case_globals)] pub const INDEX_BITS: types::GLenum = 0x0D51;
#[allow(dead_code, non_upper_case_globals)] pub const INDEX_CLEAR_VALUE: types::GLenum = 0x0C20;
#[allow(dead_code, non_upper_case_globals)] pub const INDEX_LOGIC_OP: types::GLenum = 0x0BF1;
#[allow(dead_code, non_upper_case_globals)] pub const INDEX_MODE: types::GLenum = 0x0C30;
#[allow(dead_code, non_upper_case_globals)] pub const INDEX_OFFSET: types::GLenum = 0x0D13;
#[allow(dead_code, non_upper_case_globals)] pub const INDEX_SHIFT: types::GLenum = 0x0D12;
#[allow(dead_code, non_upper_case_globals)] pub const INDEX_WRITEMASK: types::GLenum = 0x0C21;
#[allow(dead_code, non_upper_case_globals)] pub const INFO_LOG_LENGTH: types::GLenum = 0x8B84;
#[allow(dead_code, non_upper_case_globals)] pub const INT: types::GLenum = 0x1404;
#[allow(dead_code, non_upper_case_globals)] pub const INTENSITY: types::GLenum = 0x8049;
#[allow(dead_code, non_upper_case_globals)] pub const INTENSITY12: types::GLenum = 0x804C;
#[allow(dead_code, non_upper_case_globals)] pub const INTENSITY16: types::GLenum = 0x804D;
#[allow(dead_code, non_upper_case_globals)] pub const INTENSITY4: types::GLenum = 0x804A;
#[allow(dead_code, non_upper_case_globals)] pub const INTENSITY8: types::GLenum = 0x804B;
#[allow(dead_code, non_upper_case_globals)] pub const INTERLEAVED_ATTRIBS: types::GLenum = 0x8C8C;
#[allow(dead_code, non_upper_case_globals)] pub const INTERPOLATE: types::GLenum = 0x8575;
#[allow(dead_code, non_upper_case_globals)] pub const INT_2_10_10_10_REV: types::GLenum = 0x8D9F;
#[allow(dead_code, non_upper_case_globals)] pub const INT_IMAGE_2D: types::GLenum = 0x9058;
#[allow(dead_code, non_upper_case_globals)] pub const INT_IMAGE_2D_ARRAY: types::GLenum = 0x905E;
#[allow(dead_code, non_upper_case_globals)] pub const INT_IMAGE_3D: types::GLenum = 0x9059;
#[allow(dead_code, non_upper_case_globals)] pub const INT_IMAGE_CUBE: types::GLenum = 0x905B;
#[allow(dead_code, non_upper_case_globals)] pub const INT_SAMPLER_1D: types::GLenum = 0x8DC9;
#[allow(dead_code, non_upper_case_globals)] pub const INT_SAMPLER_1D_ARRAY: types::GLenum = 0x8DCE;
#[allow(dead_code, non_upper_case_globals)] pub const INT_SAMPLER_2D: types::GLenum = 0x8DCA;
#[allow(dead_code, non_upper_case_globals)] pub const INT_SAMPLER_2D_ARRAY: types::GLenum = 0x8DCF;
#[allow(dead_code, non_upper_case_globals)] pub const INT_SAMPLER_2D_MULTISAMPLE: types::GLenum = 0x9109;
#[allow(dead_code, non_upper_case_globals)] pub const INT_SAMPLER_2D_MULTISAMPLE_ARRAY: types::GLenum = 0x910C;
#[allow(dead_code, non_upper_case_globals)] pub const INT_SAMPLER_2D_RECT: types::GLenum = 0x8DCD;
#[allow(dead_code, non_upper_case_globals)] pub const INT_SAMPLER_3D: types::GLenum = 0x8DCB;
#[allow(dead_code, non_upper_case_globals)] pub const INT_SAMPLER_BUFFER: types::GLenum = 0x8DD0;
#[allow(dead_code, non_upper_case_globals)] pub const INT_SAMPLER_CUBE: types::GLenum = 0x8DCC;
#[allow(dead_code, non_upper_case_globals)] pub const INT_VEC2: types::GLenum = 0x8B53;
#[allow(dead_code, non_upper_case_globals)] pub const INT_VEC3: types::GLenum = 0x8B54;
#[allow(dead_code, non_upper_case_globals)] pub const INT_VEC4: types::GLenum = 0x8B55;
#[allow(dead_code, non_upper_case_globals)] pub const INVALID_ENUM: types::GLenum = 0x0500;
#[allow(dead_code, non_upper_case_globals)] pub const INVALID_FRAMEBUFFER_OPERATION: types::GLenum = 0x0506;
#[allow(dead_code, non_upper_case_globals)] pub const INVALID_INDEX: types::GLuint = 0xFFFFFFFF;
#[allow(dead_code, non_upper_case_globals)] pub const INVALID_OPERATION: types::GLenum = 0x0502;
#[allow(dead_code, non_upper_case_globals)] pub const INVALID_VALUE: types::GLenum = 0x0501;
#[allow(dead_code, non_upper_case_globals)] pub const INVERT: types::GLenum = 0x150A;
#[allow(dead_code, non_upper_case_globals)] pub const IS_ROW_MAJOR: types::GLenum = 0x9300;
#[allow(dead_code, non_upper_case_globals)] pub const KEEP: types::GLenum = 0x1E00;
#[allow(dead_code, non_upper_case_globals)] pub const LAST_VERTEX_CONVENTION: types::GLenum = 0x8E4E;
#[allow(dead_code, non_upper_case_globals)] pub const LEFT: types::GLenum = 0x0406;
#[allow(dead_code, non_upper_case_globals)] pub const LEQUAL: types::GLenum = 0x0203;
#[allow(dead_code, non_upper_case_globals)] pub const LESS: types::GLenum = 0x0201;
#[allow(dead_code, non_upper_case_globals)] pub const LIGHT0: types::GLenum = 0x4000;
#[allow(dead_code, non_upper_case_globals)] pub const LIGHT1: types::GLenum = 0x4001;
#[allow(dead_code, non_upper_case_globals)] pub const LIGHT2: types::GLenum = 0x4002;
#[allow(dead_code, non_upper_case_globals)] pub const LIGHT3: types::GLenum = 0x4003;
#[allow(dead_code, non_upper_case_globals)] pub const LIGHT4: types::GLenum = 0x4004;
#[allow(dead_code, non_upper_case_globals)] pub const LIGHT5: types::GLenum = 0x4005;
#[allow(dead_code, non_upper_case_globals)] pub const LIGHT6: types::GLenum = 0x4006;
#[allow(dead_code, non_upper_case_globals)] pub const LIGHT7: types::GLenum = 0x4007;
#[allow(dead_code, non_upper_case_globals)] pub const LIGHTEN_KHR: types::GLenum = 0x9298;
#[allow(dead_code, non_upper_case_globals)] pub const LIGHTING: types::GLenum = 0x0B50;
#[allow(dead_code, non_upper_case_globals)] pub const LIGHTING_BIT: types::GLenum = 0x00000040;
#[allow(dead_code, non_upper_case_globals)] pub const LIGHT_MODEL_AMBIENT: types::GLenum = 0x0B53;
#[allow(dead_code, non_upper_case_globals)] pub const LIGHT_MODEL_COLOR_CONTROL: types::GLenum = 0x81F8;
#[allow(dead_code, non_upper_case_globals)] pub const LIGHT_MODEL_LOCAL_VIEWER: types::GLenum = 0x0B51;
#[allow(dead_code, non_upper_case_globals)] pub const LIGHT_MODEL_TWO_SIDE: types::GLenum = 0x0B52;
#[allow(dead_code, non_upper_case_globals)] pub const LINE: types::GLenum = 0x1B01;
#[allow(dead_code, non_upper_case_globals)] pub const LINEAR: types::GLenum = 0x2601;
#[allow(dead_code, non_upper_case_globals)] pub const LINEAR_ATTENUATION: types::GLenum = 0x1208;
#[allow(dead_code, non_upper_case_globals)] pub const LINEAR_MIPMAP_LINEAR: types::GLenum = 0x2703;
#[allow(dead_code, non_upper_case_globals)] pub const LINEAR_MIPMAP_NEAREST: types::GLenum = 0x2701;
#[allow(dead_code, non_upper_case_globals)] pub const LINES: types::GLenum = 0x0001;
#[allow(dead_code, non_upper_case_globals)] pub const LINES_ADJACENCY: types::GLenum = 0x000A;
#[allow(dead_code, non_upper_case_globals)] pub const LINE_BIT: types::GLenum = 0x00000004;
#[allow(dead_code, non_upper_case_globals)] pub const LINE_LOOP: types::GLenum = 0x0002;
#[allow(dead_code, non_upper_case_globals)] pub const LINE_RESET_TOKEN: types::GLenum = 0x0707;
#[allow(dead_code, non_upper_case_globals)] pub const LINE_SMOOTH: types::GLenum = 0x0B20;
#[allow(dead_code, non_upper_case_globals)] pub const LINE_SMOOTH_HINT: types::GLenum = 0x0C52;
#[allow(dead_code, non_upper_case_globals)] pub const LINE_STIPPLE: types::GLenum = 0x0B24;
#[allow(dead_code, non_upper_case_globals)] pub const LINE_STIPPLE_PATTERN: types::GLenum = 0x0B25;
#[allow(dead_code, non_upper_case_globals)] pub const LINE_STIPPLE_REPEAT: types::GLenum = 0x0B26;
#[allow(dead_code, non_upper_case_globals)] pub const LINE_STRIP: types::GLenum = 0x0003;
#[allow(dead_code, non_upper_case_globals)] pub const LINE_STRIP_ADJACENCY: types::GLenum = 0x000B;
#[allow(dead_code, non_upper_case_globals)] pub const LINE_TOKEN: types::GLenum = 0x0702;
#[allow(dead_code, non_upper_case_globals)] pub const LINE_WIDTH: types::GLenum = 0x0B21;
#[allow(dead_code, non_upper_case_globals)] pub const LINE_WIDTH_GRANULARITY: types::GLenum = 0x0B23;
#[allow(dead_code, non_upper_case_globals)] pub const LINE_WIDTH_RANGE: types::GLenum = 0x0B22;
#[allow(dead_code, non_upper_case_globals)] pub const LINK_STATUS: types::GLenum = 0x8B82;
#[allow(dead_code, non_upper_case_globals)] pub const LIST_BASE: types::GLenum = 0x0B32;
#[allow(dead_code, non_upper_case_globals)] pub const LIST_BIT: types::GLenum = 0x00020000;
#[allow(dead_code, non_upper_case_globals)] pub const LIST_INDEX: types::GLenum = 0x0B33;
#[allow(dead_code, non_upper_case_globals)] pub const LIST_MODE: types::GLenum = 0x0B30;
#[allow(dead_code, non_upper_case_globals)] pub const LOAD: types::GLenum = 0x0101;
#[allow(dead_code, non_upper_case_globals)] pub const LOCATION: types::GLenum = 0x930E;
#[allow(dead_code, non_upper_case_globals)] pub const LOGIC_OP: types::GLenum = 0x0BF1;
#[allow(dead_code, non_upper_case_globals)] pub const LOGIC_OP_MODE: types::GLenum = 0x0BF0;
#[allow(dead_code, non_upper_case_globals)] pub const LOWER_LEFT: types::GLenum = 0x8CA1;
#[allow(dead_code, non_upper_case_globals)] pub const LOW_FLOAT: types::GLenum = 0x8DF0;
#[allow(dead_code, non_upper_case_globals)] pub const LOW_INT: types::GLenum = 0x8DF3;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE: types::GLenum = 0x1909;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE12: types::GLenum = 0x8041;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE12_ALPHA12: types::GLenum = 0x8047;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE12_ALPHA4: types::GLenum = 0x8046;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE16: types::GLenum = 0x8042;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE16F_EXT: types::GLenum = 0x881E;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE16_ALPHA16: types::GLenum = 0x8048;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE32F_EXT: types::GLenum = 0x8818;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE4: types::GLenum = 0x803F;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE4_ALPHA4: types::GLenum = 0x8043;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE6_ALPHA2: types::GLenum = 0x8044;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE8: types::GLenum = 0x8040;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE8_ALPHA8: types::GLenum = 0x8045;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE8_ALPHA8_EXT: types::GLenum = 0x8045;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE8_EXT: types::GLenum = 0x8040;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE_ALPHA: types::GLenum = 0x190A;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE_ALPHA16F_EXT: types::GLenum = 0x881F;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE_ALPHA32F_EXT: types::GLenum = 0x8819;
#[allow(dead_code, non_upper_case_globals)] pub const MAJOR_VERSION: types::GLenum = 0x821B;
#[allow(dead_code, non_upper_case_globals)] pub const MAP1_COLOR_4: types::GLenum = 0x0D90;
#[allow(dead_code, non_upper_case_globals)] pub const MAP1_GRID_DOMAIN: types::GLenum = 0x0DD0;
#[allow(dead_code, non_upper_case_globals)] pub const MAP1_GRID_SEGMENTS: types::GLenum = 0x0DD1;
#[allow(dead_code, non_upper_case_globals)] pub const MAP1_INDEX: types::GLenum = 0x0D91;
#[allow(dead_code, non_upper_case_globals)] pub const MAP1_NORMAL: types::GLenum = 0x0D92;
#[allow(dead_code, non_upper_case_globals)] pub const MAP1_TEXTURE_COORD_1: types::GLenum = 0x0D93;
#[allow(dead_code, non_upper_case_globals)] pub const MAP1_TEXTURE_COORD_2: types::GLenum = 0x0D94;
#[allow(dead_code, non_upper_case_globals)] pub const MAP1_TEXTURE_COORD_3: types::GLenum = 0x0D95;
#[allow(dead_code, non_upper_case_globals)] pub const MAP1_TEXTURE_COORD_4: types::GLenum = 0x0D96;
#[allow(dead_code, non_upper_case_globals)] pub const MAP1_VERTEX_3: types::GLenum = 0x0D97;
#[allow(dead_code, non_upper_case_globals)] pub const MAP1_VERTEX_4: types::GLenum = 0x0D98;
#[allow(dead_code, non_upper_case_globals)] pub const MAP2_COLOR_4: types::GLenum = 0x0DB0;
#[allow(dead_code, non_upper_case_globals)] pub const MAP2_GRID_DOMAIN: types::GLenum = 0x0DD2;
#[allow(dead_code, non_upper_case_globals)] pub const MAP2_GRID_SEGMENTS: types::GLenum = 0x0DD3;
#[allow(dead_code, non_upper_case_globals)] pub const MAP2_INDEX: types::GLenum = 0x0DB1;
#[allow(dead_code, non_upper_case_globals)] pub const MAP2_NORMAL: types::GLenum = 0x0DB2;
#[allow(dead_code, non_upper_case_globals)] pub const MAP2_TEXTURE_COORD_1: types::GLenum = 0x0DB3;
#[allow(dead_code, non_upper_case_globals)] pub const MAP2_TEXTURE_COORD_2: types::GLenum = 0x0DB4;
#[allow(dead_code, non_upper_case_globals)] pub const MAP2_TEXTURE_COORD_3: types::GLenum = 0x0DB5;
#[allow(dead_code, non_upper_case_globals)] pub const MAP2_TEXTURE_COORD_4: types::GLenum = 0x0DB6;
#[allow(dead_code, non_upper_case_globals)] pub const MAP2_VERTEX_3: types::GLenum = 0x0DB7;
#[allow(dead_code, non_upper_case_globals)] pub const MAP2_VERTEX_4: types::GLenum = 0x0DB8;
#[allow(dead_code, non_upper_case_globals)] pub const MAP_COHERENT_BIT: types::GLenum = 0x0080;
#[allow(dead_code, non_upper_case_globals)] pub const MAP_COHERENT_BIT_EXT: types::GLenum = 0x0080;
#[allow(dead_code, non_upper_case_globals)] pub const MAP_COLOR: types::GLenum = 0x0D10;
#[allow(dead_code, non_upper_case_globals)] pub const MAP_FLUSH_EXPLICIT_BIT: types::GLenum = 0x0010;
#[allow(dead_code, non_upper_case_globals)] pub const MAP_INVALIDATE_BUFFER_BIT: types::GLenum = 0x0008;
#[allow(dead_code, non_upper_case_globals)] pub const MAP_INVALIDATE_RANGE_BIT: types::GLenum = 0x0004;
#[allow(dead_code, non_upper_case_globals)] pub const MAP_PERSISTENT_BIT: types::GLenum = 0x0040;
#[allow(dead_code, non_upper_case_globals)] pub const MAP_PERSISTENT_BIT_EXT: types::GLenum = 0x0040;
#[allow(dead_code, non_upper_case_globals)] pub const MAP_READ_BIT: types::GLenum = 0x0001;
#[allow(dead_code, non_upper_case_globals)] pub const MAP_STENCIL: types::GLenum = 0x0D11;
#[allow(dead_code, non_upper_case_globals)] pub const MAP_UNSYNCHRONIZED_BIT: types::GLenum = 0x0020;
#[allow(dead_code, non_upper_case_globals)] pub const MAP_WRITE_BIT: types::GLenum = 0x0002;
#[allow(dead_code, non_upper_case_globals)] pub const MATRIX_MODE: types::GLenum = 0x0BA0;
#[allow(dead_code, non_upper_case_globals)] pub const MATRIX_STRIDE: types::GLenum = 0x92FF;
#[allow(dead_code, non_upper_case_globals)] pub const MAX: types::GLenum = 0x8008;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_3D_TEXTURE_SIZE: types::GLenum = 0x8073;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_ARRAY_TEXTURE_LAYERS: types::GLenum = 0x88FF;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_ATOMIC_COUNTER_BUFFER_BINDINGS: types::GLenum = 0x92DC;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_ATOMIC_COUNTER_BUFFER_SIZE: types::GLenum = 0x92D8;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_ATTRIB_STACK_DEPTH: types::GLenum = 0x0D35;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_CLIENT_ATTRIB_STACK_DEPTH: types::GLenum = 0x0D3B;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_CLIP_DISTANCES: types::GLenum = 0x0D32;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_CLIP_PLANES: types::GLenum = 0x0D32;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COLOR_ATTACHMENTS: types::GLenum = 0x8CDF;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COLOR_TEXTURE_SAMPLES: types::GLenum = 0x910E;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMBINED_ATOMIC_COUNTERS: types::GLenum = 0x92D7;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMBINED_ATOMIC_COUNTER_BUFFERS: types::GLenum = 0x92D1;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMBINED_COMPUTE_UNIFORM_COMPONENTS: types::GLenum = 0x8266;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMBINED_FRAGMENT_UNIFORM_COMPONENTS: types::GLenum = 0x8A33;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMBINED_GEOMETRY_UNIFORM_COMPONENTS: types::GLenum = 0x8A32;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMBINED_IMAGE_UNIFORMS: types::GLenum = 0x90CF;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMBINED_IMAGE_UNITS_AND_FRAGMENT_OUTPUTS: types::GLenum = 0x8F39;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMBINED_SHADER_OUTPUT_RESOURCES: types::GLenum = 0x8F39;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMBINED_SHADER_STORAGE_BLOCKS: types::GLenum = 0x90DC;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMBINED_TEXTURE_IMAGE_UNITS: types::GLenum = 0x8B4D;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMBINED_UNIFORM_BLOCKS: types::GLenum = 0x8A2E;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMBINED_VERTEX_UNIFORM_COMPONENTS: types::GLenum = 0x8A31;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMPUTE_ATOMIC_COUNTERS: types::GLenum = 0x8265;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMPUTE_ATOMIC_COUNTER_BUFFERS: types::GLenum = 0x8264;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMPUTE_IMAGE_UNIFORMS: types::GLenum = 0x91BD;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMPUTE_SHADER_STORAGE_BLOCKS: types::GLenum = 0x90DB;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMPUTE_SHARED_MEMORY_SIZE: types::GLenum = 0x8262;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMPUTE_TEXTURE_IMAGE_UNITS: types::GLenum = 0x91BC;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMPUTE_UNIFORM_BLOCKS: types::GLenum = 0x91BB;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMPUTE_UNIFORM_COMPONENTS: types::GLenum = 0x8263;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMPUTE_WORK_GROUP_COUNT: types::GLenum = 0x91BE;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMPUTE_WORK_GROUP_INVOCATIONS: types::GLenum = 0x90EB;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMPUTE_WORK_GROUP_SIZE: types::GLenum = 0x91BF;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_CUBE_MAP_TEXTURE_SIZE: types::GLenum = 0x851C;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_DEBUG_GROUP_STACK_DEPTH: types::GLenum = 0x826C;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_DEBUG_GROUP_STACK_DEPTH_KHR: types::GLenum = 0x826C;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_DEBUG_LOGGED_MESSAGES: types::GLenum = 0x9144;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_DEBUG_LOGGED_MESSAGES_KHR: types::GLenum = 0x9144;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_DEBUG_MESSAGE_LENGTH: types::GLenum = 0x9143;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_DEBUG_MESSAGE_LENGTH_KHR: types::GLenum = 0x9143;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_DEPTH_TEXTURE_SAMPLES: types::GLenum = 0x910F;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_DRAW_BUFFERS: types::GLenum = 0x8824;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_DUAL_SOURCE_DRAW_BUFFERS: types::GLenum = 0x88FC;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_ELEMENTS_INDICES: types::GLenum = 0x80E9;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_ELEMENTS_VERTICES: types::GLenum = 0x80E8;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_ELEMENT_INDEX: types::GLenum = 0x8D6B;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_EVAL_ORDER: types::GLenum = 0x0D30;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_FRAGMENT_ATOMIC_COUNTERS: types::GLenum = 0x92D6;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_FRAGMENT_ATOMIC_COUNTER_BUFFERS: types::GLenum = 0x92D0;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_FRAGMENT_IMAGE_UNIFORMS: types::GLenum = 0x90CE;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_FRAGMENT_INPUT_COMPONENTS: types::GLenum = 0x9125;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_FRAGMENT_SHADER_STORAGE_BLOCKS: types::GLenum = 0x90DA;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_FRAGMENT_UNIFORM_BLOCKS: types::GLenum = 0x8A2D;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_FRAGMENT_UNIFORM_COMPONENTS: types::GLenum = 0x8B49;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_FRAGMENT_UNIFORM_VECTORS: types::GLenum = 0x8DFD;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_FRAMEBUFFER_HEIGHT: types::GLenum = 0x9316;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_FRAMEBUFFER_SAMPLES: types::GLenum = 0x9318;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_FRAMEBUFFER_WIDTH: types::GLenum = 0x9315;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_GEOMETRY_INPUT_COMPONENTS: types::GLenum = 0x9123;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_GEOMETRY_OUTPUT_COMPONENTS: types::GLenum = 0x9124;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_GEOMETRY_OUTPUT_VERTICES: types::GLenum = 0x8DE0;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_GEOMETRY_SHADER_STORAGE_BLOCKS: types::GLenum = 0x90D7;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_GEOMETRY_TEXTURE_IMAGE_UNITS: types::GLenum = 0x8C29;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_GEOMETRY_TOTAL_OUTPUT_COMPONENTS: types::GLenum = 0x8DE1;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_GEOMETRY_UNIFORM_BLOCKS: types::GLenum = 0x8A2C;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_GEOMETRY_UNIFORM_COMPONENTS: types::GLenum = 0x8DDF;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_IMAGE_UNITS: types::GLenum = 0x8F38;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_INTEGER_SAMPLES: types::GLenum = 0x9110;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_LABEL_LENGTH: types::GLenum = 0x82E8;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_LABEL_LENGTH_KHR: types::GLenum = 0x82E8;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_LIGHTS: types::GLenum = 0x0D31;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_LIST_NESTING: types::GLenum = 0x0B31;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_MODELVIEW_STACK_DEPTH: types::GLenum = 0x0D36;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_NAME_LENGTH: types::GLenum = 0x92F6;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_NAME_STACK_DEPTH: types::GLenum = 0x0D37;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_NUM_ACTIVE_VARIABLES: types::GLenum = 0x92F7;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_PIXEL_MAP_TABLE: types::GLenum = 0x0D34;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_PROGRAM_TEXEL_OFFSET: types::GLenum = 0x8905;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_PROGRAM_TEXTURE_GATHER_OFFSET: types::GLenum = 0x8E5F;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_PROJECTION_STACK_DEPTH: types::GLenum = 0x0D38;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_RECTANGLE_TEXTURE_SIZE: types::GLenum = 0x84F8;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_RECTANGLE_TEXTURE_SIZE_ARB: types::GLenum = 0x84F8;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_RENDERBUFFER_SIZE: types::GLenum = 0x84E8;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_SAMPLES: types::GLenum = 0x8D57;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_SAMPLE_MASK_WORDS: types::GLenum = 0x8E59;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_SERVER_WAIT_TIMEOUT: types::GLenum = 0x9111;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_SHADER_PIXEL_LOCAL_STORAGE_FAST_SIZE_EXT: types::GLenum = 0x8F63;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_SHADER_PIXEL_LOCAL_STORAGE_SIZE_EXT: types::GLenum = 0x8F67;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_SHADER_STORAGE_BLOCK_SIZE: types::GLenum = 0x90DE;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_SHADER_STORAGE_BUFFER_BINDINGS: types::GLenum = 0x90DD;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_TESS_CONTROL_SHADER_STORAGE_BLOCKS: types::GLenum = 0x90D8;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_TESS_EVALUATION_SHADER_STORAGE_BLOCKS: types::GLenum = 0x90D9;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_TEXTURE_BUFFER_SIZE: types::GLenum = 0x8C2B;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_TEXTURE_COORDS: types::GLenum = 0x8871;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_TEXTURE_IMAGE_UNITS: types::GLenum = 0x8872;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_TEXTURE_LOD_BIAS: types::GLenum = 0x84FD;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_TEXTURE_MAX_ANISOTROPY_EXT: types::GLenum = 0x84FF;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_TEXTURE_SIZE: types::GLenum = 0x0D33;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_TEXTURE_STACK_DEPTH: types::GLenum = 0x0D39;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_TEXTURE_UNITS: types::GLenum = 0x84E2;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_TRANSFORM_FEEDBACK_INTERLEAVED_COMPONENTS: types::GLenum = 0x8C8A;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_TRANSFORM_FEEDBACK_SEPARATE_ATTRIBS: types::GLenum = 0x8C8B;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_TRANSFORM_FEEDBACK_SEPARATE_COMPONENTS: types::GLenum = 0x8C80;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_UNIFORM_BLOCK_SIZE: types::GLenum = 0x8A30;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_UNIFORM_BUFFER_BINDINGS: types::GLenum = 0x8A2F;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_UNIFORM_LOCATIONS: types::GLenum = 0x826E;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_VARYING_COMPONENTS: types::GLenum = 0x8B4B;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_VARYING_FLOATS: types::GLenum = 0x8B4B;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_VARYING_VECTORS: types::GLenum = 0x8DFC;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_VERTEX_ATOMIC_COUNTERS: types::GLenum = 0x92D2;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_VERTEX_ATOMIC_COUNTER_BUFFERS: types::GLenum = 0x92CC;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_VERTEX_ATTRIBS: types::GLenum = 0x8869;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_VERTEX_ATTRIB_BINDINGS: types::GLenum = 0x82DA;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_VERTEX_ATTRIB_RELATIVE_OFFSET: types::GLenum = 0x82D9;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_VERTEX_ATTRIB_STRIDE: types::GLenum = 0x82E5;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_VERTEX_IMAGE_UNIFORMS: types::GLenum = 0x90CA;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_VERTEX_OUTPUT_COMPONENTS: types::GLenum = 0x9122;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_VERTEX_SHADER_STORAGE_BLOCKS: types::GLenum = 0x90D6;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_VERTEX_TEXTURE_IMAGE_UNITS: types::GLenum = 0x8B4C;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_VERTEX_UNIFORM_BLOCKS: types::GLenum = 0x8A2B;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_VERTEX_UNIFORM_COMPONENTS: types::GLenum = 0x8B4A;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_VERTEX_UNIFORM_VECTORS: types::GLenum = 0x8DFB;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_VIEWPORT_DIMS: types::GLenum = 0x0D3A;
#[allow(dead_code, non_upper_case_globals)] pub const MEDIUM_FLOAT: types::GLenum = 0x8DF1;
#[allow(dead_code, non_upper_case_globals)] pub const MEDIUM_INT: types::GLenum = 0x8DF4;
#[allow(dead_code, non_upper_case_globals)] pub const MIN: types::GLenum = 0x8007;
#[allow(dead_code, non_upper_case_globals)] pub const MINOR_VERSION: types::GLenum = 0x821C;
#[allow(dead_code, non_upper_case_globals)] pub const MIN_PROGRAM_TEXEL_OFFSET: types::GLenum = 0x8904;
#[allow(dead_code, non_upper_case_globals)] pub const MIN_PROGRAM_TEXTURE_GATHER_OFFSET: types::GLenum = 0x8E5E;
#[allow(dead_code, non_upper_case_globals)] pub const MIRRORED_REPEAT: types::GLenum = 0x8370;
#[allow(dead_code, non_upper_case_globals)] pub const MODELVIEW: types::GLenum = 0x1700;
#[allow(dead_code, non_upper_case_globals)] pub const MODELVIEW_MATRIX: types::GLenum = 0x0BA6;
#[allow(dead_code, non_upper_case_globals)] pub const MODELVIEW_STACK_DEPTH: types::GLenum = 0x0BA3;
#[allow(dead_code, non_upper_case_globals)] pub const MODULATE: types::GLenum = 0x2100;
#[allow(dead_code, non_upper_case_globals)] pub const MULT: types::GLenum = 0x0103;
#[allow(dead_code, non_upper_case_globals)] pub const MULTIPLY_KHR: types::GLenum = 0x9294;
#[allow(dead_code, non_upper_case_globals)] pub const MULTISAMPLE: types::GLenum = 0x809D;
#[allow(dead_code, non_upper_case_globals)] pub const MULTISAMPLE_BIT: types::GLenum = 0x20000000;
#[allow(dead_code, non_upper_case_globals)] pub const N3F_V3F: types::GLenum = 0x2A25;
#[allow(dead_code, non_upper_case_globals)] pub const NAME_LENGTH: types::GLenum = 0x92F9;
#[allow(dead_code, non_upper_case_globals)] pub const NAME_STACK_DEPTH: types::GLenum = 0x0D70;
#[allow(dead_code, non_upper_case_globals)] pub const NAND: types::GLenum = 0x150E;
#[allow(dead_code, non_upper_case_globals)] pub const NEAREST: types::GLenum = 0x2600;
#[allow(dead_code, non_upper_case_globals)] pub const NEAREST_MIPMAP_LINEAR: types::GLenum = 0x2702;
#[allow(dead_code, non_upper_case_globals)] pub const NEAREST_MIPMAP_NEAREST: types::GLenum = 0x2700;
#[allow(dead_code, non_upper_case_globals)] pub const NEVER: types::GLenum = 0x0200;
#[allow(dead_code, non_upper_case_globals)] pub const NICEST: types::GLenum = 0x1102;
#[allow(dead_code, non_upper_case_globals)] pub const NONE: types::GLenum = 0;
#[allow(dead_code, non_upper_case_globals)] pub const NOOP: types::GLenum = 0x1505;
#[allow(dead_code, non_upper_case_globals)] pub const NOR: types::GLenum = 0x1508;
#[allow(dead_code, non_upper_case_globals)] pub const NORMALIZE: types::GLenum = 0x0BA1;
#[allow(dead_code, non_upper_case_globals)] pub const NORMAL_ARRAY: types::GLenum = 0x8075;
#[allow(dead_code, non_upper_case_globals)] pub const NORMAL_ARRAY_BUFFER_BINDING: types::GLenum = 0x8897;
#[allow(dead_code, non_upper_case_globals)] pub const NORMAL_ARRAY_POINTER: types::GLenum = 0x808F;
#[allow(dead_code, non_upper_case_globals)] pub const NORMAL_ARRAY_STRIDE: types::GLenum = 0x807F;
#[allow(dead_code, non_upper_case_globals)] pub const NORMAL_ARRAY_TYPE: types::GLenum = 0x807E;
#[allow(dead_code, non_upper_case_globals)] pub const NORMAL_MAP: types::GLenum = 0x8511;
#[allow(dead_code, non_upper_case_globals)] pub const NOTEQUAL: types::GLenum = 0x0205;
#[allow(dead_code, non_upper_case_globals)] pub const NO_ERROR: types::GLenum = 0;
#[allow(dead_code, non_upper_case_globals)] pub const NUM_ACTIVE_VARIABLES: types::GLenum = 0x9304;
#[allow(dead_code, non_upper_case_globals)] pub const NUM_COMPRESSED_TEXTURE_FORMATS: types::GLenum = 0x86A2;
#[allow(dead_code, non_upper_case_globals)] pub const NUM_EXTENSIONS: types::GLenum = 0x821D;
#[allow(dead_code, non_upper_case_globals)] pub const NUM_PROGRAM_BINARY_FORMATS: types::GLenum = 0x87FE;
#[allow(dead_code, non_upper_case_globals)] pub const NUM_SAMPLE_COUNTS: types::GLenum = 0x9380;
#[allow(dead_code, non_upper_case_globals)] pub const NUM_SHADER_BINARY_FORMATS: types::GLenum = 0x8DF9;
#[allow(dead_code, non_upper_case_globals)] pub const OBJECT_LINEAR: types::GLenum = 0x2401;
#[allow(dead_code, non_upper_case_globals)] pub const OBJECT_PLANE: types::GLenum = 0x2501;
#[allow(dead_code, non_upper_case_globals)] pub const OBJECT_TYPE: types::GLenum = 0x9112;
#[allow(dead_code, non_upper_case_globals)] pub const OFFSET: types::GLenum = 0x92FC;
#[allow(dead_code, non_upper_case_globals)] pub const ONE: types::GLenum = 1;
#[allow(dead_code, non_upper_case_globals)] pub const ONE_MINUS_CONSTANT_ALPHA: types::GLenum = 0x8004;
#[allow(dead_code, non_upper_case_globals)] pub const ONE_MINUS_CONSTANT_COLOR: types::GLenum = 0x8002;
#[allow(dead_code, non_upper_case_globals)] pub const ONE_MINUS_DST_ALPHA: types::GLenum = 0x0305;
#[allow(dead_code, non_upper_case_globals)] pub const ONE_MINUS_DST_COLOR: types::GLenum = 0x0307;
#[allow(dead_code, non_upper_case_globals)] pub const ONE_MINUS_SRC1_ALPHA: types::GLenum = 0x88FB;
#[allow(dead_code, non_upper_case_globals)] pub const ONE_MINUS_SRC1_COLOR: types::GLenum = 0x88FA;
#[allow(dead_code, non_upper_case_globals)] pub const ONE_MINUS_SRC_ALPHA: types::GLenum = 0x0303;
#[allow(dead_code, non_upper_case_globals)] pub const ONE_MINUS_SRC_COLOR: types::GLenum = 0x0301;
#[allow(dead_code, non_upper_case_globals)] pub const OPERAND0_ALPHA: types::GLenum = 0x8598;
#[allow(dead_code, non_upper_case_globals)] pub const OPERAND0_RGB: types::GLenum = 0x8590;
#[allow(dead_code, non_upper_case_globals)] pub const OPERAND1_ALPHA: types::GLenum = 0x8599;
#[allow(dead_code, non_upper_case_globals)] pub const OPERAND1_RGB: types::GLenum = 0x8591;
#[allow(dead_code, non_upper_case_globals)] pub const OPERAND2_ALPHA: types::GLenum = 0x859A;
#[allow(dead_code, non_upper_case_globals)] pub const OPERAND2_RGB: types::GLenum = 0x8592;
#[allow(dead_code, non_upper_case_globals)] pub const OR: types::GLenum = 0x1507;
#[allow(dead_code, non_upper_case_globals)] pub const ORDER: types::GLenum = 0x0A01;
#[allow(dead_code, non_upper_case_globals)] pub const OR_INVERTED: types::GLenum = 0x150D;
#[allow(dead_code, non_upper_case_globals)] pub const OR_REVERSE: types::GLenum = 0x150B;
#[allow(dead_code, non_upper_case_globals)] pub const OUT_OF_MEMORY: types::GLenum = 0x0505;
#[allow(dead_code, non_upper_case_globals)] pub const OVERLAY_KHR: types::GLenum = 0x9296;
#[allow(dead_code, non_upper_case_globals)] pub const PACK_ALIGNMENT: types::GLenum = 0x0D05;
#[allow(dead_code, non_upper_case_globals)] pub const PACK_IMAGE_HEIGHT: types::GLenum = 0x806C;
#[allow(dead_code, non_upper_case_globals)] pub const PACK_LSB_FIRST: types::GLenum = 0x0D01;
#[allow(dead_code, non_upper_case_globals)] pub const PACK_ROW_LENGTH: types::GLenum = 0x0D02;
#[allow(dead_code, non_upper_case_globals)] pub const PACK_SKIP_IMAGES: types::GLenum = 0x806B;
#[allow(dead_code, non_upper_case_globals)] pub const PACK_SKIP_PIXELS: types::GLenum = 0x0D04;
#[allow(dead_code, non_upper_case_globals)] pub const PACK_SKIP_ROWS: types::GLenum = 0x0D03;
#[allow(dead_code, non_upper_case_globals)] pub const PACK_SWAP_BYTES: types::GLenum = 0x0D00;
#[allow(dead_code, non_upper_case_globals)] pub const PASS_THROUGH_TOKEN: types::GLenum = 0x0700;
#[allow(dead_code, non_upper_case_globals)] pub const PERSPECTIVE_CORRECTION_HINT: types::GLenum = 0x0C50;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_BUFFER_BARRIER_BIT: types::GLenum = 0x00000080;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_MAP_A_TO_A: types::GLenum = 0x0C79;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_MAP_A_TO_A_SIZE: types::GLenum = 0x0CB9;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_MAP_B_TO_B: types::GLenum = 0x0C78;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_MAP_B_TO_B_SIZE: types::GLenum = 0x0CB8;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_MAP_G_TO_G: types::GLenum = 0x0C77;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_MAP_G_TO_G_SIZE: types::GLenum = 0x0CB7;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_MAP_I_TO_A: types::GLenum = 0x0C75;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_MAP_I_TO_A_SIZE: types::GLenum = 0x0CB5;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_MAP_I_TO_B: types::GLenum = 0x0C74;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_MAP_I_TO_B_SIZE: types::GLenum = 0x0CB4;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_MAP_I_TO_G: types::GLenum = 0x0C73;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_MAP_I_TO_G_SIZE: types::GLenum = 0x0CB3;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_MAP_I_TO_I: types::GLenum = 0x0C70;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_MAP_I_TO_I_SIZE: types::GLenum = 0x0CB0;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_MAP_I_TO_R: types::GLenum = 0x0C72;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_MAP_I_TO_R_SIZE: types::GLenum = 0x0CB2;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_MAP_R_TO_R: types::GLenum = 0x0C76;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_MAP_R_TO_R_SIZE: types::GLenum = 0x0CB6;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_MAP_S_TO_S: types::GLenum = 0x0C71;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_MAP_S_TO_S_SIZE: types::GLenum = 0x0CB1;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_MODE_BIT: types::GLenum = 0x00000020;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_PACK_BUFFER: types::GLenum = 0x88EB;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_PACK_BUFFER_BINDING: types::GLenum = 0x88ED;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_UNPACK_BUFFER: types::GLenum = 0x88EC;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_UNPACK_BUFFER_BINDING: types::GLenum = 0x88EF;
#[allow(dead_code, non_upper_case_globals)] pub const POINT: types::GLenum = 0x1B00;
#[allow(dead_code, non_upper_case_globals)] pub const POINTS: types::GLenum = 0x0000;
#[allow(dead_code, non_upper_case_globals)] pub const POINT_BIT: types::GLenum = 0x00000002;
#[allow(dead_code, non_upper_case_globals)] pub const POINT_DISTANCE_ATTENUATION: types::GLenum = 0x8129;
#[allow(dead_code, non_upper_case_globals)] pub const POINT_FADE_THRESHOLD_SIZE: types::GLenum = 0x8128;
#[allow(dead_code, non_upper_case_globals)] pub const POINT_SIZE: types::GLenum = 0x0B11;
#[allow(dead_code, non_upper_case_globals)] pub const POINT_SIZE_GRANULARITY: types::GLenum = 0x0B13;
#[allow(dead_code, non_upper_case_globals)] pub const POINT_SIZE_MAX: types::GLenum = 0x8127;
#[allow(dead_code, non_upper_case_globals)] pub const POINT_SIZE_MIN: types::GLenum = 0x8126;
#[allow(dead_code, non_upper_case_globals)] pub const POINT_SIZE_RANGE: types::GLenum = 0x0B12;
#[allow(dead_code, non_upper_case_globals)] pub const POINT_SMOOTH: types::GLenum = 0x0B10;
#[allow(dead_code, non_upper_case_globals)] pub const POINT_SMOOTH_HINT: types::GLenum = 0x0C51;
#[allow(dead_code, non_upper_case_globals)] pub const POINT_SPRITE: types::GLenum = 0x8861;
#[allow(dead_code, non_upper_case_globals)] pub const POINT_SPRITE_COORD_ORIGIN: types::GLenum = 0x8CA0;
#[allow(dead_code, non_upper_case_globals)] pub const POINT_TOKEN: types::GLenum = 0x0701;
#[allow(dead_code, non_upper_case_globals)] pub const POLYGON: types::GLenum = 0x0009;
#[allow(dead_code, non_upper_case_globals)] pub const POLYGON_BIT: types::GLenum = 0x00000008;
#[allow(dead_code, non_upper_case_globals)] pub const POLYGON_MODE: types::GLenum = 0x0B40;
#[allow(dead_code, non_upper_case_globals)] pub const POLYGON_OFFSET_FACTOR: types::GLenum = 0x8038;
#[allow(dead_code, non_upper_case_globals)] pub const POLYGON_OFFSET_FILL: types::GLenum = 0x8037;
#[allow(dead_code, non_upper_case_globals)] pub const POLYGON_OFFSET_LINE: types::GLenum = 0x2A02;
#[allow(dead_code, non_upper_case_globals)] pub const POLYGON_OFFSET_POINT: types::GLenum = 0x2A01;
#[allow(dead_code, non_upper_case_globals)] pub const POLYGON_OFFSET_UNITS: types::GLenum = 0x2A00;
#[allow(dead_code, non_upper_case_globals)] pub const POLYGON_SMOOTH: types::GLenum = 0x0B41;
#[allow(dead_code, non_upper_case_globals)] pub const POLYGON_SMOOTH_HINT: types::GLenum = 0x0C53;
#[allow(dead_code, non_upper_case_globals)] pub const POLYGON_STIPPLE: types::GLenum = 0x0B42;
#[allow(dead_code, non_upper_case_globals)] pub const POLYGON_STIPPLE_BIT: types::GLenum = 0x00000010;
#[allow(dead_code, non_upper_case_globals)] pub const POLYGON_TOKEN: types::GLenum = 0x0703;
#[allow(dead_code, non_upper_case_globals)] pub const POSITION: types::GLenum = 0x1203;
#[allow(dead_code, non_upper_case_globals)] pub const PREVIOUS: types::GLenum = 0x8578;
#[allow(dead_code, non_upper_case_globals)] pub const PRIMARY_COLOR: types::GLenum = 0x8577;
#[allow(dead_code, non_upper_case_globals)] pub const PRIMITIVES_GENERATED: types::GLenum = 0x8C87;
#[allow(dead_code, non_upper_case_globals)] pub const PRIMITIVE_RESTART: types::GLenum = 0x8F9D;
#[allow(dead_code, non_upper_case_globals)] pub const PRIMITIVE_RESTART_FIXED_INDEX: types::GLenum = 0x8D69;
#[allow(dead_code, non_upper_case_globals)] pub const PRIMITIVE_RESTART_INDEX: types::GLenum = 0x8F9E;
#[allow(dead_code, non_upper_case_globals)] pub const PROGRAM: types::GLenum = 0x82E2;
#[allow(dead_code, non_upper_case_globals)] pub const PROGRAM_BINARY_FORMATS: types::GLenum = 0x87FF;
#[allow(dead_code, non_upper_case_globals)] pub const PROGRAM_BINARY_LENGTH: types::GLenum = 0x8741;
#[allow(dead_code, non_upper_case_globals)] pub const PROGRAM_BINARY_RETRIEVABLE_HINT: types::GLenum = 0x8257;
#[allow(dead_code, non_upper_case_globals)] pub const PROGRAM_INPUT: types::GLenum = 0x92E3;
#[allow(dead_code, non_upper_case_globals)] pub const PROGRAM_KHR: types::GLenum = 0x82E2;
#[allow(dead_code, non_upper_case_globals)] pub const PROGRAM_OUTPUT: types::GLenum = 0x92E4;
#[allow(dead_code, non_upper_case_globals)] pub const PROGRAM_PIPELINE: types::GLenum = 0x82E4;
#[allow(dead_code, non_upper_case_globals)] pub const PROGRAM_PIPELINE_BINDING: types::GLenum = 0x825A;
#[allow(dead_code, non_upper_case_globals)] pub const PROGRAM_PIPELINE_KHR: types::GLenum = 0x82E4;
#[allow(dead_code, non_upper_case_globals)] pub const PROGRAM_POINT_SIZE: types::GLenum = 0x8642;
#[allow(dead_code, non_upper_case_globals)] pub const PROGRAM_SEPARABLE: types::GLenum = 0x8258;
#[allow(dead_code, non_upper_case_globals)] pub const PROJECTION: types::GLenum = 0x1701;
#[allow(dead_code, non_upper_case_globals)] pub const PROJECTION_MATRIX: types::GLenum = 0x0BA7;
#[allow(dead_code, non_upper_case_globals)] pub const PROJECTION_STACK_DEPTH: types::GLenum = 0x0BA4;
#[allow(dead_code, non_upper_case_globals)] pub const PROVOKING_VERTEX: types::GLenum = 0x8E4F;
#[allow(dead_code, non_upper_case_globals)] pub const PROXY_TEXTURE_1D: types::GLenum = 0x8063;
#[allow(dead_code, non_upper_case_globals)] pub const PROXY_TEXTURE_1D_ARRAY: types::GLenum = 0x8C19;
#[allow(dead_code, non_upper_case_globals)] pub const PROXY_TEXTURE_2D: types::GLenum = 0x8064;
#[allow(dead_code, non_upper_case_globals)] pub const PROXY_TEXTURE_2D_ARRAY: types::GLenum = 0x8C1B;
#[allow(dead_code, non_upper_case_globals)] pub const PROXY_TEXTURE_2D_MULTISAMPLE: types::GLenum = 0x9101;
#[allow(dead_code, non_upper_case_globals)] pub const PROXY_TEXTURE_2D_MULTISAMPLE_ARRAY: types::GLenum = 0x9103;
#[allow(dead_code, non_upper_case_globals)] pub const PROXY_TEXTURE_3D: types::GLenum = 0x8070;
#[allow(dead_code, non_upper_case_globals)] pub const PROXY_TEXTURE_CUBE_MAP: types::GLenum = 0x851B;
#[allow(dead_code, non_upper_case_globals)] pub const PROXY_TEXTURE_RECTANGLE: types::GLenum = 0x84F7;
#[allow(dead_code, non_upper_case_globals)] pub const PROXY_TEXTURE_RECTANGLE_ARB: types::GLenum = 0x84F7;
#[allow(dead_code, non_upper_case_globals)] pub const Q: types::GLenum = 0x2003;
#[allow(dead_code, non_upper_case_globals)] pub const QUADRATIC_ATTENUATION: types::GLenum = 0x1209;
#[allow(dead_code, non_upper_case_globals)] pub const QUADS: types::GLenum = 0x0007;
#[allow(dead_code, non_upper_case_globals)] pub const QUADS_FOLLOW_PROVOKING_VERTEX_CONVENTION: types::GLenum = 0x8E4C;
#[allow(dead_code, non_upper_case_globals)] pub const QUAD_STRIP: types::GLenum = 0x0008;
#[allow(dead_code, non_upper_case_globals)] pub const QUERY: types::GLenum = 0x82E3;
#[allow(dead_code, non_upper_case_globals)] pub const QUERY_BY_REGION_NO_WAIT: types::GLenum = 0x8E16;
#[allow(dead_code, non_upper_case_globals)] pub const QUERY_BY_REGION_WAIT: types::GLenum = 0x8E15;
#[allow(dead_code, non_upper_case_globals)] pub const QUERY_COUNTER_BITS: types::GLenum = 0x8864;
#[allow(dead_code, non_upper_case_globals)] pub const QUERY_COUNTER_BITS_EXT: types::GLenum = 0x8864;
#[allow(dead_code, non_upper_case_globals)] pub const QUERY_KHR: types::GLenum = 0x82E3;
#[allow(dead_code, non_upper_case_globals)] pub const QUERY_NO_WAIT: types::GLenum = 0x8E14;
#[allow(dead_code, non_upper_case_globals)] pub const QUERY_RESULT: types::GLenum = 0x8866;
#[allow(dead_code, non_upper_case_globals)] pub const QUERY_RESULT_AVAILABLE: types::GLenum = 0x8867;
#[allow(dead_code, non_upper_case_globals)] pub const QUERY_RESULT_AVAILABLE_EXT: types::GLenum = 0x8867;
#[allow(dead_code, non_upper_case_globals)] pub const QUERY_RESULT_EXT: types::GLenum = 0x8866;
#[allow(dead_code, non_upper_case_globals)] pub const QUERY_WAIT: types::GLenum = 0x8E13;
#[allow(dead_code, non_upper_case_globals)] pub const R: types::GLenum = 0x2002;
#[allow(dead_code, non_upper_case_globals)] pub const R11F_G11F_B10F: types::GLenum = 0x8C3A;
#[allow(dead_code, non_upper_case_globals)] pub const R16: types::GLenum = 0x822A;
#[allow(dead_code, non_upper_case_globals)] pub const R16F: types::GLenum = 0x822D;
#[allow(dead_code, non_upper_case_globals)] pub const R16F_EXT: types::GLenum = 0x822D;
#[allow(dead_code, non_upper_case_globals)] pub const R16I: types::GLenum = 0x8233;
#[allow(dead_code, non_upper_case_globals)] pub const R16UI: types::GLenum = 0x8234;
#[allow(dead_code, non_upper_case_globals)] pub const R16_SNORM: types::GLenum = 0x8F98;
#[allow(dead_code, non_upper_case_globals)] pub const R32F: types::GLenum = 0x822E;
#[allow(dead_code, non_upper_case_globals)] pub const R32F_EXT: types::GLenum = 0x822E;
#[allow(dead_code, non_upper_case_globals)] pub const R32I: types::GLenum = 0x8235;
#[allow(dead_code, non_upper_case_globals)] pub const R32UI: types::GLenum = 0x8236;
#[allow(dead_code, non_upper_case_globals)] pub const R3_G3_B2: types::GLenum = 0x2A10;
#[allow(dead_code, non_upper_case_globals)] pub const R8: types::GLenum = 0x8229;
#[allow(dead_code, non_upper_case_globals)] pub const R8I: types::GLenum = 0x8231;
#[allow(dead_code, non_upper_case_globals)] pub const R8UI: types::GLenum = 0x8232;
#[allow(dead_code, non_upper_case_globals)] pub const R8_EXT: types::GLenum = 0x8229;
#[allow(dead_code, non_upper_case_globals)] pub const R8_SNORM: types::GLenum = 0x8F94;
#[allow(dead_code, non_upper_case_globals)] pub const RASTERIZER_DISCARD: types::GLenum = 0x8C89;
#[allow(dead_code, non_upper_case_globals)] pub const READ_BUFFER: types::GLenum = 0x0C02;
#[allow(dead_code, non_upper_case_globals)] pub const READ_FRAMEBUFFER: types::GLenum = 0x8CA8;
#[allow(dead_code, non_upper_case_globals)] pub const READ_FRAMEBUFFER_BINDING: types::GLenum = 0x8CAA;
#[allow(dead_code, non_upper_case_globals)] pub const READ_ONLY: types::GLenum = 0x88B8;
#[allow(dead_code, non_upper_case_globals)] pub const READ_WRITE: types::GLenum = 0x88BA;
#[allow(dead_code, non_upper_case_globals)] pub const RED: types::GLenum = 0x1903;
#[allow(dead_code, non_upper_case_globals)] pub const RED_BIAS: types::GLenum = 0x0D15;
#[allow(dead_code, non_upper_case_globals)] pub const RED_BITS: types::GLenum = 0x0D52;
#[allow(dead_code, non_upper_case_globals)] pub const RED_INTEGER: types::GLenum = 0x8D94;
#[allow(dead_code, non_upper_case_globals)] pub const RED_SCALE: types::GLenum = 0x0D14;
#[allow(dead_code, non_upper_case_globals)] pub const REFERENCED_BY_COMPUTE_SHADER: types::GLenum = 0x930B;
#[allow(dead_code, non_upper_case_globals)] pub const REFERENCED_BY_FRAGMENT_SHADER: types::GLenum = 0x930A;
#[allow(dead_code, non_upper_case_globals)] pub const REFERENCED_BY_VERTEX_SHADER: types::GLenum = 0x9306;
#[allow(dead_code, non_upper_case_globals)] pub const REFLECTION_MAP: types::GLenum = 0x8512;
#[allow(dead_code, non_upper_case_globals)] pub const RENDER: types::GLenum = 0x1C00;
#[allow(dead_code, non_upper_case_globals)] pub const RENDERBUFFER: types::GLenum = 0x8D41;
#[allow(dead_code, non_upper_case_globals)] pub const RENDERBUFFER_ALPHA_SIZE: types::GLenum = 0x8D53;
#[allow(dead_code, non_upper_case_globals)] pub const RENDERBUFFER_BINDING: types::GLenum = 0x8CA7;
#[allow(dead_code, non_upper_case_globals)] pub const RENDERBUFFER_BLUE_SIZE: types::GLenum = 0x8D52;
#[allow(dead_code, non_upper_case_globals)] pub const RENDERBUFFER_DEPTH_SIZE: types::GLenum = 0x8D54;
#[allow(dead_code, non_upper_case_globals)] pub const RENDERBUFFER_GREEN_SIZE: types::GLenum = 0x8D51;
#[allow(dead_code, non_upper_case_globals)] pub const RENDERBUFFER_HEIGHT: types::GLenum = 0x8D43;
#[allow(dead_code, non_upper_case_globals)] pub const RENDERBUFFER_INTERNAL_FORMAT: types::GLenum = 0x8D44;
#[allow(dead_code, non_upper_case_globals)] pub const RENDERBUFFER_RED_SIZE: types::GLenum = 0x8D50;
#[allow(dead_code, non_upper_case_globals)] pub const RENDERBUFFER_SAMPLES: types::GLenum = 0x8CAB;
#[allow(dead_code, non_upper_case_globals)] pub const RENDERBUFFER_STENCIL_SIZE: types::GLenum = 0x8D55;
#[allow(dead_code, non_upper_case_globals)] pub const RENDERBUFFER_WIDTH: types::GLenum = 0x8D42;
#[allow(dead_code, non_upper_case_globals)] pub const RENDERER: types::GLenum = 0x1F01;
#[allow(dead_code, non_upper_case_globals)] pub const RENDER_MODE: types::GLenum = 0x0C40;
#[allow(dead_code, non_upper_case_globals)] pub const REPEAT: types::GLenum = 0x2901;
#[allow(dead_code, non_upper_case_globals)] pub const REPLACE: types::GLenum = 0x1E01;
#[allow(dead_code, non_upper_case_globals)] pub const REQUIRED_TEXTURE_IMAGE_UNITS_OES: types::GLenum = 0x8D68;
#[allow(dead_code, non_upper_case_globals)] pub const RESCALE_NORMAL: types::GLenum = 0x803A;
#[allow(dead_code, non_upper_case_globals)] pub const RETURN: types::GLenum = 0x0102;
#[allow(dead_code, non_upper_case_globals)] pub const RG: types::GLenum = 0x8227;
#[allow(dead_code, non_upper_case_globals)] pub const RG16: types::GLenum = 0x822C;
#[allow(dead_code, non_upper_case_globals)] pub const RG16F: types::GLenum = 0x822F;
#[allow(dead_code, non_upper_case_globals)] pub const RG16F_EXT: types::GLenum = 0x822F;
#[allow(dead_code, non_upper_case_globals)] pub const RG16I: types::GLenum = 0x8239;
#[allow(dead_code, non_upper_case_globals)] pub const RG16UI: types::GLenum = 0x823A;
#[allow(dead_code, non_upper_case_globals)] pub const RG16_SNORM: types::GLenum = 0x8F99;
#[allow(dead_code, non_upper_case_globals)] pub const RG32F: types::GLenum = 0x8230;
#[allow(dead_code, non_upper_case_globals)] pub const RG32F_EXT: types::GLenum = 0x8230;
#[allow(dead_code, non_upper_case_globals)] pub const RG32I: types::GLenum = 0x823B;
#[allow(dead_code, non_upper_case_globals)] pub const RG32UI: types::GLenum = 0x823C;
#[allow(dead_code, non_upper_case_globals)] pub const RG8: types::GLenum = 0x822B;
#[allow(dead_code, non_upper_case_globals)] pub const RG8I: types::GLenum = 0x8237;
#[allow(dead_code, non_upper_case_globals)] pub const RG8UI: types::GLenum = 0x8238;
#[allow(dead_code, non_upper_case_globals)] pub const RG8_EXT: types::GLenum = 0x822B;
#[allow(dead_code, non_upper_case_globals)] pub const RG8_SNORM: types::GLenum = 0x8F95;
#[allow(dead_code, non_upper_case_globals)] pub const RGB: types::GLenum = 0x1907;
#[allow(dead_code, non_upper_case_globals)] pub const RGB10: types::GLenum = 0x8052;
#[allow(dead_code, non_upper_case_globals)] pub const RGB10_A2: types::GLenum = 0x8059;
#[allow(dead_code, non_upper_case_globals)] pub const RGB10_A2UI: types::GLenum = 0x906F;
#[allow(dead_code, non_upper_case_globals)] pub const RGB10_A2_EXT: types::GLenum = 0x8059;
#[allow(dead_code, non_upper_case_globals)] pub const RGB10_EXT: types::GLenum = 0x8052;
#[allow(dead_code, non_upper_case_globals)] pub const RGB12: types::GLenum = 0x8053;
#[allow(dead_code, non_upper_case_globals)] pub const RGB16: types::GLenum = 0x8054;
#[allow(dead_code, non_upper_case_globals)] pub const RGB16F: types::GLenum = 0x881B;
#[allow(dead_code, non_upper_case_globals)] pub const RGB16F_EXT: types::GLenum = 0x881B;
#[allow(dead_code, non_upper_case_globals)] pub const RGB16I: types::GLenum = 0x8D89;
#[allow(dead_code, non_upper_case_globals)] pub const RGB16UI: types::GLenum = 0x8D77;
#[allow(dead_code, non_upper_case_globals)] pub const RGB16_SNORM: types::GLenum = 0x8F9A;
#[allow(dead_code, non_upper_case_globals)] pub const RGB32F: types::GLenum = 0x8815;
#[allow(dead_code, non_upper_case_globals)] pub const RGB32F_EXT: types::GLenum = 0x8815;
#[allow(dead_code, non_upper_case_globals)] pub const RGB32I: types::GLenum = 0x8D83;
#[allow(dead_code, non_upper_case_globals)] pub const RGB32UI: types::GLenum = 0x8D71;
#[allow(dead_code, non_upper_case_globals)] pub const RGB4: types::GLenum = 0x804F;
#[allow(dead_code, non_upper_case_globals)] pub const RGB5: types::GLenum = 0x8050;
#[allow(dead_code, non_upper_case_globals)] pub const RGB565: types::GLenum = 0x8D62;
#[allow(dead_code, non_upper_case_globals)] pub const RGB5_A1: types::GLenum = 0x8057;
#[allow(dead_code, non_upper_case_globals)] pub const RGB8: types::GLenum = 0x8051;
#[allow(dead_code, non_upper_case_globals)] pub const RGB8I: types::GLenum = 0x8D8F;
#[allow(dead_code, non_upper_case_globals)] pub const RGB8UI: types::GLenum = 0x8D7D;
#[allow(dead_code, non_upper_case_globals)] pub const RGB8_SNORM: types::GLenum = 0x8F96;
#[allow(dead_code, non_upper_case_globals)] pub const RGB9_E5: types::GLenum = 0x8C3D;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA: types::GLenum = 0x1908;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA12: types::GLenum = 0x805A;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA16: types::GLenum = 0x805B;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA16F: types::GLenum = 0x881A;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA16F_EXT: types::GLenum = 0x881A;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA16I: types::GLenum = 0x8D88;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA16UI: types::GLenum = 0x8D76;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA16_SNORM: types::GLenum = 0x8F9B;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA2: types::GLenum = 0x8055;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA32F: types::GLenum = 0x8814;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA32F_EXT: types::GLenum = 0x8814;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA32I: types::GLenum = 0x8D82;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA32UI: types::GLenum = 0x8D70;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA4: types::GLenum = 0x8056;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA8: types::GLenum = 0x8058;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA8I: types::GLenum = 0x8D8E;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA8UI: types::GLenum = 0x8D7C;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA8_SNORM: types::GLenum = 0x8F97;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA_INTEGER: types::GLenum = 0x8D99;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA_MODE: types::GLenum = 0x0C31;
#[allow(dead_code, non_upper_case_globals)] pub const RGB_INTEGER: types::GLenum = 0x8D98;
#[allow(dead_code, non_upper_case_globals)] pub const RGB_SCALE: types::GLenum = 0x8573;
#[allow(dead_code, non_upper_case_globals)] pub const RG_INTEGER: types::GLenum = 0x8228;
#[allow(dead_code, non_upper_case_globals)] pub const RIGHT: types::GLenum = 0x0407;
#[allow(dead_code, non_upper_case_globals)] pub const S: types::GLenum = 0x2000;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER: types::GLenum = 0x82E6;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_1D: types::GLenum = 0x8B5D;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_1D_ARRAY: types::GLenum = 0x8DC0;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_1D_ARRAY_SHADOW: types::GLenum = 0x8DC3;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_1D_SHADOW: types::GLenum = 0x8B61;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_2D: types::GLenum = 0x8B5E;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_2D_ARRAY: types::GLenum = 0x8DC1;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_2D_ARRAY_SHADOW: types::GLenum = 0x8DC4;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_2D_MULTISAMPLE: types::GLenum = 0x9108;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_2D_MULTISAMPLE_ARRAY: types::GLenum = 0x910B;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_2D_RECT: types::GLenum = 0x8B63;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_2D_RECT_SHADOW: types::GLenum = 0x8B64;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_2D_SHADOW: types::GLenum = 0x8B62;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_3D: types::GLenum = 0x8B5F;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_BINDING: types::GLenum = 0x8919;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_BUFFER: types::GLenum = 0x8DC2;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_CUBE: types::GLenum = 0x8B60;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_CUBE_SHADOW: types::GLenum = 0x8DC5;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_EXTERNAL_OES: types::GLenum = 0x8D66;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_KHR: types::GLenum = 0x82E6;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLES: types::GLenum = 0x80A9;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLES_PASSED: types::GLenum = 0x8914;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLE_ALPHA_TO_COVERAGE: types::GLenum = 0x809E;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLE_ALPHA_TO_ONE: types::GLenum = 0x809F;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLE_BUFFERS: types::GLenum = 0x80A8;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLE_COVERAGE: types::GLenum = 0x80A0;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLE_COVERAGE_INVERT: types::GLenum = 0x80AB;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLE_COVERAGE_VALUE: types::GLenum = 0x80AA;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLE_MASK: types::GLenum = 0x8E51;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLE_MASK_VALUE: types::GLenum = 0x8E52;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLE_POSITION: types::GLenum = 0x8E50;
#[allow(dead_code, non_upper_case_globals)] pub const SCISSOR_BIT: types::GLenum = 0x00080000;
#[allow(dead_code, non_upper_case_globals)] pub const SCISSOR_BOX: types::GLenum = 0x0C10;
#[allow(dead_code, non_upper_case_globals)] pub const SCISSOR_TEST: types::GLenum = 0x0C11;
#[allow(dead_code, non_upper_case_globals)] pub const SCREEN_KHR: types::GLenum = 0x9295;
#[allow(dead_code, non_upper_case_globals)] pub const SECONDARY_COLOR_ARRAY: types::GLenum = 0x845E;
#[allow(dead_code, non_upper_case_globals)] pub const SECONDARY_COLOR_ARRAY_BUFFER_BINDING: types::GLenum = 0x889C;
#[allow(dead_code, non_upper_case_globals)] pub const SECONDARY_COLOR_ARRAY_POINTER: types::GLenum = 0x845D;
#[allow(dead_code, non_upper_case_globals)] pub const SECONDARY_COLOR_ARRAY_SIZE: types::GLenum = 0x845A;
#[allow(dead_code, non_upper_case_globals)] pub const SECONDARY_COLOR_ARRAY_STRIDE: types::GLenum = 0x845C;
#[allow(dead_code, non_upper_case_globals)] pub const SECONDARY_COLOR_ARRAY_TYPE: types::GLenum = 0x845B;
#[allow(dead_code, non_upper_case_globals)] pub const SELECT: types::GLenum = 0x1C02;
#[allow(dead_code, non_upper_case_globals)] pub const SELECTION_BUFFER_POINTER: types::GLenum = 0x0DF3;
#[allow(dead_code, non_upper_case_globals)] pub const SELECTION_BUFFER_SIZE: types::GLenum = 0x0DF4;
#[allow(dead_code, non_upper_case_globals)] pub const SEPARATE_ATTRIBS: types::GLenum = 0x8C8D;
#[allow(dead_code, non_upper_case_globals)] pub const SEPARATE_SPECULAR_COLOR: types::GLenum = 0x81FA;
#[allow(dead_code, non_upper_case_globals)] pub const SET: types::GLenum = 0x150F;
#[allow(dead_code, non_upper_case_globals)] pub const SHADER: types::GLenum = 0x82E1;
#[allow(dead_code, non_upper_case_globals)] pub const SHADER_BINARY_FORMATS: types::GLenum = 0x8DF8;
#[allow(dead_code, non_upper_case_globals)] pub const SHADER_COMPILER: types::GLenum = 0x8DFA;
#[allow(dead_code, non_upper_case_globals)] pub const SHADER_IMAGE_ACCESS_BARRIER_BIT: types::GLenum = 0x00000020;
#[allow(dead_code, non_upper_case_globals)] pub const SHADER_KHR: types::GLenum = 0x82E1;
#[allow(dead_code, non_upper_case_globals)] pub const SHADER_PIXEL_LOCAL_STORAGE_EXT: types::GLenum = 0x8F64;
#[allow(dead_code, non_upper_case_globals)] pub const SHADER_SOURCE_LENGTH: types::GLenum = 0x8B88;
#[allow(dead_code, non_upper_case_globals)] pub const SHADER_STORAGE_BARRIER_BIT: types::GLenum = 0x00002000;
#[allow(dead_code, non_upper_case_globals)] pub const SHADER_STORAGE_BLOCK: types::GLenum = 0x92E6;
#[allow(dead_code, non_upper_case_globals)] pub const SHADER_STORAGE_BUFFER: types::GLenum = 0x90D2;
#[allow(dead_code, non_upper_case_globals)] pub const SHADER_STORAGE_BUFFER_BINDING: types::GLenum = 0x90D3;
#[allow(dead_code, non_upper_case_globals)] pub const SHADER_STORAGE_BUFFER_OFFSET_ALIGNMENT: types::GLenum = 0x90DF;
#[allow(dead_code, non_upper_case_globals)] pub const SHADER_STORAGE_BUFFER_SIZE: types::GLenum = 0x90D5;
#[allow(dead_code, non_upper_case_globals)] pub const SHADER_STORAGE_BUFFER_START: types::GLenum = 0x90D4;
#[allow(dead_code, non_upper_case_globals)] pub const SHADER_TYPE: types::GLenum = 0x8B4F;
#[allow(dead_code, non_upper_case_globals)] pub const SHADE_MODEL: types::GLenum = 0x0B54;
#[allow(dead_code, non_upper_case_globals)] pub const SHADING_LANGUAGE_VERSION: types::GLenum = 0x8B8C;
#[allow(dead_code, non_upper_case_globals)] pub const SHININESS: types::GLenum = 0x1601;
#[allow(dead_code, non_upper_case_globals)] pub const SHORT: types::GLenum = 0x1402;
#[allow(dead_code, non_upper_case_globals)] pub const SIGNALED: types::GLenum = 0x9119;
#[allow(dead_code, non_upper_case_globals)] pub const SIGNED_NORMALIZED: types::GLenum = 0x8F9C;
#[allow(dead_code, non_upper_case_globals)] pub const SINGLE_COLOR: types::GLenum = 0x81F9;
#[allow(dead_code, non_upper_case_globals)] pub const SLUMINANCE: types::GLenum = 0x8C46;
#[allow(dead_code, non_upper_case_globals)] pub const SLUMINANCE8: types::GLenum = 0x8C47;
#[allow(dead_code, non_upper_case_globals)] pub const SLUMINANCE8_ALPHA8: types::GLenum = 0x8C45;
#[allow(dead_code, non_upper_case_globals)] pub const SLUMINANCE_ALPHA: types::GLenum = 0x8C44;
#[allow(dead_code, non_upper_case_globals)] pub const SMOOTH: types::GLenum = 0x1D01;
#[allow(dead_code, non_upper_case_globals)] pub const SMOOTH_LINE_WIDTH_GRANULARITY: types::GLenum = 0x0B23;
#[allow(dead_code, non_upper_case_globals)] pub const SMOOTH_LINE_WIDTH_RANGE: types::GLenum = 0x0B22;
#[allow(dead_code, non_upper_case_globals)] pub const SMOOTH_POINT_SIZE_GRANULARITY: types::GLenum = 0x0B13;
#[allow(dead_code, non_upper_case_globals)] pub const SMOOTH_POINT_SIZE_RANGE: types::GLenum = 0x0B12;
#[allow(dead_code, non_upper_case_globals)] pub const SOFTLIGHT_KHR: types::GLenum = 0x929C;
#[allow(dead_code, non_upper_case_globals)] pub const SOURCE0_ALPHA: types::GLenum = 0x8588;
#[allow(dead_code, non_upper_case_globals)] pub const SOURCE0_RGB: types::GLenum = 0x8580;
#[allow(dead_code, non_upper_case_globals)] pub const SOURCE1_ALPHA: types::GLenum = 0x8589;
#[allow(dead_code, non_upper_case_globals)] pub const SOURCE1_RGB: types::GLenum = 0x8581;
#[allow(dead_code, non_upper_case_globals)] pub const SOURCE2_ALPHA: types::GLenum = 0x858A;
#[allow(dead_code, non_upper_case_globals)] pub const SOURCE2_RGB: types::GLenum = 0x8582;
#[allow(dead_code, non_upper_case_globals)] pub const SPECULAR: types::GLenum = 0x1202;
#[allow(dead_code, non_upper_case_globals)] pub const SPHERE_MAP: types::GLenum = 0x2402;
#[allow(dead_code, non_upper_case_globals)] pub const SPOT_CUTOFF: types::GLenum = 0x1206;
#[allow(dead_code, non_upper_case_globals)] pub const SPOT_DIRECTION: types::GLenum = 0x1204;
#[allow(dead_code, non_upper_case_globals)] pub const SPOT_EXPONENT: types::GLenum = 0x1205;
#[allow(dead_code, non_upper_case_globals)] pub const SRC0_ALPHA: types::GLenum = 0x8588;
#[allow(dead_code, non_upper_case_globals)] pub const SRC0_RGB: types::GLenum = 0x8580;
#[allow(dead_code, non_upper_case_globals)] pub const SRC1_ALPHA: types::GLenum = 0x8589;
#[allow(dead_code, non_upper_case_globals)] pub const SRC1_COLOR: types::GLenum = 0x88F9;
#[allow(dead_code, non_upper_case_globals)] pub const SRC1_RGB: types::GLenum = 0x8581;
#[allow(dead_code, non_upper_case_globals)] pub const SRC2_ALPHA: types::GLenum = 0x858A;
#[allow(dead_code, non_upper_case_globals)] pub const SRC2_RGB: types::GLenum = 0x8582;
#[allow(dead_code, non_upper_case_globals)] pub const SRC_ALPHA: types::GLenum = 0x0302;
#[allow(dead_code, non_upper_case_globals)] pub const SRC_ALPHA_SATURATE: types::GLenum = 0x0308;
#[allow(dead_code, non_upper_case_globals)] pub const SRC_COLOR: types::GLenum = 0x0300;
#[allow(dead_code, non_upper_case_globals)] pub const SRGB: types::GLenum = 0x8C40;
#[allow(dead_code, non_upper_case_globals)] pub const SRGB8: types::GLenum = 0x8C41;
#[allow(dead_code, non_upper_case_globals)] pub const SRGB8_ALPHA8: types::GLenum = 0x8C43;
#[allow(dead_code, non_upper_case_globals)] pub const SRGB_ALPHA: types::GLenum = 0x8C42;
#[allow(dead_code, non_upper_case_globals)] pub const STACK_OVERFLOW: types::GLenum = 0x0503;
#[allow(dead_code, non_upper_case_globals)] pub const STACK_OVERFLOW_KHR: types::GLenum = 0x0503;
#[allow(dead_code, non_upper_case_globals)] pub const STACK_UNDERFLOW: types::GLenum = 0x0504;
#[allow(dead_code, non_upper_case_globals)] pub const STACK_UNDERFLOW_KHR: types::GLenum = 0x0504;
#[allow(dead_code, non_upper_case_globals)] pub const STATIC_COPY: types::GLenum = 0x88E6;
#[allow(dead_code, non_upper_case_globals)] pub const STATIC_DRAW: types::GLenum = 0x88E4;
#[allow(dead_code, non_upper_case_globals)] pub const STATIC_READ: types::GLenum = 0x88E5;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL: types::GLenum = 0x1802;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_ATTACHMENT: types::GLenum = 0x8D20;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_BACK_FAIL: types::GLenum = 0x8801;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_BACK_FUNC: types::GLenum = 0x8800;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_BACK_PASS_DEPTH_FAIL: types::GLenum = 0x8802;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_BACK_PASS_DEPTH_PASS: types::GLenum = 0x8803;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_BACK_REF: types::GLenum = 0x8CA3;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_BACK_VALUE_MASK: types::GLenum = 0x8CA4;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_BACK_WRITEMASK: types::GLenum = 0x8CA5;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_BITS: types::GLenum = 0x0D57;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_BUFFER_BIT: types::GLenum = 0x00000400;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_CLEAR_VALUE: types::GLenum = 0x0B91;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_FAIL: types::GLenum = 0x0B94;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_FUNC: types::GLenum = 0x0B92;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_INDEX: types::GLenum = 0x1901;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_INDEX1: types::GLenum = 0x8D46;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_INDEX16: types::GLenum = 0x8D49;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_INDEX4: types::GLenum = 0x8D47;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_INDEX8: types::GLenum = 0x8D48;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_PASS_DEPTH_FAIL: types::GLenum = 0x0B95;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_PASS_DEPTH_PASS: types::GLenum = 0x0B96;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_REF: types::GLenum = 0x0B97;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_TEST: types::GLenum = 0x0B90;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_VALUE_MASK: types::GLenum = 0x0B93;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_WRITEMASK: types::GLenum = 0x0B98;
#[allow(dead_code, non_upper_case_globals)] pub const STEREO: types::GLenum = 0x0C33;
#[allow(dead_code, non_upper_case_globals)] pub const STORAGE_CACHED_APPLE: types::GLenum = 0x85BE;
#[allow(dead_code, non_upper_case_globals)] pub const STORAGE_PRIVATE_APPLE: types::GLenum = 0x85BD;
#[allow(dead_code, non_upper_case_globals)] pub const STORAGE_SHARED_APPLE: types::GLenum = 0x85BF;
#[allow(dead_code, non_upper_case_globals)] pub const STREAM_COPY: types::GLenum = 0x88E2;
#[allow(dead_code, non_upper_case_globals)] pub const STREAM_DRAW: types::GLenum = 0x88E0;
#[allow(dead_code, non_upper_case_globals)] pub const STREAM_READ: types::GLenum = 0x88E1;
#[allow(dead_code, non_upper_case_globals)] pub const SUBPIXEL_BITS: types::GLenum = 0x0D50;
#[allow(dead_code, non_upper_case_globals)] pub const SUBTRACT: types::GLenum = 0x84E7;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_CONDITION: types::GLenum = 0x9113;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_FENCE: types::GLenum = 0x9116;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_FLAGS: types::GLenum = 0x9115;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_FLUSH_COMMANDS_BIT: types::GLenum = 0x00000001;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_GPU_COMMANDS_COMPLETE: types::GLenum = 0x9117;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_STATUS: types::GLenum = 0x9114;
#[allow(dead_code, non_upper_case_globals)] pub const T: types::GLenum = 0x2001;
#[allow(dead_code, non_upper_case_globals)] pub const T2F_C3F_V3F: types::GLenum = 0x2A2A;
#[allow(dead_code, non_upper_case_globals)] pub const T2F_C4F_N3F_V3F: types::GLenum = 0x2A2C;
#[allow(dead_code, non_upper_case_globals)] pub const T2F_C4UB_V3F: types::GLenum = 0x2A29;
#[allow(dead_code, non_upper_case_globals)] pub const T2F_N3F_V3F: types::GLenum = 0x2A2B;
#[allow(dead_code, non_upper_case_globals)] pub const T2F_V3F: types::GLenum = 0x2A27;
#[allow(dead_code, non_upper_case_globals)] pub const T4F_C4F_N3F_V4F: types::GLenum = 0x2A2D;
#[allow(dead_code, non_upper_case_globals)] pub const T4F_V4F: types::GLenum = 0x2A28;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE: types::GLenum = 0x1702;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE0: types::GLenum = 0x84C0;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE1: types::GLenum = 0x84C1;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE10: types::GLenum = 0x84CA;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE11: types::GLenum = 0x84CB;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE12: types::GLenum = 0x84CC;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE13: types::GLenum = 0x84CD;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE14: types::GLenum = 0x84CE;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE15: types::GLenum = 0x84CF;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE16: types::GLenum = 0x84D0;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE17: types::GLenum = 0x84D1;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE18: types::GLenum = 0x84D2;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE19: types::GLenum = 0x84D3;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE2: types::GLenum = 0x84C2;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE20: types::GLenum = 0x84D4;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE21: types::GLenum = 0x84D5;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE22: types::GLenum = 0x84D6;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE23: types::GLenum = 0x84D7;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE24: types::GLenum = 0x84D8;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE25: types::GLenum = 0x84D9;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE26: types::GLenum = 0x84DA;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE27: types::GLenum = 0x84DB;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE28: types::GLenum = 0x84DC;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE29: types::GLenum = 0x84DD;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE3: types::GLenum = 0x84C3;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE30: types::GLenum = 0x84DE;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE31: types::GLenum = 0x84DF;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE4: types::GLenum = 0x84C4;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE5: types::GLenum = 0x84C5;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE6: types::GLenum = 0x84C6;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE7: types::GLenum = 0x84C7;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE8: types::GLenum = 0x84C8;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE9: types::GLenum = 0x84C9;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_1D: types::GLenum = 0x0DE0;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_1D_ARRAY: types::GLenum = 0x8C18;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_2D: types::GLenum = 0x0DE1;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_2D_ARRAY: types::GLenum = 0x8C1A;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_2D_MULTISAMPLE: types::GLenum = 0x9100;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_2D_MULTISAMPLE_ARRAY: types::GLenum = 0x9102;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_3D: types::GLenum = 0x806F;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_ALPHA_SIZE: types::GLenum = 0x805F;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_ALPHA_TYPE: types::GLenum = 0x8C13;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BASE_LEVEL: types::GLenum = 0x813C;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BINDING_1D: types::GLenum = 0x8068;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BINDING_1D_ARRAY: types::GLenum = 0x8C1C;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BINDING_2D: types::GLenum = 0x8069;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BINDING_2D_ARRAY: types::GLenum = 0x8C1D;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BINDING_2D_MULTISAMPLE: types::GLenum = 0x9104;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BINDING_2D_MULTISAMPLE_ARRAY: types::GLenum = 0x9105;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BINDING_3D: types::GLenum = 0x806A;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BINDING_BUFFER: types::GLenum = 0x8C2C;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BINDING_CUBE_MAP: types::GLenum = 0x8514;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BINDING_EXTERNAL_OES: types::GLenum = 0x8D67;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BINDING_RECTANGLE: types::GLenum = 0x84F6;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BINDING_RECTANGLE_ARB: types::GLenum = 0x84F6;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BIT: types::GLenum = 0x00040000;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BLUE_SIZE: types::GLenum = 0x805E;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BLUE_TYPE: types::GLenum = 0x8C12;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BORDER: types::GLenum = 0x1005;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BORDER_COLOR: types::GLenum = 0x1004;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BUFFER: types::GLenum = 0x8C2A;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BUFFER_DATA_STORE_BINDING: types::GLenum = 0x8C2D;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_COMPARE_FUNC: types::GLenum = 0x884D;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_COMPARE_MODE: types::GLenum = 0x884C;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_COMPONENTS: types::GLenum = 0x1003;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_COMPRESSED: types::GLenum = 0x86A1;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_COMPRESSED_IMAGE_SIZE: types::GLenum = 0x86A0;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_COMPRESSION_HINT: types::GLenum = 0x84EF;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_COORD_ARRAY: types::GLenum = 0x8078;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_COORD_ARRAY_BUFFER_BINDING: types::GLenum = 0x889A;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_COORD_ARRAY_POINTER: types::GLenum = 0x8092;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_COORD_ARRAY_SIZE: types::GLenum = 0x8088;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_COORD_ARRAY_STRIDE: types::GLenum = 0x808A;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_COORD_ARRAY_TYPE: types::GLenum = 0x8089;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_CUBE_MAP: types::GLenum = 0x8513;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_CUBE_MAP_NEGATIVE_X: types::GLenum = 0x8516;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_CUBE_MAP_NEGATIVE_Y: types::GLenum = 0x8518;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_CUBE_MAP_NEGATIVE_Z: types::GLenum = 0x851A;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_CUBE_MAP_POSITIVE_X: types::GLenum = 0x8515;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_CUBE_MAP_POSITIVE_Y: types::GLenum = 0x8517;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_CUBE_MAP_POSITIVE_Z: types::GLenum = 0x8519;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_CUBE_MAP_SEAMLESS: types::GLenum = 0x884F;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_DEPTH: types::GLenum = 0x8071;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_DEPTH_SIZE: types::GLenum = 0x884A;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_DEPTH_TYPE: types::GLenum = 0x8C16;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_ENV: types::GLenum = 0x2300;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_ENV_COLOR: types::GLenum = 0x2201;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_ENV_MODE: types::GLenum = 0x2200;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_EXTERNAL_OES: types::GLenum = 0x8D65;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_FETCH_BARRIER_BIT: types::GLenum = 0x00000008;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_FILTER_CONTROL: types::GLenum = 0x8500;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_FIXED_SAMPLE_LOCATIONS: types::GLenum = 0x9107;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_GEN_MODE: types::GLenum = 0x2500;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_GEN_Q: types::GLenum = 0x0C63;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_GEN_R: types::GLenum = 0x0C62;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_GEN_S: types::GLenum = 0x0C60;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_GEN_T: types::GLenum = 0x0C61;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_GREEN_SIZE: types::GLenum = 0x805D;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_GREEN_TYPE: types::GLenum = 0x8C11;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_HEIGHT: types::GLenum = 0x1001;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_IMMUTABLE_FORMAT: types::GLenum = 0x912F;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_IMMUTABLE_FORMAT_EXT: types::GLenum = 0x912F;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_IMMUTABLE_LEVELS: types::GLenum = 0x82DF;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_INTENSITY_SIZE: types::GLenum = 0x8061;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_INTENSITY_TYPE: types::GLenum = 0x8C15;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_INTERNAL_FORMAT: types::GLenum = 0x1003;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_LOD_BIAS: types::GLenum = 0x8501;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_LUMINANCE_SIZE: types::GLenum = 0x8060;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_LUMINANCE_TYPE: types::GLenum = 0x8C14;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_MAG_FILTER: types::GLenum = 0x2800;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_MATRIX: types::GLenum = 0x0BA8;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_MAX_ANISOTROPY_EXT: types::GLenum = 0x84FE;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_MAX_LEVEL: types::GLenum = 0x813D;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_MAX_LOD: types::GLenum = 0x813B;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_MIN_FILTER: types::GLenum = 0x2801;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_MIN_LOD: types::GLenum = 0x813A;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_PRIORITY: types::GLenum = 0x8066;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_RANGE_LENGTH_APPLE: types::GLenum = 0x85B7;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_RANGE_POINTER_APPLE: types::GLenum = 0x85B8;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_RECTANGLE: types::GLenum = 0x84F5;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_RECTANGLE_ARB: types::GLenum = 0x84F5;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_RED_SIZE: types::GLenum = 0x805C;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_RED_TYPE: types::GLenum = 0x8C10;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_RESIDENT: types::GLenum = 0x8067;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_SAMPLES: types::GLenum = 0x9106;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_SHARED_SIZE: types::GLenum = 0x8C3F;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_STACK_DEPTH: types::GLenum = 0x0BA5;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_STENCIL_SIZE: types::GLenum = 0x88F1;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_STORAGE_HINT_APPLE: types::GLenum = 0x85BC;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_SWIZZLE_A: types::GLenum = 0x8E45;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_SWIZZLE_B: types::GLenum = 0x8E44;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_SWIZZLE_G: types::GLenum = 0x8E43;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_SWIZZLE_R: types::GLenum = 0x8E42;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_SWIZZLE_RGBA: types::GLenum = 0x8E46;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_UPDATE_BARRIER_BIT: types::GLenum = 0x00000100;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_USAGE_ANGLE: types::GLenum = 0x93A2;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_WIDTH: types::GLenum = 0x1000;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_WRAP_R: types::GLenum = 0x8072;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_WRAP_S: types::GLenum = 0x2802;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_WRAP_T: types::GLenum = 0x2803;
#[allow(dead_code, non_upper_case_globals)] pub const TIMEOUT_EXPIRED: types::GLenum = 0x911B;
#[allow(dead_code, non_upper_case_globals)] pub const TIMEOUT_IGNORED: types::GLuint64 = 0xFFFFFFFFFFFFFFFF;
#[allow(dead_code, non_upper_case_globals)] pub const TIMESTAMP: types::GLenum = 0x8E28;
#[allow(dead_code, non_upper_case_globals)] pub const TIMESTAMP_EXT: types::GLenum = 0x8E28;
#[allow(dead_code, non_upper_case_globals)] pub const TIME_ELAPSED: types::GLenum = 0x88BF;
#[allow(dead_code, non_upper_case_globals)] pub const TIME_ELAPSED_EXT: types::GLenum = 0x88BF;
#[allow(dead_code, non_upper_case_globals)] pub const TOP_LEVEL_ARRAY_SIZE: types::GLenum = 0x930C;
#[allow(dead_code, non_upper_case_globals)] pub const TOP_LEVEL_ARRAY_STRIDE: types::GLenum = 0x930D;
#[allow(dead_code, non_upper_case_globals)] pub const TRANSFORM_BIT: types::GLenum = 0x00001000;
#[allow(dead_code, non_upper_case_globals)] pub const TRANSFORM_FEEDBACK: types::GLenum = 0x8E22;
#[allow(dead_code, non_upper_case_globals)] pub const TRANSFORM_FEEDBACK_ACTIVE: types::GLenum = 0x8E24;
#[allow(dead_code, non_upper_case_globals)] pub const TRANSFORM_FEEDBACK_BARRIER_BIT: types::GLenum = 0x00000800;
#[allow(dead_code, non_upper_case_globals)] pub const TRANSFORM_FEEDBACK_BINDING: types::GLenum = 0x8E25;
#[allow(dead_code, non_upper_case_globals)] pub const TRANSFORM_FEEDBACK_BUFFER: types::GLenum = 0x8C8E;
#[allow(dead_code, non_upper_case_globals)] pub const TRANSFORM_FEEDBACK_BUFFER_BINDING: types::GLenum = 0x8C8F;
#[allow(dead_code, non_upper_case_globals)] pub const TRANSFORM_FEEDBACK_BUFFER_MODE: types::GLenum = 0x8C7F;
#[allow(dead_code, non_upper_case_globals)] pub const TRANSFORM_FEEDBACK_BUFFER_SIZE: types::GLenum = 0x8C85;
#[allow(dead_code, non_upper_case_globals)] pub const TRANSFORM_FEEDBACK_BUFFER_START: types::GLenum = 0x8C84;
#[allow(dead_code, non_upper_case_globals)] pub const TRANSFORM_FEEDBACK_PAUSED: types::GLenum = 0x8E23;
#[allow(dead_code, non_upper_case_globals)] pub const TRANSFORM_FEEDBACK_PRIMITIVES_WRITTEN: types::GLenum = 0x8C88;
#[allow(dead_code, non_upper_case_globals)] pub const TRANSFORM_FEEDBACK_VARYING: types::GLenum = 0x92F4;
#[allow(dead_code, non_upper_case_globals)] pub const TRANSFORM_FEEDBACK_VARYINGS: types::GLenum = 0x8C83;
#[allow(dead_code, non_upper_case_globals)] pub const TRANSFORM_FEEDBACK_VARYING_MAX_LENGTH: types::GLenum = 0x8C76;
#[allow(dead_code, non_upper_case_globals)] pub const TRANSPOSE_COLOR_MATRIX: types::GLenum = 0x84E6;
#[allow(dead_code, non_upper_case_globals)] pub const TRANSPOSE_MODELVIEW_MATRIX: types::GLenum = 0x84E3;
#[allow(dead_code, non_upper_case_globals)] pub const TRANSPOSE_PROJECTION_MATRIX: types::GLenum = 0x84E4;
#[allow(dead_code, non_upper_case_globals)] pub const TRANSPOSE_TEXTURE_MATRIX: types::GLenum = 0x84E5;
#[allow(dead_code, non_upper_case_globals)] pub const TRIANGLES: types::GLenum = 0x0004;
#[allow(dead_code, non_upper_case_globals)] pub const TRIANGLES_ADJACENCY: types::GLenum = 0x000C;
#[allow(dead_code, non_upper_case_globals)] pub const TRIANGLE_FAN: types::GLenum = 0x0006;
#[allow(dead_code, non_upper_case_globals)] pub const TRIANGLE_STRIP: types::GLenum = 0x0005;
#[allow(dead_code, non_upper_case_globals)] pub const TRIANGLE_STRIP_ADJACENCY: types::GLenum = 0x000D;
#[allow(dead_code, non_upper_case_globals)] pub const TRUE: types::GLboolean = 1;
#[allow(dead_code, non_upper_case_globals)] pub const TYPE: types::GLenum = 0x92FA;
#[allow(dead_code, non_upper_case_globals)] pub const UNIFORM: types::GLenum = 0x92E1;
#[allow(dead_code, non_upper_case_globals)] pub const UNIFORM_ARRAY_STRIDE: types::GLenum = 0x8A3C;
#[allow(dead_code, non_upper_case_globals)] pub const UNIFORM_BARRIER_BIT: types::GLenum = 0x00000004;
#[allow(dead_code, non_upper_case_globals)] pub const UNIFORM_BLOCK: types::GLenum = 0x92E2;
#[allow(dead_code, non_upper_case_globals)] pub const UNIFORM_BLOCK_ACTIVE_UNIFORMS: types::GLenum = 0x8A42;
#[allow(dead_code, non_upper_case_globals)] pub const UNIFORM_BLOCK_ACTIVE_UNIFORM_INDICES: types::GLenum = 0x8A43;
#[allow(dead_code, non_upper_case_globals)] pub const UNIFORM_BLOCK_BINDING: types::GLenum = 0x8A3F;
#[allow(dead_code, non_upper_case_globals)] pub const UNIFORM_BLOCK_DATA_SIZE: types::GLenum = 0x8A40;
#[allow(dead_code, non_upper_case_globals)] pub const UNIFORM_BLOCK_INDEX: types::GLenum = 0x8A3A;
#[allow(dead_code, non_upper_case_globals)] pub const UNIFORM_BLOCK_NAME_LENGTH: types::GLenum = 0x8A41;
#[allow(dead_code, non_upper_case_globals)] pub const UNIFORM_BLOCK_REFERENCED_BY_FRAGMENT_SHADER: types::GLenum = 0x8A46;
#[allow(dead_code, non_upper_case_globals)] pub const UNIFORM_BLOCK_REFERENCED_BY_GEOMETRY_SHADER: types::GLenum = 0x8A45;
#[allow(dead_code, non_upper_case_globals)] pub const UNIFORM_BLOCK_REFERENCED_BY_VERTEX_SHADER: types::GLenum = 0x8A44;
#[allow(dead_code, non_upper_case_globals)] pub const UNIFORM_BUFFER: types::GLenum = 0x8A11;
#[allow(dead_code, non_upper_case_globals)] pub const UNIFORM_BUFFER_BINDING: types::GLenum = 0x8A28;
#[allow(dead_code, non_upper_case_globals)] pub const UNIFORM_BUFFER_OFFSET_ALIGNMENT: types::GLenum = 0x8A34;
#[allow(dead_code, non_upper_case_globals)] pub const UNIFORM_BUFFER_SIZE: types::GLenum = 0x8A2A;
#[allow(dead_code, non_upper_case_globals)] pub const UNIFORM_BUFFER_START: types::GLenum = 0x8A29;
#[allow(dead_code, non_upper_case_globals)] pub const UNIFORM_IS_ROW_MAJOR: types::GLenum = 0x8A3E;
#[allow(dead_code, non_upper_case_globals)] pub const UNIFORM_MATRIX_STRIDE: types::GLenum = 0x8A3D;
#[allow(dead_code, non_upper_case_globals)] pub const UNIFORM_NAME_LENGTH: types::GLenum = 0x8A39;
#[allow(dead_code, non_upper_case_globals)] pub const UNIFORM_OFFSET: types::GLenum = 0x8A3B;
#[allow(dead_code, non_upper_case_globals)] pub const UNIFORM_SIZE: types::GLenum = 0x8A38;
#[allow(dead_code, non_upper_case_globals)] pub const UNIFORM_TYPE: types::GLenum = 0x8A37;
#[allow(dead_code, non_upper_case_globals)] pub const UNPACK_ALIGNMENT: types::GLenum = 0x0CF5;
#[allow(dead_code, non_upper_case_globals)] pub const UNPACK_CLIENT_STORAGE_APPLE: types::GLenum = 0x85B2;
#[allow(dead_code, non_upper_case_globals)] pub const UNPACK_IMAGE_HEIGHT: types::GLenum = 0x806E;
#[allow(dead_code, non_upper_case_globals)] pub const UNPACK_LSB_FIRST: types::GLenum = 0x0CF1;
#[allow(dead_code, non_upper_case_globals)] pub const UNPACK_ROW_LENGTH: types::GLenum = 0x0CF2;
#[allow(dead_code, non_upper_case_globals)] pub const UNPACK_SKIP_IMAGES: types::GLenum = 0x806D;
#[allow(dead_code, non_upper_case_globals)] pub const UNPACK_SKIP_PIXELS: types::GLenum = 0x0CF4;
#[allow(dead_code, non_upper_case_globals)] pub const UNPACK_SKIP_ROWS: types::GLenum = 0x0CF3;
#[allow(dead_code, non_upper_case_globals)] pub const UNPACK_SWAP_BYTES: types::GLenum = 0x0CF0;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNALED: types::GLenum = 0x9118;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_BYTE: types::GLenum = 0x1401;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_BYTE_2_3_3_REV: types::GLenum = 0x8362;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_BYTE_3_3_2: types::GLenum = 0x8032;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT: types::GLenum = 0x1405;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_10F_11F_11F_REV: types::GLenum = 0x8C3B;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_10_10_10_2: types::GLenum = 0x8036;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_24_8: types::GLenum = 0x84FA;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_2_10_10_10_REV: types::GLenum = 0x8368;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_5_9_9_9_REV: types::GLenum = 0x8C3E;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_8_8_8_8: types::GLenum = 0x8035;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_8_8_8_8_REV: types::GLenum = 0x8367;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_ATOMIC_COUNTER: types::GLenum = 0x92DB;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_IMAGE_2D: types::GLenum = 0x9063;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_IMAGE_2D_ARRAY: types::GLenum = 0x9069;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_IMAGE_3D: types::GLenum = 0x9064;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_IMAGE_CUBE: types::GLenum = 0x9066;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_SAMPLER_1D: types::GLenum = 0x8DD1;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_SAMPLER_1D_ARRAY: types::GLenum = 0x8DD6;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_SAMPLER_2D: types::GLenum = 0x8DD2;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_SAMPLER_2D_ARRAY: types::GLenum = 0x8DD7;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_SAMPLER_2D_MULTISAMPLE: types::GLenum = 0x910A;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_SAMPLER_2D_MULTISAMPLE_ARRAY: types::GLenum = 0x910D;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_SAMPLER_2D_RECT: types::GLenum = 0x8DD5;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_SAMPLER_3D: types::GLenum = 0x8DD3;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_SAMPLER_BUFFER: types::GLenum = 0x8DD8;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_SAMPLER_CUBE: types::GLenum = 0x8DD4;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_VEC2: types::GLenum = 0x8DC6;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_VEC3: types::GLenum = 0x8DC7;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_VEC4: types::GLenum = 0x8DC8;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_NORMALIZED: types::GLenum = 0x8C17;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_SHORT: types::GLenum = 0x1403;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_SHORT_1_5_5_5_REV: types::GLenum = 0x8366;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_SHORT_4_4_4_4: types::GLenum = 0x8033;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_SHORT_4_4_4_4_REV: types::GLenum = 0x8365;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_SHORT_5_5_5_1: types::GLenum = 0x8034;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_SHORT_5_6_5: types::GLenum = 0x8363;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_SHORT_5_6_5_REV: types::GLenum = 0x8364;
#[allow(dead_code, non_upper_case_globals)] pub const UPPER_LEFT: types::GLenum = 0x8CA2;
#[allow(dead_code, non_upper_case_globals)] pub const V2F: types::GLenum = 0x2A20;
#[allow(dead_code, non_upper_case_globals)] pub const V3F: types::GLenum = 0x2A21;
#[allow(dead_code, non_upper_case_globals)] pub const VALIDATE_STATUS: types::GLenum = 0x8B83;
#[allow(dead_code, non_upper_case_globals)] pub const VENDOR: types::GLenum = 0x1F00;
#[allow(dead_code, non_upper_case_globals)] pub const VERSION: types::GLenum = 0x1F02;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_ARRAY: types::GLenum = 0x8074;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_ARRAY_BINDING: types::GLenum = 0x85B5;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_ARRAY_BINDING_APPLE: types::GLenum = 0x85B5;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_ARRAY_BUFFER_BINDING: types::GLenum = 0x8896;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_ARRAY_KHR: types::GLenum = 0x8074;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_ARRAY_POINTER: types::GLenum = 0x808E;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_ARRAY_SIZE: types::GLenum = 0x807A;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_ARRAY_STRIDE: types::GLenum = 0x807C;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_ARRAY_TYPE: types::GLenum = 0x807B;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_ATTRIB_ARRAY_BARRIER_BIT: types::GLenum = 0x00000001;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_ATTRIB_ARRAY_BUFFER_BINDING: types::GLenum = 0x889F;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_ATTRIB_ARRAY_DIVISOR: types::GLenum = 0x88FE;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_ATTRIB_ARRAY_ENABLED: types::GLenum = 0x8622;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_ATTRIB_ARRAY_INTEGER: types::GLenum = 0x88FD;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_ATTRIB_ARRAY_NORMALIZED: types::GLenum = 0x886A;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_ATTRIB_ARRAY_POINTER: types::GLenum = 0x8645;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_ATTRIB_ARRAY_SIZE: types::GLenum = 0x8623;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_ATTRIB_ARRAY_STRIDE: types::GLenum = 0x8624;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_ATTRIB_ARRAY_TYPE: types::GLenum = 0x8625;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_ATTRIB_BINDING: types::GLenum = 0x82D4;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_ATTRIB_RELATIVE_OFFSET: types::GLenum = 0x82D5;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_BINDING_BUFFER: types::GLenum = 0x8F4F;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_BINDING_DIVISOR: types::GLenum = 0x82D6;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_BINDING_OFFSET: types::GLenum = 0x82D7;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_BINDING_STRIDE: types::GLenum = 0x82D8;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_PROGRAM_POINT_SIZE: types::GLenum = 0x8642;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_PROGRAM_TWO_SIDE: types::GLenum = 0x8643;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_SHADER: types::GLenum = 0x8B31;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_SHADER_BIT: types::GLenum = 0x00000001;
#[allow(dead_code, non_upper_case_globals)] pub const VIEWPORT: types::GLenum = 0x0BA2;
#[allow(dead_code, non_upper_case_globals)] pub const VIEWPORT_BIT: types::GLenum = 0x00000800;
#[allow(dead_code, non_upper_case_globals)] pub const WAIT_FAILED: types::GLenum = 0x911D;
#[allow(dead_code, non_upper_case_globals)] pub const WEIGHT_ARRAY_BUFFER_BINDING: types::GLenum = 0x889E;
#[allow(dead_code, non_upper_case_globals)] pub const WRITE_ONLY: types::GLenum = 0x88B9;
#[allow(dead_code, non_upper_case_globals)] pub const XOR: types::GLenum = 0x1506;
#[allow(dead_code, non_upper_case_globals)] pub const ZERO: types::GLenum = 0;
#[allow(dead_code, non_upper_case_globals)] pub const ZOOM_X: types::GLenum = 0x0D16;
#[allow(dead_code, non_upper_case_globals)] pub const ZOOM_Y: types::GLenum = 0x0D17;

        #[allow(dead_code, missing_copy_implementations)]
        #[derive(Clone)]
        pub struct FnPtr {
            /// The function pointer that will be used when calling the function.
            f: *const __gl_imports::raw::c_void,
            /// True if the pointer points to a real function, false if points to a `panic!` fn.
            is_loaded: bool,
        }

        impl FnPtr {
            /// Creates a `FnPtr` from a load attempt.
            fn new(ptr: *const __gl_imports::raw::c_void) -> FnPtr {
                if ptr.is_null() {
                    FnPtr {
                        f: missing_fn_panic as *const __gl_imports::raw::c_void,
                        is_loaded: false
                    }
                } else {
                    FnPtr { f: ptr, is_loaded: true }
                }
            }

            /// Returns `true` if the function has been successfully loaded.
            ///
            /// If it returns `false`, calling the corresponding function will fail.
            #[inline]
            #[allow(dead_code)]
            pub fn is_loaded(&self) -> bool {
                self.is_loaded
            }
        }
    
#[inline(never)]
        fn missing_fn_panic() -> ! {
            panic!("gl function was not loaded")
        }

        #[allow(non_camel_case_types, non_snake_case, dead_code)]
        #[derive(Clone)]
        pub struct Gl {
pub Accum: FnPtr,
pub ActiveShaderProgram: FnPtr,
/// Fallbacks: ActiveTextureARB
pub ActiveTexture: FnPtr,
pub AlphaFunc: FnPtr,
pub AreTexturesResident: FnPtr,
/// Fallbacks: ArrayElementEXT
pub ArrayElement: FnPtr,
/// Fallbacks: AttachObjectARB
pub AttachShader: FnPtr,
pub Begin: FnPtr,
/// Fallbacks: BeginConditionalRenderNV
pub BeginConditionalRender: FnPtr,
/// Fallbacks: BeginQueryARB
pub BeginQuery: FnPtr,
pub BeginQueryEXT: FnPtr,
/// Fallbacks: BeginTransformFeedbackEXT, BeginTransformFeedbackNV
pub BeginTransformFeedback: FnPtr,
/// Fallbacks: BindAttribLocationARB
pub BindAttribLocation: FnPtr,
/// Fallbacks: BindBufferARB
pub BindBuffer: FnPtr,
/// Fallbacks: BindBufferBaseEXT, BindBufferBaseNV
pub BindBufferBase: FnPtr,
/// Fallbacks: BindBufferRangeEXT, BindBufferRangeNV
pub BindBufferRange: FnPtr,
/// Fallbacks: BindFragDataLocationEXT
pub BindFragDataLocation: FnPtr,
/// Fallbacks: BindFragDataLocationIndexedEXT
pub BindFragDataLocationIndexed: FnPtr,
pub BindFramebuffer: FnPtr,
pub BindImageTexture: FnPtr,
pub BindProgramPipeline: FnPtr,
pub BindRenderbuffer: FnPtr,
pub BindSampler: FnPtr,
/// Fallbacks: BindTextureEXT
pub BindTexture: FnPtr,
pub BindTransformFeedback: FnPtr,
/// Fallbacks: BindVertexArrayOES
pub BindVertexArray: FnPtr,
pub BindVertexArrayAPPLE: FnPtr,
pub BindVertexBuffer: FnPtr,
pub Bitmap: FnPtr,
pub BlendBarrierKHR: FnPtr,
/// Fallbacks: BlendColorEXT
pub BlendColor: FnPtr,
/// Fallbacks: BlendEquationEXT
pub BlendEquation: FnPtr,
/// Fallbacks: BlendEquationSeparateEXT
pub BlendEquationSeparate: FnPtr,
pub BlendFunc: FnPtr,
/// Fallbacks: BlendFuncSeparateEXT, BlendFuncSeparateINGR
pub BlendFuncSeparate: FnPtr,
/// Fallbacks: BlitFramebufferEXT, BlitFramebufferNV
pub BlitFramebuffer: FnPtr,
/// Fallbacks: BufferDataARB
pub BufferData: FnPtr,
/// Fallbacks: BufferStorageEXT
pub BufferStorage: FnPtr,
pub BufferStorageEXT: FnPtr,
/// Fallbacks: BufferSubDataARB
pub BufferSubData: FnPtr,
pub CallList: FnPtr,
pub CallLists: FnPtr,
/// Fallbacks: CheckFramebufferStatusEXT
pub CheckFramebufferStatus: FnPtr,
/// Fallbacks: ClampColorARB
pub ClampColor: FnPtr,
pub Clear: FnPtr,
pub ClearAccum: FnPtr,
pub ClearBufferfi: FnPtr,
pub ClearBufferfv: FnPtr,
pub ClearBufferiv: FnPtr,
pub ClearBufferuiv: FnPtr,
pub ClearColor: FnPtr,
pub ClearDepth: FnPtr,
/// Fallbacks: ClearDepthfOES
pub ClearDepthf: FnPtr,
pub ClearIndex: FnPtr,
pub ClearStencil: FnPtr,
/// Fallbacks: ClientActiveTextureARB
pub ClientActiveTexture: FnPtr,
/// Fallbacks: ClientWaitSyncAPPLE
pub ClientWaitSync: FnPtr,
pub ClipPlane: FnPtr,
pub Color3b: FnPtr,
pub Color3bv: FnPtr,
pub Color3d: FnPtr,
pub Color3dv: FnPtr,
pub Color3f: FnPtr,
pub Color3fv: FnPtr,
pub Color3i: FnPtr,
pub Color3iv: FnPtr,
pub Color3s: FnPtr,
pub Color3sv: FnPtr,
pub Color3ub: FnPtr,
pub Color3ubv: FnPtr,
pub Color3ui: FnPtr,
pub Color3uiv: FnPtr,
pub Color3us: FnPtr,
pub Color3usv: FnPtr,
pub Color4b: FnPtr,
pub Color4bv: FnPtr,
pub Color4d: FnPtr,
pub Color4dv: FnPtr,
pub Color4f: FnPtr,
pub Color4fv: FnPtr,
pub Color4i: FnPtr,
pub Color4iv: FnPtr,
pub Color4s: FnPtr,
pub Color4sv: FnPtr,
pub Color4ub: FnPtr,
pub Color4ubv: FnPtr,
pub Color4ui: FnPtr,
pub Color4uiv: FnPtr,
pub Color4us: FnPtr,
pub Color4usv: FnPtr,
pub ColorMask: FnPtr,
/// Fallbacks: ColorMaskIndexedEXT, ColorMaskiEXT, ColorMaskiOES
pub ColorMaski: FnPtr,
pub ColorMaterial: FnPtr,
pub ColorP3ui: FnPtr,
pub ColorP3uiv: FnPtr,
pub ColorP4ui: FnPtr,
pub ColorP4uiv: FnPtr,
pub ColorPointer: FnPtr,
/// Fallbacks: CompileShaderARB
pub CompileShader: FnPtr,
/// Fallbacks: CompressedTexImage1DARB
pub CompressedTexImage1D: FnPtr,
/// Fallbacks: CompressedTexImage2DARB
pub CompressedTexImage2D: FnPtr,
/// Fallbacks: CompressedTexImage3DARB
pub CompressedTexImage3D: FnPtr,
/// Fallbacks: CompressedTexSubImage1DARB
pub CompressedTexSubImage1D: FnPtr,
/// Fallbacks: CompressedTexSubImage2DARB
pub CompressedTexSubImage2D: FnPtr,
/// Fallbacks: CompressedTexSubImage3DARB
pub CompressedTexSubImage3D: FnPtr,
/// Fallbacks: CopyBufferSubDataNV
pub CopyBufferSubData: FnPtr,
/// Fallbacks: CopyImageSubDataEXT, CopyImageSubDataOES
pub CopyImageSubData: FnPtr,
pub CopyImageSubDataEXT: FnPtr,
pub CopyPixels: FnPtr,
pub CopySubTexture3DANGLE: FnPtr,
pub CopySubTextureCHROMIUM: FnPtr,
/// Fallbacks: CopyTexImage1DEXT
pub CopyTexImage1D: FnPtr,
/// Fallbacks: CopyTexImage2DEXT
pub CopyTexImage2D: FnPtr,
/// Fallbacks: CopyTexSubImage1DEXT
pub CopyTexSubImage1D: FnPtr,
/// Fallbacks: CopyTexSubImage2DEXT
pub CopyTexSubImage2D: FnPtr,
/// Fallbacks: CopyTexSubImage3DEXT
pub CopyTexSubImage3D: FnPtr,
pub CopyTexture3DANGLE: FnPtr,
pub CopyTextureCHROMIUM: FnPtr,
/// Fallbacks: CreateProgramObjectARB
pub CreateProgram: FnPtr,
/// Fallbacks: CreateShaderObjectARB
pub CreateShader: FnPtr,
pub CreateShaderProgramv: FnPtr,
pub CullFace: FnPtr,
/// Fallbacks: DebugMessageCallbackARB, DebugMessageCallbackKHR
pub DebugMessageCallback: FnPtr,
pub DebugMessageCallbackKHR: FnPtr,
/// Fallbacks: DebugMessageControlARB, DebugMessageControlKHR
pub DebugMessageControl: FnPtr,
pub DebugMessageControlKHR: FnPtr,
/// Fallbacks: DebugMessageInsertARB, DebugMessageInsertKHR
pub DebugMessageInsert: FnPtr,
pub DebugMessageInsertKHR: FnPtr,
/// Fallbacks: DeleteBuffersARB
pub DeleteBuffers: FnPtr,
pub DeleteFencesAPPLE: FnPtr,
/// Fallbacks: DeleteFramebuffersEXT
pub DeleteFramebuffers: FnPtr,
pub DeleteLists: FnPtr,
pub DeleteProgram: FnPtr,
pub DeleteProgramPipelines: FnPtr,
/// Fallbacks: DeleteQueriesARB
pub DeleteQueries: FnPtr,
pub DeleteQueriesEXT: FnPtr,
/// Fallbacks: DeleteRenderbuffersEXT
pub DeleteRenderbuffers: FnPtr,
pub DeleteSamplers: FnPtr,
pub DeleteShader: FnPtr,
/// Fallbacks: DeleteSyncAPPLE
pub DeleteSync: FnPtr,
pub DeleteTextures: FnPtr,
/// Fallbacks: DeleteTransformFeedbacksNV
pub DeleteTransformFeedbacks: FnPtr,
/// Fallbacks: DeleteVertexArraysAPPLE, DeleteVertexArraysOES
pub DeleteVertexArrays: FnPtr,
pub DeleteVertexArraysAPPLE: FnPtr,
pub DepthFunc: FnPtr,
pub DepthMask: FnPtr,
pub DepthRange: FnPtr,
/// Fallbacks: DepthRangefOES
pub DepthRangef: FnPtr,
/// Fallbacks: DetachObjectARB
pub DetachShader: FnPtr,
pub Disable: FnPtr,
pub DisableClientState: FnPtr,
/// Fallbacks: DisableVertexAttribArrayARB
pub DisableVertexAttribArray: FnPtr,
/// Fallbacks: DisableIndexedEXT, DisableiEXT, DisableiNV, DisableiOES
pub Disablei: FnPtr,
pub DispatchCompute: FnPtr,
pub DispatchComputeIndirect: FnPtr,
/// Fallbacks: DrawArraysEXT
pub DrawArrays: FnPtr,
pub DrawArraysIndirect: FnPtr,
/// Fallbacks: DrawArraysInstancedANGLE, DrawArraysInstancedARB, DrawArraysInstancedEXT, DrawArraysInstancedNV
pub DrawArraysInstanced: FnPtr,
pub DrawBuffer: FnPtr,
/// Fallbacks: DrawBuffersARB, DrawBuffersATI, DrawBuffersEXT
pub DrawBuffers: FnPtr,
pub DrawElements: FnPtr,
/// Fallbacks: DrawElementsBaseVertexEXT, DrawElementsBaseVertexOES
pub DrawElementsBaseVertex: FnPtr,
pub DrawElementsIndirect: FnPtr,
/// Fallbacks: DrawElementsInstancedANGLE, DrawElementsInstancedARB, DrawElementsInstancedEXT, DrawElementsInstancedNV
pub DrawElementsInstanced: FnPtr,
/// Fallbacks: DrawElementsInstancedBaseVertexEXT, DrawElementsInstancedBaseVertexOES
pub DrawElementsInstancedBaseVertex: FnPtr,
pub DrawPixels: FnPtr,
/// Fallbacks: DrawRangeElementsEXT
pub DrawRangeElements: FnPtr,
/// Fallbacks: DrawRangeElementsBaseVertexEXT, DrawRangeElementsBaseVertexOES
pub DrawRangeElementsBaseVertex: FnPtr,
pub EGLImageTargetRenderbufferStorageOES: FnPtr,
pub EGLImageTargetTexture2DOES: FnPtr,
pub EdgeFlag: FnPtr,
pub EdgeFlagPointer: FnPtr,
pub EdgeFlagv: FnPtr,
pub Enable: FnPtr,
pub EnableClientState: FnPtr,
/// Fallbacks: EnableVertexAttribArrayARB
pub EnableVertexAttribArray: FnPtr,
/// Fallbacks: EnableIndexedEXT, EnableiEXT, EnableiNV, EnableiOES
pub Enablei: FnPtr,
pub End: FnPtr,
/// Fallbacks: EndConditionalRenderNV, EndConditionalRenderNVX
pub EndConditionalRender: FnPtr,
pub EndList: FnPtr,
/// Fallbacks: EndQueryARB
pub EndQuery: FnPtr,
pub EndQueryEXT: FnPtr,
/// Fallbacks: EndTransformFeedbackEXT, EndTransformFeedbackNV
pub EndTransformFeedback: FnPtr,
pub EvalCoord1d: FnPtr,
pub EvalCoord1dv: FnPtr,
pub EvalCoord1f: FnPtr,
pub EvalCoord1fv: FnPtr,
pub EvalCoord2d: FnPtr,
pub EvalCoord2dv: FnPtr,
pub EvalCoord2f: FnPtr,
pub EvalCoord2fv: FnPtr,
pub EvalMesh1: FnPtr,
pub EvalMesh2: FnPtr,
pub EvalPoint1: FnPtr,
pub EvalPoint2: FnPtr,
pub FeedbackBuffer: FnPtr,
/// Fallbacks: FenceSyncAPPLE
pub FenceSync: FnPtr,
pub Finish: FnPtr,
pub FinishFenceAPPLE: FnPtr,
pub FinishObjectAPPLE: FnPtr,
pub Flush: FnPtr,
/// Fallbacks: FlushMappedBufferRangeAPPLE, FlushMappedBufferRangeEXT
pub FlushMappedBufferRange: FnPtr,
/// Fallbacks: FogCoordPointerEXT
pub FogCoordPointer: FnPtr,
/// Fallbacks: FogCoorddEXT
pub FogCoordd: FnPtr,
/// Fallbacks: FogCoorddvEXT
pub FogCoorddv: FnPtr,
/// Fallbacks: FogCoordfEXT
pub FogCoordf: FnPtr,
/// Fallbacks: FogCoordfvEXT
pub FogCoordfv: FnPtr,
pub Fogf: FnPtr,
pub Fogfv: FnPtr,
pub Fogi: FnPtr,
pub Fogiv: FnPtr,
pub FramebufferParameteri: FnPtr,
/// Fallbacks: FramebufferRenderbufferEXT
pub FramebufferRenderbuffer: FnPtr,
/// Fallbacks: FramebufferTextureARB, FramebufferTextureEXT, FramebufferTextureOES
pub FramebufferTexture: FnPtr,
/// Fallbacks: FramebufferTexture1DEXT
pub FramebufferTexture1D: FnPtr,
/// Fallbacks: FramebufferTexture2DEXT
pub FramebufferTexture2D: FnPtr,
/// Fallbacks: FramebufferTexture3DEXT
pub FramebufferTexture3D: FnPtr,
/// Fallbacks: FramebufferTextureLayerARB, FramebufferTextureLayerEXT
pub FramebufferTextureLayer: FnPtr,
pub FrontFace: FnPtr,
pub Frustum: FnPtr,
/// Fallbacks: GenBuffersARB
pub GenBuffers: FnPtr,
pub GenFencesAPPLE: FnPtr,
/// Fallbacks: GenFramebuffersEXT
pub GenFramebuffers: FnPtr,
pub GenLists: FnPtr,
pub GenProgramPipelines: FnPtr,
/// Fallbacks: GenQueriesARB
pub GenQueries: FnPtr,
pub GenQueriesEXT: FnPtr,
/// Fallbacks: GenRenderbuffersEXT
pub GenRenderbuffers: FnPtr,
pub GenSamplers: FnPtr,
pub GenTextures: FnPtr,
/// Fallbacks: GenTransformFeedbacksNV
pub GenTransformFeedbacks: FnPtr,
/// Fallbacks: GenVertexArraysAPPLE, GenVertexArraysOES
pub GenVertexArrays: FnPtr,
pub GenVertexArraysAPPLE: FnPtr,
/// Fallbacks: GenerateMipmapEXT
pub GenerateMipmap: FnPtr,
/// Fallbacks: GetActiveAttribARB
pub GetActiveAttrib: FnPtr,
/// Fallbacks: GetActiveUniformARB
pub GetActiveUniform: FnPtr,
pub GetActiveUniformBlockName: FnPtr,
pub GetActiveUniformBlockiv: FnPtr,
pub GetActiveUniformName: FnPtr,
pub GetActiveUniformsiv: FnPtr,
pub GetAttachedShaders: FnPtr,
/// Fallbacks: GetAttribLocationARB
pub GetAttribLocation: FnPtr,
/// Fallbacks: GetBooleanIndexedvEXT
pub GetBooleani_v: FnPtr,
pub GetBooleanv: FnPtr,
pub GetBufferParameteri64v: FnPtr,
/// Fallbacks: GetBufferParameterivARB
pub GetBufferParameteriv: FnPtr,
/// Fallbacks: GetBufferPointervARB, GetBufferPointervOES
pub GetBufferPointerv: FnPtr,
/// Fallbacks: GetBufferSubDataARB
pub GetBufferSubData: FnPtr,
pub GetClipPlane: FnPtr,
/// Fallbacks: GetCompressedTexImageARB
pub GetCompressedTexImage: FnPtr,
/// Fallbacks: GetDebugMessageLogARB, GetDebugMessageLogKHR
pub GetDebugMessageLog: FnPtr,
pub GetDebugMessageLogKHR: FnPtr,
pub GetDoublev: FnPtr,
pub GetError: FnPtr,
pub GetFloatv: FnPtr,
/// Fallbacks: GetFragDataIndexEXT
pub GetFragDataIndex: FnPtr,
/// Fallbacks: GetFragDataLocationEXT
pub GetFragDataLocation: FnPtr,
/// Fallbacks: GetFramebufferAttachmentParameterivEXT
pub GetFramebufferAttachmentParameteriv: FnPtr,
pub GetFramebufferParameteriv: FnPtr,
pub GetInteger64i_v: FnPtr,
/// Fallbacks: GetInteger64vAPPLE
pub GetInteger64v: FnPtr,
/// Fallbacks: GetIntegerIndexedvEXT
pub GetIntegeri_v: FnPtr,
pub GetIntegerv: FnPtr,
pub GetInternalformativ: FnPtr,
pub GetLightfv: FnPtr,
pub GetLightiv: FnPtr,
pub GetMapdv: FnPtr,
pub GetMapfv: FnPtr,
pub GetMapiv: FnPtr,
pub GetMaterialfv: FnPtr,
pub GetMaterialiv: FnPtr,
/// Fallbacks: GetMultisamplefvNV
pub GetMultisamplefv: FnPtr,
/// Fallbacks: GetObjectLabelKHR
pub GetObjectLabel: FnPtr,
pub GetObjectLabelKHR: FnPtr,
/// Fallbacks: GetObjectPtrLabelKHR
pub GetObjectPtrLabel: FnPtr,
pub GetObjectPtrLabelKHR: FnPtr,
pub GetPixelMapfv: FnPtr,
pub GetPixelMapuiv: FnPtr,
pub GetPixelMapusv: FnPtr,
/// Fallbacks: GetPointervEXT, GetPointervKHR
pub GetPointerv: FnPtr,
pub GetPointervKHR: FnPtr,
pub GetPolygonStipple: FnPtr,
/// Fallbacks: GetProgramBinaryOES
pub GetProgramBinary: FnPtr,
pub GetProgramInfoLog: FnPtr,
pub GetProgramInterfaceiv: FnPtr,
pub GetProgramPipelineInfoLog: FnPtr,
pub GetProgramPipelineiv: FnPtr,
pub GetProgramResourceIndex: FnPtr,
pub GetProgramResourceLocation: FnPtr,
pub GetProgramResourceName: FnPtr,
pub GetProgramResourceiv: FnPtr,
pub GetProgramiv: FnPtr,
/// Fallbacks: GetQueryObjecti64vEXT
pub GetQueryObjecti64v: FnPtr,
pub GetQueryObjecti64vEXT: FnPtr,
/// Fallbacks: GetQueryObjectivARB, GetQueryObjectivEXT
pub GetQueryObjectiv: FnPtr,
pub GetQueryObjectivEXT: FnPtr,
/// Fallbacks: GetQueryObjectui64vEXT
pub GetQueryObjectui64v: FnPtr,
pub GetQueryObjectui64vEXT: FnPtr,
/// Fallbacks: GetQueryObjectuivARB
pub GetQueryObjectuiv: FnPtr,
pub GetQueryObjectuivEXT: FnPtr,
/// Fallbacks: GetQueryivARB
pub GetQueryiv: FnPtr,
pub GetQueryivEXT: FnPtr,
/// Fallbacks: GetRenderbufferParameterivEXT
pub GetRenderbufferParameteriv: FnPtr,
/// Fallbacks: GetSamplerParameterIivEXT, GetSamplerParameterIivOES
pub GetSamplerParameterIiv: FnPtr,
/// Fallbacks: GetSamplerParameterIuivEXT, GetSamplerParameterIuivOES
pub GetSamplerParameterIuiv: FnPtr,
pub GetSamplerParameterfv: FnPtr,
pub GetSamplerParameteriv: FnPtr,
pub GetShaderInfoLog: FnPtr,
pub GetShaderPrecisionFormat: FnPtr,
/// Fallbacks: GetShaderSourceARB
pub GetShaderSource: FnPtr,
pub GetShaderiv: FnPtr,
pub GetString: FnPtr,
pub GetStringi: FnPtr,
/// Fallbacks: GetSyncivAPPLE
pub GetSynciv: FnPtr,
pub GetTexEnvfv: FnPtr,
pub GetTexEnviv: FnPtr,
pub GetTexGendv: FnPtr,
pub GetTexGenfv: FnPtr,
pub GetTexGeniv: FnPtr,
pub GetTexImage: FnPtr,
pub GetTexLevelParameterfv: FnPtr,
pub GetTexLevelParameteriv: FnPtr,
/// Fallbacks: GetTexParameterIivEXT, GetTexParameterIivOES
pub GetTexParameterIiv: FnPtr,
/// Fallbacks: GetTexParameterIuivEXT, GetTexParameterIuivOES
pub GetTexParameterIuiv: FnPtr,
pub GetTexParameterPointervAPPLE: FnPtr,
pub GetTexParameterfv: FnPtr,
pub GetTexParameteriv: FnPtr,
/// Fallbacks: GetTransformFeedbackVaryingEXT
pub GetTransformFeedbackVarying: FnPtr,
pub GetUniformBlockIndex: FnPtr,
pub GetUniformIndices: FnPtr,
/// Fallbacks: GetUniformLocationARB
pub GetUniformLocation: FnPtr,
/// Fallbacks: GetUniformfvARB
pub GetUniformfv: FnPtr,
/// Fallbacks: GetUniformivARB
pub GetUniformiv: FnPtr,
/// Fallbacks: GetUniformuivEXT
pub GetUniformuiv: FnPtr,
/// Fallbacks: GetVertexAttribIivEXT
pub GetVertexAttribIiv: FnPtr,
/// Fallbacks: GetVertexAttribIuivEXT
pub GetVertexAttribIuiv: FnPtr,
/// Fallbacks: GetVertexAttribPointervARB, GetVertexAttribPointervNV
pub GetVertexAttribPointerv: FnPtr,
/// Fallbacks: GetVertexAttribdvARB, GetVertexAttribdvNV
pub GetVertexAttribdv: FnPtr,
/// Fallbacks: GetVertexAttribfvARB, GetVertexAttribfvNV
pub GetVertexAttribfv: FnPtr,
/// Fallbacks: GetVertexAttribivARB, GetVertexAttribivNV
pub GetVertexAttribiv: FnPtr,
pub Hint: FnPtr,
pub IndexMask: FnPtr,
pub IndexPointer: FnPtr,
pub Indexd: FnPtr,
pub Indexdv: FnPtr,
pub Indexf: FnPtr,
pub Indexfv: FnPtr,
pub Indexi: FnPtr,
pub Indexiv: FnPtr,
pub Indexs: FnPtr,
pub Indexsv: FnPtr,
pub Indexub: FnPtr,
pub Indexubv: FnPtr,
pub InitNames: FnPtr,
pub InsertEventMarkerEXT: FnPtr,
pub InterleavedArrays: FnPtr,
pub InvalidateBufferData: FnPtr,
pub InvalidateBufferSubData: FnPtr,
pub InvalidateFramebuffer: FnPtr,
pub InvalidateSubFramebuffer: FnPtr,
pub InvalidateTexImage: FnPtr,
pub InvalidateTexSubImage: FnPtr,
/// Fallbacks: IsBufferARB
pub IsBuffer: FnPtr,
pub IsEnabled: FnPtr,
/// Fallbacks: IsEnabledIndexedEXT, IsEnablediEXT, IsEnablediNV, IsEnablediOES
pub IsEnabledi: FnPtr,
pub IsFenceAPPLE: FnPtr,
/// Fallbacks: IsFramebufferEXT
pub IsFramebuffer: FnPtr,
pub IsList: FnPtr,
pub IsProgram: FnPtr,
pub IsProgramPipeline: FnPtr,
/// Fallbacks: IsQueryARB
pub IsQuery: FnPtr,
pub IsQueryEXT: FnPtr,
/// Fallbacks: IsRenderbufferEXT
pub IsRenderbuffer: FnPtr,
pub IsSampler: FnPtr,
pub IsShader: FnPtr,
/// Fallbacks: IsSyncAPPLE
pub IsSync: FnPtr,
pub IsTexture: FnPtr,
/// Fallbacks: IsTransformFeedbackNV
pub IsTransformFeedback: FnPtr,
/// Fallbacks: IsVertexArrayAPPLE, IsVertexArrayOES
pub IsVertexArray: FnPtr,
pub IsVertexArrayAPPLE: FnPtr,
pub LightModelf: FnPtr,
pub LightModelfv: FnPtr,
pub LightModeli: FnPtr,
pub LightModeliv: FnPtr,
pub Lightf: FnPtr,
pub Lightfv: FnPtr,
pub Lighti: FnPtr,
pub Lightiv: FnPtr,
pub LineStipple: FnPtr,
pub LineWidth: FnPtr,
/// Fallbacks: LinkProgramARB
pub LinkProgram: FnPtr,
pub ListBase: FnPtr,
pub LoadIdentity: FnPtr,
pub LoadMatrixd: FnPtr,
pub LoadMatrixf: FnPtr,
pub LoadName: FnPtr,
/// Fallbacks: LoadTransposeMatrixdARB
pub LoadTransposeMatrixd: FnPtr,
/// Fallbacks: LoadTransposeMatrixfARB
pub LoadTransposeMatrixf: FnPtr,
pub LogicOp: FnPtr,
pub Map1d: FnPtr,
pub Map1f: FnPtr,
pub Map2d: FnPtr,
pub Map2f: FnPtr,
/// Fallbacks: MapBufferARB, MapBufferOES
pub MapBuffer: FnPtr,
/// Fallbacks: MapBufferRangeEXT
pub MapBufferRange: FnPtr,
pub MapGrid1d: FnPtr,
pub MapGrid1f: FnPtr,
pub MapGrid2d: FnPtr,
pub MapGrid2f: FnPtr,
pub Materialf: FnPtr,
pub Materialfv: FnPtr,
pub Materiali: FnPtr,
pub Materialiv: FnPtr,
pub MatrixMode: FnPtr,
/// Fallbacks: MemoryBarrierEXT
pub MemoryBarrier: FnPtr,
pub MemoryBarrierByRegion: FnPtr,
pub MultMatrixd: FnPtr,
pub MultMatrixf: FnPtr,
/// Fallbacks: MultTransposeMatrixdARB
pub MultTransposeMatrixd: FnPtr,
/// Fallbacks: MultTransposeMatrixfARB
pub MultTransposeMatrixf: FnPtr,
/// Fallbacks: MultiDrawArraysEXT
pub MultiDrawArrays: FnPtr,
/// Fallbacks: MultiDrawElementsEXT
pub MultiDrawElements: FnPtr,
/// Fallbacks: MultiDrawElementsBaseVertexEXT
pub MultiDrawElementsBaseVertex: FnPtr,
/// Fallbacks: MultiTexCoord1dARB
pub MultiTexCoord1d: FnPtr,
/// Fallbacks: MultiTexCoord1dvARB
pub MultiTexCoord1dv: FnPtr,
/// Fallbacks: MultiTexCoord1fARB
pub MultiTexCoord1f: FnPtr,
/// Fallbacks: MultiTexCoord1fvARB
pub MultiTexCoord1fv: FnPtr,
/// Fallbacks: MultiTexCoord1iARB
pub MultiTexCoord1i: FnPtr,
/// Fallbacks: MultiTexCoord1ivARB
pub MultiTexCoord1iv: FnPtr,
/// Fallbacks: MultiTexCoord1sARB
pub MultiTexCoord1s: FnPtr,
/// Fallbacks: MultiTexCoord1svARB
pub MultiTexCoord1sv: FnPtr,
/// Fallbacks: MultiTexCoord2dARB
pub MultiTexCoord2d: FnPtr,
/// Fallbacks: MultiTexCoord2dvARB
pub MultiTexCoord2dv: FnPtr,
/// Fallbacks: MultiTexCoord2fARB
pub MultiTexCoord2f: FnPtr,
/// Fallbacks: MultiTexCoord2fvARB
pub MultiTexCoord2fv: FnPtr,
/// Fallbacks: MultiTexCoord2iARB
pub MultiTexCoord2i: FnPtr,
/// Fallbacks: MultiTexCoord2ivARB
pub MultiTexCoord2iv: FnPtr,
/// Fallbacks: MultiTexCoord2sARB
pub MultiTexCoord2s: FnPtr,
/// Fallbacks: MultiTexCoord2svARB
pub MultiTexCoord2sv: FnPtr,
/// Fallbacks: MultiTexCoord3dARB
pub MultiTexCoord3d: FnPtr,
/// Fallbacks: MultiTexCoord3dvARB
pub MultiTexCoord3dv: FnPtr,
/// Fallbacks: MultiTexCoord3fARB
pub MultiTexCoord3f: FnPtr,
/// Fallbacks: MultiTexCoord3fvARB
pub MultiTexCoord3fv: FnPtr,
/// Fallbacks: MultiTexCoord3iARB
pub MultiTexCoord3i: FnPtr,
/// Fallbacks: MultiTexCoord3ivARB
pub MultiTexCoord3iv: FnPtr,
/// Fallbacks: MultiTexCoord3sARB
pub MultiTexCoord3s: FnPtr,
/// Fallbacks: MultiTexCoord3svARB
pub MultiTexCoord3sv: FnPtr,
/// Fallbacks: MultiTexCoord4dARB
pub MultiTexCoord4d: FnPtr,
/// Fallbacks: MultiTexCoord4dvARB
pub MultiTexCoord4dv: FnPtr,
/// Fallbacks: MultiTexCoord4fARB
pub MultiTexCoord4f: FnPtr,
/// Fallbacks: MultiTexCoord4fvARB
pub MultiTexCoord4fv: FnPtr,
/// Fallbacks: MultiTexCoord4iARB
pub MultiTexCoord4i: FnPtr,
/// Fallbacks: MultiTexCoord4ivARB
pub MultiTexCoord4iv: FnPtr,
/// Fallbacks: MultiTexCoord4sARB
pub MultiTexCoord4s: FnPtr,
/// Fallbacks: MultiTexCoord4svARB
pub MultiTexCoord4sv: FnPtr,
pub MultiTexCoordP1ui: FnPtr,
pub MultiTexCoordP1uiv: FnPtr,
pub MultiTexCoordP2ui: FnPtr,
pub MultiTexCoordP2uiv: FnPtr,
pub MultiTexCoordP3ui: FnPtr,
pub MultiTexCoordP3uiv: FnPtr,
pub MultiTexCoordP4ui: FnPtr,
pub MultiTexCoordP4uiv: FnPtr,
pub NewList: FnPtr,
pub Normal3b: FnPtr,
pub Normal3bv: FnPtr,
pub Normal3d: FnPtr,
pub Normal3dv: FnPtr,
pub Normal3f: FnPtr,
pub Normal3fv: FnPtr,
pub Normal3i: FnPtr,
pub Normal3iv: FnPtr,
pub Normal3s: FnPtr,
pub Normal3sv: FnPtr,
pub NormalP3ui: FnPtr,
pub NormalP3uiv: FnPtr,
pub NormalPointer: FnPtr,
/// Fallbacks: ObjectLabelKHR
pub ObjectLabel: FnPtr,
pub ObjectLabelKHR: FnPtr,
/// Fallbacks: ObjectPtrLabelKHR
pub ObjectPtrLabel: FnPtr,
pub ObjectPtrLabelKHR: FnPtr,
pub Ortho: FnPtr,
pub PassThrough: FnPtr,
/// Fallbacks: PauseTransformFeedbackNV
pub PauseTransformFeedback: FnPtr,
pub PixelMapfv: FnPtr,
pub PixelMapuiv: FnPtr,
pub PixelMapusv: FnPtr,
pub PixelStoref: FnPtr,
pub PixelStorei: FnPtr,
pub PixelTransferf: FnPtr,
pub PixelTransferi: FnPtr,
pub PixelZoom: FnPtr,
/// Fallbacks: PointParameterfARB, PointParameterfEXT, PointParameterfSGIS
pub PointParameterf: FnPtr,
/// Fallbacks: PointParameterfvARB, PointParameterfvEXT, PointParameterfvSGIS
pub PointParameterfv: FnPtr,
/// Fallbacks: PointParameteriNV
pub PointParameteri: FnPtr,
/// Fallbacks: PointParameterivNV
pub PointParameteriv: FnPtr,
pub PointSize: FnPtr,
/// Fallbacks: PolygonModeNV
pub PolygonMode: FnPtr,
pub PolygonOffset: FnPtr,
pub PolygonStipple: FnPtr,
pub PopAttrib: FnPtr,
pub PopClientAttrib: FnPtr,
/// Fallbacks: PopDebugGroupKHR
pub PopDebugGroup: FnPtr,
pub PopDebugGroupKHR: FnPtr,
pub PopGroupMarkerEXT: FnPtr,
pub PopMatrix: FnPtr,
pub PopName: FnPtr,
pub PrimitiveRestartIndex: FnPtr,
/// Fallbacks: PrioritizeTexturesEXT
pub PrioritizeTextures: FnPtr,
/// Fallbacks: ProgramBinaryOES
pub ProgramBinary: FnPtr,
/// Fallbacks: ProgramParameteriARB, ProgramParameteriEXT
pub ProgramParameteri: FnPtr,
/// Fallbacks: ProgramUniform1fEXT
pub ProgramUniform1f: FnPtr,
/// Fallbacks: ProgramUniform1fvEXT
pub ProgramUniform1fv: FnPtr,
/// Fallbacks: ProgramUniform1iEXT
pub ProgramUniform1i: FnPtr,
/// Fallbacks: ProgramUniform1ivEXT
pub ProgramUniform1iv: FnPtr,
/// Fallbacks: ProgramUniform1uiEXT
pub ProgramUniform1ui: FnPtr,
/// Fallbacks: ProgramUniform1uivEXT
pub ProgramUniform1uiv: FnPtr,
/// Fallbacks: ProgramUniform2fEXT
pub ProgramUniform2f: FnPtr,
/// Fallbacks: ProgramUniform2fvEXT
pub ProgramUniform2fv: FnPtr,
/// Fallbacks: ProgramUniform2iEXT
pub ProgramUniform2i: FnPtr,
/// Fallbacks: ProgramUniform2ivEXT
pub ProgramUniform2iv: FnPtr,
/// Fallbacks: ProgramUniform2uiEXT
pub ProgramUniform2ui: FnPtr,
/// Fallbacks: ProgramUniform2uivEXT
pub ProgramUniform2uiv: FnPtr,
/// Fallbacks: ProgramUniform3fEXT
pub ProgramUniform3f: FnPtr,
/// Fallbacks: ProgramUniform3fvEXT
pub ProgramUniform3fv: FnPtr,
/// Fallbacks: ProgramUniform3iEXT
pub ProgramUniform3i: FnPtr,
/// Fallbacks: ProgramUniform3ivEXT
pub ProgramUniform3iv: FnPtr,
/// Fallbacks: ProgramUniform3uiEXT
pub ProgramUniform3ui: FnPtr,
/// Fallbacks: ProgramUniform3uivEXT
pub ProgramUniform3uiv: FnPtr,
/// Fallbacks: ProgramUniform4fEXT
pub ProgramUniform4f: FnPtr,
/// Fallbacks: ProgramUniform4fvEXT
pub ProgramUniform4fv: FnPtr,
/// Fallbacks: ProgramUniform4iEXT
pub ProgramUniform4i: FnPtr,
/// Fallbacks: ProgramUniform4ivEXT
pub ProgramUniform4iv: FnPtr,
/// Fallbacks: ProgramUniform4uiEXT
pub ProgramUniform4ui: FnPtr,
/// Fallbacks: ProgramUniform4uivEXT
pub ProgramUniform4uiv: FnPtr,
/// Fallbacks: ProgramUniformMatrix2fvEXT
pub ProgramUniformMatrix2fv: FnPtr,
/// Fallbacks: ProgramUniformMatrix2x3fvEXT
pub ProgramUniformMatrix2x3fv: FnPtr,
/// Fallbacks: ProgramUniformMatrix2x4fvEXT
pub ProgramUniformMatrix2x4fv: FnPtr,
/// Fallbacks: ProgramUniformMatrix3fvEXT
pub ProgramUniformMatrix3fv: FnPtr,
/// Fallbacks: ProgramUniformMatrix3x2fvEXT
pub ProgramUniformMatrix3x2fv: FnPtr,
/// Fallbacks: ProgramUniformMatrix3x4fvEXT
pub ProgramUniformMatrix3x4fv: FnPtr,
/// Fallbacks: ProgramUniformMatrix4fvEXT
pub ProgramUniformMatrix4fv: FnPtr,
/// Fallbacks: ProgramUniformMatrix4x2fvEXT
pub ProgramUniformMatrix4x2fv: FnPtr,
/// Fallbacks: ProgramUniformMatrix4x3fvEXT
pub ProgramUniformMatrix4x3fv: FnPtr,
/// Fallbacks: ProvokingVertexEXT
pub ProvokingVertex: FnPtr,
pub ProvokingVertexANGLE: FnPtr,
pub PushAttrib: FnPtr,
pub PushClientAttrib: FnPtr,
/// Fallbacks: PushDebugGroupKHR
pub PushDebugGroup: FnPtr,
pub PushDebugGroupKHR: FnPtr,
pub PushGroupMarkerEXT: FnPtr,
pub PushMatrix: FnPtr,
pub PushName: FnPtr,
/// Fallbacks: QueryCounterEXT
pub QueryCounter: FnPtr,
pub QueryCounterEXT: FnPtr,
pub RasterPos2d: FnPtr,
pub RasterPos2dv: FnPtr,
pub RasterPos2f: FnPtr,
pub RasterPos2fv: FnPtr,
pub RasterPos2i: FnPtr,
pub RasterPos2iv: FnPtr,
pub RasterPos2s: FnPtr,
pub RasterPos2sv: FnPtr,
pub RasterPos3d: FnPtr,
pub RasterPos3dv: FnPtr,
pub RasterPos3f: FnPtr,
pub RasterPos3fv: FnPtr,
pub RasterPos3i: FnPtr,
pub RasterPos3iv: FnPtr,
pub RasterPos3s: FnPtr,
pub RasterPos3sv: FnPtr,
pub RasterPos4d: FnPtr,
pub RasterPos4dv: FnPtr,
pub RasterPos4f: FnPtr,
pub RasterPos4fv: FnPtr,
pub RasterPos4i: FnPtr,
pub RasterPos4iv: FnPtr,
pub RasterPos4s: FnPtr,
pub RasterPos4sv: FnPtr,
pub ReadBuffer: FnPtr,
pub ReadPixels: FnPtr,
pub Rectd: FnPtr,
pub Rectdv: FnPtr,
pub Rectf: FnPtr,
pub Rectfv: FnPtr,
pub Recti: FnPtr,
pub Rectiv: FnPtr,
pub Rects: FnPtr,
pub Rectsv: FnPtr,
pub ReleaseShaderCompiler: FnPtr,
pub RenderMode: FnPtr,
/// Fallbacks: RenderbufferStorageEXT
pub RenderbufferStorage: FnPtr,
/// Fallbacks: RenderbufferStorageMultisampleEXT, RenderbufferStorageMultisampleNV
pub RenderbufferStorageMultisample: FnPtr,
/// Fallbacks: ResumeTransformFeedbackNV
pub ResumeTransformFeedback: FnPtr,
pub Rotated: FnPtr,
pub Rotatef: FnPtr,
/// Fallbacks: SampleCoverageARB
pub SampleCoverage: FnPtr,
pub SampleMaski: FnPtr,
/// Fallbacks: SamplerParameterIivEXT, SamplerParameterIivOES
pub SamplerParameterIiv: FnPtr,
/// Fallbacks: SamplerParameterIuivEXT, SamplerParameterIuivOES
pub SamplerParameterIuiv: FnPtr,
pub SamplerParameterf: FnPtr,
pub SamplerParameterfv: FnPtr,
pub SamplerParameteri: FnPtr,
pub SamplerParameteriv: FnPtr,
pub Scaled: FnPtr,
pub Scalef: FnPtr,
pub Scissor: FnPtr,
/// Fallbacks: SecondaryColor3bEXT
pub SecondaryColor3b: FnPtr,
/// Fallbacks: SecondaryColor3bvEXT
pub SecondaryColor3bv: FnPtr,
/// Fallbacks: SecondaryColor3dEXT
pub SecondaryColor3d: FnPtr,
/// Fallbacks: SecondaryColor3dvEXT
pub SecondaryColor3dv: FnPtr,
/// Fallbacks: SecondaryColor3fEXT
pub SecondaryColor3f: FnPtr,
/// Fallbacks: SecondaryColor3fvEXT
pub SecondaryColor3fv: FnPtr,
/// Fallbacks: SecondaryColor3iEXT
pub SecondaryColor3i: FnPtr,
/// Fallbacks: SecondaryColor3ivEXT
pub SecondaryColor3iv: FnPtr,
/// Fallbacks: SecondaryColor3sEXT
pub SecondaryColor3s: FnPtr,
/// Fallbacks: SecondaryColor3svEXT
pub SecondaryColor3sv: FnPtr,
/// Fallbacks: SecondaryColor3ubEXT
pub SecondaryColor3ub: FnPtr,
/// Fallbacks: SecondaryColor3ubvEXT
pub SecondaryColor3ubv: FnPtr,
/// Fallbacks: SecondaryColor3uiEXT
pub SecondaryColor3ui: FnPtr,
/// Fallbacks: SecondaryColor3uivEXT
pub SecondaryColor3uiv: FnPtr,
/// Fallbacks: SecondaryColor3usEXT
pub SecondaryColor3us: FnPtr,
/// Fallbacks: SecondaryColor3usvEXT
pub SecondaryColor3usv: FnPtr,
pub SecondaryColorP3ui: FnPtr,
pub SecondaryColorP3uiv: FnPtr,
/// Fallbacks: SecondaryColorPointerEXT
pub SecondaryColorPointer: FnPtr,
pub SelectBuffer: FnPtr,
pub SetFenceAPPLE: FnPtr,
pub ShadeModel: FnPtr,
pub ShaderBinary: FnPtr,
/// Fallbacks: ShaderSourceARB
pub ShaderSource: FnPtr,
pub ShaderStorageBlockBinding: FnPtr,
pub StencilFunc: FnPtr,
pub StencilFuncSeparate: FnPtr,
pub StencilMask: FnPtr,
pub StencilMaskSeparate: FnPtr,
pub StencilOp: FnPtr,
/// Fallbacks: StencilOpSeparateATI
pub StencilOpSeparate: FnPtr,
pub TestFenceAPPLE: FnPtr,
pub TestObjectAPPLE: FnPtr,
/// Fallbacks: TexBufferARB, TexBufferEXT, TexBufferOES
pub TexBuffer: FnPtr,
pub TexCoord1d: FnPtr,
pub TexCoord1dv: FnPtr,
pub TexCoord1f: FnPtr,
pub TexCoord1fv: FnPtr,
pub TexCoord1i: FnPtr,
pub TexCoord1iv: FnPtr,
pub TexCoord1s: FnPtr,
pub TexCoord1sv: FnPtr,
pub TexCoord2d: FnPtr,
pub TexCoord2dv: FnPtr,
pub TexCoord2f: FnPtr,
pub TexCoord2fv: FnPtr,
pub TexCoord2i: FnPtr,
pub TexCoord2iv: FnPtr,
pub TexCoord2s: FnPtr,
pub TexCoord2sv: FnPtr,
pub TexCoord3d: FnPtr,
pub TexCoord3dv: FnPtr,
pub TexCoord3f: FnPtr,
pub TexCoord3fv: FnPtr,
pub TexCoord3i: FnPtr,
pub TexCoord3iv: FnPtr,
pub TexCoord3s: FnPtr,
pub TexCoord3sv: FnPtr,
pub TexCoord4d: FnPtr,
pub TexCoord4dv: FnPtr,
pub TexCoord4f: FnPtr,
pub TexCoord4fv: FnPtr,
pub TexCoord4i: FnPtr,
pub TexCoord4iv: FnPtr,
pub TexCoord4s: FnPtr,
pub TexCoord4sv: FnPtr,
pub TexCoordP1ui: FnPtr,
pub TexCoordP1uiv: FnPtr,
pub TexCoordP2ui: FnPtr,
pub TexCoordP2uiv: FnPtr,
pub TexCoordP3ui: FnPtr,
pub TexCoordP3uiv: FnPtr,
pub TexCoordP4ui: FnPtr,
pub TexCoordP4uiv: FnPtr,
pub TexCoordPointer: FnPtr,
pub TexEnvf: FnPtr,
pub TexEnvfv: FnPtr,
pub TexEnvi: FnPtr,
pub TexEnviv: FnPtr,
pub TexGend: FnPtr,
pub TexGendv: FnPtr,
pub TexGenf: FnPtr,
pub TexGenfv: FnPtr,
pub TexGeni: FnPtr,
pub TexGeniv: FnPtr,
pub TexImage1D: FnPtr,
pub TexImage2D: FnPtr,
pub TexImage2DMultisample: FnPtr,
/// Fallbacks: TexImage3DEXT
pub TexImage3D: FnPtr,
pub TexImage3DMultisample: FnPtr,
/// Fallbacks: TexParameterIivEXT, TexParameterIivOES
pub TexParameterIiv: FnPtr,
/// Fallbacks: TexParameterIuivEXT, TexParameterIuivOES
pub TexParameterIuiv: FnPtr,
pub TexParameterf: FnPtr,
pub TexParameterfv: FnPtr,
pub TexParameteri: FnPtr,
pub TexParameteriv: FnPtr,
/// Fallbacks: TexStorage1DEXT
pub TexStorage1D: FnPtr,
pub TexStorage1DEXT: FnPtr,
/// Fallbacks: TexStorage2DEXT
pub TexStorage2D: FnPtr,
pub TexStorage2DEXT: FnPtr,
pub TexStorage2DMultisample: FnPtr,
/// Fallbacks: TexStorage3DEXT
pub TexStorage3D: FnPtr,
pub TexStorage3DEXT: FnPtr,
/// Fallbacks: TexSubImage1DEXT
pub TexSubImage1D: FnPtr,
/// Fallbacks: TexSubImage2DEXT
pub TexSubImage2D: FnPtr,
/// Fallbacks: TexSubImage3DEXT
pub TexSubImage3D: FnPtr,
pub TextureRangeAPPLE: FnPtr,
pub TextureStorage1DEXT: FnPtr,
pub TextureStorage2DEXT: FnPtr,
pub TextureStorage3DEXT: FnPtr,
/// Fallbacks: TransformFeedbackVaryingsEXT
pub TransformFeedbackVaryings: FnPtr,
pub Translated: FnPtr,
pub Translatef: FnPtr,
/// Fallbacks: Uniform1fARB
pub Uniform1f: FnPtr,
/// Fallbacks: Uniform1fvARB
pub Uniform1fv: FnPtr,
/// Fallbacks: Uniform1iARB
pub Uniform1i: FnPtr,
/// Fallbacks: Uniform1ivARB
pub Uniform1iv: FnPtr,
/// Fallbacks: Uniform1uiEXT
pub Uniform1ui: FnPtr,
/// Fallbacks: Uniform1uivEXT
pub Uniform1uiv: FnPtr,
/// Fallbacks: Uniform2fARB
pub Uniform2f: FnPtr,
/// Fallbacks: Uniform2fvARB
pub Uniform2fv: FnPtr,
/// Fallbacks: Uniform2iARB
pub Uniform2i: FnPtr,
/// Fallbacks: Uniform2ivARB
pub Uniform2iv: FnPtr,
/// Fallbacks: Uniform2uiEXT
pub Uniform2ui: FnPtr,
/// Fallbacks: Uniform2uivEXT
pub Uniform2uiv: FnPtr,
/// Fallbacks: Uniform3fARB
pub Uniform3f: FnPtr,
/// Fallbacks: Uniform3fvARB
pub Uniform3fv: FnPtr,
/// Fallbacks: Uniform3iARB
pub Uniform3i: FnPtr,
/// Fallbacks: Uniform3ivARB
pub Uniform3iv: FnPtr,
/// Fallbacks: Uniform3uiEXT
pub Uniform3ui: FnPtr,
/// Fallbacks: Uniform3uivEXT
pub Uniform3uiv: FnPtr,
/// Fallbacks: Uniform4fARB
pub Uniform4f: FnPtr,
/// Fallbacks: Uniform4fvARB
pub Uniform4fv: FnPtr,
/// Fallbacks: Uniform4iARB
pub Uniform4i: FnPtr,
/// Fallbacks: Uniform4ivARB
pub Uniform4iv: FnPtr,
/// Fallbacks: Uniform4uiEXT
pub Uniform4ui: FnPtr,
/// Fallbacks: Uniform4uivEXT
pub Uniform4uiv: FnPtr,
pub UniformBlockBinding: FnPtr,
/// Fallbacks: UniformMatrix2fvARB
pub UniformMatrix2fv: FnPtr,
/// Fallbacks: UniformMatrix2x3fvNV
pub UniformMatrix2x3fv: FnPtr,
/// Fallbacks: UniformMatrix2x4fvNV
pub UniformMatrix2x4fv: FnPtr,
/// Fallbacks: UniformMatrix3fvARB
pub UniformMatrix3fv: FnPtr,
/// Fallbacks: UniformMatrix3x2fvNV
pub UniformMatrix3x2fv: FnPtr,
/// Fallbacks: UniformMatrix3x4fvNV
pub UniformMatrix3x4fv: FnPtr,
/// Fallbacks: UniformMatrix4fvARB
pub UniformMatrix4fv: FnPtr,
/// Fallbacks: UniformMatrix4x2fvNV
pub UniformMatrix4x2fv: FnPtr,
/// Fallbacks: UniformMatrix4x3fvNV
pub UniformMatrix4x3fv: FnPtr,
/// Fallbacks: UnmapBufferARB, UnmapBufferOES
pub UnmapBuffer: FnPtr,
/// Fallbacks: UseProgramObjectARB
pub UseProgram: FnPtr,
pub UseProgramStages: FnPtr,
/// Fallbacks: ValidateProgramARB
pub ValidateProgram: FnPtr,
pub ValidateProgramPipeline: FnPtr,
pub Vertex2d: FnPtr,
pub Vertex2dv: FnPtr,
pub Vertex2f: FnPtr,
pub Vertex2fv: FnPtr,
pub Vertex2i: FnPtr,
pub Vertex2iv: FnPtr,
pub Vertex2s: FnPtr,
pub Vertex2sv: FnPtr,
pub Vertex3d: FnPtr,
pub Vertex3dv: FnPtr,
pub Vertex3f: FnPtr,
pub Vertex3fv: FnPtr,
pub Vertex3i: FnPtr,
pub Vertex3iv: FnPtr,
pub Vertex3s: FnPtr,
pub Vertex3sv: FnPtr,
pub Vertex4d: FnPtr,
pub Vertex4dv: FnPtr,
pub Vertex4f: FnPtr,
pub Vertex4fv: FnPtr,
pub Vertex4i: FnPtr,
pub Vertex4iv: FnPtr,
pub Vertex4s: FnPtr,
pub Vertex4sv: FnPtr,
/// Fallbacks: VertexAttrib1dARB, VertexAttrib1dNV
pub VertexAttrib1d: FnPtr,
/// Fallbacks: VertexAttrib1dvARB, VertexAttrib1dvNV
pub VertexAttrib1dv: FnPtr,
/// Fallbacks: VertexAttrib1fARB, VertexAttrib1fNV
pub VertexAttrib1f: FnPtr,
/// Fallbacks: VertexAttrib1fvARB, VertexAttrib1fvNV
pub VertexAttrib1fv: FnPtr,
/// Fallbacks: VertexAttrib1sARB, VertexAttrib1sNV
pub VertexAttrib1s: FnPtr,
/// Fallbacks: VertexAttrib1svARB, VertexAttrib1svNV
pub VertexAttrib1sv: FnPtr,
/// Fallbacks: VertexAttrib2dARB, VertexAttrib2dNV
pub VertexAttrib2d: FnPtr,
/// Fallbacks: VertexAttrib2dvARB, VertexAttrib2dvNV
pub VertexAttrib2dv: FnPtr,
/// Fallbacks: VertexAttrib2fARB, VertexAttrib2fNV
pub VertexAttrib2f: FnPtr,
/// Fallbacks: VertexAttrib2fvARB, VertexAttrib2fvNV
pub VertexAttrib2fv: FnPtr,
/// Fallbacks: VertexAttrib2sARB, VertexAttrib2sNV
pub VertexAttrib2s: FnPtr,
/// Fallbacks: VertexAttrib2svARB, VertexAttrib2svNV
pub VertexAttrib2sv: FnPtr,
/// Fallbacks: VertexAttrib3dARB, VertexAttrib3dNV
pub VertexAttrib3d: FnPtr,
/// Fallbacks: VertexAttrib3dvARB, VertexAttrib3dvNV
pub VertexAttrib3dv: FnPtr,
/// Fallbacks: VertexAttrib3fARB, VertexAttrib3fNV
pub VertexAttrib3f: FnPtr,
/// Fallbacks: VertexAttrib3fvARB, VertexAttrib3fvNV
pub VertexAttrib3fv: FnPtr,
/// Fallbacks: VertexAttrib3sARB, VertexAttrib3sNV
pub VertexAttrib3s: FnPtr,
/// Fallbacks: VertexAttrib3svARB, VertexAttrib3svNV
pub VertexAttrib3sv: FnPtr,
/// Fallbacks: VertexAttrib4NbvARB
pub VertexAttrib4Nbv: FnPtr,
/// Fallbacks: VertexAttrib4NivARB
pub VertexAttrib4Niv: FnPtr,
/// Fallbacks: VertexAttrib4NsvARB
pub VertexAttrib4Nsv: FnPtr,
/// Fallbacks: VertexAttrib4NubARB, VertexAttrib4ubNV
pub VertexAttrib4Nub: FnPtr,
/// Fallbacks: VertexAttrib4NubvARB, VertexAttrib4ubvNV
pub VertexAttrib4Nubv: FnPtr,
/// Fallbacks: VertexAttrib4NuivARB
pub VertexAttrib4Nuiv: FnPtr,
/// Fallbacks: VertexAttrib4NusvARB
pub VertexAttrib4Nusv: FnPtr,
/// Fallbacks: VertexAttrib4bvARB
pub VertexAttrib4bv: FnPtr,
/// Fallbacks: VertexAttrib4dARB, VertexAttrib4dNV
pub VertexAttrib4d: FnPtr,
/// Fallbacks: VertexAttrib4dvARB, VertexAttrib4dvNV
pub VertexAttrib4dv: FnPtr,
/// Fallbacks: VertexAttrib4fARB, VertexAttrib4fNV
pub VertexAttrib4f: FnPtr,
/// Fallbacks: VertexAttrib4fvARB, VertexAttrib4fvNV
pub VertexAttrib4fv: FnPtr,
/// Fallbacks: VertexAttrib4ivARB
pub VertexAttrib4iv: FnPtr,
/// Fallbacks: VertexAttrib4sARB, VertexAttrib4sNV
pub VertexAttrib4s: FnPtr,
/// Fallbacks: VertexAttrib4svARB, VertexAttrib4svNV
pub VertexAttrib4sv: FnPtr,
/// Fallbacks: VertexAttrib4ubvARB
pub VertexAttrib4ubv: FnPtr,
/// Fallbacks: VertexAttrib4uivARB
pub VertexAttrib4uiv: FnPtr,
/// Fallbacks: VertexAttrib4usvARB
pub VertexAttrib4usv: FnPtr,
pub VertexAttribBinding: FnPtr,
/// Fallbacks: VertexAttribDivisorANGLE, VertexAttribDivisorARB, VertexAttribDivisorEXT, VertexAttribDivisorNV
pub VertexAttribDivisor: FnPtr,
pub VertexAttribFormat: FnPtr,
/// Fallbacks: VertexAttribI1iEXT
pub VertexAttribI1i: FnPtr,
/// Fallbacks: VertexAttribI1ivEXT
pub VertexAttribI1iv: FnPtr,
/// Fallbacks: VertexAttribI1uiEXT
pub VertexAttribI1ui: FnPtr,
/// Fallbacks: VertexAttribI1uivEXT
pub VertexAttribI1uiv: FnPtr,
/// Fallbacks: VertexAttribI2iEXT
pub VertexAttribI2i: FnPtr,
/// Fallbacks: VertexAttribI2ivEXT
pub VertexAttribI2iv: FnPtr,
/// Fallbacks: VertexAttribI2uiEXT
pub VertexAttribI2ui: FnPtr,
/// Fallbacks: VertexAttribI2uivEXT
pub VertexAttribI2uiv: FnPtr,
/// Fallbacks: VertexAttribI3iEXT
pub VertexAttribI3i: FnPtr,
/// Fallbacks: VertexAttribI3ivEXT
pub VertexAttribI3iv: FnPtr,
/// Fallbacks: VertexAttribI3uiEXT
pub VertexAttribI3ui: FnPtr,
/// Fallbacks: VertexAttribI3uivEXT
pub VertexAttribI3uiv: FnPtr,
/// Fallbacks: VertexAttribI4bvEXT
pub VertexAttribI4bv: FnPtr,
/// Fallbacks: VertexAttribI4iEXT
pub VertexAttribI4i: FnPtr,
/// Fallbacks: VertexAttribI4ivEXT
pub VertexAttribI4iv: FnPtr,
/// Fallbacks: VertexAttribI4svEXT
pub VertexAttribI4sv: FnPtr,
/// Fallbacks: VertexAttribI4ubvEXT
pub VertexAttribI4ubv: FnPtr,
/// Fallbacks: VertexAttribI4uiEXT
pub VertexAttribI4ui: FnPtr,
/// Fallbacks: VertexAttribI4uivEXT
pub VertexAttribI4uiv: FnPtr,
/// Fallbacks: VertexAttribI4usvEXT
pub VertexAttribI4usv: FnPtr,
pub VertexAttribIFormat: FnPtr,
/// Fallbacks: VertexAttribIPointerEXT
pub VertexAttribIPointer: FnPtr,
pub VertexAttribP1ui: FnPtr,
pub VertexAttribP1uiv: FnPtr,
pub VertexAttribP2ui: FnPtr,
pub VertexAttribP2uiv: FnPtr,
pub VertexAttribP3ui: FnPtr,
pub VertexAttribP3uiv: FnPtr,
pub VertexAttribP4ui: FnPtr,
pub VertexAttribP4uiv: FnPtr,
/// Fallbacks: VertexAttribPointerARB
pub VertexAttribPointer: FnPtr,
pub VertexBindingDivisor: FnPtr,
pub VertexP2ui: FnPtr,
pub VertexP2uiv: FnPtr,
pub VertexP3ui: FnPtr,
pub VertexP3uiv: FnPtr,
pub VertexP4ui: FnPtr,
pub VertexP4uiv: FnPtr,
pub VertexPointer: FnPtr,
pub Viewport: FnPtr,
/// Fallbacks: WaitSyncAPPLE
pub WaitSync: FnPtr,
/// Fallbacks: WindowPos2dARB, WindowPos2dMESA
pub WindowPos2d: FnPtr,
/// Fallbacks: WindowPos2dvARB, WindowPos2dvMESA
pub WindowPos2dv: FnPtr,
/// Fallbacks: WindowPos2fARB, WindowPos2fMESA
pub WindowPos2f: FnPtr,
/// Fallbacks: WindowPos2fvARB, WindowPos2fvMESA
pub WindowPos2fv: FnPtr,
/// Fallbacks: WindowPos2iARB, WindowPos2iMESA
pub WindowPos2i: FnPtr,
/// Fallbacks: WindowPos2ivARB, WindowPos2ivMESA
pub WindowPos2iv: FnPtr,
/// Fallbacks: WindowPos2sARB, WindowPos2sMESA
pub WindowPos2s: FnPtr,
/// Fallbacks: WindowPos2svARB, WindowPos2svMESA
pub WindowPos2sv: FnPtr,
/// Fallbacks: WindowPos3dARB, WindowPos3dMESA
pub WindowPos3d: FnPtr,
/// Fallbacks: WindowPos3dvARB, WindowPos3dvMESA
pub WindowPos3dv: FnPtr,
/// Fallbacks: WindowPos3fARB, WindowPos3fMESA
pub WindowPos3f: FnPtr,
/// Fallbacks: WindowPos3fvARB, WindowPos3fvMESA
pub WindowPos3fv: FnPtr,
/// Fallbacks: WindowPos3iARB, WindowPos3iMESA
pub WindowPos3i: FnPtr,
/// Fallbacks: WindowPos3ivARB, WindowPos3ivMESA
pub WindowPos3iv: FnPtr,
/// Fallbacks: WindowPos3sARB, WindowPos3sMESA
pub WindowPos3s: FnPtr,
/// Fallbacks: WindowPos3svARB, WindowPos3svMESA
pub WindowPos3sv: FnPtr,
_priv: ()
}
impl Gl {
            /// Load each OpenGL symbol using a custom load function. This allows for the
            /// use of functions like `glfwGetProcAddress` or `SDL_GL_GetProcAddress`.
            ///
            /// ~~~ignore
            /// let gl = Gl::load_with(|s| glfw.get_proc_address(s));
            /// ~~~
            #[allow(dead_code, unused_variables)]
            pub fn load_with<F>(mut loadfn: F) -> Gl where F: FnMut(&'static str) -> *const __gl_imports::raw::c_void {
                #[inline(never)]
                fn do_metaloadfn(loadfn: &mut dyn FnMut(&'static str) -> *const __gl_imports::raw::c_void,
                                 symbol: &'static str,
                                 symbols: &[&'static str])
                                 -> *const __gl_imports::raw::c_void {
                    let mut ptr = loadfn(symbol);
                    if ptr.is_null() {
                        for &sym in symbols {
                            ptr = loadfn(sym);
                            if !ptr.is_null() { break; }
                        }
                    }
                    ptr
                }
                let mut metaloadfn = |symbol: &'static str, symbols: &[&'static str]| {
                    do_metaloadfn(&mut loadfn, symbol, symbols)
                };
                Gl {
Accum: FnPtr::new(metaloadfn("glAccum", &[])),
ActiveShaderProgram: FnPtr::new(metaloadfn("glActiveShaderProgram", &[])),
ActiveTexture: FnPtr::new(metaloadfn("glActiveTexture", &["glActiveTextureARB"])),
AlphaFunc: FnPtr::new(metaloadfn("glAlphaFunc", &[])),
AreTexturesResident: FnPtr::new(metaloadfn("glAreTexturesResident", &[])),
ArrayElement: FnPtr::new(metaloadfn("glArrayElement", &["glArrayElementEXT"])),
AttachShader: FnPtr::new(metaloadfn("glAttachShader", &["glAttachObjectARB"])),
Begin: FnPtr::new(metaloadfn("glBegin", &[])),
BeginConditionalRender: FnPtr::new(metaloadfn("glBeginConditionalRender", &["glBeginConditionalRenderNV"])),
BeginQuery: FnPtr::new(metaloadfn("glBeginQuery", &["glBeginQueryARB"])),
BeginQueryEXT: FnPtr::new(metaloadfn("glBeginQueryEXT", &[])),
BeginTransformFeedback: FnPtr::new(metaloadfn("glBeginTransformFeedback", &["glBeginTransformFeedbackEXT", "glBeginTransformFeedbackNV"])),
BindAttribLocation: FnPtr::new(metaloadfn("glBindAttribLocation", &["glBindAttribLocationARB"])),
BindBuffer: FnPtr::new(metaloadfn("glBindBuffer", &["glBindBufferARB"])),
BindBufferBase: FnPtr::new(metaloadfn("glBindBufferBase", &["glBindBufferBaseEXT", "glBindBufferBaseNV"])),
BindBufferRange: FnPtr::new(metaloadfn("glBindBufferRange", &["glBindBufferRangeEXT", "glBindBufferRangeNV"])),
BindFragDataLocation: FnPtr::new(metaloadfn("glBindFragDataLocation", &["glBindFragDataLocationEXT"])),
BindFragDataLocationIndexed: FnPtr::new(metaloadfn("glBindFragDataLocationIndexed", &["glBindFragDataLocationIndexedEXT"])),
BindFramebuffer: FnPtr::new(metaloadfn("glBindFramebuffer", &[])),
BindImageTexture: FnPtr::new(metaloadfn("glBindImageTexture", &[])),
BindProgramPipeline: FnPtr::new(metaloadfn("glBindProgramPipeline", &[])),
BindRenderbuffer: FnPtr::new(metaloadfn("glBindRenderbuffer", &[])),
BindSampler: FnPtr::new(metaloadfn("glBindSampler", &[])),
BindTexture: FnPtr::new(metaloadfn("glBindTexture", &["glBindTextureEXT"])),
BindTransformFeedback: FnPtr::new(metaloadfn("glBindTransformFeedback", &[])),
BindVertexArray: FnPtr::new(metaloadfn("glBindVertexArray", &["glBindVertexArrayOES"])),
BindVertexArrayAPPLE: FnPtr::new(metaloadfn("glBindVertexArrayAPPLE", &[])),
BindVertexBuffer: FnPtr::new(metaloadfn("glBindVertexBuffer", &[])),
Bitmap: FnPtr::new(metaloadfn("glBitmap", &[])),
BlendBarrierKHR: FnPtr::new(metaloadfn("glBlendBarrierKHR", &[])),
BlendColor: FnPtr::new(metaloadfn("glBlendColor", &["glBlendColorEXT"])),
BlendEquation: FnPtr::new(metaloadfn("glBlendEquation", &["glBlendEquationEXT"])),
BlendEquationSeparate: FnPtr::new(metaloadfn("glBlendEquationSeparate", &["glBlendEquationSeparateEXT"])),
BlendFunc: FnPtr::new(metaloadfn("glBlendFunc", &[])),
BlendFuncSeparate: FnPtr::new(metaloadfn("glBlendFuncSeparate", &["glBlendFuncSeparateEXT", "glBlendFuncSeparateINGR"])),
BlitFramebuffer: FnPtr::new(metaloadfn("glBlitFramebuffer", &["glBlitFramebufferEXT", "glBlitFramebufferNV"])),
BufferData: FnPtr::new(metaloadfn("glBufferData", &["glBufferDataARB"])),
BufferStorage: FnPtr::new(metaloadfn("glBufferStorage", &["glBufferStorageEXT"])),
BufferStorageEXT: FnPtr::new(metaloadfn("glBufferStorageEXT", &[])),
BufferSubData: FnPtr::new(metaloadfn("glBufferSubData", &["glBufferSubDataARB"])),
CallList: FnPtr::new(metaloadfn("glCallList", &[])),
CallLists: FnPtr::new(metaloadfn("glCallLists", &[])),
CheckFramebufferStatus: FnPtr::new(metaloadfn("glCheckFramebufferStatus", &["glCheckFramebufferStatusEXT"])),
ClampColor: FnPtr::new(metaloadfn("glClampColor", &["glClampColorARB"])),
Clear: FnPtr::new(metaloadfn("glClear", &[])),
ClearAccum: FnPtr::new(metaloadfn("glClearAccum", &[])),
ClearBufferfi: FnPtr::new(metaloadfn("glClearBufferfi", &[])),
ClearBufferfv: FnPtr::new(metaloadfn("glClearBufferfv", &[])),
ClearBufferiv: FnPtr::new(metaloadfn("glClearBufferiv", &[])),
ClearBufferuiv: FnPtr::new(metaloadfn("glClearBufferuiv", &[])),
ClearColor: FnPtr::new(metaloadfn("glClearColor", &[])),
ClearDepth: FnPtr::new(metaloadfn("glClearDepth", &[])),
ClearDepthf: FnPtr::new(metaloadfn("glClearDepthf", &["glClearDepthfOES"])),
ClearIndex: FnPtr::new(metaloadfn("glClearIndex", &[])),
ClearStencil: FnPtr::new(metaloadfn("glClearStencil", &[])),
ClientActiveTexture: FnPtr::new(metaloadfn("glClientActiveTexture", &["glClientActiveTextureARB"])),
ClientWaitSync: FnPtr::new(metaloadfn("glClientWaitSync", &["glClientWaitSyncAPPLE"])),
ClipPlane: FnPtr::new(metaloadfn("glClipPlane", &[])),
Color3b: FnPtr::new(metaloadfn("glColor3b", &[])),
Color3bv: FnPtr::new(metaloadfn("glColor3bv", &[])),
Color3d: FnPtr::new(metaloadfn("glColor3d", &[])),
Color3dv: FnPtr::new(metaloadfn("glColor3dv", &[])),
Color3f: FnPtr::new(metaloadfn("glColor3f", &[])),
Color3fv: FnPtr::new(metaloadfn("glColor3fv", &[])),
Color3i: FnPtr::new(metaloadfn("glColor3i", &[])),
Color3iv: FnPtr::new(metaloadfn("glColor3iv", &[])),
Color3s: FnPtr::new(metaloadfn("glColor3s", &[])),
Color3sv: FnPtr::new(metaloadfn("glColor3sv", &[])),
Color3ub: FnPtr::new(metaloadfn("glColor3ub", &[])),
Color3ubv: FnPtr::new(metaloadfn("glColor3ubv", &[])),
Color3ui: FnPtr::new(metaloadfn("glColor3ui", &[])),
Color3uiv: FnPtr::new(metaloadfn("glColor3uiv", &[])),
Color3us: FnPtr::new(metaloadfn("glColor3us", &[])),
Color3usv: FnPtr::new(metaloadfn("glColor3usv", &[])),
Color4b: FnPtr::new(metaloadfn("glColor4b", &[])),
Color4bv: FnPtr::new(metaloadfn("glColor4bv", &[])),
Color4d: FnPtr::new(metaloadfn("glColor4d", &[])),
Color4dv: FnPtr::new(metaloadfn("glColor4dv", &[])),
Color4f: FnPtr::new(metaloadfn("glColor4f", &[])),
Color4fv: FnPtr::new(metaloadfn("glColor4fv", &[])),
Color4i: FnPtr::new(metaloadfn("glColor4i", &[])),
Color4iv: FnPtr::new(metaloadfn("glColor4iv", &[])),
Color4s: FnPtr::new(metaloadfn("glColor4s", &[])),
Color4sv: FnPtr::new(metaloadfn("glColor4sv", &[])),
Color4ub: FnPtr::new(metaloadfn("glColor4ub", &[])),
Color4ubv: FnPtr::new(metaloadfn("glColor4ubv", &[])),
Color4ui: FnPtr::new(metaloadfn("glColor4ui", &[])),
Color4uiv: FnPtr::new(metaloadfn("glColor4uiv", &[])),
Color4us: FnPtr::new(metaloadfn("glColor4us", &[])),
Color4usv: FnPtr::new(metaloadfn("glColor4usv", &[])),
ColorMask: FnPtr::new(metaloadfn("glColorMask", &[])),
ColorMaski: FnPtr::new(metaloadfn("glColorMaski", &["glColorMaskIndexedEXT", "glColorMaskiEXT", "glColorMaskiOES"])),
ColorMaterial: FnPtr::new(metaloadfn("glColorMaterial", &[])),
ColorP3ui: FnPtr::new(metaloadfn("glColorP3ui", &[])),
ColorP3uiv: FnPtr::new(metaloadfn("glColorP3uiv", &[])),
ColorP4ui: FnPtr::new(metaloadfn("glColorP4ui", &[])),
ColorP4uiv: FnPtr::new(metaloadfn("glColorP4uiv", &[])),
ColorPointer: FnPtr::new(metaloadfn("glColorPointer", &[])),
CompileShader: FnPtr::new(metaloadfn("glCompileShader", &["glCompileShaderARB"])),
CompressedTexImage1D: FnPtr::new(metaloadfn("glCompressedTexImage1D", &["glCompressedTexImage1DARB"])),
CompressedTexImage2D: FnPtr::new(metaloadfn("glCompressedTexImage2D", &["glCompressedTexImage2DARB"])),
CompressedTexImage3D: FnPtr::new(metaloadfn("glCompressedTexImage3D", &["glCompressedTexImage3DARB"])),
CompressedTexSubImage1D: FnPtr::new(metaloadfn("glCompressedTexSubImage1D", &["glCompressedTexSubImage1DARB"])),
CompressedTexSubImage2D: FnPtr::new(metaloadfn("glCompressedTexSubImage2D", &["glCompressedTexSubImage2DARB"])),
CompressedTexSubImage3D: FnPtr::new(metaloadfn("glCompressedTexSubImage3D", &["glCompressedTexSubImage3DARB"])),
CopyBufferSubData: FnPtr::new(metaloadfn("glCopyBufferSubData", &["glCopyBufferSubDataNV"])),
CopyImageSubData: FnPtr::new(metaloadfn("glCopyImageSubData", &["glCopyImageSubDataEXT", "glCopyImageSubDataOES"])),
CopyImageSubDataEXT: FnPtr::new(metaloadfn("glCopyImageSubDataEXT", &[])),
CopyPixels: FnPtr::new(metaloadfn("glCopyPixels", &[])),
CopySubTexture3DANGLE: FnPtr::new(metaloadfn("glCopySubTexture3DANGLE", &[])),
CopySubTextureCHROMIUM: FnPtr::new(metaloadfn("glCopySubTextureCHROMIUM", &[])),
CopyTexImage1D: FnPtr::new(metaloadfn("glCopyTexImage1D", &["glCopyTexImage1DEXT"])),
CopyTexImage2D: FnPtr::new(metaloadfn("glCopyTexImage2D", &["glCopyTexImage2DEXT"])),
CopyTexSubImage1D: FnPtr::new(metaloadfn("glCopyTexSubImage1D", &["glCopyTexSubImage1DEXT"])),
CopyTexSubImage2D: FnPtr::new(metaloadfn("glCopyTexSubImage2D", &["glCopyTexSubImage2DEXT"])),
CopyTexSubImage3D: FnPtr::new(metaloadfn("glCopyTexSubImage3D", &["glCopyTexSubImage3DEXT"])),
CopyTexture3DANGLE: FnPtr::new(metaloadfn("glCopyTexture3DANGLE", &[])),
CopyTextureCHROMIUM: FnPtr::new(metaloadfn("glCopyTextureCHROMIUM", &[])),
CreateProgram: FnPtr::new(metaloadfn("glCreateProgram", &["glCreateProgramObjectARB"])),
CreateShader: FnPtr::new(metaloadfn("glCreateShader", &["glCreateShaderObjectARB"])),
CreateShaderProgramv: FnPtr::new(metaloadfn("glCreateShaderProgramv", &[])),
CullFace: FnPtr::new(metaloadfn("glCullFace", &[])),
DebugMessageCallback: FnPtr::new(metaloadfn("glDebugMessageCallback", &["glDebugMessageCallbackARB", "glDebugMessageCallbackKHR"])),
DebugMessageCallbackKHR: FnPtr::new(metaloadfn("glDebugMessageCallbackKHR", &[])),
DebugMessageControl: FnPtr::new(metaloadfn("glDebugMessageControl", &["glDebugMessageControlARB", "glDebugMessageControlKHR"])),
DebugMessageControlKHR: FnPtr::new(metaloadfn("glDebugMessageControlKHR", &[])),
DebugMessageInsert: FnPtr::new(metaloadfn("glDebugMessageInsert", &["glDebugMessageInsertARB", "glDebugMessageInsertKHR"])),
DebugMessageInsertKHR: FnPtr::new(metaloadfn("glDebugMessageInsertKHR", &[])),
DeleteBuffers: FnPtr::new(metaloadfn("glDeleteBuffers", &["glDeleteBuffersARB"])),
DeleteFencesAPPLE: FnPtr::new(metaloadfn("glDeleteFencesAPPLE", &[])),
DeleteFramebuffers: FnPtr::new(metaloadfn("glDeleteFramebuffers", &["glDeleteFramebuffersEXT"])),
DeleteLists: FnPtr::new(metaloadfn("glDeleteLists", &[])),
DeleteProgram: FnPtr::new(metaloadfn("glDeleteProgram", &[])),
DeleteProgramPipelines: FnPtr::new(metaloadfn("glDeleteProgramPipelines", &[])),
DeleteQueries: FnPtr::new(metaloadfn("glDeleteQueries", &["glDeleteQueriesARB"])),
DeleteQueriesEXT: FnPtr::new(metaloadfn("glDeleteQueriesEXT", &[])),
DeleteRenderbuffers: FnPtr::new(metaloadfn("glDeleteRenderbuffers", &["glDeleteRenderbuffersEXT"])),
DeleteSamplers: FnPtr::new(metaloadfn("glDeleteSamplers", &[])),
DeleteShader: FnPtr::new(metaloadfn("glDeleteShader", &[])),
DeleteSync: FnPtr::new(metaloadfn("glDeleteSync", &["glDeleteSyncAPPLE"])),
DeleteTextures: FnPtr::new(metaloadfn("glDeleteTextures", &[])),
DeleteTransformFeedbacks: FnPtr::new(metaloadfn("glDeleteTransformFeedbacks", &["glDeleteTransformFeedbacksNV"])),
DeleteVertexArrays: FnPtr::new(metaloadfn("glDeleteVertexArrays", &["glDeleteVertexArraysAPPLE", "glDeleteVertexArraysOES"])),
DeleteVertexArraysAPPLE: FnPtr::new(metaloadfn("glDeleteVertexArraysAPPLE", &[])),
DepthFunc: FnPtr::new(metaloadfn("glDepthFunc", &[])),
DepthMask: FnPtr::new(metaloadfn("glDepthMask", &[])),
DepthRange: FnPtr::new(metaloadfn("glDepthRange", &[])),
DepthRangef: FnPtr::new(metaloadfn("glDepthRangef", &["glDepthRangefOES"])),
DetachShader: FnPtr::new(metaloadfn("glDetachShader", &["glDetachObjectARB"])),
Disable: FnPtr::new(metaloadfn("glDisable", &[])),
DisableClientState: FnPtr::new(metaloadfn("glDisableClientState", &[])),
DisableVertexAttribArray: FnPtr::new(metaloadfn("glDisableVertexAttribArray", &["glDisableVertexAttribArrayARB"])),
Disablei: FnPtr::new(metaloadfn("glDisablei", &["glDisableIndexedEXT", "glDisableiEXT", "glDisableiNV", "glDisableiOES"])),
DispatchCompute: FnPtr::new(metaloadfn("glDispatchCompute", &[])),
DispatchComputeIndirect: FnPtr::new(metaloadfn("glDispatchComputeIndirect", &[])),
DrawArrays: FnPtr::new(metaloadfn("glDrawArrays", &["glDrawArraysEXT"])),
DrawArraysIndirect: FnPtr::new(metaloadfn("glDrawArraysIndirect", &[])),
DrawArraysInstanced: FnPtr::new(metaloadfn("glDrawArraysInstanced", &["glDrawArraysInstancedANGLE", "glDrawArraysInstancedARB", "glDrawArraysInstancedEXT", "glDrawArraysInstancedNV"])),
DrawBuffer: FnPtr::new(metaloadfn("glDrawBuffer", &[])),
DrawBuffers: FnPtr::new(metaloadfn("glDrawBuffers", &["glDrawBuffersARB", "glDrawBuffersATI", "glDrawBuffersEXT"])),
DrawElements: FnPtr::new(metaloadfn("glDrawElements", &[])),
DrawElementsBaseVertex: FnPtr::new(metaloadfn("glDrawElementsBaseVertex", &["glDrawElementsBaseVertexEXT", "glDrawElementsBaseVertexOES"])),
DrawElementsIndirect: FnPtr::new(metaloadfn("glDrawElementsIndirect", &[])),
DrawElementsInstanced: FnPtr::new(metaloadfn("glDrawElementsInstanced", &["glDrawElementsInstancedANGLE", "glDrawElementsInstancedARB", "glDrawElementsInstancedEXT", "glDrawElementsInstancedNV"])),
DrawElementsInstancedBaseVertex: FnPtr::new(metaloadfn("glDrawElementsInstancedBaseVertex", &["glDrawElementsInstancedBaseVertexEXT", "glDrawElementsInstancedBaseVertexOES"])),
DrawPixels: FnPtr::new(metaloadfn("glDrawPixels", &[])),
DrawRangeElements: FnPtr::new(metaloadfn("glDrawRangeElements", &["glDrawRangeElementsEXT"])),
DrawRangeElementsBaseVertex: FnPtr::new(metaloadfn("glDrawRangeElementsBaseVertex", &["glDrawRangeElementsBaseVertexEXT", "glDrawRangeElementsBaseVertexOES"])),
EGLImageTargetRenderbufferStorageOES: FnPtr::new(metaloadfn("glEGLImageTargetRenderbufferStorageOES", &[])),
EGLImageTargetTexture2DOES: FnPtr::new(metaloadfn("glEGLImageTargetTexture2DOES", &[])),
EdgeFlag: FnPtr::new(metaloadfn("glEdgeFlag", &[])),
EdgeFlagPointer: FnPtr::new(metaloadfn("glEdgeFlagPointer", &[])),
EdgeFlagv: FnPtr::new(metaloadfn("glEdgeFlagv", &[])),
Enable: FnPtr::new(metaloadfn("glEnable", &[])),
EnableClientState: FnPtr::new(metaloadfn("glEnableClientState", &[])),
EnableVertexAttribArray: FnPtr::new(metaloadfn("glEnableVertexAttribArray", &["glEnableVertexAttribArrayARB"])),
Enablei: FnPtr::new(metaloadfn("glEnablei", &["glEnableIndexedEXT", "glEnableiEXT", "glEnableiNV", "glEnableiOES"])),
End: FnPtr::new(metaloadfn("glEnd", &[])),
EndConditionalRender: FnPtr::new(metaloadfn("glEndConditionalRender", &["glEndConditionalRenderNV", "glEndConditionalRenderNVX"])),
EndList: FnPtr::new(metaloadfn("glEndList", &[])),
EndQuery: FnPtr::new(metaloadfn("glEndQuery", &["glEndQueryARB"])),
EndQueryEXT: FnPtr::new(metaloadfn("glEndQueryEXT", &[])),
EndTransformFeedback: FnPtr::new(metaloadfn("glEndTransformFeedback", &["glEndTransformFeedbackEXT", "glEndTransformFeedbackNV"])),
EvalCoord1d: FnPtr::new(metaloadfn("glEvalCoord1d", &[])),
EvalCoord1dv: FnPtr::new(metaloadfn("glEvalCoord1dv", &[])),
EvalCoord1f: FnPtr::new(metaloadfn("glEvalCoord1f", &[])),
EvalCoord1fv: FnPtr::new(metaloadfn("glEvalCoord1fv", &[])),
EvalCoord2d: FnPtr::new(metaloadfn("glEvalCoord2d", &[])),
EvalCoord2dv: FnPtr::new(metaloadfn("glEvalCoord2dv", &[])),
EvalCoord2f: FnPtr::new(metaloadfn("glEvalCoord2f", &[])),
EvalCoord2fv: FnPtr::new(metaloadfn("glEvalCoord2fv", &[])),
EvalMesh1: FnPtr::new(metaloadfn("glEvalMesh1", &[])),
EvalMesh2: FnPtr::new(metaloadfn("glEvalMesh2", &[])),
EvalPoint1: FnPtr::new(metaloadfn("glEvalPoint1", &[])),
EvalPoint2: FnPtr::new(metaloadfn("glEvalPoint2", &[])),
FeedbackBuffer: FnPtr::new(metaloadfn("glFeedbackBuffer", &[])),
FenceSync: FnPtr::new(metaloadfn("glFenceSync", &["glFenceSyncAPPLE"])),
Finish: FnPtr::new(metaloadfn("glFinish", &[])),
FinishFenceAPPLE: FnPtr::new(metaloadfn("glFinishFenceAPPLE", &[])),
FinishObjectAPPLE: FnPtr::new(metaloadfn("glFinishObjectAPPLE", &[])),
Flush: FnPtr::new(metaloadfn("glFlush", &[])),
FlushMappedBufferRange: FnPtr::new(metaloadfn("glFlushMappedBufferRange", &["glFlushMappedBufferRangeAPPLE", "glFlushMappedBufferRangeEXT"])),
FogCoordPointer: FnPtr::new(metaloadfn("glFogCoordPointer", &["glFogCoordPointerEXT"])),
FogCoordd: FnPtr::new(metaloadfn("glFogCoordd", &["glFogCoorddEXT"])),
FogCoorddv: FnPtr::new(metaloadfn("glFogCoorddv", &["glFogCoorddvEXT"])),
FogCoordf: FnPtr::new(metaloadfn("glFogCoordf", &["glFogCoordfEXT"])),
FogCoordfv: FnPtr::new(metaloadfn("glFogCoordfv", &["glFogCoordfvEXT"])),
Fogf: FnPtr::new(metaloadfn("glFogf", &[])),
Fogfv: FnPtr::new(metaloadfn("glFogfv", &[])),
Fogi: FnPtr::new(metaloadfn("glFogi", &[])),
Fogiv: FnPtr::new(metaloadfn("glFogiv", &[])),
FramebufferParameteri: FnPtr::new(metaloadfn("glFramebufferParameteri", &[])),
FramebufferRenderbuffer: FnPtr::new(metaloadfn("glFramebufferRenderbuffer", &["glFramebufferRenderbufferEXT"])),
FramebufferTexture: FnPtr::new(metaloadfn("glFramebufferTexture", &["glFramebufferTextureARB", "glFramebufferTextureEXT", "glFramebufferTextureOES"])),
FramebufferTexture1D: FnPtr::new(metaloadfn("glFramebufferTexture1D", &["glFramebufferTexture1DEXT"])),
FramebufferTexture2D: FnPtr::new(metaloadfn("glFramebufferTexture2D", &["glFramebufferTexture2DEXT"])),
FramebufferTexture3D: FnPtr::new(metaloadfn("glFramebufferTexture3D", &["glFramebufferTexture3DEXT"])),
FramebufferTextureLayer: FnPtr::new(metaloadfn("glFramebufferTextureLayer", &["glFramebufferTextureLayerARB", "glFramebufferTextureLayerEXT"])),
FrontFace: FnPtr::new(metaloadfn("glFrontFace", &[])),
Frustum: FnPtr::new(metaloadfn("glFrustum", &[])),
GenBuffers: FnPtr::new(metaloadfn("glGenBuffers", &["glGenBuffersARB"])),
GenFencesAPPLE: FnPtr::new(metaloadfn("glGenFencesAPPLE", &[])),
GenFramebuffers: FnPtr::new(metaloadfn("glGenFramebuffers", &["glGenFramebuffersEXT"])),
GenLists: FnPtr::new(metaloadfn("glGenLists", &[])),
GenProgramPipelines: FnPtr::new(metaloadfn("glGenProgramPipelines", &[])),
GenQueries: FnPtr::new(metaloadfn("glGenQueries", &["glGenQueriesARB"])),
GenQueriesEXT: FnPtr::new(metaloadfn("glGenQueriesEXT", &[])),
GenRenderbuffers: FnPtr::new(metaloadfn("glGenRenderbuffers", &["glGenRenderbuffersEXT"])),
GenSamplers: FnPtr::new(metaloadfn("glGenSamplers", &[])),
GenTextures: FnPtr::new(metaloadfn("glGenTextures", &[])),
GenTransformFeedbacks: FnPtr::new(metaloadfn("glGenTransformFeedbacks", &["glGenTransformFeedbacksNV"])),
GenVertexArrays: FnPtr::new(metaloadfn("glGenVertexArrays", &["glGenVertexArraysAPPLE", "glGenVertexArraysOES"])),
GenVertexArraysAPPLE: FnPtr::new(metaloadfn("glGenVertexArraysAPPLE", &[])),
GenerateMipmap: FnPtr::new(metaloadfn("glGenerateMipmap", &["glGenerateMipmapEXT"])),
GetActiveAttrib: FnPtr::new(metaloadfn("glGetActiveAttrib", &["glGetActiveAttribARB"])),
GetActiveUniform: FnPtr::new(metaloadfn("glGetActiveUniform", &["glGetActiveUniformARB"])),
GetActiveUniformBlockName: FnPtr::new(metaloadfn("glGetActiveUniformBlockName", &[])),
GetActiveUniformBlockiv: FnPtr::new(metaloadfn("glGetActiveUniformBlockiv", &[])),
GetActiveUniformName: FnPtr::new(metaloadfn("glGetActiveUniformName", &[])),
GetActiveUniformsiv: FnPtr::new(metaloadfn("glGetActiveUniformsiv", &[])),
GetAttachedShaders: FnPtr::new(metaloadfn("glGetAttachedShaders", &[])),
GetAttribLocation: FnPtr::new(metaloadfn("glGetAttribLocation", &["glGetAttribLocationARB"])),
GetBooleani_v: FnPtr::new(metaloadfn("glGetBooleani_v", &["glGetBooleanIndexedvEXT"])),
GetBooleanv: FnPtr::new(metaloadfn("glGetBooleanv", &[])),
GetBufferParameteri64v: FnPtr::new(metaloadfn("glGetBufferParameteri64v", &[])),
GetBufferParameteriv: FnPtr::new(metaloadfn("glGetBufferParameteriv", &["glGetBufferParameterivARB"])),
GetBufferPointerv: FnPtr::new(metaloadfn("glGetBufferPointerv", &["glGetBufferPointervARB", "glGetBufferPointervOES"])),
GetBufferSubData: FnPtr::new(metaloadfn("glGetBufferSubData", &["glGetBufferSubDataARB"])),
GetClipPlane: FnPtr::new(metaloadfn("glGetClipPlane", &[])),
GetCompressedTexImage: FnPtr::new(metaloadfn("glGetCompressedTexImage", &["glGetCompressedTexImageARB"])),
GetDebugMessageLog: FnPtr::new(metaloadfn("glGetDebugMessageLog", &["glGetDebugMessageLogARB", "glGetDebugMessageLogKHR"])),
GetDebugMessageLogKHR: FnPtr::new(metaloadfn("glGetDebugMessageLogKHR", &[])),
GetDoublev: FnPtr::new(metaloadfn("glGetDoublev", &[])),
GetError: FnPtr::new(metaloadfn("glGetError", &[])),
GetFloatv: FnPtr::new(metaloadfn("glGetFloatv", &[])),
GetFragDataIndex: FnPtr::new(metaloadfn("glGetFragDataIndex", &["glGetFragDataIndexEXT"])),
GetFragDataLocation: FnPtr::new(metaloadfn("glGetFragDataLocation", &["glGetFragDataLocationEXT"])),
GetFramebufferAttachmentParameteriv: FnPtr::new(metaloadfn("glGetFramebufferAttachmentParameteriv", &["glGetFramebufferAttachmentParameterivEXT"])),
GetFramebufferParameteriv: FnPtr::new(metaloadfn("glGetFramebufferParameteriv", &[])),
GetInteger64i_v: FnPtr::new(metaloadfn("glGetInteger64i_v", &[])),
GetInteger64v: FnPtr::new(metaloadfn("glGetInteger64v", &["glGetInteger64vAPPLE"])),
GetIntegeri_v: FnPtr::new(metaloadfn("glGetIntegeri_v", &["glGetIntegerIndexedvEXT"])),
GetIntegerv: FnPtr::new(metaloadfn("glGetIntegerv", &[])),
GetInternalformativ: FnPtr::new(metaloadfn("glGetInternalformativ", &[])),
GetLightfv: FnPtr::new(metaloadfn("glGetLightfv", &[])),
GetLightiv: FnPtr::new(metaloadfn("glGetLightiv", &[])),
GetMapdv: FnPtr::new(metaloadfn("glGetMapdv", &[])),
GetMapfv: FnPtr::new(metaloadfn("glGetMapfv", &[])),
GetMapiv: FnPtr::new(metaloadfn("glGetMapiv", &[])),
GetMaterialfv: FnPtr::new(metaloadfn("glGetMaterialfv", &[])),
GetMaterialiv: FnPtr::new(metaloadfn("glGetMaterialiv", &[])),
GetMultisamplefv: FnPtr::new(metaloadfn("glGetMultisamplefv", &["glGetMultisamplefvNV"])),
GetObjectLabel: FnPtr::new(metaloadfn("glGetObjectLabel", &["glGetObjectLabelKHR"])),
GetObjectLabelKHR: FnPtr::new(metaloadfn("glGetObjectLabelKHR", &[])),
GetObjectPtrLabel: FnPtr::new(metaloadfn("glGetObjectPtrLabel", &["glGetObjectPtrLabelKHR"])),
GetObjectPtrLabelKHR: FnPtr::new(metaloadfn("glGetObjectPtrLabelKHR", &[])),
GetPixelMapfv: FnPtr::new(metaloadfn("glGetPixelMapfv", &[])),
GetPixelMapuiv: FnPtr::new(metaloadfn("glGetPixelMapuiv", &[])),
GetPixelMapusv: FnPtr::new(metaloadfn("glGetPixelMapusv", &[])),
GetPointerv: FnPtr::new(metaloadfn("glGetPointerv", &["glGetPointervEXT", "glGetPointervKHR"])),
GetPointervKHR: FnPtr::new(metaloadfn("glGetPointervKHR", &[])),
GetPolygonStipple: FnPtr::new(metaloadfn("glGetPolygonStipple", &[])),
GetProgramBinary: FnPtr::new(metaloadfn("glGetProgramBinary", &["glGetProgramBinaryOES"])),
GetProgramInfoLog: FnPtr::new(metaloadfn("glGetProgramInfoLog", &[])),
GetProgramInterfaceiv: FnPtr::new(metaloadfn("glGetProgramInterfaceiv", &[])),
GetProgramPipelineInfoLog: FnPtr::new(metaloadfn("glGetProgramPipelineInfoLog", &[])),
GetProgramPipelineiv: FnPtr::new(metaloadfn("glGetProgramPipelineiv", &[])),
GetProgramResourceIndex: FnPtr::new(metaloadfn("glGetProgramResourceIndex", &[])),
GetProgramResourceLocation: FnPtr::new(metaloadfn("glGetProgramResourceLocation", &[])),
GetProgramResourceName: FnPtr::new(metaloadfn("glGetProgramResourceName", &[])),
GetProgramResourceiv: FnPtr::new(metaloadfn("glGetProgramResourceiv", &[])),
GetProgramiv: FnPtr::new(metaloadfn("glGetProgramiv", &[])),
GetQueryObjecti64v: FnPtr::new(metaloadfn("glGetQueryObjecti64v", &["glGetQueryObjecti64vEXT"])),
GetQueryObjecti64vEXT: FnPtr::new(metaloadfn("glGetQueryObjecti64vEXT", &[])),
GetQueryObjectiv: FnPtr::new(metaloadfn("glGetQueryObjectiv", &["glGetQueryObjectivARB", "glGetQueryObjectivEXT"])),
GetQueryObjectivEXT: FnPtr::new(metaloadfn("glGetQueryObjectivEXT", &[])),
GetQueryObjectui64v: FnPtr::new(metaloadfn("glGetQueryObjectui64v", &["glGetQueryObjectui64vEXT"])),
GetQueryObjectui64vEXT: FnPtr::new(metaloadfn("glGetQueryObjectui64vEXT", &[])),
GetQueryObjectuiv: FnPtr::new(metaloadfn("glGetQueryObjectuiv", &["glGetQueryObjectuivARB"])),
GetQueryObjectuivEXT: FnPtr::new(metaloadfn("glGetQueryObjectuivEXT", &[])),
GetQueryiv: FnPtr::new(metaloadfn("glGetQueryiv", &["glGetQueryivARB"])),
GetQueryivEXT: FnPtr::new(metaloadfn("glGetQueryivEXT", &[])),
GetRenderbufferParameteriv: FnPtr::new(metaloadfn("glGetRenderbufferParameteriv", &["glGetRenderbufferParameterivEXT"])),
GetSamplerParameterIiv: FnPtr::new(metaloadfn("glGetSamplerParameterIiv", &["glGetSamplerParameterIivEXT", "glGetSamplerParameterIivOES"])),
GetSamplerParameterIuiv: FnPtr::new(metaloadfn("glGetSamplerParameterIuiv", &["glGetSamplerParameterIuivEXT", "glGetSamplerParameterIuivOES"])),
GetSamplerParameterfv: FnPtr::new(metaloadfn("glGetSamplerParameterfv", &[])),
GetSamplerParameteriv: FnPtr::new(metaloadfn("glGetSamplerParameteriv", &[])),
GetShaderInfoLog: FnPtr::new(metaloadfn("glGetShaderInfoLog", &[])),
GetShaderPrecisionFormat: FnPtr::new(metaloadfn("glGetShaderPrecisionFormat", &[])),
GetShaderSource: FnPtr::new(metaloadfn("glGetShaderSource", &["glGetShaderSourceARB"])),
GetShaderiv: FnPtr::new(metaloadfn("glGetShaderiv", &[])),
GetString: FnPtr::new(metaloadfn("glGetString", &[])),
GetStringi: FnPtr::new(metaloadfn("glGetStringi", &[])),
GetSynciv: FnPtr::new(metaloadfn("glGetSynciv", &["glGetSyncivAPPLE"])),
GetTexEnvfv: FnPtr::new(metaloadfn("glGetTexEnvfv", &[])),
GetTexEnviv: FnPtr::new(metaloadfn("glGetTexEnviv", &[])),
GetTexGendv: FnPtr::new(metaloadfn("glGetTexGendv", &[])),
GetTexGenfv: FnPtr::new(metaloadfn("glGetTexGenfv", &[])),
GetTexGeniv: FnPtr::new(metaloadfn("glGetTexGeniv", &[])),
GetTexImage: FnPtr::new(metaloadfn("glGetTexImage", &[])),
GetTexLevelParameterfv: FnPtr::new(metaloadfn("glGetTexLevelParameterfv", &[])),
GetTexLevelParameteriv: FnPtr::new(metaloadfn("glGetTexLevelParameteriv", &[])),
GetTexParameterIiv: FnPtr::new(metaloadfn("glGetTexParameterIiv", &["glGetTexParameterIivEXT", "glGetTexParameterIivOES"])),
GetTexParameterIuiv: FnPtr::new(metaloadfn("glGetTexParameterIuiv", &["glGetTexParameterIuivEXT", "glGetTexParameterIuivOES"])),
GetTexParameterPointervAPPLE: FnPtr::new(metaloadfn("glGetTexParameterPointervAPPLE", &[])),
GetTexParameterfv: FnPtr::new(metaloadfn("glGetTexParameterfv", &[])),
GetTexParameteriv: FnPtr::new(metaloadfn("glGetTexParameteriv", &[])),
GetTransformFeedbackVarying: FnPtr::new(metaloadfn("glGetTransformFeedbackVarying", &["glGetTransformFeedbackVaryingEXT"])),
GetUniformBlockIndex: FnPtr::new(metaloadfn("glGetUniformBlockIndex", &[])),
GetUniformIndices: FnPtr::new(metaloadfn("glGetUniformIndices", &[])),
GetUniformLocation: FnPtr::new(metaloadfn("glGetUniformLocation", &["glGetUniformLocationARB"])),
GetUniformfv: FnPtr::new(metaloadfn("glGetUniformfv", &["glGetUniformfvARB"])),
GetUniformiv: FnPtr::new(metaloadfn("glGetUniformiv", &["glGetUniformivARB"])),
GetUniformuiv: FnPtr::new(metaloadfn("glGetUniformuiv", &["glGetUniformuivEXT"])),
GetVertexAttribIiv: FnPtr::new(metaloadfn("glGetVertexAttribIiv", &["glGetVertexAttribIivEXT"])),
GetVertexAttribIuiv: FnPtr::new(metaloadfn("glGetVertexAttribIuiv", &["glGetVertexAttribIuivEXT"])),
GetVertexAttribPointerv: FnPtr::new(metaloadfn("glGetVertexAttribPointerv", &["glGetVertexAttribPointervARB", "glGetVertexAttribPointervNV"])),
GetVertexAttribdv: FnPtr::new(metaloadfn("glGetVertexAttribdv", &["glGetVertexAttribdvARB", "glGetVertexAttribdvNV"])),
GetVertexAttribfv: FnPtr::new(metaloadfn("glGetVertexAttribfv", &["glGetVertexAttribfvARB", "glGetVertexAttribfvNV"])),
GetVertexAttribiv: FnPtr::new(metaloadfn("glGetVertexAttribiv", &["glGetVertexAttribivARB", "glGetVertexAttribivNV"])),
Hint: FnPtr::new(metaloadfn("glHint", &[])),
IndexMask: FnPtr::new(metaloadfn("glIndexMask", &[])),
IndexPointer: FnPtr::new(metaloadfn("glIndexPointer", &[])),
Indexd: FnPtr::new(metaloadfn("glIndexd", &[])),
Indexdv: FnPtr::new(metaloadfn("glIndexdv", &[])),
Indexf: FnPtr::new(metaloadfn("glIndexf", &[])),
Indexfv: FnPtr::new(metaloadfn("glIndexfv", &[])),
Indexi: FnPtr::new(metaloadfn("glIndexi", &[])),
Indexiv: FnPtr::new(metaloadfn("glIndexiv", &[])),
Indexs: FnPtr::new(metaloadfn("glIndexs", &[])),
Indexsv: FnPtr::new(metaloadfn("glIndexsv", &[])),
Indexub: FnPtr::new(metaloadfn("glIndexub", &[])),
Indexubv: FnPtr::new(metaloadfn("glIndexubv", &[])),
InitNames: FnPtr::new(metaloadfn("glInitNames", &[])),
InsertEventMarkerEXT: FnPtr::new(metaloadfn("glInsertEventMarkerEXT", &[])),
InterleavedArrays: FnPtr::new(metaloadfn("glInterleavedArrays", &[])),
InvalidateBufferData: FnPtr::new(metaloadfn("glInvalidateBufferData", &[])),
InvalidateBufferSubData: FnPtr::new(metaloadfn("glInvalidateBufferSubData", &[])),
InvalidateFramebuffer: FnPtr::new(metaloadfn("glInvalidateFramebuffer", &[])),
InvalidateSubFramebuffer: FnPtr::new(metaloadfn("glInvalidateSubFramebuffer", &[])),
InvalidateTexImage: FnPtr::new(metaloadfn("glInvalidateTexImage", &[])),
InvalidateTexSubImage: FnPtr::new(metaloadfn("glInvalidateTexSubImage", &[])),
IsBuffer: FnPtr::new(metaloadfn("glIsBuffer", &["glIsBufferARB"])),
IsEnabled: FnPtr::new(metaloadfn("glIsEnabled", &[])),
IsEnabledi: FnPtr::new(metaloadfn("glIsEnabledi", &["glIsEnabledIndexedEXT", "glIsEnablediEXT", "glIsEnablediNV", "glIsEnablediOES"])),
IsFenceAPPLE: FnPtr::new(metaloadfn("glIsFenceAPPLE", &[])),
IsFramebuffer: FnPtr::new(metaloadfn("glIsFramebuffer", &["glIsFramebufferEXT"])),
IsList: FnPtr::new(metaloadfn("glIsList", &[])),
IsProgram: FnPtr::new(metaloadfn("glIsProgram", &[])),
IsProgramPipeline: FnPtr::new(metaloadfn("glIsProgramPipeline", &[])),
IsQuery: FnPtr::new(metaloadfn("glIsQuery", &["glIsQueryARB"])),
IsQueryEXT: FnPtr::new(metaloadfn("glIsQueryEXT", &[])),
IsRenderbuffer: FnPtr::new(metaloadfn("glIsRenderbuffer", &["glIsRenderbufferEXT"])),
IsSampler: FnPtr::new(metaloadfn("glIsSampler", &[])),
IsShader: FnPtr::new(metaloadfn("glIsShader", &[])),
IsSync: FnPtr::new(metaloadfn("glIsSync", &["glIsSyncAPPLE"])),
IsTexture: FnPtr::new(metaloadfn("glIsTexture", &[])),
IsTransformFeedback: FnPtr::new(metaloadfn("glIsTransformFeedback", &["glIsTransformFeedbackNV"])),
IsVertexArray: FnPtr::new(metaloadfn("glIsVertexArray", &["glIsVertexArrayAPPLE", "glIsVertexArrayOES"])),
IsVertexArrayAPPLE: FnPtr::new(metaloadfn("glIsVertexArrayAPPLE", &[])),
LightModelf: FnPtr::new(metaloadfn("glLightModelf", &[])),
LightModelfv: FnPtr::new(metaloadfn("glLightModelfv", &[])),
LightModeli: FnPtr::new(metaloadfn("glLightModeli", &[])),
LightModeliv: FnPtr::new(metaloadfn("glLightModeliv", &[])),
Lightf: FnPtr::new(metaloadfn("glLightf", &[])),
Lightfv: FnPtr::new(metaloadfn("glLightfv", &[])),
Lighti: FnPtr::new(metaloadfn("glLighti", &[])),
Lightiv: FnPtr::new(metaloadfn("glLightiv", &[])),
LineStipple: FnPtr::new(metaloadfn("glLineStipple", &[])),
LineWidth: FnPtr::new(metaloadfn("glLineWidth", &[])),
LinkProgram: FnPtr::new(metaloadfn("glLinkProgram", &["glLinkProgramARB"])),
ListBase: FnPtr::new(metaloadfn("glListBase", &[])),
LoadIdentity: FnPtr::new(metaloadfn("glLoadIdentity", &[])),
LoadMatrixd: FnPtr::new(metaloadfn("glLoadMatrixd", &[])),
LoadMatrixf: FnPtr::new(metaloadfn("glLoadMatrixf", &[])),
LoadName: FnPtr::new(metaloadfn("glLoadName", &[])),
LoadTransposeMatrixd: FnPtr::new(metaloadfn("glLoadTransposeMatrixd", &["glLoadTransposeMatrixdARB"])),
LoadTransposeMatrixf: FnPtr::new(metaloadfn("glLoadTransposeMatrixf", &["glLoadTransposeMatrixfARB"])),
LogicOp: FnPtr::new(metaloadfn("glLogicOp", &[])),
Map1d: FnPtr::new(metaloadfn("glMap1d", &[])),
Map1f: FnPtr::new(metaloadfn("glMap1f", &[])),
Map2d: FnPtr::new(metaloadfn("glMap2d", &[])),
Map2f: FnPtr::new(metaloadfn("glMap2f", &[])),
MapBuffer: FnPtr::new(metaloadfn("glMapBuffer", &["glMapBufferARB", "glMapBufferOES"])),
MapBufferRange: FnPtr::new(metaloadfn("glMapBufferRange", &["glMapBufferRangeEXT"])),
MapGrid1d: FnPtr::new(metaloadfn("glMapGrid1d", &[])),
MapGrid1f: FnPtr::new(metaloadfn("glMapGrid1f", &[])),
MapGrid2d: FnPtr::new(metaloadfn("glMapGrid2d", &[])),
MapGrid2f: FnPtr::new(metaloadfn("glMapGrid2f", &[])),
Materialf: FnPtr::new(metaloadfn("glMaterialf", &[])),
Materialfv: FnPtr::new(metaloadfn("glMaterialfv", &[])),
Materiali: FnPtr::new(metaloadfn("glMateriali", &[])),
Materialiv: FnPtr::new(metaloadfn("glMaterialiv", &[])),
MatrixMode: FnPtr::new(metaloadfn("glMatrixMode", &[])),
MemoryBarrier: FnPtr::new(metaloadfn("glMemoryBarrier", &["glMemoryBarrierEXT"])),
MemoryBarrierByRegion: FnPtr::new(metaloadfn("glMemoryBarrierByRegion", &[])),
MultMatrixd: FnPtr::new(metaloadfn("glMultMatrixd", &[])),
MultMatrixf: FnPtr::new(metaloadfn("glMultMatrixf", &[])),
MultTransposeMatrixd: FnPtr::new(metaloadfn("glMultTransposeMatrixd", &["glMultTransposeMatrixdARB"])),
MultTransposeMatrixf: FnPtr::new(metaloadfn("glMultTransposeMatrixf", &["glMultTransposeMatrixfARB"])),
MultiDrawArrays: FnPtr::new(metaloadfn("glMultiDrawArrays", &["glMultiDrawArraysEXT"])),
MultiDrawElements: FnPtr::new(metaloadfn("glMultiDrawElements", &["glMultiDrawElementsEXT"])),
MultiDrawElementsBaseVertex: FnPtr::new(metaloadfn("glMultiDrawElementsBaseVertex", &["glMultiDrawElementsBaseVertexEXT"])),
MultiTexCoord1d: FnPtr::new(metaloadfn("glMultiTexCoord1d", &["glMultiTexCoord1dARB"])),
MultiTexCoord1dv: FnPtr::new(metaloadfn("glMultiTexCoord1dv", &["glMultiTexCoord1dvARB"])),
MultiTexCoord1f: FnPtr::new(metaloadfn("glMultiTexCoord1f", &["glMultiTexCoord1fARB"])),
MultiTexCoord1fv: FnPtr::new(metaloadfn("glMultiTexCoord1fv", &["glMultiTexCoord1fvARB"])),
MultiTexCoord1i: FnPtr::new(metaloadfn("glMultiTexCoord1i", &["glMultiTexCoord1iARB"])),
MultiTexCoord1iv: FnPtr::new(metaloadfn("glMultiTexCoord1iv", &["glMultiTexCoord1ivARB"])),
MultiTexCoord1s: FnPtr::new(metaloadfn("glMultiTexCoord1s", &["glMultiTexCoord1sARB"])),
MultiTexCoord1sv: FnPtr::new(metaloadfn("glMultiTexCoord1sv", &["glMultiTexCoord1svARB"])),
MultiTexCoord2d: FnPtr::new(metaloadfn("glMultiTexCoord2d", &["glMultiTexCoord2dARB"])),
MultiTexCoord2dv: FnPtr::new(metaloadfn("glMultiTexCoord2dv", &["glMultiTexCoord2dvARB"])),
MultiTexCoord2f: FnPtr::new(metaloadfn("glMultiTexCoord2f", &["glMultiTexCoord2fARB"])),
MultiTexCoord2fv: FnPtr::new(metaloadfn("glMultiTexCoord2fv", &["glMultiTexCoord2fvARB"])),
MultiTexCoord2i: FnPtr::new(metaloadfn("glMultiTexCoord2i", &["glMultiTexCoord2iARB"])),
MultiTexCoord2iv: FnPtr::new(metaloadfn("glMultiTexCoord2iv", &["glMultiTexCoord2ivARB"])),
MultiTexCoord2s: FnPtr::new(metaloadfn("glMultiTexCoord2s", &["glMultiTexCoord2sARB"])),
MultiTexCoord2sv: FnPtr::new(metaloadfn("glMultiTexCoord2sv", &["glMultiTexCoord2svARB"])),
MultiTexCoord3d: FnPtr::new(metaloadfn("glMultiTexCoord3d", &["glMultiTexCoord3dARB"])),
MultiTexCoord3dv: FnPtr::new(metaloadfn("glMultiTexCoord3dv", &["glMultiTexCoord3dvARB"])),
MultiTexCoord3f: FnPtr::new(metaloadfn("glMultiTexCoord3f", &["glMultiTexCoord3fARB"])),
MultiTexCoord3fv: FnPtr::new(metaloadfn("glMultiTexCoord3fv", &["glMultiTexCoord3fvARB"])),
MultiTexCoord3i: FnPtr::new(metaloadfn("glMultiTexCoord3i", &["glMultiTexCoord3iARB"])),
MultiTexCoord3iv: FnPtr::new(metaloadfn("glMultiTexCoord3iv", &["glMultiTexCoord3ivARB"])),
MultiTexCoord3s: FnPtr::new(metaloadfn("glMultiTexCoord3s", &["glMultiTexCoord3sARB"])),
MultiTexCoord3sv: FnPtr::new(metaloadfn("glMultiTexCoord3sv", &["glMultiTexCoord3svARB"])),
MultiTexCoord4d: FnPtr::new(metaloadfn("glMultiTexCoord4d", &["glMultiTexCoord4dARB"])),
MultiTexCoord4dv: FnPtr::new(metaloadfn("glMultiTexCoord4dv", &["glMultiTexCoord4dvARB"])),
MultiTexCoord4f: FnPtr::new(metaloadfn("glMultiTexCoord4f", &["glMultiTexCoord4fARB"])),
MultiTexCoord4fv: FnPtr::new(metaloadfn("glMultiTexCoord4fv", &["glMultiTexCoord4fvARB"])),
MultiTexCoord4i: FnPtr::new(metaloadfn("glMultiTexCoord4i", &["glMultiTexCoord4iARB"])),
MultiTexCoord4iv: FnPtr::new(metaloadfn("glMultiTexCoord4iv", &["glMultiTexCoord4ivARB"])),
MultiTexCoord4s: FnPtr::new(metaloadfn("glMultiTexCoord4s", &["glMultiTexCoord4sARB"])),
MultiTexCoord4sv: FnPtr::new(metaloadfn("glMultiTexCoord4sv", &["glMultiTexCoord4svARB"])),
MultiTexCoordP1ui: FnPtr::new(metaloadfn("glMultiTexCoordP1ui", &[])),
MultiTexCoordP1uiv: FnPtr::new(metaloadfn("glMultiTexCoordP1uiv", &[])),
MultiTexCoordP2ui: FnPtr::new(metaloadfn("glMultiTexCoordP2ui", &[])),
MultiTexCoordP2uiv: FnPtr::new(metaloadfn("glMultiTexCoordP2uiv", &[])),
MultiTexCoordP3ui: FnPtr::new(metaloadfn("glMultiTexCoordP3ui", &[])),
MultiTexCoordP3uiv: FnPtr::new(metaloadfn("glMultiTexCoordP3uiv", &[])),
MultiTexCoordP4ui: FnPtr::new(metaloadfn("glMultiTexCoordP4ui", &[])),
MultiTexCoordP4uiv: FnPtr::new(metaloadfn("glMultiTexCoordP4uiv", &[])),
NewList: FnPtr::new(metaloadfn("glNewList", &[])),
Normal3b: FnPtr::new(metaloadfn("glNormal3b", &[])),
Normal3bv: FnPtr::new(metaloadfn("glNormal3bv", &[])),
Normal3d: FnPtr::new(metaloadfn("glNormal3d", &[])),
Normal3dv: FnPtr::new(metaloadfn("glNormal3dv", &[])),
Normal3f: FnPtr::new(metaloadfn("glNormal3f", &[])),
Normal3fv: FnPtr::new(metaloadfn("glNormal3fv", &[])),
Normal3i: FnPtr::new(metaloadfn("glNormal3i", &[])),
Normal3iv: FnPtr::new(metaloadfn("glNormal3iv", &[])),
Normal3s: FnPtr::new(metaloadfn("glNormal3s", &[])),
Normal3sv: FnPtr::new(metaloadfn("glNormal3sv", &[])),
NormalP3ui: FnPtr::new(metaloadfn("glNormalP3ui", &[])),
NormalP3uiv: FnPtr::new(metaloadfn("glNormalP3uiv", &[])),
NormalPointer: FnPtr::new(metaloadfn("glNormalPointer", &[])),
ObjectLabel: FnPtr::new(metaloadfn("glObjectLabel", &["glObjectLabelKHR"])),
ObjectLabelKHR: FnPtr::new(metaloadfn("glObjectLabelKHR", &[])),
ObjectPtrLabel: FnPtr::new(metaloadfn("glObjectPtrLabel", &["glObjectPtrLabelKHR"])),
ObjectPtrLabelKHR: FnPtr::new(metaloadfn("glObjectPtrLabelKHR", &[])),
Ortho: FnPtr::new(metaloadfn("glOrtho", &[])),
PassThrough: FnPtr::new(metaloadfn("glPassThrough", &[])),
PauseTransformFeedback: FnPtr::new(metaloadfn("glPauseTransformFeedback", &["glPauseTransformFeedbackNV"])),
PixelMapfv: FnPtr::new(metaloadfn("glPixelMapfv", &[])),
PixelMapuiv: FnPtr::new(metaloadfn("glPixelMapuiv", &[])),
PixelMapusv: FnPtr::new(metaloadfn("glPixelMapusv", &[])),
PixelStoref: FnPtr::new(metaloadfn("glPixelStoref", &[])),
PixelStorei: FnPtr::new(metaloadfn("glPixelStorei", &[])),
PixelTransferf: FnPtr::new(metaloadfn("glPixelTransferf", &[])),
PixelTransferi: FnPtr::new(metaloadfn("glPixelTransferi", &[])),
PixelZoom: FnPtr::new(metaloadfn("glPixelZoom", &[])),
PointParameterf: FnPtr::new(metaloadfn("glPointParameterf", &["glPointParameterfARB", "glPointParameterfEXT", "glPointParameterfSGIS"])),
PointParameterfv: FnPtr::new(metaloadfn("glPointParameterfv", &["glPointParameterfvARB", "glPointParameterfvEXT", "glPointParameterfvSGIS"])),
PointParameteri: FnPtr::new(metaloadfn("glPointParameteri", &["glPointParameteriNV"])),
PointParameteriv: FnPtr::new(metaloadfn("glPointParameteriv", &["glPointParameterivNV"])),
PointSize: FnPtr::new(metaloadfn("glPointSize", &[])),
PolygonMode: FnPtr::new(metaloadfn("glPolygonMode", &["glPolygonModeNV"])),
PolygonOffset: FnPtr::new(metaloadfn("glPolygonOffset", &[])),
PolygonStipple: FnPtr::new(metaloadfn("glPolygonStipple", &[])),
PopAttrib: FnPtr::new(metaloadfn("glPopAttrib", &[])),
PopClientAttrib: FnPtr::new(metaloadfn("glPopClientAttrib", &[])),
PopDebugGroup: FnPtr::new(metaloadfn("glPopDebugGroup", &["glPopDebugGroupKHR"])),
PopDebugGroupKHR: FnPtr::new(metaloadfn("glPopDebugGroupKHR", &[])),
PopGroupMarkerEXT: FnPtr::new(metaloadfn("glPopGroupMarkerEXT", &[])),
PopMatrix: FnPtr::new(metaloadfn("glPopMatrix", &[])),
PopName: FnPtr::new(metaloadfn("glPopName", &[])),
PrimitiveRestartIndex: FnPtr::new(metaloadfn("glPrimitiveRestartIndex", &[])),
PrioritizeTextures: FnPtr::new(metaloadfn("glPrioritizeTextures", &["glPrioritizeTexturesEXT"])),
ProgramBinary: FnPtr::new(metaloadfn("glProgramBinary", &["glProgramBinaryOES"])),
ProgramParameteri: FnPtr::new(metaloadfn("glProgramParameteri", &["glProgramParameteriARB", "glProgramParameteriEXT"])),
ProgramUniform1f: FnPtr::new(metaloadfn("glProgramUniform1f", &["glProgramUniform1fEXT"])),
ProgramUniform1fv: FnPtr::new(metaloadfn("glProgramUniform1fv", &["glProgramUniform1fvEXT"])),
ProgramUniform1i: FnPtr::new(metaloadfn("glProgramUniform1i", &["glProgramUniform1iEXT"])),
ProgramUniform1iv: FnPtr::new(metaloadfn("glProgramUniform1iv", &["glProgramUniform1ivEXT"])),
ProgramUniform1ui: FnPtr::new(metaloadfn("glProgramUniform1ui", &["glProgramUniform1uiEXT"])),
ProgramUniform1uiv: FnPtr::new(metaloadfn("glProgramUniform1uiv", &["glProgramUniform1uivEXT"])),
ProgramUniform2f: FnPtr::new(metaloadfn("glProgramUniform2f", &["glProgramUniform2fEXT"])),
ProgramUniform2fv: FnPtr::new(metaloadfn("glProgramUniform2fv", &["glProgramUniform2fvEXT"])),
ProgramUniform2i: FnPtr::new(metaloadfn("glProgramUniform2i", &["glProgramUniform2iEXT"])),
ProgramUniform2iv: FnPtr::new(metaloadfn("glProgramUniform2iv", &["glProgramUniform2ivEXT"])),
ProgramUniform2ui: FnPtr::new(metaloadfn("glProgramUniform2ui", &["glProgramUniform2uiEXT"])),
ProgramUniform2uiv: FnPtr::new(metaloadfn("glProgramUniform2uiv", &["glProgramUniform2uivEXT"])),
ProgramUniform3f: FnPtr::new(metaloadfn("glProgramUniform3f", &["glProgramUniform3fEXT"])),
ProgramUniform3fv: FnPtr::new(metaloadfn("glProgramUniform3fv", &["glProgramUniform3fvEXT"])),
ProgramUniform3i: FnPtr::new(metaloadfn("glProgramUniform3i", &["glProgramUniform3iEXT"])),
ProgramUniform3iv: FnPtr::new(metaloadfn("glProgramUniform3iv", &["glProgramUniform3ivEXT"])),
ProgramUniform3ui: FnPtr::new(metaloadfn("glProgramUniform3ui", &["glProgramUniform3uiEXT"])),
ProgramUniform3uiv: FnPtr::new(metaloadfn("glProgramUniform3uiv", &["glProgramUniform3uivEXT"])),
ProgramUniform4f: FnPtr::new(metaloadfn("glProgramUniform4f", &["glProgramUniform4fEXT"])),
ProgramUniform4fv: FnPtr::new(metaloadfn("glProgramUniform4fv", &["glProgramUniform4fvEXT"])),
ProgramUniform4i: FnPtr::new(metaloadfn("glProgramUniform4i", &["glProgramUniform4iEXT"])),
ProgramUniform4iv: FnPtr::new(metaloadfn("glProgramUniform4iv", &["glProgramUniform4ivEXT"])),
ProgramUniform4ui: FnPtr::new(metaloadfn("glProgramUniform4ui", &["glProgramUniform4uiEXT"])),
ProgramUniform4uiv: FnPtr::new(metaloadfn("glProgramUniform4uiv", &["glProgramUniform4uivEXT"])),
ProgramUniformMatrix2fv: FnPtr::new(metaloadfn("glProgramUniformMatrix2fv", &["glProgramUniformMatrix2fvEXT"])),
ProgramUniformMatrix2x3fv: FnPtr::new(metaloadfn("glProgramUniformMatrix2x3fv", &["glProgramUniformMatrix2x3fvEXT"])),
ProgramUniformMatrix2x4fv: FnPtr::new(metaloadfn("glProgramUniformMatrix2x4fv", &["glProgramUniformMatrix2x4fvEXT"])),
ProgramUniformMatrix3fv: FnPtr::new(metaloadfn("glProgramUniformMatrix3fv", &["glProgramUniformMatrix3fvEXT"])),
ProgramUniformMatrix3x2fv: FnPtr::new(metaloadfn("glProgramUniformMatrix3x2fv", &["glProgramUniformMatrix3x2fvEXT"])),
ProgramUniformMatrix3x4fv: FnPtr::new(metaloadfn("glProgramUniformMatrix3x4fv", &["glProgramUniformMatrix3x4fvEXT"])),
ProgramUniformMatrix4fv: FnPtr::new(metaloadfn("glProgramUniformMatrix4fv", &["glProgramUniformMatrix4fvEXT"])),
ProgramUniformMatrix4x2fv: FnPtr::new(metaloadfn("glProgramUniformMatrix4x2fv", &["glProgramUniformMatrix4x2fvEXT"])),
ProgramUniformMatrix4x3fv: FnPtr::new(metaloadfn("glProgramUniformMatrix4x3fv", &["glProgramUniformMatrix4x3fvEXT"])),
ProvokingVertex: FnPtr::new(metaloadfn("glProvokingVertex", &["glProvokingVertexEXT"])),
ProvokingVertexANGLE: FnPtr::new(metaloadfn("glProvokingVertexANGLE", &[])),
PushAttrib: FnPtr::new(metaloadfn("glPushAttrib", &[])),
PushClientAttrib: FnPtr::new(metaloadfn("glPushClientAttrib", &[])),
PushDebugGroup: FnPtr::new(metaloadfn("glPushDebugGroup", &["glPushDebugGroupKHR"])),
PushDebugGroupKHR: FnPtr::new(metaloadfn("glPushDebugGroupKHR", &[])),
PushGroupMarkerEXT: FnPtr::new(metaloadfn("glPushGroupMarkerEXT", &[])),
PushMatrix: FnPtr::new(metaloadfn("glPushMatrix", &[])),
PushName: FnPtr::new(metaloadfn("glPushName", &[])),
QueryCounter: FnPtr::new(metaloadfn("glQueryCounter", &["glQueryCounterEXT"])),
QueryCounterEXT: FnPtr::new(metaloadfn("glQueryCounterEXT", &[])),
RasterPos2d: FnPtr::new(metaloadfn("glRasterPos2d", &[])),
RasterPos2dv: FnPtr::new(metaloadfn("glRasterPos2dv", &[])),
RasterPos2f: FnPtr::new(metaloadfn("glRasterPos2f", &[])),
RasterPos2fv: FnPtr::new(metaloadfn("glRasterPos2fv", &[])),
RasterPos2i: FnPtr::new(metaloadfn("glRasterPos2i", &[])),
RasterPos2iv: FnPtr::new(metaloadfn("glRasterPos2iv", &[])),
RasterPos2s: FnPtr::new(metaloadfn("glRasterPos2s", &[])),
RasterPos2sv: FnPtr::new(metaloadfn("glRasterPos2sv", &[])),
RasterPos3d: FnPtr::new(metaloadfn("glRasterPos3d", &[])),
RasterPos3dv: FnPtr::new(metaloadfn("glRasterPos3dv", &[])),
RasterPos3f: FnPtr::new(metaloadfn("glRasterPos3f", &[])),
RasterPos3fv: FnPtr::new(metaloadfn("glRasterPos3fv", &[])),
RasterPos3i: FnPtr::new(metaloadfn("glRasterPos3i", &[])),
RasterPos3iv: FnPtr::new(metaloadfn("glRasterPos3iv", &[])),
RasterPos3s: FnPtr::new(metaloadfn("glRasterPos3s", &[])),
RasterPos3sv: FnPtr::new(metaloadfn("glRasterPos3sv", &[])),
RasterPos4d: FnPtr::new(metaloadfn("glRasterPos4d", &[])),
RasterPos4dv: FnPtr::new(metaloadfn("glRasterPos4dv", &[])),
RasterPos4f: FnPtr::new(metaloadfn("glRasterPos4f", &[])),
RasterPos4fv: FnPtr::new(metaloadfn("glRasterPos4fv", &[])),
RasterPos4i: FnPtr::new(metaloadfn("glRasterPos4i", &[])),
RasterPos4iv: FnPtr::new(metaloadfn("glRasterPos4iv", &[])),
RasterPos4s: FnPtr::new(metaloadfn("glRasterPos4s", &[])),
RasterPos4sv: FnPtr::new(metaloadfn("glRasterPos4sv", &[])),
ReadBuffer: FnPtr::new(metaloadfn("glReadBuffer", &[])),
ReadPixels: FnPtr::new(metaloadfn("glReadPixels", &[])),
Rectd: FnPtr::new(metaloadfn("glRectd", &[])),
Rectdv: FnPtr::new(metaloadfn("glRectdv", &[])),
Rectf: FnPtr::new(metaloadfn("glRectf", &[])),
Rectfv: FnPtr::new(metaloadfn("glRectfv", &[])),
Recti: FnPtr::new(metaloadfn("glRecti", &[])),
Rectiv: FnPtr::new(metaloadfn("glRectiv", &[])),
Rects: FnPtr::new(metaloadfn("glRects", &[])),
Rectsv: FnPtr::new(metaloadfn("glRectsv", &[])),
ReleaseShaderCompiler: FnPtr::new(metaloadfn("glReleaseShaderCompiler", &[])),
RenderMode: FnPtr::new(metaloadfn("glRenderMode", &[])),
RenderbufferStorage: FnPtr::new(metaloadfn("glRenderbufferStorage", &["glRenderbufferStorageEXT"])),
RenderbufferStorageMultisample: FnPtr::new(metaloadfn("glRenderbufferStorageMultisample", &["glRenderbufferStorageMultisampleEXT", "glRenderbufferStorageMultisampleNV"])),
ResumeTransformFeedback: FnPtr::new(metaloadfn("glResumeTransformFeedback", &["glResumeTransformFeedbackNV"])),
Rotated: FnPtr::new(metaloadfn("glRotated", &[])),
Rotatef: FnPtr::new(metaloadfn("glRotatef", &[])),
SampleCoverage: FnPtr::new(metaloadfn("glSampleCoverage", &["glSampleCoverageARB"])),
SampleMaski: FnPtr::new(metaloadfn("glSampleMaski", &[])),
SamplerParameterIiv: FnPtr::new(metaloadfn("glSamplerParameterIiv", &["glSamplerParameterIivEXT", "glSamplerParameterIivOES"])),
SamplerParameterIuiv: FnPtr::new(metaloadfn("glSamplerParameterIuiv", &["glSamplerParameterIuivEXT", "glSamplerParameterIuivOES"])),
SamplerParameterf: FnPtr::new(metaloadfn("glSamplerParameterf", &[])),
SamplerParameterfv: FnPtr::new(metaloadfn("glSamplerParameterfv", &[])),
SamplerParameteri: FnPtr::new(metaloadfn("glSamplerParameteri", &[])),
SamplerParameteriv: FnPtr::new(metaloadfn("glSamplerParameteriv", &[])),
Scaled: FnPtr::new(metaloadfn("glScaled", &[])),
Scalef: FnPtr::new(metaloadfn("glScalef", &[])),
Scissor: FnPtr::new(metaloadfn("glScissor", &[])),
SecondaryColor3b: FnPtr::new(metaloadfn("glSecondaryColor3b", &["glSecondaryColor3bEXT"])),
SecondaryColor3bv: FnPtr::new(metaloadfn("glSecondaryColor3bv", &["glSecondaryColor3bvEXT"])),
SecondaryColor3d: FnPtr::new(metaloadfn("glSecondaryColor3d", &["glSecondaryColor3dEXT"])),
SecondaryColor3dv: FnPtr::new(metaloadfn("glSecondaryColor3dv", &["glSecondaryColor3dvEXT"])),
SecondaryColor3f: FnPtr::new(metaloadfn("glSecondaryColor3f", &["glSecondaryColor3fEXT"])),
SecondaryColor3fv: FnPtr::new(metaloadfn("glSecondaryColor3fv", &["glSecondaryColor3fvEXT"])),
SecondaryColor3i: FnPtr::new(metaloadfn("glSecondaryColor3i", &["glSecondaryColor3iEXT"])),
SecondaryColor3iv: FnPtr::new(metaloadfn("glSecondaryColor3iv", &["glSecondaryColor3ivEXT"])),
SecondaryColor3s: FnPtr::new(metaloadfn("glSecondaryColor3s", &["glSecondaryColor3sEXT"])),
SecondaryColor3sv: FnPtr::new(metaloadfn("glSecondaryColor3sv", &["glSecondaryColor3svEXT"])),
SecondaryColor3ub: FnPtr::new(metaloadfn("glSecondaryColor3ub", &["glSecondaryColor3ubEXT"])),
SecondaryColor3ubv: FnPtr::new(metaloadfn("glSecondaryColor3ubv", &["glSecondaryColor3ubvEXT"])),
SecondaryColor3ui: FnPtr::new(metaloadfn("glSecondaryColor3ui", &["glSecondaryColor3uiEXT"])),
SecondaryColor3uiv: FnPtr::new(metaloadfn("glSecondaryColor3uiv", &["glSecondaryColor3uivEXT"])),
SecondaryColor3us: FnPtr::new(metaloadfn("glSecondaryColor3us", &["glSecondaryColor3usEXT"])),
SecondaryColor3usv: FnPtr::new(metaloadfn("glSecondaryColor3usv", &["glSecondaryColor3usvEXT"])),
SecondaryColorP3ui: FnPtr::new(metaloadfn("glSecondaryColorP3ui", &[])),
SecondaryColorP3uiv: FnPtr::new(metaloadfn("glSecondaryColorP3uiv", &[])),
SecondaryColorPointer: FnPtr::new(metaloadfn("glSecondaryColorPointer", &["glSecondaryColorPointerEXT"])),
SelectBuffer: FnPtr::new(metaloadfn("glSelectBuffer", &[])),
SetFenceAPPLE: FnPtr::new(metaloadfn("glSetFenceAPPLE", &[])),
ShadeModel: FnPtr::new(metaloadfn("glShadeModel", &[])),
ShaderBinary: FnPtr::new(metaloadfn("glShaderBinary", &[])),
ShaderSource: FnPtr::new(metaloadfn("glShaderSource", &["glShaderSourceARB"])),
ShaderStorageBlockBinding: FnPtr::new(metaloadfn("glShaderStorageBlockBinding", &[])),
StencilFunc: FnPtr::new(metaloadfn("glStencilFunc", &[])),
StencilFuncSeparate: FnPtr::new(metaloadfn("glStencilFuncSeparate", &[])),
StencilMask: FnPtr::new(metaloadfn("glStencilMask", &[])),
StencilMaskSeparate: FnPtr::new(metaloadfn("glStencilMaskSeparate", &[])),
StencilOp: FnPtr::new(metaloadfn("glStencilOp", &[])),
StencilOpSeparate: FnPtr::new(metaloadfn("glStencilOpSeparate", &["glStencilOpSeparateATI"])),
TestFenceAPPLE: FnPtr::new(metaloadfn("glTestFenceAPPLE", &[])),
TestObjectAPPLE: FnPtr::new(metaloadfn("glTestObjectAPPLE", &[])),
TexBuffer: FnPtr::new(metaloadfn("glTexBuffer", &["glTexBufferARB", "glTexBufferEXT", "glTexBufferOES"])),
TexCoord1d: FnPtr::new(metaloadfn("glTexCoord1d", &[])),
TexCoord1dv: FnPtr::new(metaloadfn("glTexCoord1dv", &[])),
TexCoord1f: FnPtr::new(metaloadfn("glTexCoord1f", &[])),
TexCoord1fv: FnPtr::new(metaloadfn("glTexCoord1fv", &[])),
TexCoord1i: FnPtr::new(metaloadfn("glTexCoord1i", &[])),
TexCoord1iv: FnPtr::new(metaloadfn("glTexCoord1iv", &[])),
TexCoord1s: FnPtr::new(metaloadfn("glTexCoord1s", &[])),
TexCoord1sv: FnPtr::new(metaloadfn("glTexCoord1sv", &[])),
TexCoord2d: FnPtr::new(metaloadfn("glTexCoord2d", &[])),
TexCoord2dv: FnPtr::new(metaloadfn("glTexCoord2dv", &[])),
TexCoord2f: FnPtr::new(metaloadfn("glTexCoord2f", &[])),
TexCoord2fv: FnPtr::new(metaloadfn("glTexCoord2fv", &[])),
TexCoord2i: FnPtr::new(metaloadfn("glTexCoord2i", &[])),
TexCoord2iv: FnPtr::new(metaloadfn("glTexCoord2iv", &[])),
TexCoord2s: FnPtr::new(metaloadfn("glTexCoord2s", &[])),
TexCoord2sv: FnPtr::new(metaloadfn("glTexCoord2sv", &[])),
TexCoord3d: FnPtr::new(metaloadfn("glTexCoord3d", &[])),
TexCoord3dv: FnPtr::new(metaloadfn("glTexCoord3dv", &[])),
TexCoord3f: FnPtr::new(metaloadfn("glTexCoord3f", &[])),
TexCoord3fv: FnPtr::new(metaloadfn("glTexCoord3fv", &[])),
TexCoord3i: FnPtr::new(metaloadfn("glTexCoord3i", &[])),
TexCoord3iv: FnPtr::new(metaloadfn("glTexCoord3iv", &[])),
TexCoord3s: FnPtr::new(metaloadfn("glTexCoord3s", &[])),
TexCoord3sv: FnPtr::new(metaloadfn("glTexCoord3sv", &[])),
TexCoord4d: FnPtr::new(metaloadfn("glTexCoord4d", &[])),
TexCoord4dv: FnPtr::new(metaloadfn("glTexCoord4dv", &[])),
TexCoord4f: FnPtr::new(metaloadfn("glTexCoord4f", &[])),
TexCoord4fv: FnPtr::new(metaloadfn("glTexCoord4fv", &[])),
TexCoord4i: FnPtr::new(metaloadfn("glTexCoord4i", &[])),
TexCoord4iv: FnPtr::new(metaloadfn("glTexCoord4iv", &[])),
TexCoord4s: FnPtr::new(metaloadfn("glTexCoord4s", &[])),
TexCoord4sv: FnPtr::new(metaloadfn("glTexCoord4sv", &[])),
TexCoordP1ui: FnPtr::new(metaloadfn("glTexCoordP1ui", &[])),
TexCoordP1uiv: FnPtr::new(metaloadfn("glTexCoordP1uiv", &[])),
TexCoordP2ui: FnPtr::new(metaloadfn("glTexCoordP2ui", &[])),
TexCoordP2uiv: FnPtr::new(metaloadfn("glTexCoordP2uiv", &[])),
TexCoordP3ui: FnPtr::new(metaloadfn("glTexCoordP3ui", &[])),
TexCoordP3uiv: FnPtr::new(metaloadfn("glTexCoordP3uiv", &[])),
TexCoordP4ui: FnPtr::new(metaloadfn("glTexCoordP4ui", &[])),
TexCoordP4uiv: FnPtr::new(metaloadfn("glTexCoordP4uiv", &[])),
TexCoordPointer: FnPtr::new(metaloadfn("glTexCoordPointer", &[])),
TexEnvf: FnPtr::new(metaloadfn("glTexEnvf", &[])),
TexEnvfv: FnPtr::new(metaloadfn("glTexEnvfv", &[])),
TexEnvi: FnPtr::new(metaloadfn("glTexEnvi", &[])),
TexEnviv: FnPtr::new(metaloadfn("glTexEnviv", &[])),
TexGend: FnPtr::new(metaloadfn("glTexGend", &[])),
TexGendv: FnPtr::new(metaloadfn("glTexGendv", &[])),
TexGenf: FnPtr::new(metaloadfn("glTexGenf", &[])),
TexGenfv: FnPtr::new(metaloadfn("glTexGenfv", &[])),
TexGeni: FnPtr::new(metaloadfn("glTexGeni", &[])),
TexGeniv: FnPtr::new(metaloadfn("glTexGeniv", &[])),
TexImage1D: FnPtr::new(metaloadfn("glTexImage1D", &[])),
TexImage2D: FnPtr::new(metaloadfn("glTexImage2D", &[])),
TexImage2DMultisample: FnPtr::new(metaloadfn("glTexImage2DMultisample", &[])),
TexImage3D: FnPtr::new(metaloadfn("glTexImage3D", &["glTexImage3DEXT"])),
TexImage3DMultisample: FnPtr::new(metaloadfn("glTexImage3DMultisample", &[])),
TexParameterIiv: FnPtr::new(metaloadfn("glTexParameterIiv", &["glTexParameterIivEXT", "glTexParameterIivOES"])),
TexParameterIuiv: FnPtr::new(metaloadfn("glTexParameterIuiv", &["glTexParameterIuivEXT", "glTexParameterIuivOES"])),
TexParameterf: FnPtr::new(metaloadfn("glTexParameterf", &[])),
TexParameterfv: FnPtr::new(metaloadfn("glTexParameterfv", &[])),
TexParameteri: FnPtr::new(metaloadfn("glTexParameteri", &[])),
TexParameteriv: FnPtr::new(metaloadfn("glTexParameteriv", &[])),
TexStorage1D: FnPtr::new(metaloadfn("glTexStorage1D", &["glTexStorage1DEXT"])),
TexStorage1DEXT: FnPtr::new(metaloadfn("glTexStorage1DEXT", &[])),
TexStorage2D: FnPtr::new(metaloadfn("glTexStorage2D", &["glTexStorage2DEXT"])),
TexStorage2DEXT: FnPtr::new(metaloadfn("glTexStorage2DEXT", &[])),
TexStorage2DMultisample: FnPtr::new(metaloadfn("glTexStorage2DMultisample", &[])),
TexStorage3D: FnPtr::new(metaloadfn("glTexStorage3D", &["glTexStorage3DEXT"])),
TexStorage3DEXT: FnPtr::new(metaloadfn("glTexStorage3DEXT", &[])),
TexSubImage1D: FnPtr::new(metaloadfn("glTexSubImage1D", &["glTexSubImage1DEXT"])),
TexSubImage2D: FnPtr::new(metaloadfn("glTexSubImage2D", &["glTexSubImage2DEXT"])),
TexSubImage3D: FnPtr::new(metaloadfn("glTexSubImage3D", &["glTexSubImage3DEXT"])),
TextureRangeAPPLE: FnPtr::new(metaloadfn("glTextureRangeAPPLE", &[])),
TextureStorage1DEXT: FnPtr::new(metaloadfn("glTextureStorage1DEXT", &[])),
TextureStorage2DEXT: FnPtr::new(metaloadfn("glTextureStorage2DEXT", &[])),
TextureStorage3DEXT: FnPtr::new(metaloadfn("glTextureStorage3DEXT", &[])),
TransformFeedbackVaryings: FnPtr::new(metaloadfn("glTransformFeedbackVaryings", &["glTransformFeedbackVaryingsEXT"])),
Translated: FnPtr::new(metaloadfn("glTranslated", &[])),
Translatef: FnPtr::new(metaloadfn("glTranslatef", &[])),
Uniform1f: FnPtr::new(metaloadfn("glUniform1f", &["glUniform1fARB"])),
Uniform1fv: FnPtr::new(metaloadfn("glUniform1fv", &["glUniform1fvARB"])),
Uniform1i: FnPtr::new(metaloadfn("glUniform1i", &["glUniform1iARB"])),
Uniform1iv: FnPtr::new(metaloadfn("glUniform1iv", &["glUniform1ivARB"])),
Uniform1ui: FnPtr::new(metaloadfn("glUniform1ui", &["glUniform1uiEXT"])),
Uniform1uiv: FnPtr::new(metaloadfn("glUniform1uiv", &["glUniform1uivEXT"])),
Uniform2f: FnPtr::new(metaloadfn("glUniform2f", &["glUniform2fARB"])),
Uniform2fv: FnPtr::new(metaloadfn("glUniform2fv", &["glUniform2fvARB"])),
Uniform2i: FnPtr::new(metaloadfn("glUniform2i", &["glUniform2iARB"])),
Uniform2iv: FnPtr::new(metaloadfn("glUniform2iv", &["glUniform2ivARB"])),
Uniform2ui: FnPtr::new(metaloadfn("glUniform2ui", &["glUniform2uiEXT"])),
Uniform2uiv: FnPtr::new(metaloadfn("glUniform2uiv", &["glUniform2uivEXT"])),
Uniform3f: FnPtr::new(metaloadfn("glUniform3f", &["glUniform3fARB"])),
Uniform3fv: FnPtr::new(metaloadfn("glUniform3fv", &["glUniform3fvARB"])),
Uniform3i: FnPtr::new(metaloadfn("glUniform3i", &["glUniform3iARB"])),
Uniform3iv: FnPtr::new(metaloadfn("glUniform3iv", &["glUniform3ivARB"])),
Uniform3ui: FnPtr::new(metaloadfn("glUniform3ui", &["glUniform3uiEXT"])),
Uniform3uiv: FnPtr::new(metaloadfn("glUniform3uiv", &["glUniform3uivEXT"])),
Uniform4f: FnPtr::new(metaloadfn("glUniform4f", &["glUniform4fARB"])),
Uniform4fv: FnPtr::new(metaloadfn("glUniform4fv", &["glUniform4fvARB"])),
Uniform4i: FnPtr::new(metaloadfn("glUniform4i", &["glUniform4iARB"])),
Uniform4iv: FnPtr::new(metaloadfn("glUniform4iv", &["glUniform4ivARB"])),
Uniform4ui: FnPtr::new(metaloadfn("glUniform4ui", &["glUniform4uiEXT"])),
Uniform4uiv: FnPtr::new(metaloadfn("glUniform4uiv", &["glUniform4uivEXT"])),
UniformBlockBinding: FnPtr::new(metaloadfn("glUniformBlockBinding", &[])),
UniformMatrix2fv: FnPtr::new(metaloadfn("glUniformMatrix2fv", &["glUniformMatrix2fvARB"])),
UniformMatrix2x3fv: FnPtr::new(metaloadfn("glUniformMatrix2x3fv", &["glUniformMatrix2x3fvNV"])),
UniformMatrix2x4fv: FnPtr::new(metaloadfn("glUniformMatrix2x4fv", &["glUniformMatrix2x4fvNV"])),
UniformMatrix3fv: FnPtr::new(metaloadfn("glUniformMatrix3fv", &["glUniformMatrix3fvARB"])),
UniformMatrix3x2fv: FnPtr::new(metaloadfn("glUniformMatrix3x2fv", &["glUniformMatrix3x2fvNV"])),
UniformMatrix3x4fv: FnPtr::new(metaloadfn("glUniformMatrix3x4fv", &["glUniformMatrix3x4fvNV"])),
UniformMatrix4fv: FnPtr::new(metaloadfn("glUniformMatrix4fv", &["glUniformMatrix4fvARB"])),
UniformMatrix4x2fv: FnPtr::new(metaloadfn("glUniformMatrix4x2fv", &["glUniformMatrix4x2fvNV"])),
UniformMatrix4x3fv: FnPtr::new(metaloadfn("glUniformMatrix4x3fv", &["glUniformMatrix4x3fvNV"])),
UnmapBuffer: FnPtr::new(metaloadfn("glUnmapBuffer", &["glUnmapBufferARB", "glUnmapBufferOES"])),
UseProgram: FnPtr::new(metaloadfn("glUseProgram", &["glUseProgramObjectARB"])),
UseProgramStages: FnPtr::new(metaloadfn("glUseProgramStages", &[])),
ValidateProgram: FnPtr::new(metaloadfn("glValidateProgram", &["glValidateProgramARB"])),
ValidateProgramPipeline: FnPtr::new(metaloadfn("glValidateProgramPipeline", &[])),
Vertex2d: FnPtr::new(metaloadfn("glVertex2d", &[])),
Vertex2dv: FnPtr::new(metaloadfn("glVertex2dv", &[])),
Vertex2f: FnPtr::new(metaloadfn("glVertex2f", &[])),
Vertex2fv: FnPtr::new(metaloadfn("glVertex2fv", &[])),
Vertex2i: FnPtr::new(metaloadfn("glVertex2i", &[])),
Vertex2iv: FnPtr::new(metaloadfn("glVertex2iv", &[])),
Vertex2s: FnPtr::new(metaloadfn("glVertex2s", &[])),
Vertex2sv: FnPtr::new(metaloadfn("glVertex2sv", &[])),
Vertex3d: FnPtr::new(metaloadfn("glVertex3d", &[])),
Vertex3dv: FnPtr::new(metaloadfn("glVertex3dv", &[])),
Vertex3f: FnPtr::new(metaloadfn("glVertex3f", &[])),
Vertex3fv: FnPtr::new(metaloadfn("glVertex3fv", &[])),
Vertex3i: FnPtr::new(metaloadfn("glVertex3i", &[])),
Vertex3iv: FnPtr::new(metaloadfn("glVertex3iv", &[])),
Vertex3s: FnPtr::new(metaloadfn("glVertex3s", &[])),
Vertex3sv: FnPtr::new(metaloadfn("glVertex3sv", &[])),
Vertex4d: FnPtr::new(metaloadfn("glVertex4d", &[])),
Vertex4dv: FnPtr::new(metaloadfn("glVertex4dv", &[])),
Vertex4f: FnPtr::new(metaloadfn("glVertex4f", &[])),
Vertex4fv: FnPtr::new(metaloadfn("glVertex4fv", &[])),
Vertex4i: FnPtr::new(metaloadfn("glVertex4i", &[])),
Vertex4iv: FnPtr::new(metaloadfn("glVertex4iv", &[])),
Vertex4s: FnPtr::new(metaloadfn("glVertex4s", &[])),
Vertex4sv: FnPtr::new(metaloadfn("glVertex4sv", &[])),
VertexAttrib1d: FnPtr::new(metaloadfn("glVertexAttrib1d", &["glVertexAttrib1dARB", "glVertexAttrib1dNV"])),
VertexAttrib1dv: FnPtr::new(metaloadfn("glVertexAttrib1dv", &["glVertexAttrib1dvARB", "glVertexAttrib1dvNV"])),
VertexAttrib1f: FnPtr::new(metaloadfn("glVertexAttrib1f", &["glVertexAttrib1fARB", "glVertexAttrib1fNV"])),
VertexAttrib1fv: FnPtr::new(metaloadfn("glVertexAttrib1fv", &["glVertexAttrib1fvARB", "glVertexAttrib1fvNV"])),
VertexAttrib1s: FnPtr::new(metaloadfn("glVertexAttrib1s", &["glVertexAttrib1sARB", "glVertexAttrib1sNV"])),
VertexAttrib1sv: FnPtr::new(metaloadfn("glVertexAttrib1sv", &["glVertexAttrib1svARB", "glVertexAttrib1svNV"])),
VertexAttrib2d: FnPtr::new(metaloadfn("glVertexAttrib2d", &["glVertexAttrib2dARB", "glVertexAttrib2dNV"])),
VertexAttrib2dv: FnPtr::new(metaloadfn("glVertexAttrib2dv", &["glVertexAttrib2dvARB", "glVertexAttrib2dvNV"])),
VertexAttrib2f: FnPtr::new(metaloadfn("glVertexAttrib2f", &["glVertexAttrib2fARB", "glVertexAttrib2fNV"])),
VertexAttrib2fv: FnPtr::new(metaloadfn("glVertexAttrib2fv", &["glVertexAttrib2fvARB", "glVertexAttrib2fvNV"])),
VertexAttrib2s: FnPtr::new(metaloadfn("glVertexAttrib2s", &["glVertexAttrib2sARB", "glVertexAttrib2sNV"])),
VertexAttrib2sv: FnPtr::new(metaloadfn("glVertexAttrib2sv", &["glVertexAttrib2svARB", "glVertexAttrib2svNV"])),
VertexAttrib3d: FnPtr::new(metaloadfn("glVertexAttrib3d", &["glVertexAttrib3dARB", "glVertexAttrib3dNV"])),
VertexAttrib3dv: FnPtr::new(metaloadfn("glVertexAttrib3dv", &["glVertexAttrib3dvARB", "glVertexAttrib3dvNV"])),
VertexAttrib3f: FnPtr::new(metaloadfn("glVertexAttrib3f", &["glVertexAttrib3fARB", "glVertexAttrib3fNV"])),
VertexAttrib3fv: FnPtr::new(metaloadfn("glVertexAttrib3fv", &["glVertexAttrib3fvARB", "glVertexAttrib3fvNV"])),
VertexAttrib3s: FnPtr::new(metaloadfn("glVertexAttrib3s", &["glVertexAttrib3sARB", "glVertexAttrib3sNV"])),
VertexAttrib3sv: FnPtr::new(metaloadfn("glVertexAttrib3sv", &["glVertexAttrib3svARB", "glVertexAttrib3svNV"])),
VertexAttrib4Nbv: FnPtr::new(metaloadfn("glVertexAttrib4Nbv", &["glVertexAttrib4NbvARB"])),
VertexAttrib4Niv: FnPtr::new(metaloadfn("glVertexAttrib4Niv", &["glVertexAttrib4NivARB"])),
VertexAttrib4Nsv: FnPtr::new(metaloadfn("glVertexAttrib4Nsv", &["glVertexAttrib4NsvARB"])),
VertexAttrib4Nub: FnPtr::new(metaloadfn("glVertexAttrib4Nub", &["glVertexAttrib4NubARB", "glVertexAttrib4ubNV"])),
VertexAttrib4Nubv: FnPtr::new(metaloadfn("glVertexAttrib4Nubv", &["glVertexAttrib4NubvARB", "glVertexAttrib4ubvNV"])),
VertexAttrib4Nuiv: FnPtr::new(metaloadfn("glVertexAttrib4Nuiv", &["glVertexAttrib4NuivARB"])),
VertexAttrib4Nusv: FnPtr::new(metaloadfn("glVertexAttrib4Nusv", &["glVertexAttrib4NusvARB"])),
VertexAttrib4bv: FnPtr::new(metaloadfn("glVertexAttrib4bv", &["glVertexAttrib4bvARB"])),
VertexAttrib4d: FnPtr::new(metaloadfn("glVertexAttrib4d", &["glVertexAttrib4dARB", "glVertexAttrib4dNV"])),
VertexAttrib4dv: FnPtr::new(metaloadfn("glVertexAttrib4dv", &["glVertexAttrib4dvARB", "glVertexAttrib4dvNV"])),
VertexAttrib4f: FnPtr::new(metaloadfn("glVertexAttrib4f", &["glVertexAttrib4fARB", "glVertexAttrib4fNV"])),
VertexAttrib4fv: FnPtr::new(metaloadfn("glVertexAttrib4fv", &["glVertexAttrib4fvARB", "glVertexAttrib4fvNV"])),
VertexAttrib4iv: FnPtr::new(metaloadfn("glVertexAttrib4iv", &["glVertexAttrib4ivARB"])),
VertexAttrib4s: FnPtr::new(metaloadfn("glVertexAttrib4s", &["glVertexAttrib4sARB", "glVertexAttrib4sNV"])),
VertexAttrib4sv: FnPtr::new(metaloadfn("glVertexAttrib4sv", &["glVertexAttrib4svARB", "glVertexAttrib4svNV"])),
VertexAttrib4ubv: FnPtr::new(metaloadfn("glVertexAttrib4ubv", &["glVertexAttrib4ubvARB"])),
VertexAttrib4uiv: FnPtr::new(metaloadfn("glVertexAttrib4uiv", &["glVertexAttrib4uivARB"])),
VertexAttrib4usv: FnPtr::new(metaloadfn("glVertexAttrib4usv", &["glVertexAttrib4usvARB"])),
VertexAttribBinding: FnPtr::new(metaloadfn("glVertexAttribBinding", &[])),
VertexAttribDivisor: FnPtr::new(metaloadfn("glVertexAttribDivisor", &["glVertexAttribDivisorANGLE", "glVertexAttribDivisorARB", "glVertexAttribDivisorEXT", "glVertexAttribDivisorNV"])),
VertexAttribFormat: FnPtr::new(metaloadfn("glVertexAttribFormat", &[])),
VertexAttribI1i: FnPtr::new(metaloadfn("glVertexAttribI1i", &["glVertexAttribI1iEXT"])),
VertexAttribI1iv: FnPtr::new(metaloadfn("glVertexAttribI1iv", &["glVertexAttribI1ivEXT"])),
VertexAttribI1ui: FnPtr::new(metaloadfn("glVertexAttribI1ui", &["glVertexAttribI1uiEXT"])),
VertexAttribI1uiv: FnPtr::new(metaloadfn("glVertexAttribI1uiv", &["glVertexAttribI1uivEXT"])),
VertexAttribI2i: FnPtr::new(metaloadfn("glVertexAttribI2i", &["glVertexAttribI2iEXT"])),
VertexAttribI2iv: FnPtr::new(metaloadfn("glVertexAttribI2iv", &["glVertexAttribI2ivEXT"])),
VertexAttribI2ui: FnPtr::new(metaloadfn("glVertexAttribI2ui", &["glVertexAttribI2uiEXT"])),
VertexAttribI2uiv: FnPtr::new(metaloadfn("glVertexAttribI2uiv", &["glVertexAttribI2uivEXT"])),
VertexAttribI3i: FnPtr::new(metaloadfn("glVertexAttribI3i", &["glVertexAttribI3iEXT"])),
VertexAttribI3iv: FnPtr::new(metaloadfn("glVertexAttribI3iv", &["glVertexAttribI3ivEXT"])),
VertexAttribI3ui: FnPtr::new(metaloadfn("glVertexAttribI3ui", &["glVertexAttribI3uiEXT"])),
VertexAttribI3uiv: FnPtr::new(metaloadfn("glVertexAttribI3uiv", &["glVertexAttribI3uivEXT"])),
VertexAttribI4bv: FnPtr::new(metaloadfn("glVertexAttribI4bv", &["glVertexAttribI4bvEXT"])),
VertexAttribI4i: FnPtr::new(metaloadfn("glVertexAttribI4i", &["glVertexAttribI4iEXT"])),
VertexAttribI4iv: FnPtr::new(metaloadfn("glVertexAttribI4iv", &["glVertexAttribI4ivEXT"])),
VertexAttribI4sv: FnPtr::new(metaloadfn("glVertexAttribI4sv", &["glVertexAttribI4svEXT"])),
VertexAttribI4ubv: FnPtr::new(metaloadfn("glVertexAttribI4ubv", &["glVertexAttribI4ubvEXT"])),
VertexAttribI4ui: FnPtr::new(metaloadfn("glVertexAttribI4ui", &["glVertexAttribI4uiEXT"])),
VertexAttribI4uiv: FnPtr::new(metaloadfn("glVertexAttribI4uiv", &["glVertexAttribI4uivEXT"])),
VertexAttribI4usv: FnPtr::new(metaloadfn("glVertexAttribI4usv", &["glVertexAttribI4usvEXT"])),
VertexAttribIFormat: FnPtr::new(metaloadfn("glVertexAttribIFormat", &[])),
VertexAttribIPointer: FnPtr::new(metaloadfn("glVertexAttribIPointer", &["glVertexAttribIPointerEXT"])),
VertexAttribP1ui: FnPtr::new(metaloadfn("glVertexAttribP1ui", &[])),
VertexAttribP1uiv: FnPtr::new(metaloadfn("glVertexAttribP1uiv", &[])),
VertexAttribP2ui: FnPtr::new(metaloadfn("glVertexAttribP2ui", &[])),
VertexAttribP2uiv: FnPtr::new(metaloadfn("glVertexAttribP2uiv", &[])),
VertexAttribP3ui: FnPtr::new(metaloadfn("glVertexAttribP3ui", &[])),
VertexAttribP3uiv: FnPtr::new(metaloadfn("glVertexAttribP3uiv", &[])),
VertexAttribP4ui: FnPtr::new(metaloadfn("glVertexAttribP4ui", &[])),
VertexAttribP4uiv: FnPtr::new(metaloadfn("glVertexAttribP4uiv", &[])),
VertexAttribPointer: FnPtr::new(metaloadfn("glVertexAttribPointer", &["glVertexAttribPointerARB"])),
VertexBindingDivisor: FnPtr::new(metaloadfn("glVertexBindingDivisor", &[])),
VertexP2ui: FnPtr::new(metaloadfn("glVertexP2ui", &[])),
VertexP2uiv: FnPtr::new(metaloadfn("glVertexP2uiv", &[])),
VertexP3ui: FnPtr::new(metaloadfn("glVertexP3ui", &[])),
VertexP3uiv: FnPtr::new(metaloadfn("glVertexP3uiv", &[])),
VertexP4ui: FnPtr::new(metaloadfn("glVertexP4ui", &[])),
VertexP4uiv: FnPtr::new(metaloadfn("glVertexP4uiv", &[])),
VertexPointer: FnPtr::new(metaloadfn("glVertexPointer", &[])),
Viewport: FnPtr::new(metaloadfn("glViewport", &[])),
WaitSync: FnPtr::new(metaloadfn("glWaitSync", &["glWaitSyncAPPLE"])),
WindowPos2d: FnPtr::new(metaloadfn("glWindowPos2d", &["glWindowPos2dARB", "glWindowPos2dMESA"])),
WindowPos2dv: FnPtr::new(metaloadfn("glWindowPos2dv", &["glWindowPos2dvARB", "glWindowPos2dvMESA"])),
WindowPos2f: FnPtr::new(metaloadfn("glWindowPos2f", &["glWindowPos2fARB", "glWindowPos2fMESA"])),
WindowPos2fv: FnPtr::new(metaloadfn("glWindowPos2fv", &["glWindowPos2fvARB", "glWindowPos2fvMESA"])),
WindowPos2i: FnPtr::new(metaloadfn("glWindowPos2i", &["glWindowPos2iARB", "glWindowPos2iMESA"])),
WindowPos2iv: FnPtr::new(metaloadfn("glWindowPos2iv", &["glWindowPos2ivARB", "glWindowPos2ivMESA"])),
WindowPos2s: FnPtr::new(metaloadfn("glWindowPos2s", &["glWindowPos2sARB", "glWindowPos2sMESA"])),
WindowPos2sv: FnPtr::new(metaloadfn("glWindowPos2sv", &["glWindowPos2svARB", "glWindowPos2svMESA"])),
WindowPos3d: FnPtr::new(metaloadfn("glWindowPos3d", &["glWindowPos3dARB", "glWindowPos3dMESA"])),
WindowPos3dv: FnPtr::new(metaloadfn("glWindowPos3dv", &["glWindowPos3dvARB", "glWindowPos3dvMESA"])),
WindowPos3f: FnPtr::new(metaloadfn("glWindowPos3f", &["glWindowPos3fARB", "glWindowPos3fMESA"])),
WindowPos3fv: FnPtr::new(metaloadfn("glWindowPos3fv", &["glWindowPos3fvARB", "glWindowPos3fvMESA"])),
WindowPos3i: FnPtr::new(metaloadfn("glWindowPos3i", &["glWindowPos3iARB", "glWindowPos3iMESA"])),
WindowPos3iv: FnPtr::new(metaloadfn("glWindowPos3iv", &["glWindowPos3ivARB", "glWindowPos3ivMESA"])),
WindowPos3s: FnPtr::new(metaloadfn("glWindowPos3s", &["glWindowPos3sARB", "glWindowPos3sMESA"])),
WindowPos3sv: FnPtr::new(metaloadfn("glWindowPos3sv", &["glWindowPos3svARB", "glWindowPos3svMESA"])),
_priv: ()
}
        }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Accum(&self, op: types::GLenum, value: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLfloat) -> ()>(self.Accum.f)(op, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ActiveShaderProgram(&self, pipeline: types::GLuint, program: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint) -> ()>(self.ActiveShaderProgram.f)(pipeline, program) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ActiveTexture(&self, texture: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.ActiveTexture.f)(texture) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn AlphaFunc(&self, func: types::GLenum, ref_: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLfloat) -> ()>(self.AlphaFunc.f)(func, ref_) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn AreTexturesResident(&self, n: types::GLsizei, textures: *const types::GLuint, residences: *mut types::GLboolean) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *const types::GLuint, *mut types::GLboolean) -> types::GLboolean>(self.AreTexturesResident.f)(n, textures, residences) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ArrayElement(&self, i: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint) -> ()>(self.ArrayElement.f)(i) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn AttachShader(&self, program: types::GLuint, shader: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint) -> ()>(self.AttachShader.f)(program, shader) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Begin(&self, mode: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.Begin.f)(mode) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BeginConditionalRender(&self, id: types::GLuint, mode: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum) -> ()>(self.BeginConditionalRender.f)(id, mode) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BeginQuery(&self, target: types::GLenum, id: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint) -> ()>(self.BeginQuery.f)(target, id) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BeginQueryEXT(&self, target: types::GLenum, id: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint) -> ()>(self.BeginQueryEXT.f)(target, id) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BeginTransformFeedback(&self, primitiveMode: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.BeginTransformFeedback.f)(primitiveMode) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BindAttribLocation(&self, program: types::GLuint, index: types::GLuint, name: *const types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint, *const types::GLchar) -> ()>(self.BindAttribLocation.f)(program, index, name) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BindBuffer(&self, target: types::GLenum, buffer: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint) -> ()>(self.BindBuffer.f)(target, buffer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BindBufferBase(&self, target: types::GLenum, index: types::GLuint, buffer: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint, types::GLuint) -> ()>(self.BindBufferBase.f)(target, index, buffer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BindBufferRange(&self, target: types::GLenum, index: types::GLuint, buffer: types::GLuint, offset: types::GLintptr, size: types::GLsizeiptr) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint, types::GLuint, types::GLintptr, types::GLsizeiptr) -> ()>(self.BindBufferRange.f)(target, index, buffer, offset, size) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BindFragDataLocation(&self, program: types::GLuint, color: types::GLuint, name: *const types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint, *const types::GLchar) -> ()>(self.BindFragDataLocation.f)(program, color, name) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BindFragDataLocationIndexed(&self, program: types::GLuint, colorNumber: types::GLuint, index: types::GLuint, name: *const types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint, types::GLuint, *const types::GLchar) -> ()>(self.BindFragDataLocationIndexed.f)(program, colorNumber, index, name) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BindFramebuffer(&self, target: types::GLenum, framebuffer: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint) -> ()>(self.BindFramebuffer.f)(target, framebuffer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BindImageTexture(&self, unit: types::GLuint, texture: types::GLuint, level: types::GLint, layered: types::GLboolean, layer: types::GLint, access: types::GLenum, format: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint, types::GLint, types::GLboolean, types::GLint, types::GLenum, types::GLenum) -> ()>(self.BindImageTexture.f)(unit, texture, level, layered, layer, access, format) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BindProgramPipeline(&self, pipeline: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.BindProgramPipeline.f)(pipeline) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BindRenderbuffer(&self, target: types::GLenum, renderbuffer: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint) -> ()>(self.BindRenderbuffer.f)(target, renderbuffer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BindSampler(&self, unit: types::GLuint, sampler: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint) -> ()>(self.BindSampler.f)(unit, sampler) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BindTexture(&self, target: types::GLenum, texture: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint) -> ()>(self.BindTexture.f)(target, texture) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BindTransformFeedback(&self, target: types::GLenum, id: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint) -> ()>(self.BindTransformFeedback.f)(target, id) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BindVertexArray(&self, array: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.BindVertexArray.f)(array) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BindVertexArrayAPPLE(&self, array: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.BindVertexArrayAPPLE.f)(array) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BindVertexBuffer(&self, bindingindex: types::GLuint, buffer: types::GLuint, offset: types::GLintptr, stride: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint, types::GLintptr, types::GLsizei) -> ()>(self.BindVertexBuffer.f)(bindingindex, buffer, offset, stride) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Bitmap(&self, width: types::GLsizei, height: types::GLsizei, xorig: types::GLfloat, yorig: types::GLfloat, xmove: types::GLfloat, ymove: types::GLfloat, bitmap: *const types::GLubyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, types::GLsizei, types::GLfloat, types::GLfloat, types::GLfloat, types::GLfloat, *const types::GLubyte) -> ()>(self.Bitmap.f)(width, height, xorig, yorig, xmove, ymove, bitmap) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BlendBarrierKHR(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.BlendBarrierKHR.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BlendColor(&self, red: types::GLfloat, green: types::GLfloat, blue: types::GLfloat, alpha: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.BlendColor.f)(red, green, blue, alpha) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BlendEquation(&self, mode: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.BlendEquation.f)(mode) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BlendEquationSeparate(&self, modeRGB: types::GLenum, modeAlpha: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum) -> ()>(self.BlendEquationSeparate.f)(modeRGB, modeAlpha) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BlendFunc(&self, sfactor: types::GLenum, dfactor: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum) -> ()>(self.BlendFunc.f)(sfactor, dfactor) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BlendFuncSeparate(&self, sfactorRGB: types::GLenum, dfactorRGB: types::GLenum, sfactorAlpha: types::GLenum, dfactorAlpha: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLenum, types::GLenum) -> ()>(self.BlendFuncSeparate.f)(sfactorRGB, dfactorRGB, sfactorAlpha, dfactorAlpha) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BlitFramebuffer(&self, srcX0: types::GLint, srcY0: types::GLint, srcX1: types::GLint, srcY1: types::GLint, dstX0: types::GLint, dstY0: types::GLint, dstX1: types::GLint, dstY1: types::GLint, mask: types::GLbitfield, filter: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLbitfield, types::GLenum) -> ()>(self.BlitFramebuffer.f)(srcX0, srcY0, srcX1, srcY1, dstX0, dstY0, dstX1, dstY1, mask, filter) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BufferData(&self, target: types::GLenum, size: types::GLsizeiptr, data: *const __gl_imports::raw::c_void, usage: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizeiptr, *const __gl_imports::raw::c_void, types::GLenum) -> ()>(self.BufferData.f)(target, size, data, usage) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BufferStorage(&self, target: types::GLenum, size: types::GLsizeiptr, data: *const __gl_imports::raw::c_void, flags: types::GLbitfield) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizeiptr, *const __gl_imports::raw::c_void, types::GLbitfield) -> ()>(self.BufferStorage.f)(target, size, data, flags) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BufferStorageEXT(&self, target: types::GLenum, size: types::GLsizeiptr, data: *const __gl_imports::raw::c_void, flags: types::GLbitfield) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizeiptr, *const __gl_imports::raw::c_void, types::GLbitfield) -> ()>(self.BufferStorageEXT.f)(target, size, data, flags) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BufferSubData(&self, target: types::GLenum, offset: types::GLintptr, size: types::GLsizeiptr, data: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLintptr, types::GLsizeiptr, *const __gl_imports::raw::c_void) -> ()>(self.BufferSubData.f)(target, offset, size, data) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CallList(&self, list: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.CallList.f)(list) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CallLists(&self, n: types::GLsizei, type_: types::GLenum, lists: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, types::GLenum, *const __gl_imports::raw::c_void) -> ()>(self.CallLists.f)(n, type_, lists) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CheckFramebufferStatus(&self, target: types::GLenum) -> types::GLenum { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> types::GLenum>(self.CheckFramebufferStatus.f)(target) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ClampColor(&self, target: types::GLenum, clamp: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum) -> ()>(self.ClampColor.f)(target, clamp) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Clear(&self, mask: types::GLbitfield) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLbitfield) -> ()>(self.Clear.f)(mask) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ClearAccum(&self, red: types::GLfloat, green: types::GLfloat, blue: types::GLfloat, alpha: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.ClearAccum.f)(red, green, blue, alpha) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ClearBufferfi(&self, buffer: types::GLenum, drawbuffer: types::GLint, depth: types::GLfloat, stencil: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLfloat, types::GLint) -> ()>(self.ClearBufferfi.f)(buffer, drawbuffer, depth, stencil) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ClearBufferfv(&self, buffer: types::GLenum, drawbuffer: types::GLint, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, *const types::GLfloat) -> ()>(self.ClearBufferfv.f)(buffer, drawbuffer, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ClearBufferiv(&self, buffer: types::GLenum, drawbuffer: types::GLint, value: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, *const types::GLint) -> ()>(self.ClearBufferiv.f)(buffer, drawbuffer, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ClearBufferuiv(&self, buffer: types::GLenum, drawbuffer: types::GLint, value: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, *const types::GLuint) -> ()>(self.ClearBufferuiv.f)(buffer, drawbuffer, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ClearColor(&self, red: types::GLfloat, green: types::GLfloat, blue: types::GLfloat, alpha: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.ClearColor.f)(red, green, blue, alpha) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ClearDepth(&self, depth: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble) -> ()>(self.ClearDepth.f)(depth) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ClearDepthf(&self, d: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat) -> ()>(self.ClearDepthf.f)(d) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ClearIndex(&self, c: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat) -> ()>(self.ClearIndex.f)(c) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ClearStencil(&self, s: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint) -> ()>(self.ClearStencil.f)(s) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ClientActiveTexture(&self, texture: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.ClientActiveTexture.f)(texture) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ClientWaitSync(&self, sync: types::GLsync, flags: types::GLbitfield, timeout: types::GLuint64) -> types::GLenum { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsync, types::GLbitfield, types::GLuint64) -> types::GLenum>(self.ClientWaitSync.f)(sync, flags, timeout) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ClipPlane(&self, plane: types::GLenum, equation: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLdouble) -> ()>(self.ClipPlane.f)(plane, equation) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color3b(&self, red: types::GLbyte, green: types::GLbyte, blue: types::GLbyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLbyte, types::GLbyte, types::GLbyte) -> ()>(self.Color3b.f)(red, green, blue) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color3bv(&self, v: *const types::GLbyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLbyte) -> ()>(self.Color3bv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color3d(&self, red: types::GLdouble, green: types::GLdouble, blue: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble, types::GLdouble, types::GLdouble) -> ()>(self.Color3d.f)(red, green, blue) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color3dv(&self, v: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble) -> ()>(self.Color3dv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color3f(&self, red: types::GLfloat, green: types::GLfloat, blue: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.Color3f.f)(red, green, blue) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color3fv(&self, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat) -> ()>(self.Color3fv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color3i(&self, red: types::GLint, green: types::GLint, blue: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint, types::GLint) -> ()>(self.Color3i.f)(red, green, blue) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color3iv(&self, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLint) -> ()>(self.Color3iv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color3s(&self, red: types::GLshort, green: types::GLshort, blue: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLshort, types::GLshort, types::GLshort) -> ()>(self.Color3s.f)(red, green, blue) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color3sv(&self, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLshort) -> ()>(self.Color3sv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color3ub(&self, red: types::GLubyte, green: types::GLubyte, blue: types::GLubyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLubyte, types::GLubyte, types::GLubyte) -> ()>(self.Color3ub.f)(red, green, blue) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color3ubv(&self, v: *const types::GLubyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLubyte) -> ()>(self.Color3ubv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color3ui(&self, red: types::GLuint, green: types::GLuint, blue: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint, types::GLuint) -> ()>(self.Color3ui.f)(red, green, blue) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color3uiv(&self, v: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLuint) -> ()>(self.Color3uiv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color3us(&self, red: types::GLushort, green: types::GLushort, blue: types::GLushort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLushort, types::GLushort, types::GLushort) -> ()>(self.Color3us.f)(red, green, blue) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color3usv(&self, v: *const types::GLushort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLushort) -> ()>(self.Color3usv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color4b(&self, red: types::GLbyte, green: types::GLbyte, blue: types::GLbyte, alpha: types::GLbyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLbyte, types::GLbyte, types::GLbyte, types::GLbyte) -> ()>(self.Color4b.f)(red, green, blue, alpha) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color4bv(&self, v: *const types::GLbyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLbyte) -> ()>(self.Color4bv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color4d(&self, red: types::GLdouble, green: types::GLdouble, blue: types::GLdouble, alpha: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble, types::GLdouble, types::GLdouble, types::GLdouble) -> ()>(self.Color4d.f)(red, green, blue, alpha) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color4dv(&self, v: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble) -> ()>(self.Color4dv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color4f(&self, red: types::GLfloat, green: types::GLfloat, blue: types::GLfloat, alpha: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.Color4f.f)(red, green, blue, alpha) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color4fv(&self, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat) -> ()>(self.Color4fv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color4i(&self, red: types::GLint, green: types::GLint, blue: types::GLint, alpha: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint, types::GLint, types::GLint) -> ()>(self.Color4i.f)(red, green, blue, alpha) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color4iv(&self, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLint) -> ()>(self.Color4iv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color4s(&self, red: types::GLshort, green: types::GLshort, blue: types::GLshort, alpha: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLshort, types::GLshort, types::GLshort, types::GLshort) -> ()>(self.Color4s.f)(red, green, blue, alpha) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color4sv(&self, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLshort) -> ()>(self.Color4sv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color4ub(&self, red: types::GLubyte, green: types::GLubyte, blue: types::GLubyte, alpha: types::GLubyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLubyte, types::GLubyte, types::GLubyte, types::GLubyte) -> ()>(self.Color4ub.f)(red, green, blue, alpha) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color4ubv(&self, v: *const types::GLubyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLubyte) -> ()>(self.Color4ubv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color4ui(&self, red: types::GLuint, green: types::GLuint, blue: types::GLuint, alpha: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint, types::GLuint, types::GLuint) -> ()>(self.Color4ui.f)(red, green, blue, alpha) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color4uiv(&self, v: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLuint) -> ()>(self.Color4uiv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color4us(&self, red: types::GLushort, green: types::GLushort, blue: types::GLushort, alpha: types::GLushort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLushort, types::GLushort, types::GLushort, types::GLushort) -> ()>(self.Color4us.f)(red, green, blue, alpha) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Color4usv(&self, v: *const types::GLushort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLushort) -> ()>(self.Color4usv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ColorMask(&self, red: types::GLboolean, green: types::GLboolean, blue: types::GLboolean, alpha: types::GLboolean) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLboolean, types::GLboolean, types::GLboolean, types::GLboolean) -> ()>(self.ColorMask.f)(red, green, blue, alpha) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ColorMaski(&self, index: types::GLuint, r: types::GLboolean, g: types::GLboolean, b: types::GLboolean, a: types::GLboolean) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLboolean, types::GLboolean, types::GLboolean, types::GLboolean) -> ()>(self.ColorMaski.f)(index, r, g, b, a) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ColorMaterial(&self, face: types::GLenum, mode: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum) -> ()>(self.ColorMaterial.f)(face, mode) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ColorP3ui(&self, type_: types::GLenum, color: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint) -> ()>(self.ColorP3ui.f)(type_, color) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ColorP3uiv(&self, type_: types::GLenum, color: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLuint) -> ()>(self.ColorP3uiv.f)(type_, color) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ColorP4ui(&self, type_: types::GLenum, color: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint) -> ()>(self.ColorP4ui.f)(type_, color) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ColorP4uiv(&self, type_: types::GLenum, color: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLuint) -> ()>(self.ColorP4uiv.f)(type_, color) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ColorPointer(&self, size: types::GLint, type_: types::GLenum, stride: types::GLsizei, pointer: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLenum, types::GLsizei, *const __gl_imports::raw::c_void) -> ()>(self.ColorPointer.f)(size, type_, stride, pointer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CompileShader(&self, shader: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.CompileShader.f)(shader) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CompressedTexImage1D(&self, target: types::GLenum, level: types::GLint, internalformat: types::GLenum, width: types::GLsizei, border: types::GLint, imageSize: types::GLsizei, data: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLenum, types::GLsizei, types::GLint, types::GLsizei, *const __gl_imports::raw::c_void) -> ()>(self.CompressedTexImage1D.f)(target, level, internalformat, width, border, imageSize, data) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CompressedTexImage2D(&self, target: types::GLenum, level: types::GLint, internalformat: types::GLenum, width: types::GLsizei, height: types::GLsizei, border: types::GLint, imageSize: types::GLsizei, data: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLenum, types::GLsizei, types::GLsizei, types::GLint, types::GLsizei, *const __gl_imports::raw::c_void) -> ()>(self.CompressedTexImage2D.f)(target, level, internalformat, width, height, border, imageSize, data) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CompressedTexImage3D(&self, target: types::GLenum, level: types::GLint, internalformat: types::GLenum, width: types::GLsizei, height: types::GLsizei, depth: types::GLsizei, border: types::GLint, imageSize: types::GLsizei, data: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLenum, types::GLsizei, types::GLsizei, types::GLsizei, types::GLint, types::GLsizei, *const __gl_imports::raw::c_void) -> ()>(self.CompressedTexImage3D.f)(target, level, internalformat, width, height, depth, border, imageSize, data) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CompressedTexSubImage1D(&self, target: types::GLenum, level: types::GLint, xoffset: types::GLint, width: types::GLsizei, format: types::GLenum, imageSize: types::GLsizei, data: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLint, types::GLsizei, types::GLenum, types::GLsizei, *const __gl_imports::raw::c_void) -> ()>(self.CompressedTexSubImage1D.f)(target, level, xoffset, width, format, imageSize, data) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CompressedTexSubImage2D(&self, target: types::GLenum, level: types::GLint, xoffset: types::GLint, yoffset: types::GLint, width: types::GLsizei, height: types::GLsizei, format: types::GLenum, imageSize: types::GLsizei, data: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLint, types::GLint, types::GLsizei, types::GLsizei, types::GLenum, types::GLsizei, *const __gl_imports::raw::c_void) -> ()>(self.CompressedTexSubImage2D.f)(target, level, xoffset, yoffset, width, height, format, imageSize, data) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CompressedTexSubImage3D(&self, target: types::GLenum, level: types::GLint, xoffset: types::GLint, yoffset: types::GLint, zoffset: types::GLint, width: types::GLsizei, height: types::GLsizei, depth: types::GLsizei, format: types::GLenum, imageSize: types::GLsizei, data: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLint, types::GLint, types::GLint, types::GLsizei, types::GLsizei, types::GLsizei, types::GLenum, types::GLsizei, *const __gl_imports::raw::c_void) -> ()>(self.CompressedTexSubImage3D.f)(target, level, xoffset, yoffset, zoffset, width, height, depth, format, imageSize, data) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CopyBufferSubData(&self, readTarget: types::GLenum, writeTarget: types::GLenum, readOffset: types::GLintptr, writeOffset: types::GLintptr, size: types::GLsizeiptr) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLintptr, types::GLintptr, types::GLsizeiptr) -> ()>(self.CopyBufferSubData.f)(readTarget, writeTarget, readOffset, writeOffset, size) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CopyImageSubData(&self, srcName: types::GLuint, srcTarget: types::GLenum, srcLevel: types::GLint, srcX: types::GLint, srcY: types::GLint, srcZ: types::GLint, dstName: types::GLuint, dstTarget: types::GLenum, dstLevel: types::GLint, dstX: types::GLint, dstY: types::GLint, dstZ: types::GLint, srcWidth: types::GLsizei, srcHeight: types::GLsizei, srcDepth: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLint, types::GLint, types::GLint, types::GLint, types::GLuint, types::GLenum, types::GLint, types::GLint, types::GLint, types::GLint, types::GLsizei, types::GLsizei, types::GLsizei) -> ()>(self.CopyImageSubData.f)(srcName, srcTarget, srcLevel, srcX, srcY, srcZ, dstName, dstTarget, dstLevel, dstX, dstY, dstZ, srcWidth, srcHeight, srcDepth) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CopyImageSubDataEXT(&self, srcName: types::GLuint, srcTarget: types::GLenum, srcLevel: types::GLint, srcX: types::GLint, srcY: types::GLint, srcZ: types::GLint, dstName: types::GLuint, dstTarget: types::GLenum, dstLevel: types::GLint, dstX: types::GLint, dstY: types::GLint, dstZ: types::GLint, srcWidth: types::GLsizei, srcHeight: types::GLsizei, srcDepth: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLint, types::GLint, types::GLint, types::GLint, types::GLuint, types::GLenum, types::GLint, types::GLint, types::GLint, types::GLint, types::GLsizei, types::GLsizei, types::GLsizei) -> ()>(self.CopyImageSubDataEXT.f)(srcName, srcTarget, srcLevel, srcX, srcY, srcZ, dstName, dstTarget, dstLevel, dstX, dstY, dstZ, srcWidth, srcHeight, srcDepth) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CopyPixels(&self, x: types::GLint, y: types::GLint, width: types::GLsizei, height: types::GLsizei, type_: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint, types::GLsizei, types::GLsizei, types::GLenum) -> ()>(self.CopyPixels.f)(x, y, width, height, type_) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CopySubTexture3DANGLE(&self, sourceId: types::GLuint, sourceLevel: types::GLint, destTarget: types::GLenum, destId: types::GLuint, destLevel: types::GLint, xoffset: types::GLint, yoffset: types::GLint, zoffset: types::GLint, x: types::GLint, y: types::GLint, z: types::GLint, width: types::GLint, height: types::GLint, depth: types::GLint, unpackFlipY: types::GLboolean, unpackPremultiplyAlpha: types::GLboolean, unpackUnmultiplyAlpha: types::GLboolean) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLenum, types::GLuint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLboolean, types::GLboolean, types::GLboolean) -> ()>(self.CopySubTexture3DANGLE.f)(sourceId, sourceLevel, destTarget, destId, destLevel, xoffset, yoffset, zoffset, x, y, z, width, height, depth, unpackFlipY, unpackPremultiplyAlpha, unpackUnmultiplyAlpha) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CopySubTextureCHROMIUM(&self, sourceId: types::GLuint, sourceLevel: types::GLint, destTarget: types::GLenum, destId: types::GLuint, destLevel: types::GLint, xoffset: types::GLint, yoffset: types::GLint, x: types::GLint, y: types::GLint, width: types::GLint, height: types::GLint, unpackFlipY: types::GLboolean, unpackPremultiplyAlpha: types::GLboolean, unpackUnmultiplyAlpha: types::GLboolean) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLenum, types::GLuint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLboolean, types::GLboolean, types::GLboolean) -> ()>(self.CopySubTextureCHROMIUM.f)(sourceId, sourceLevel, destTarget, destId, destLevel, xoffset, yoffset, x, y, width, height, unpackFlipY, unpackPremultiplyAlpha, unpackUnmultiplyAlpha) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CopyTexImage1D(&self, target: types::GLenum, level: types::GLint, internalformat: types::GLenum, x: types::GLint, y: types::GLint, width: types::GLsizei, border: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLenum, types::GLint, types::GLint, types::GLsizei, types::GLint) -> ()>(self.CopyTexImage1D.f)(target, level, internalformat, x, y, width, border) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CopyTexImage2D(&self, target: types::GLenum, level: types::GLint, internalformat: types::GLenum, x: types::GLint, y: types::GLint, width: types::GLsizei, height: types::GLsizei, border: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLenum, types::GLint, types::GLint, types::GLsizei, types::GLsizei, types::GLint) -> ()>(self.CopyTexImage2D.f)(target, level, internalformat, x, y, width, height, border) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CopyTexSubImage1D(&self, target: types::GLenum, level: types::GLint, xoffset: types::GLint, x: types::GLint, y: types::GLint, width: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLint, types::GLint, types::GLint, types::GLsizei) -> ()>(self.CopyTexSubImage1D.f)(target, level, xoffset, x, y, width) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CopyTexSubImage2D(&self, target: types::GLenum, level: types::GLint, xoffset: types::GLint, yoffset: types::GLint, x: types::GLint, y: types::GLint, width: types::GLsizei, height: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLsizei, types::GLsizei) -> ()>(self.CopyTexSubImage2D.f)(target, level, xoffset, yoffset, x, y, width, height) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CopyTexSubImage3D(&self, target: types::GLenum, level: types::GLint, xoffset: types::GLint, yoffset: types::GLint, zoffset: types::GLint, x: types::GLint, y: types::GLint, width: types::GLsizei, height: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLsizei, types::GLsizei) -> ()>(self.CopyTexSubImage3D.f)(target, level, xoffset, yoffset, zoffset, x, y, width, height) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CopyTexture3DANGLE(&self, sourceId: types::GLuint, sourceLevel: types::GLint, destTarget: types::GLenum, destId: types::GLuint, destLevel: types::GLint, internalFormat: types::GLint, destType: types::GLenum, unpackFlipY: types::GLboolean, unpackPremultiplyAlpha: types::GLboolean, unpackUnmultiplyAlpha: types::GLboolean) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLenum, types::GLuint, types::GLint, types::GLint, types::GLenum, types::GLboolean, types::GLboolean, types::GLboolean) -> ()>(self.CopyTexture3DANGLE.f)(sourceId, sourceLevel, destTarget, destId, destLevel, internalFormat, destType, unpackFlipY, unpackPremultiplyAlpha, unpackUnmultiplyAlpha) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CopyTextureCHROMIUM(&self, sourceId: types::GLuint, sourceLevel: types::GLint, destTarget: types::GLenum, destId: types::GLuint, destLevel: types::GLint, internalFormat: types::GLint, destType: types::GLenum, unpackFlipY: types::GLboolean, unpackPremultiplyAlpha: types::GLboolean, unpackUnmultiplyAlpha: types::GLboolean) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLenum, types::GLuint, types::GLint, types::GLint, types::GLenum, types::GLboolean, types::GLboolean, types::GLboolean) -> ()>(self.CopyTextureCHROMIUM.f)(sourceId, sourceLevel, destTarget, destId, destLevel, internalFormat, destType, unpackFlipY, unpackPremultiplyAlpha, unpackUnmultiplyAlpha) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CreateProgram(&self, ) -> types::GLuint { __gl_imports::mem::transmute::<_, extern "system" fn() -> types::GLuint>(self.CreateProgram.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CreateShader(&self, type_: types::GLenum) -> types::GLuint { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> types::GLuint>(self.CreateShader.f)(type_) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CreateShaderProgramv(&self, type_: types::GLenum, count: types::GLsizei, strings: *const *const types::GLchar) -> types::GLuint { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, *const *const types::GLchar) -> types::GLuint>(self.CreateShaderProgramv.f)(type_, count, strings) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CullFace(&self, mode: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.CullFace.f)(mode) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DebugMessageCallback(&self, callback: types::GLDEBUGPROC, userParam: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLDEBUGPROC, *const __gl_imports::raw::c_void) -> ()>(self.DebugMessageCallback.f)(callback, userParam) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DebugMessageCallbackKHR(&self, callback: types::GLDEBUGPROCKHR, userParam: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLDEBUGPROCKHR, *const __gl_imports::raw::c_void) -> ()>(self.DebugMessageCallbackKHR.f)(callback, userParam) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DebugMessageControl(&self, source: types::GLenum, type_: types::GLenum, severity: types::GLenum, count: types::GLsizei, ids: *const types::GLuint, enabled: types::GLboolean) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLenum, types::GLsizei, *const types::GLuint, types::GLboolean) -> ()>(self.DebugMessageControl.f)(source, type_, severity, count, ids, enabled) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DebugMessageControlKHR(&self, source: types::GLenum, type_: types::GLenum, severity: types::GLenum, count: types::GLsizei, ids: *const types::GLuint, enabled: types::GLboolean) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLenum, types::GLsizei, *const types::GLuint, types::GLboolean) -> ()>(self.DebugMessageControlKHR.f)(source, type_, severity, count, ids, enabled) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DebugMessageInsert(&self, source: types::GLenum, type_: types::GLenum, id: types::GLuint, severity: types::GLenum, length: types::GLsizei, buf: *const types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLuint, types::GLenum, types::GLsizei, *const types::GLchar) -> ()>(self.DebugMessageInsert.f)(source, type_, id, severity, length, buf) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DebugMessageInsertKHR(&self, source: types::GLenum, type_: types::GLenum, id: types::GLuint, severity: types::GLenum, length: types::GLsizei, buf: *const types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLuint, types::GLenum, types::GLsizei, *const types::GLchar) -> ()>(self.DebugMessageInsertKHR.f)(source, type_, id, severity, length, buf) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DeleteBuffers(&self, n: types::GLsizei, buffers: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *const types::GLuint) -> ()>(self.DeleteBuffers.f)(n, buffers) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DeleteFencesAPPLE(&self, n: types::GLsizei, fences: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *const types::GLuint) -> ()>(self.DeleteFencesAPPLE.f)(n, fences) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DeleteFramebuffers(&self, n: types::GLsizei, framebuffers: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *const types::GLuint) -> ()>(self.DeleteFramebuffers.f)(n, framebuffers) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DeleteLists(&self, list: types::GLuint, range: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLsizei) -> ()>(self.DeleteLists.f)(list, range) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DeleteProgram(&self, program: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.DeleteProgram.f)(program) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DeleteProgramPipelines(&self, n: types::GLsizei, pipelines: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *const types::GLuint) -> ()>(self.DeleteProgramPipelines.f)(n, pipelines) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DeleteQueries(&self, n: types::GLsizei, ids: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *const types::GLuint) -> ()>(self.DeleteQueries.f)(n, ids) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DeleteQueriesEXT(&self, n: types::GLsizei, ids: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *const types::GLuint) -> ()>(self.DeleteQueriesEXT.f)(n, ids) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DeleteRenderbuffers(&self, n: types::GLsizei, renderbuffers: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *const types::GLuint) -> ()>(self.DeleteRenderbuffers.f)(n, renderbuffers) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DeleteSamplers(&self, count: types::GLsizei, samplers: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *const types::GLuint) -> ()>(self.DeleteSamplers.f)(count, samplers) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DeleteShader(&self, shader: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.DeleteShader.f)(shader) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DeleteSync(&self, sync: types::GLsync) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsync) -> ()>(self.DeleteSync.f)(sync) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DeleteTextures(&self, n: types::GLsizei, textures: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *const types::GLuint) -> ()>(self.DeleteTextures.f)(n, textures) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DeleteTransformFeedbacks(&self, n: types::GLsizei, ids: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *const types::GLuint) -> ()>(self.DeleteTransformFeedbacks.f)(n, ids) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DeleteVertexArrays(&self, n: types::GLsizei, arrays: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *const types::GLuint) -> ()>(self.DeleteVertexArrays.f)(n, arrays) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DeleteVertexArraysAPPLE(&self, n: types::GLsizei, arrays: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *const types::GLuint) -> ()>(self.DeleteVertexArraysAPPLE.f)(n, arrays) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DepthFunc(&self, func: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.DepthFunc.f)(func) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DepthMask(&self, flag: types::GLboolean) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLboolean) -> ()>(self.DepthMask.f)(flag) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DepthRange(&self, n: types::GLdouble, f: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble, types::GLdouble) -> ()>(self.DepthRange.f)(n, f) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DepthRangef(&self, n: types::GLfloat, f: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat) -> ()>(self.DepthRangef.f)(n, f) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DetachShader(&self, program: types::GLuint, shader: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint) -> ()>(self.DetachShader.f)(program, shader) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Disable(&self, cap: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.Disable.f)(cap) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DisableClientState(&self, array: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.DisableClientState.f)(array) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DisableVertexAttribArray(&self, index: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.DisableVertexAttribArray.f)(index) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Disablei(&self, target: types::GLenum, index: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint) -> ()>(self.Disablei.f)(target, index) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DispatchCompute(&self, num_groups_x: types::GLuint, num_groups_y: types::GLuint, num_groups_z: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint, types::GLuint) -> ()>(self.DispatchCompute.f)(num_groups_x, num_groups_y, num_groups_z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DispatchComputeIndirect(&self, indirect: types::GLintptr) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLintptr) -> ()>(self.DispatchComputeIndirect.f)(indirect) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DrawArrays(&self, mode: types::GLenum, first: types::GLint, count: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLsizei) -> ()>(self.DrawArrays.f)(mode, first, count) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DrawArraysIndirect(&self, mode: types::GLenum, indirect: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const __gl_imports::raw::c_void) -> ()>(self.DrawArraysIndirect.f)(mode, indirect) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DrawArraysInstanced(&self, mode: types::GLenum, first: types::GLint, count: types::GLsizei, instancecount: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLsizei, types::GLsizei) -> ()>(self.DrawArraysInstanced.f)(mode, first, count, instancecount) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DrawBuffer(&self, buf: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.DrawBuffer.f)(buf) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DrawBuffers(&self, n: types::GLsizei, bufs: *const types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *const types::GLenum) -> ()>(self.DrawBuffers.f)(n, bufs) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DrawElements(&self, mode: types::GLenum, count: types::GLsizei, type_: types::GLenum, indices: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, types::GLenum, *const __gl_imports::raw::c_void) -> ()>(self.DrawElements.f)(mode, count, type_, indices) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DrawElementsBaseVertex(&self, mode: types::GLenum, count: types::GLsizei, type_: types::GLenum, indices: *const __gl_imports::raw::c_void, basevertex: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, types::GLenum, *const __gl_imports::raw::c_void, types::GLint) -> ()>(self.DrawElementsBaseVertex.f)(mode, count, type_, indices, basevertex) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DrawElementsIndirect(&self, mode: types::GLenum, type_: types::GLenum, indirect: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *const __gl_imports::raw::c_void) -> ()>(self.DrawElementsIndirect.f)(mode, type_, indirect) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DrawElementsInstanced(&self, mode: types::GLenum, count: types::GLsizei, type_: types::GLenum, indices: *const __gl_imports::raw::c_void, instancecount: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, types::GLenum, *const __gl_imports::raw::c_void, types::GLsizei) -> ()>(self.DrawElementsInstanced.f)(mode, count, type_, indices, instancecount) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DrawElementsInstancedBaseVertex(&self, mode: types::GLenum, count: types::GLsizei, type_: types::GLenum, indices: *const __gl_imports::raw::c_void, instancecount: types::GLsizei, basevertex: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, types::GLenum, *const __gl_imports::raw::c_void, types::GLsizei, types::GLint) -> ()>(self.DrawElementsInstancedBaseVertex.f)(mode, count, type_, indices, instancecount, basevertex) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DrawPixels(&self, width: types::GLsizei, height: types::GLsizei, format: types::GLenum, type_: types::GLenum, pixels: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, types::GLsizei, types::GLenum, types::GLenum, *const __gl_imports::raw::c_void) -> ()>(self.DrawPixels.f)(width, height, format, type_, pixels) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DrawRangeElements(&self, mode: types::GLenum, start: types::GLuint, end: types::GLuint, count: types::GLsizei, type_: types::GLenum, indices: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint, types::GLuint, types::GLsizei, types::GLenum, *const __gl_imports::raw::c_void) -> ()>(self.DrawRangeElements.f)(mode, start, end, count, type_, indices) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DrawRangeElementsBaseVertex(&self, mode: types::GLenum, start: types::GLuint, end: types::GLuint, count: types::GLsizei, type_: types::GLenum, indices: *const __gl_imports::raw::c_void, basevertex: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint, types::GLuint, types::GLsizei, types::GLenum, *const __gl_imports::raw::c_void, types::GLint) -> ()>(self.DrawRangeElementsBaseVertex.f)(mode, start, end, count, type_, indices, basevertex) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EGLImageTargetRenderbufferStorageOES(&self, target: types::GLenum, image: types::GLeglImageOES) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLeglImageOES) -> ()>(self.EGLImageTargetRenderbufferStorageOES.f)(target, image) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EGLImageTargetTexture2DOES(&self, target: types::GLenum, image: types::GLeglImageOES) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLeglImageOES) -> ()>(self.EGLImageTargetTexture2DOES.f)(target, image) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EdgeFlag(&self, flag: types::GLboolean) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLboolean) -> ()>(self.EdgeFlag.f)(flag) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EdgeFlagPointer(&self, stride: types::GLsizei, pointer: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *const __gl_imports::raw::c_void) -> ()>(self.EdgeFlagPointer.f)(stride, pointer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EdgeFlagv(&self, flag: *const types::GLboolean) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLboolean) -> ()>(self.EdgeFlagv.f)(flag) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Enable(&self, cap: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.Enable.f)(cap) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EnableClientState(&self, array: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.EnableClientState.f)(array) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EnableVertexAttribArray(&self, index: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.EnableVertexAttribArray.f)(index) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Enablei(&self, target: types::GLenum, index: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint) -> ()>(self.Enablei.f)(target, index) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn End(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.End.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EndConditionalRender(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.EndConditionalRender.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EndList(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.EndList.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EndQuery(&self, target: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.EndQuery.f)(target) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EndQueryEXT(&self, target: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.EndQueryEXT.f)(target) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EndTransformFeedback(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.EndTransformFeedback.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EvalCoord1d(&self, u: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble) -> ()>(self.EvalCoord1d.f)(u) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EvalCoord1dv(&self, u: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble) -> ()>(self.EvalCoord1dv.f)(u) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EvalCoord1f(&self, u: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat) -> ()>(self.EvalCoord1f.f)(u) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EvalCoord1fv(&self, u: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat) -> ()>(self.EvalCoord1fv.f)(u) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EvalCoord2d(&self, u: types::GLdouble, v: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble, types::GLdouble) -> ()>(self.EvalCoord2d.f)(u, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EvalCoord2dv(&self, u: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble) -> ()>(self.EvalCoord2dv.f)(u) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EvalCoord2f(&self, u: types::GLfloat, v: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat) -> ()>(self.EvalCoord2f.f)(u, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EvalCoord2fv(&self, u: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat) -> ()>(self.EvalCoord2fv.f)(u) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EvalMesh1(&self, mode: types::GLenum, i1: types::GLint, i2: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLint) -> ()>(self.EvalMesh1.f)(mode, i1, i2) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EvalMesh2(&self, mode: types::GLenum, i1: types::GLint, i2: types::GLint, j1: types::GLint, j2: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLint, types::GLint, types::GLint) -> ()>(self.EvalMesh2.f)(mode, i1, i2, j1, j2) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EvalPoint1(&self, i: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint) -> ()>(self.EvalPoint1.f)(i) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EvalPoint2(&self, i: types::GLint, j: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint) -> ()>(self.EvalPoint2.f)(i, j) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FeedbackBuffer(&self, size: types::GLsizei, type_: types::GLenum, buffer: *mut types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, types::GLenum, *mut types::GLfloat) -> ()>(self.FeedbackBuffer.f)(size, type_, buffer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FenceSync(&self, condition: types::GLenum, flags: types::GLbitfield) -> types::GLsync { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLbitfield) -> types::GLsync>(self.FenceSync.f)(condition, flags) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Finish(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.Finish.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FinishFenceAPPLE(&self, fence: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.FinishFenceAPPLE.f)(fence) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FinishObjectAPPLE(&self, object: types::GLenum, name: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint) -> ()>(self.FinishObjectAPPLE.f)(object, name) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Flush(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.Flush.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FlushMappedBufferRange(&self, target: types::GLenum, offset: types::GLintptr, length: types::GLsizeiptr) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLintptr, types::GLsizeiptr) -> ()>(self.FlushMappedBufferRange.f)(target, offset, length) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FogCoordPointer(&self, type_: types::GLenum, stride: types::GLsizei, pointer: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, *const __gl_imports::raw::c_void) -> ()>(self.FogCoordPointer.f)(type_, stride, pointer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FogCoordd(&self, coord: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble) -> ()>(self.FogCoordd.f)(coord) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FogCoorddv(&self, coord: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble) -> ()>(self.FogCoorddv.f)(coord) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FogCoordf(&self, coord: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat) -> ()>(self.FogCoordf.f)(coord) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FogCoordfv(&self, coord: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat) -> ()>(self.FogCoordfv.f)(coord) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Fogf(&self, pname: types::GLenum, param: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLfloat) -> ()>(self.Fogf.f)(pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Fogfv(&self, pname: types::GLenum, params: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLfloat) -> ()>(self.Fogfv.f)(pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Fogi(&self, pname: types::GLenum, param: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint) -> ()>(self.Fogi.f)(pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Fogiv(&self, pname: types::GLenum, params: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLint) -> ()>(self.Fogiv.f)(pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FramebufferParameteri(&self, target: types::GLenum, pname: types::GLenum, param: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLint) -> ()>(self.FramebufferParameteri.f)(target, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FramebufferRenderbuffer(&self, target: types::GLenum, attachment: types::GLenum, renderbuffertarget: types::GLenum, renderbuffer: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLenum, types::GLuint) -> ()>(self.FramebufferRenderbuffer.f)(target, attachment, renderbuffertarget, renderbuffer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FramebufferTexture(&self, target: types::GLenum, attachment: types::GLenum, texture: types::GLuint, level: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLuint, types::GLint) -> ()>(self.FramebufferTexture.f)(target, attachment, texture, level) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FramebufferTexture1D(&self, target: types::GLenum, attachment: types::GLenum, textarget: types::GLenum, texture: types::GLuint, level: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLenum, types::GLuint, types::GLint) -> ()>(self.FramebufferTexture1D.f)(target, attachment, textarget, texture, level) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FramebufferTexture2D(&self, target: types::GLenum, attachment: types::GLenum, textarget: types::GLenum, texture: types::GLuint, level: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLenum, types::GLuint, types::GLint) -> ()>(self.FramebufferTexture2D.f)(target, attachment, textarget, texture, level) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FramebufferTexture3D(&self, target: types::GLenum, attachment: types::GLenum, textarget: types::GLenum, texture: types::GLuint, level: types::GLint, zoffset: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLenum, types::GLuint, types::GLint, types::GLint) -> ()>(self.FramebufferTexture3D.f)(target, attachment, textarget, texture, level, zoffset) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FramebufferTextureLayer(&self, target: types::GLenum, attachment: types::GLenum, texture: types::GLuint, level: types::GLint, layer: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLuint, types::GLint, types::GLint) -> ()>(self.FramebufferTextureLayer.f)(target, attachment, texture, level, layer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FrontFace(&self, mode: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.FrontFace.f)(mode) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Frustum(&self, left: types::GLdouble, right: types::GLdouble, bottom: types::GLdouble, top: types::GLdouble, zNear: types::GLdouble, zFar: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble, types::GLdouble, types::GLdouble, types::GLdouble, types::GLdouble, types::GLdouble) -> ()>(self.Frustum.f)(left, right, bottom, top, zNear, zFar) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GenBuffers(&self, n: types::GLsizei, buffers: *mut types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *mut types::GLuint) -> ()>(self.GenBuffers.f)(n, buffers) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GenFencesAPPLE(&self, n: types::GLsizei, fences: *mut types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *mut types::GLuint) -> ()>(self.GenFencesAPPLE.f)(n, fences) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GenFramebuffers(&self, n: types::GLsizei, framebuffers: *mut types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *mut types::GLuint) -> ()>(self.GenFramebuffers.f)(n, framebuffers) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GenLists(&self, range: types::GLsizei) -> types::GLuint { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei) -> types::GLuint>(self.GenLists.f)(range) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GenProgramPipelines(&self, n: types::GLsizei, pipelines: *mut types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *mut types::GLuint) -> ()>(self.GenProgramPipelines.f)(n, pipelines) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GenQueries(&self, n: types::GLsizei, ids: *mut types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *mut types::GLuint) -> ()>(self.GenQueries.f)(n, ids) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GenQueriesEXT(&self, n: types::GLsizei, ids: *mut types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *mut types::GLuint) -> ()>(self.GenQueriesEXT.f)(n, ids) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GenRenderbuffers(&self, n: types::GLsizei, renderbuffers: *mut types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *mut types::GLuint) -> ()>(self.GenRenderbuffers.f)(n, renderbuffers) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GenSamplers(&self, count: types::GLsizei, samplers: *mut types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *mut types::GLuint) -> ()>(self.GenSamplers.f)(count, samplers) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GenTextures(&self, n: types::GLsizei, textures: *mut types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *mut types::GLuint) -> ()>(self.GenTextures.f)(n, textures) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GenTransformFeedbacks(&self, n: types::GLsizei, ids: *mut types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *mut types::GLuint) -> ()>(self.GenTransformFeedbacks.f)(n, ids) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GenVertexArrays(&self, n: types::GLsizei, arrays: *mut types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *mut types::GLuint) -> ()>(self.GenVertexArrays.f)(n, arrays) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GenVertexArraysAPPLE(&self, n: types::GLsizei, arrays: *mut types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *mut types::GLuint) -> ()>(self.GenVertexArraysAPPLE.f)(n, arrays) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GenerateMipmap(&self, target: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.GenerateMipmap.f)(target) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetActiveAttrib(&self, program: types::GLuint, index: types::GLuint, bufSize: types::GLsizei, length: *mut types::GLsizei, size: *mut types::GLint, type_: *mut types::GLenum, name: *mut types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint, types::GLsizei, *mut types::GLsizei, *mut types::GLint, *mut types::GLenum, *mut types::GLchar) -> ()>(self.GetActiveAttrib.f)(program, index, bufSize, length, size, type_, name) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetActiveUniform(&self, program: types::GLuint, index: types::GLuint, bufSize: types::GLsizei, length: *mut types::GLsizei, size: *mut types::GLint, type_: *mut types::GLenum, name: *mut types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint, types::GLsizei, *mut types::GLsizei, *mut types::GLint, *mut types::GLenum, *mut types::GLchar) -> ()>(self.GetActiveUniform.f)(program, index, bufSize, length, size, type_, name) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetActiveUniformBlockName(&self, program: types::GLuint, uniformBlockIndex: types::GLuint, bufSize: types::GLsizei, length: *mut types::GLsizei, uniformBlockName: *mut types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint, types::GLsizei, *mut types::GLsizei, *mut types::GLchar) -> ()>(self.GetActiveUniformBlockName.f)(program, uniformBlockIndex, bufSize, length, uniformBlockName) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetActiveUniformBlockiv(&self, program: types::GLuint, uniformBlockIndex: types::GLuint, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint, types::GLenum, *mut types::GLint) -> ()>(self.GetActiveUniformBlockiv.f)(program, uniformBlockIndex, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetActiveUniformName(&self, program: types::GLuint, uniformIndex: types::GLuint, bufSize: types::GLsizei, length: *mut types::GLsizei, uniformName: *mut types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint, types::GLsizei, *mut types::GLsizei, *mut types::GLchar) -> ()>(self.GetActiveUniformName.f)(program, uniformIndex, bufSize, length, uniformName) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetActiveUniformsiv(&self, program: types::GLuint, uniformCount: types::GLsizei, uniformIndices: *const types::GLuint, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLsizei, *const types::GLuint, types::GLenum, *mut types::GLint) -> ()>(self.GetActiveUniformsiv.f)(program, uniformCount, uniformIndices, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetAttachedShaders(&self, program: types::GLuint, maxCount: types::GLsizei, count: *mut types::GLsizei, shaders: *mut types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLsizei, *mut types::GLsizei, *mut types::GLuint) -> ()>(self.GetAttachedShaders.f)(program, maxCount, count, shaders) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetAttribLocation(&self, program: types::GLuint, name: *const types::GLchar) -> types::GLint { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLchar) -> types::GLint>(self.GetAttribLocation.f)(program, name) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetBooleani_v(&self, target: types::GLenum, index: types::GLuint, data: *mut types::GLboolean) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint, *mut types::GLboolean) -> ()>(self.GetBooleani_v.f)(target, index, data) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetBooleanv(&self, pname: types::GLenum, data: *mut types::GLboolean) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *mut types::GLboolean) -> ()>(self.GetBooleanv.f)(pname, data) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetBufferParameteri64v(&self, target: types::GLenum, pname: types::GLenum, params: *mut types::GLint64) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *mut types::GLint64) -> ()>(self.GetBufferParameteri64v.f)(target, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetBufferParameteriv(&self, target: types::GLenum, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *mut types::GLint) -> ()>(self.GetBufferParameteriv.f)(target, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetBufferPointerv(&self, target: types::GLenum, pname: types::GLenum, params: *const *mut __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *const *mut __gl_imports::raw::c_void) -> ()>(self.GetBufferPointerv.f)(target, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetBufferSubData(&self, target: types::GLenum, offset: types::GLintptr, size: types::GLsizeiptr, data: *mut __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLintptr, types::GLsizeiptr, *mut __gl_imports::raw::c_void) -> ()>(self.GetBufferSubData.f)(target, offset, size, data) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetClipPlane(&self, plane: types::GLenum, equation: *mut types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *mut types::GLdouble) -> ()>(self.GetClipPlane.f)(plane, equation) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetCompressedTexImage(&self, target: types::GLenum, level: types::GLint, img: *mut __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, *mut __gl_imports::raw::c_void) -> ()>(self.GetCompressedTexImage.f)(target, level, img) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetDebugMessageLog(&self, count: types::GLuint, bufSize: types::GLsizei, sources: *mut types::GLenum, types: *mut types::GLenum, ids: *mut types::GLuint, severities: *mut types::GLenum, lengths: *mut types::GLsizei, messageLog: *mut types::GLchar) -> types::GLuint { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLsizei, *mut types::GLenum, *mut types::GLenum, *mut types::GLuint, *mut types::GLenum, *mut types::GLsizei, *mut types::GLchar) -> types::GLuint>(self.GetDebugMessageLog.f)(count, bufSize, sources, types, ids, severities, lengths, messageLog) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetDebugMessageLogKHR(&self, count: types::GLuint, bufSize: types::GLsizei, sources: *mut types::GLenum, types: *mut types::GLenum, ids: *mut types::GLuint, severities: *mut types::GLenum, lengths: *mut types::GLsizei, messageLog: *mut types::GLchar) -> types::GLuint { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLsizei, *mut types::GLenum, *mut types::GLenum, *mut types::GLuint, *mut types::GLenum, *mut types::GLsizei, *mut types::GLchar) -> types::GLuint>(self.GetDebugMessageLogKHR.f)(count, bufSize, sources, types, ids, severities, lengths, messageLog) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetDoublev(&self, pname: types::GLenum, data: *mut types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *mut types::GLdouble) -> ()>(self.GetDoublev.f)(pname, data) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetError(&self, ) -> types::GLenum { __gl_imports::mem::transmute::<_, extern "system" fn() -> types::GLenum>(self.GetError.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetFloatv(&self, pname: types::GLenum, data: *mut types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *mut types::GLfloat) -> ()>(self.GetFloatv.f)(pname, data) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetFragDataIndex(&self, program: types::GLuint, name: *const types::GLchar) -> types::GLint { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLchar) -> types::GLint>(self.GetFragDataIndex.f)(program, name) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetFragDataLocation(&self, program: types::GLuint, name: *const types::GLchar) -> types::GLint { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLchar) -> types::GLint>(self.GetFragDataLocation.f)(program, name) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetFramebufferAttachmentParameteriv(&self, target: types::GLenum, attachment: types::GLenum, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLenum, *mut types::GLint) -> ()>(self.GetFramebufferAttachmentParameteriv.f)(target, attachment, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetFramebufferParameteriv(&self, target: types::GLenum, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *mut types::GLint) -> ()>(self.GetFramebufferParameteriv.f)(target, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetInteger64i_v(&self, target: types::GLenum, index: types::GLuint, data: *mut types::GLint64) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint, *mut types::GLint64) -> ()>(self.GetInteger64i_v.f)(target, index, data) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetInteger64v(&self, pname: types::GLenum, data: *mut types::GLint64) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *mut types::GLint64) -> ()>(self.GetInteger64v.f)(pname, data) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetIntegeri_v(&self, target: types::GLenum, index: types::GLuint, data: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint, *mut types::GLint) -> ()>(self.GetIntegeri_v.f)(target, index, data) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetIntegerv(&self, pname: types::GLenum, data: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *mut types::GLint) -> ()>(self.GetIntegerv.f)(pname, data) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetInternalformativ(&self, target: types::GLenum, internalformat: types::GLenum, pname: types::GLenum, bufSize: types::GLsizei, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLenum, types::GLsizei, *mut types::GLint) -> ()>(self.GetInternalformativ.f)(target, internalformat, pname, bufSize, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetLightfv(&self, light: types::GLenum, pname: types::GLenum, params: *mut types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *mut types::GLfloat) -> ()>(self.GetLightfv.f)(light, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetLightiv(&self, light: types::GLenum, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *mut types::GLint) -> ()>(self.GetLightiv.f)(light, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetMapdv(&self, target: types::GLenum, query: types::GLenum, v: *mut types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *mut types::GLdouble) -> ()>(self.GetMapdv.f)(target, query, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetMapfv(&self, target: types::GLenum, query: types::GLenum, v: *mut types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *mut types::GLfloat) -> ()>(self.GetMapfv.f)(target, query, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetMapiv(&self, target: types::GLenum, query: types::GLenum, v: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *mut types::GLint) -> ()>(self.GetMapiv.f)(target, query, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetMaterialfv(&self, face: types::GLenum, pname: types::GLenum, params: *mut types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *mut types::GLfloat) -> ()>(self.GetMaterialfv.f)(face, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetMaterialiv(&self, face: types::GLenum, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *mut types::GLint) -> ()>(self.GetMaterialiv.f)(face, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetMultisamplefv(&self, pname: types::GLenum, index: types::GLuint, val: *mut types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint, *mut types::GLfloat) -> ()>(self.GetMultisamplefv.f)(pname, index, val) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetObjectLabel(&self, identifier: types::GLenum, name: types::GLuint, bufSize: types::GLsizei, length: *mut types::GLsizei, label: *mut types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint, types::GLsizei, *mut types::GLsizei, *mut types::GLchar) -> ()>(self.GetObjectLabel.f)(identifier, name, bufSize, length, label) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetObjectLabelKHR(&self, identifier: types::GLenum, name: types::GLuint, bufSize: types::GLsizei, length: *mut types::GLsizei, label: *mut types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint, types::GLsizei, *mut types::GLsizei, *mut types::GLchar) -> ()>(self.GetObjectLabelKHR.f)(identifier, name, bufSize, length, label) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetObjectPtrLabel(&self, ptr: *const __gl_imports::raw::c_void, bufSize: types::GLsizei, length: *mut types::GLsizei, label: *mut types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const __gl_imports::raw::c_void, types::GLsizei, *mut types::GLsizei, *mut types::GLchar) -> ()>(self.GetObjectPtrLabel.f)(ptr, bufSize, length, label) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetObjectPtrLabelKHR(&self, ptr: *const __gl_imports::raw::c_void, bufSize: types::GLsizei, length: *mut types::GLsizei, label: *mut types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const __gl_imports::raw::c_void, types::GLsizei, *mut types::GLsizei, *mut types::GLchar) -> ()>(self.GetObjectPtrLabelKHR.f)(ptr, bufSize, length, label) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetPixelMapfv(&self, map: types::GLenum, values: *mut types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *mut types::GLfloat) -> ()>(self.GetPixelMapfv.f)(map, values) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetPixelMapuiv(&self, map: types::GLenum, values: *mut types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *mut types::GLuint) -> ()>(self.GetPixelMapuiv.f)(map, values) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetPixelMapusv(&self, map: types::GLenum, values: *mut types::GLushort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *mut types::GLushort) -> ()>(self.GetPixelMapusv.f)(map, values) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetPointerv(&self, pname: types::GLenum, params: *const *mut __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const *mut __gl_imports::raw::c_void) -> ()>(self.GetPointerv.f)(pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetPointervKHR(&self, pname: types::GLenum, params: *const *mut __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const *mut __gl_imports::raw::c_void) -> ()>(self.GetPointervKHR.f)(pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetPolygonStipple(&self, mask: *mut types::GLubyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*mut types::GLubyte) -> ()>(self.GetPolygonStipple.f)(mask) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetProgramBinary(&self, program: types::GLuint, bufSize: types::GLsizei, length: *mut types::GLsizei, binaryFormat: *mut types::GLenum, binary: *mut __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLsizei, *mut types::GLsizei, *mut types::GLenum, *mut __gl_imports::raw::c_void) -> ()>(self.GetProgramBinary.f)(program, bufSize, length, binaryFormat, binary) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetProgramInfoLog(&self, program: types::GLuint, bufSize: types::GLsizei, length: *mut types::GLsizei, infoLog: *mut types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLsizei, *mut types::GLsizei, *mut types::GLchar) -> ()>(self.GetProgramInfoLog.f)(program, bufSize, length, infoLog) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetProgramInterfaceiv(&self, program: types::GLuint, programInterface: types::GLenum, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLenum, *mut types::GLint) -> ()>(self.GetProgramInterfaceiv.f)(program, programInterface, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetProgramPipelineInfoLog(&self, pipeline: types::GLuint, bufSize: types::GLsizei, length: *mut types::GLsizei, infoLog: *mut types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLsizei, *mut types::GLsizei, *mut types::GLchar) -> ()>(self.GetProgramPipelineInfoLog.f)(pipeline, bufSize, length, infoLog) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetProgramPipelineiv(&self, pipeline: types::GLuint, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *mut types::GLint) -> ()>(self.GetProgramPipelineiv.f)(pipeline, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetProgramResourceIndex(&self, program: types::GLuint, programInterface: types::GLenum, name: *const types::GLchar) -> types::GLuint { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *const types::GLchar) -> types::GLuint>(self.GetProgramResourceIndex.f)(program, programInterface, name) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetProgramResourceLocation(&self, program: types::GLuint, programInterface: types::GLenum, name: *const types::GLchar) -> types::GLint { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *const types::GLchar) -> types::GLint>(self.GetProgramResourceLocation.f)(program, programInterface, name) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetProgramResourceName(&self, program: types::GLuint, programInterface: types::GLenum, index: types::GLuint, bufSize: types::GLsizei, length: *mut types::GLsizei, name: *mut types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLuint, types::GLsizei, *mut types::GLsizei, *mut types::GLchar) -> ()>(self.GetProgramResourceName.f)(program, programInterface, index, bufSize, length, name) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetProgramResourceiv(&self, program: types::GLuint, programInterface: types::GLenum, index: types::GLuint, propCount: types::GLsizei, props: *const types::GLenum, bufSize: types::GLsizei, length: *mut types::GLsizei, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLuint, types::GLsizei, *const types::GLenum, types::GLsizei, *mut types::GLsizei, *mut types::GLint) -> ()>(self.GetProgramResourceiv.f)(program, programInterface, index, propCount, props, bufSize, length, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetProgramiv(&self, program: types::GLuint, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *mut types::GLint) -> ()>(self.GetProgramiv.f)(program, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetQueryObjecti64v(&self, id: types::GLuint, pname: types::GLenum, params: *mut types::GLint64) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *mut types::GLint64) -> ()>(self.GetQueryObjecti64v.f)(id, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetQueryObjecti64vEXT(&self, id: types::GLuint, pname: types::GLenum, params: *mut types::GLint64) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *mut types::GLint64) -> ()>(self.GetQueryObjecti64vEXT.f)(id, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetQueryObjectiv(&self, id: types::GLuint, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *mut types::GLint) -> ()>(self.GetQueryObjectiv.f)(id, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetQueryObjectivEXT(&self, id: types::GLuint, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *mut types::GLint) -> ()>(self.GetQueryObjectivEXT.f)(id, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetQueryObjectui64v(&self, id: types::GLuint, pname: types::GLenum, params: *mut types::GLuint64) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *mut types::GLuint64) -> ()>(self.GetQueryObjectui64v.f)(id, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetQueryObjectui64vEXT(&self, id: types::GLuint, pname: types::GLenum, params: *mut types::GLuint64) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *mut types::GLuint64) -> ()>(self.GetQueryObjectui64vEXT.f)(id, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetQueryObjectuiv(&self, id: types::GLuint, pname: types::GLenum, params: *mut types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *mut types::GLuint) -> ()>(self.GetQueryObjectuiv.f)(id, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetQueryObjectuivEXT(&self, id: types::GLuint, pname: types::GLenum, params: *mut types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *mut types::GLuint) -> ()>(self.GetQueryObjectuivEXT.f)(id, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetQueryiv(&self, target: types::GLenum, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *mut types::GLint) -> ()>(self.GetQueryiv.f)(target, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetQueryivEXT(&self, target: types::GLenum, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *mut types::GLint) -> ()>(self.GetQueryivEXT.f)(target, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetRenderbufferParameteriv(&self, target: types::GLenum, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *mut types::GLint) -> ()>(self.GetRenderbufferParameteriv.f)(target, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetSamplerParameterIiv(&self, sampler: types::GLuint, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *mut types::GLint) -> ()>(self.GetSamplerParameterIiv.f)(sampler, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetSamplerParameterIuiv(&self, sampler: types::GLuint, pname: types::GLenum, params: *mut types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *mut types::GLuint) -> ()>(self.GetSamplerParameterIuiv.f)(sampler, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetSamplerParameterfv(&self, sampler: types::GLuint, pname: types::GLenum, params: *mut types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *mut types::GLfloat) -> ()>(self.GetSamplerParameterfv.f)(sampler, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetSamplerParameteriv(&self, sampler: types::GLuint, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *mut types::GLint) -> ()>(self.GetSamplerParameteriv.f)(sampler, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetShaderInfoLog(&self, shader: types::GLuint, bufSize: types::GLsizei, length: *mut types::GLsizei, infoLog: *mut types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLsizei, *mut types::GLsizei, *mut types::GLchar) -> ()>(self.GetShaderInfoLog.f)(shader, bufSize, length, infoLog) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetShaderPrecisionFormat(&self, shadertype: types::GLenum, precisiontype: types::GLenum, range: *mut types::GLint, precision: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *mut types::GLint, *mut types::GLint) -> ()>(self.GetShaderPrecisionFormat.f)(shadertype, precisiontype, range, precision) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetShaderSource(&self, shader: types::GLuint, bufSize: types::GLsizei, length: *mut types::GLsizei, source: *mut types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLsizei, *mut types::GLsizei, *mut types::GLchar) -> ()>(self.GetShaderSource.f)(shader, bufSize, length, source) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetShaderiv(&self, shader: types::GLuint, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *mut types::GLint) -> ()>(self.GetShaderiv.f)(shader, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetString(&self, name: types::GLenum) -> *const types::GLubyte { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> *const types::GLubyte>(self.GetString.f)(name) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetStringi(&self, name: types::GLenum, index: types::GLuint) -> *const types::GLubyte { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint) -> *const types::GLubyte>(self.GetStringi.f)(name, index) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetSynciv(&self, sync: types::GLsync, pname: types::GLenum, bufSize: types::GLsizei, length: *mut types::GLsizei, values: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsync, types::GLenum, types::GLsizei, *mut types::GLsizei, *mut types::GLint) -> ()>(self.GetSynciv.f)(sync, pname, bufSize, length, values) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetTexEnvfv(&self, target: types::GLenum, pname: types::GLenum, params: *mut types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *mut types::GLfloat) -> ()>(self.GetTexEnvfv.f)(target, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetTexEnviv(&self, target: types::GLenum, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *mut types::GLint) -> ()>(self.GetTexEnviv.f)(target, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetTexGendv(&self, coord: types::GLenum, pname: types::GLenum, params: *mut types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *mut types::GLdouble) -> ()>(self.GetTexGendv.f)(coord, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetTexGenfv(&self, coord: types::GLenum, pname: types::GLenum, params: *mut types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *mut types::GLfloat) -> ()>(self.GetTexGenfv.f)(coord, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetTexGeniv(&self, coord: types::GLenum, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *mut types::GLint) -> ()>(self.GetTexGeniv.f)(coord, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetTexImage(&self, target: types::GLenum, level: types::GLint, format: types::GLenum, type_: types::GLenum, pixels: *mut __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLenum, types::GLenum, *mut __gl_imports::raw::c_void) -> ()>(self.GetTexImage.f)(target, level, format, type_, pixels) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetTexLevelParameterfv(&self, target: types::GLenum, level: types::GLint, pname: types::GLenum, params: *mut types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLenum, *mut types::GLfloat) -> ()>(self.GetTexLevelParameterfv.f)(target, level, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetTexLevelParameteriv(&self, target: types::GLenum, level: types::GLint, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLenum, *mut types::GLint) -> ()>(self.GetTexLevelParameteriv.f)(target, level, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetTexParameterIiv(&self, target: types::GLenum, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *mut types::GLint) -> ()>(self.GetTexParameterIiv.f)(target, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetTexParameterIuiv(&self, target: types::GLenum, pname: types::GLenum, params: *mut types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *mut types::GLuint) -> ()>(self.GetTexParameterIuiv.f)(target, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetTexParameterPointervAPPLE(&self, target: types::GLenum, pname: types::GLenum, params: *const *mut __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *const *mut __gl_imports::raw::c_void) -> ()>(self.GetTexParameterPointervAPPLE.f)(target, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetTexParameterfv(&self, target: types::GLenum, pname: types::GLenum, params: *mut types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *mut types::GLfloat) -> ()>(self.GetTexParameterfv.f)(target, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetTexParameteriv(&self, target: types::GLenum, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *mut types::GLint) -> ()>(self.GetTexParameteriv.f)(target, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetTransformFeedbackVarying(&self, program: types::GLuint, index: types::GLuint, bufSize: types::GLsizei, length: *mut types::GLsizei, size: *mut types::GLsizei, type_: *mut types::GLenum, name: *mut types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint, types::GLsizei, *mut types::GLsizei, *mut types::GLsizei, *mut types::GLenum, *mut types::GLchar) -> ()>(self.GetTransformFeedbackVarying.f)(program, index, bufSize, length, size, type_, name) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetUniformBlockIndex(&self, program: types::GLuint, uniformBlockName: *const types::GLchar) -> types::GLuint { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLchar) -> types::GLuint>(self.GetUniformBlockIndex.f)(program, uniformBlockName) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetUniformIndices(&self, program: types::GLuint, uniformCount: types::GLsizei, uniformNames: *const *const types::GLchar, uniformIndices: *mut types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLsizei, *const *const types::GLchar, *mut types::GLuint) -> ()>(self.GetUniformIndices.f)(program, uniformCount, uniformNames, uniformIndices) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetUniformLocation(&self, program: types::GLuint, name: *const types::GLchar) -> types::GLint { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLchar) -> types::GLint>(self.GetUniformLocation.f)(program, name) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetUniformfv(&self, program: types::GLuint, location: types::GLint, params: *mut types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, *mut types::GLfloat) -> ()>(self.GetUniformfv.f)(program, location, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetUniformiv(&self, program: types::GLuint, location: types::GLint, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, *mut types::GLint) -> ()>(self.GetUniformiv.f)(program, location, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetUniformuiv(&self, program: types::GLuint, location: types::GLint, params: *mut types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, *mut types::GLuint) -> ()>(self.GetUniformuiv.f)(program, location, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetVertexAttribIiv(&self, index: types::GLuint, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *mut types::GLint) -> ()>(self.GetVertexAttribIiv.f)(index, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetVertexAttribIuiv(&self, index: types::GLuint, pname: types::GLenum, params: *mut types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *mut types::GLuint) -> ()>(self.GetVertexAttribIuiv.f)(index, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetVertexAttribPointerv(&self, index: types::GLuint, pname: types::GLenum, pointer: *const *mut __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *const *mut __gl_imports::raw::c_void) -> ()>(self.GetVertexAttribPointerv.f)(index, pname, pointer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetVertexAttribdv(&self, index: types::GLuint, pname: types::GLenum, params: *mut types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *mut types::GLdouble) -> ()>(self.GetVertexAttribdv.f)(index, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetVertexAttribfv(&self, index: types::GLuint, pname: types::GLenum, params: *mut types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *mut types::GLfloat) -> ()>(self.GetVertexAttribfv.f)(index, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetVertexAttribiv(&self, index: types::GLuint, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *mut types::GLint) -> ()>(self.GetVertexAttribiv.f)(index, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Hint(&self, target: types::GLenum, mode: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum) -> ()>(self.Hint.f)(target, mode) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn IndexMask(&self, mask: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.IndexMask.f)(mask) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn IndexPointer(&self, type_: types::GLenum, stride: types::GLsizei, pointer: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, *const __gl_imports::raw::c_void) -> ()>(self.IndexPointer.f)(type_, stride, pointer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Indexd(&self, c: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble) -> ()>(self.Indexd.f)(c) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Indexdv(&self, c: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble) -> ()>(self.Indexdv.f)(c) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Indexf(&self, c: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat) -> ()>(self.Indexf.f)(c) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Indexfv(&self, c: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat) -> ()>(self.Indexfv.f)(c) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Indexi(&self, c: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint) -> ()>(self.Indexi.f)(c) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Indexiv(&self, c: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLint) -> ()>(self.Indexiv.f)(c) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Indexs(&self, c: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLshort) -> ()>(self.Indexs.f)(c) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Indexsv(&self, c: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLshort) -> ()>(self.Indexsv.f)(c) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Indexub(&self, c: types::GLubyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLubyte) -> ()>(self.Indexub.f)(c) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Indexubv(&self, c: *const types::GLubyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLubyte) -> ()>(self.Indexubv.f)(c) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn InitNames(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.InitNames.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn InsertEventMarkerEXT(&self, length: types::GLsizei, marker: *const types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *const types::GLchar) -> ()>(self.InsertEventMarkerEXT.f)(length, marker) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn InterleavedArrays(&self, format: types::GLenum, stride: types::GLsizei, pointer: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, *const __gl_imports::raw::c_void) -> ()>(self.InterleavedArrays.f)(format, stride, pointer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn InvalidateBufferData(&self, buffer: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.InvalidateBufferData.f)(buffer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn InvalidateBufferSubData(&self, buffer: types::GLuint, offset: types::GLintptr, length: types::GLsizeiptr) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLintptr, types::GLsizeiptr) -> ()>(self.InvalidateBufferSubData.f)(buffer, offset, length) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn InvalidateFramebuffer(&self, target: types::GLenum, numAttachments: types::GLsizei, attachments: *const types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, *const types::GLenum) -> ()>(self.InvalidateFramebuffer.f)(target, numAttachments, attachments) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn InvalidateSubFramebuffer(&self, target: types::GLenum, numAttachments: types::GLsizei, attachments: *const types::GLenum, x: types::GLint, y: types::GLint, width: types::GLsizei, height: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, *const types::GLenum, types::GLint, types::GLint, types::GLsizei, types::GLsizei) -> ()>(self.InvalidateSubFramebuffer.f)(target, numAttachments, attachments, x, y, width, height) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn InvalidateTexImage(&self, texture: types::GLuint, level: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint) -> ()>(self.InvalidateTexImage.f)(texture, level) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn InvalidateTexSubImage(&self, texture: types::GLuint, level: types::GLint, xoffset: types::GLint, yoffset: types::GLint, zoffset: types::GLint, width: types::GLsizei, height: types::GLsizei, depth: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLsizei, types::GLsizei, types::GLsizei) -> ()>(self.InvalidateTexSubImage.f)(texture, level, xoffset, yoffset, zoffset, width, height, depth) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn IsBuffer(&self, buffer: types::GLuint) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> types::GLboolean>(self.IsBuffer.f)(buffer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn IsEnabled(&self, cap: types::GLenum) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> types::GLboolean>(self.IsEnabled.f)(cap) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn IsEnabledi(&self, target: types::GLenum, index: types::GLuint) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint) -> types::GLboolean>(self.IsEnabledi.f)(target, index) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn IsFenceAPPLE(&self, fence: types::GLuint) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> types::GLboolean>(self.IsFenceAPPLE.f)(fence) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn IsFramebuffer(&self, framebuffer: types::GLuint) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> types::GLboolean>(self.IsFramebuffer.f)(framebuffer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn IsList(&self, list: types::GLuint) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> types::GLboolean>(self.IsList.f)(list) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn IsProgram(&self, program: types::GLuint) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> types::GLboolean>(self.IsProgram.f)(program) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn IsProgramPipeline(&self, pipeline: types::GLuint) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> types::GLboolean>(self.IsProgramPipeline.f)(pipeline) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn IsQuery(&self, id: types::GLuint) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> types::GLboolean>(self.IsQuery.f)(id) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn IsQueryEXT(&self, id: types::GLuint) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> types::GLboolean>(self.IsQueryEXT.f)(id) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn IsRenderbuffer(&self, renderbuffer: types::GLuint) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> types::GLboolean>(self.IsRenderbuffer.f)(renderbuffer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn IsSampler(&self, sampler: types::GLuint) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> types::GLboolean>(self.IsSampler.f)(sampler) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn IsShader(&self, shader: types::GLuint) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> types::GLboolean>(self.IsShader.f)(shader) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn IsSync(&self, sync: types::GLsync) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsync) -> types::GLboolean>(self.IsSync.f)(sync) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn IsTexture(&self, texture: types::GLuint) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> types::GLboolean>(self.IsTexture.f)(texture) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn IsTransformFeedback(&self, id: types::GLuint) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> types::GLboolean>(self.IsTransformFeedback.f)(id) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn IsVertexArray(&self, array: types::GLuint) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> types::GLboolean>(self.IsVertexArray.f)(array) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn IsVertexArrayAPPLE(&self, array: types::GLuint) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> types::GLboolean>(self.IsVertexArrayAPPLE.f)(array) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn LightModelf(&self, pname: types::GLenum, param: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLfloat) -> ()>(self.LightModelf.f)(pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn LightModelfv(&self, pname: types::GLenum, params: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLfloat) -> ()>(self.LightModelfv.f)(pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn LightModeli(&self, pname: types::GLenum, param: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint) -> ()>(self.LightModeli.f)(pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn LightModeliv(&self, pname: types::GLenum, params: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLint) -> ()>(self.LightModeliv.f)(pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Lightf(&self, light: types::GLenum, pname: types::GLenum, param: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLfloat) -> ()>(self.Lightf.f)(light, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Lightfv(&self, light: types::GLenum, pname: types::GLenum, params: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *const types::GLfloat) -> ()>(self.Lightfv.f)(light, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Lighti(&self, light: types::GLenum, pname: types::GLenum, param: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLint) -> ()>(self.Lighti.f)(light, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Lightiv(&self, light: types::GLenum, pname: types::GLenum, params: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *const types::GLint) -> ()>(self.Lightiv.f)(light, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn LineStipple(&self, factor: types::GLint, pattern: types::GLushort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLushort) -> ()>(self.LineStipple.f)(factor, pattern) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn LineWidth(&self, width: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat) -> ()>(self.LineWidth.f)(width) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn LinkProgram(&self, program: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.LinkProgram.f)(program) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ListBase(&self, base: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.ListBase.f)(base) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn LoadIdentity(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.LoadIdentity.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn LoadMatrixd(&self, m: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble) -> ()>(self.LoadMatrixd.f)(m) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn LoadMatrixf(&self, m: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat) -> ()>(self.LoadMatrixf.f)(m) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn LoadName(&self, name: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.LoadName.f)(name) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn LoadTransposeMatrixd(&self, m: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble) -> ()>(self.LoadTransposeMatrixd.f)(m) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn LoadTransposeMatrixf(&self, m: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat) -> ()>(self.LoadTransposeMatrixf.f)(m) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn LogicOp(&self, opcode: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.LogicOp.f)(opcode) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Map1d(&self, target: types::GLenum, u1: types::GLdouble, u2: types::GLdouble, stride: types::GLint, order: types::GLint, points: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLdouble, types::GLdouble, types::GLint, types::GLint, *const types::GLdouble) -> ()>(self.Map1d.f)(target, u1, u2, stride, order, points) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Map1f(&self, target: types::GLenum, u1: types::GLfloat, u2: types::GLfloat, stride: types::GLint, order: types::GLint, points: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLfloat, types::GLfloat, types::GLint, types::GLint, *const types::GLfloat) -> ()>(self.Map1f.f)(target, u1, u2, stride, order, points) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Map2d(&self, target: types::GLenum, u1: types::GLdouble, u2: types::GLdouble, ustride: types::GLint, uorder: types::GLint, v1: types::GLdouble, v2: types::GLdouble, vstride: types::GLint, vorder: types::GLint, points: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLdouble, types::GLdouble, types::GLint, types::GLint, types::GLdouble, types::GLdouble, types::GLint, types::GLint, *const types::GLdouble) -> ()>(self.Map2d.f)(target, u1, u2, ustride, uorder, v1, v2, vstride, vorder, points) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Map2f(&self, target: types::GLenum, u1: types::GLfloat, u2: types::GLfloat, ustride: types::GLint, uorder: types::GLint, v1: types::GLfloat, v2: types::GLfloat, vstride: types::GLint, vorder: types::GLint, points: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLfloat, types::GLfloat, types::GLint, types::GLint, types::GLfloat, types::GLfloat, types::GLint, types::GLint, *const types::GLfloat) -> ()>(self.Map2f.f)(target, u1, u2, ustride, uorder, v1, v2, vstride, vorder, points) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MapBuffer(&self, target: types::GLenum, access: types::GLenum) -> *mut __gl_imports::raw::c_void { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum) -> *mut __gl_imports::raw::c_void>(self.MapBuffer.f)(target, access) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MapBufferRange(&self, target: types::GLenum, offset: types::GLintptr, length: types::GLsizeiptr, access: types::GLbitfield) -> *mut __gl_imports::raw::c_void { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLintptr, types::GLsizeiptr, types::GLbitfield) -> *mut __gl_imports::raw::c_void>(self.MapBufferRange.f)(target, offset, length, access) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MapGrid1d(&self, un: types::GLint, u1: types::GLdouble, u2: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLdouble, types::GLdouble) -> ()>(self.MapGrid1d.f)(un, u1, u2) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MapGrid1f(&self, un: types::GLint, u1: types::GLfloat, u2: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLfloat, types::GLfloat) -> ()>(self.MapGrid1f.f)(un, u1, u2) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MapGrid2d(&self, un: types::GLint, u1: types::GLdouble, u2: types::GLdouble, vn: types::GLint, v1: types::GLdouble, v2: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLdouble, types::GLdouble, types::GLint, types::GLdouble, types::GLdouble) -> ()>(self.MapGrid2d.f)(un, u1, u2, vn, v1, v2) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MapGrid2f(&self, un: types::GLint, u1: types::GLfloat, u2: types::GLfloat, vn: types::GLint, v1: types::GLfloat, v2: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLfloat, types::GLfloat, types::GLint, types::GLfloat, types::GLfloat) -> ()>(self.MapGrid2f.f)(un, u1, u2, vn, v1, v2) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Materialf(&self, face: types::GLenum, pname: types::GLenum, param: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLfloat) -> ()>(self.Materialf.f)(face, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Materialfv(&self, face: types::GLenum, pname: types::GLenum, params: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *const types::GLfloat) -> ()>(self.Materialfv.f)(face, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Materiali(&self, face: types::GLenum, pname: types::GLenum, param: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLint) -> ()>(self.Materiali.f)(face, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Materialiv(&self, face: types::GLenum, pname: types::GLenum, params: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *const types::GLint) -> ()>(self.Materialiv.f)(face, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MatrixMode(&self, mode: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.MatrixMode.f)(mode) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MemoryBarrier(&self, barriers: types::GLbitfield) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLbitfield) -> ()>(self.MemoryBarrier.f)(barriers) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MemoryBarrierByRegion(&self, barriers: types::GLbitfield) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLbitfield) -> ()>(self.MemoryBarrierByRegion.f)(barriers) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultMatrixd(&self, m: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble) -> ()>(self.MultMatrixd.f)(m) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultMatrixf(&self, m: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat) -> ()>(self.MultMatrixf.f)(m) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultTransposeMatrixd(&self, m: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble) -> ()>(self.MultTransposeMatrixd.f)(m) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultTransposeMatrixf(&self, m: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat) -> ()>(self.MultTransposeMatrixf.f)(m) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiDrawArrays(&self, mode: types::GLenum, first: *const types::GLint, count: *const types::GLsizei, drawcount: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLint, *const types::GLsizei, types::GLsizei) -> ()>(self.MultiDrawArrays.f)(mode, first, count, drawcount) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiDrawElements(&self, mode: types::GLenum, count: *const types::GLsizei, type_: types::GLenum, indices: *const *const __gl_imports::raw::c_void, drawcount: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLsizei, types::GLenum, *const *const __gl_imports::raw::c_void, types::GLsizei) -> ()>(self.MultiDrawElements.f)(mode, count, type_, indices, drawcount) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiDrawElementsBaseVertex(&self, mode: types::GLenum, count: *const types::GLsizei, type_: types::GLenum, indices: *const *const __gl_imports::raw::c_void, drawcount: types::GLsizei, basevertex: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLsizei, types::GLenum, *const *const __gl_imports::raw::c_void, types::GLsizei, *const types::GLint) -> ()>(self.MultiDrawElementsBaseVertex.f)(mode, count, type_, indices, drawcount, basevertex) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord1d(&self, target: types::GLenum, s: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLdouble) -> ()>(self.MultiTexCoord1d.f)(target, s) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord1dv(&self, target: types::GLenum, v: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLdouble) -> ()>(self.MultiTexCoord1dv.f)(target, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord1f(&self, target: types::GLenum, s: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLfloat) -> ()>(self.MultiTexCoord1f.f)(target, s) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord1fv(&self, target: types::GLenum, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLfloat) -> ()>(self.MultiTexCoord1fv.f)(target, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord1i(&self, target: types::GLenum, s: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint) -> ()>(self.MultiTexCoord1i.f)(target, s) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord1iv(&self, target: types::GLenum, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLint) -> ()>(self.MultiTexCoord1iv.f)(target, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord1s(&self, target: types::GLenum, s: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLshort) -> ()>(self.MultiTexCoord1s.f)(target, s) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord1sv(&self, target: types::GLenum, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLshort) -> ()>(self.MultiTexCoord1sv.f)(target, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord2d(&self, target: types::GLenum, s: types::GLdouble, t: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLdouble, types::GLdouble) -> ()>(self.MultiTexCoord2d.f)(target, s, t) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord2dv(&self, target: types::GLenum, v: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLdouble) -> ()>(self.MultiTexCoord2dv.f)(target, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord2f(&self, target: types::GLenum, s: types::GLfloat, t: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLfloat, types::GLfloat) -> ()>(self.MultiTexCoord2f.f)(target, s, t) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord2fv(&self, target: types::GLenum, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLfloat) -> ()>(self.MultiTexCoord2fv.f)(target, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord2i(&self, target: types::GLenum, s: types::GLint, t: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLint) -> ()>(self.MultiTexCoord2i.f)(target, s, t) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord2iv(&self, target: types::GLenum, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLint) -> ()>(self.MultiTexCoord2iv.f)(target, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord2s(&self, target: types::GLenum, s: types::GLshort, t: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLshort, types::GLshort) -> ()>(self.MultiTexCoord2s.f)(target, s, t) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord2sv(&self, target: types::GLenum, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLshort) -> ()>(self.MultiTexCoord2sv.f)(target, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord3d(&self, target: types::GLenum, s: types::GLdouble, t: types::GLdouble, r: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLdouble, types::GLdouble, types::GLdouble) -> ()>(self.MultiTexCoord3d.f)(target, s, t, r) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord3dv(&self, target: types::GLenum, v: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLdouble) -> ()>(self.MultiTexCoord3dv.f)(target, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord3f(&self, target: types::GLenum, s: types::GLfloat, t: types::GLfloat, r: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.MultiTexCoord3f.f)(target, s, t, r) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord3fv(&self, target: types::GLenum, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLfloat) -> ()>(self.MultiTexCoord3fv.f)(target, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord3i(&self, target: types::GLenum, s: types::GLint, t: types::GLint, r: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLint, types::GLint) -> ()>(self.MultiTexCoord3i.f)(target, s, t, r) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord3iv(&self, target: types::GLenum, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLint) -> ()>(self.MultiTexCoord3iv.f)(target, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord3s(&self, target: types::GLenum, s: types::GLshort, t: types::GLshort, r: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLshort, types::GLshort, types::GLshort) -> ()>(self.MultiTexCoord3s.f)(target, s, t, r) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord3sv(&self, target: types::GLenum, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLshort) -> ()>(self.MultiTexCoord3sv.f)(target, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord4d(&self, target: types::GLenum, s: types::GLdouble, t: types::GLdouble, r: types::GLdouble, q: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLdouble, types::GLdouble, types::GLdouble, types::GLdouble) -> ()>(self.MultiTexCoord4d.f)(target, s, t, r, q) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord4dv(&self, target: types::GLenum, v: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLdouble) -> ()>(self.MultiTexCoord4dv.f)(target, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord4f(&self, target: types::GLenum, s: types::GLfloat, t: types::GLfloat, r: types::GLfloat, q: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLfloat, types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.MultiTexCoord4f.f)(target, s, t, r, q) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord4fv(&self, target: types::GLenum, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLfloat) -> ()>(self.MultiTexCoord4fv.f)(target, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord4i(&self, target: types::GLenum, s: types::GLint, t: types::GLint, r: types::GLint, q: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLint, types::GLint, types::GLint) -> ()>(self.MultiTexCoord4i.f)(target, s, t, r, q) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord4iv(&self, target: types::GLenum, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLint) -> ()>(self.MultiTexCoord4iv.f)(target, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord4s(&self, target: types::GLenum, s: types::GLshort, t: types::GLshort, r: types::GLshort, q: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLshort, types::GLshort, types::GLshort, types::GLshort) -> ()>(self.MultiTexCoord4s.f)(target, s, t, r, q) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoord4sv(&self, target: types::GLenum, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLshort) -> ()>(self.MultiTexCoord4sv.f)(target, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoordP1ui(&self, texture: types::GLenum, type_: types::GLenum, coords: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLuint) -> ()>(self.MultiTexCoordP1ui.f)(texture, type_, coords) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoordP1uiv(&self, texture: types::GLenum, type_: types::GLenum, coords: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *const types::GLuint) -> ()>(self.MultiTexCoordP1uiv.f)(texture, type_, coords) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoordP2ui(&self, texture: types::GLenum, type_: types::GLenum, coords: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLuint) -> ()>(self.MultiTexCoordP2ui.f)(texture, type_, coords) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoordP2uiv(&self, texture: types::GLenum, type_: types::GLenum, coords: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *const types::GLuint) -> ()>(self.MultiTexCoordP2uiv.f)(texture, type_, coords) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoordP3ui(&self, texture: types::GLenum, type_: types::GLenum, coords: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLuint) -> ()>(self.MultiTexCoordP3ui.f)(texture, type_, coords) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoordP3uiv(&self, texture: types::GLenum, type_: types::GLenum, coords: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *const types::GLuint) -> ()>(self.MultiTexCoordP3uiv.f)(texture, type_, coords) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoordP4ui(&self, texture: types::GLenum, type_: types::GLenum, coords: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLuint) -> ()>(self.MultiTexCoordP4ui.f)(texture, type_, coords) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MultiTexCoordP4uiv(&self, texture: types::GLenum, type_: types::GLenum, coords: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *const types::GLuint) -> ()>(self.MultiTexCoordP4uiv.f)(texture, type_, coords) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn NewList(&self, list: types::GLuint, mode: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum) -> ()>(self.NewList.f)(list, mode) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Normal3b(&self, nx: types::GLbyte, ny: types::GLbyte, nz: types::GLbyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLbyte, types::GLbyte, types::GLbyte) -> ()>(self.Normal3b.f)(nx, ny, nz) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Normal3bv(&self, v: *const types::GLbyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLbyte) -> ()>(self.Normal3bv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Normal3d(&self, nx: types::GLdouble, ny: types::GLdouble, nz: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble, types::GLdouble, types::GLdouble) -> ()>(self.Normal3d.f)(nx, ny, nz) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Normal3dv(&self, v: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble) -> ()>(self.Normal3dv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Normal3f(&self, nx: types::GLfloat, ny: types::GLfloat, nz: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.Normal3f.f)(nx, ny, nz) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Normal3fv(&self, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat) -> ()>(self.Normal3fv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Normal3i(&self, nx: types::GLint, ny: types::GLint, nz: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint, types::GLint) -> ()>(self.Normal3i.f)(nx, ny, nz) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Normal3iv(&self, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLint) -> ()>(self.Normal3iv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Normal3s(&self, nx: types::GLshort, ny: types::GLshort, nz: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLshort, types::GLshort, types::GLshort) -> ()>(self.Normal3s.f)(nx, ny, nz) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Normal3sv(&self, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLshort) -> ()>(self.Normal3sv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn NormalP3ui(&self, type_: types::GLenum, coords: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint) -> ()>(self.NormalP3ui.f)(type_, coords) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn NormalP3uiv(&self, type_: types::GLenum, coords: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLuint) -> ()>(self.NormalP3uiv.f)(type_, coords) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn NormalPointer(&self, type_: types::GLenum, stride: types::GLsizei, pointer: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, *const __gl_imports::raw::c_void) -> ()>(self.NormalPointer.f)(type_, stride, pointer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ObjectLabel(&self, identifier: types::GLenum, name: types::GLuint, length: types::GLsizei, label: *const types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint, types::GLsizei, *const types::GLchar) -> ()>(self.ObjectLabel.f)(identifier, name, length, label) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ObjectLabelKHR(&self, identifier: types::GLenum, name: types::GLuint, length: types::GLsizei, label: *const types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint, types::GLsizei, *const types::GLchar) -> ()>(self.ObjectLabelKHR.f)(identifier, name, length, label) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ObjectPtrLabel(&self, ptr: *const __gl_imports::raw::c_void, length: types::GLsizei, label: *const types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const __gl_imports::raw::c_void, types::GLsizei, *const types::GLchar) -> ()>(self.ObjectPtrLabel.f)(ptr, length, label) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ObjectPtrLabelKHR(&self, ptr: *const __gl_imports::raw::c_void, length: types::GLsizei, label: *const types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const __gl_imports::raw::c_void, types::GLsizei, *const types::GLchar) -> ()>(self.ObjectPtrLabelKHR.f)(ptr, length, label) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Ortho(&self, left: types::GLdouble, right: types::GLdouble, bottom: types::GLdouble, top: types::GLdouble, zNear: types::GLdouble, zFar: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble, types::GLdouble, types::GLdouble, types::GLdouble, types::GLdouble, types::GLdouble) -> ()>(self.Ortho.f)(left, right, bottom, top, zNear, zFar) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PassThrough(&self, token: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat) -> ()>(self.PassThrough.f)(token) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PauseTransformFeedback(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.PauseTransformFeedback.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PixelMapfv(&self, map: types::GLenum, mapsize: types::GLsizei, values: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, *const types::GLfloat) -> ()>(self.PixelMapfv.f)(map, mapsize, values) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PixelMapuiv(&self, map: types::GLenum, mapsize: types::GLsizei, values: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, *const types::GLuint) -> ()>(self.PixelMapuiv.f)(map, mapsize, values) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PixelMapusv(&self, map: types::GLenum, mapsize: types::GLsizei, values: *const types::GLushort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, *const types::GLushort) -> ()>(self.PixelMapusv.f)(map, mapsize, values) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PixelStoref(&self, pname: types::GLenum, param: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLfloat) -> ()>(self.PixelStoref.f)(pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PixelStorei(&self, pname: types::GLenum, param: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint) -> ()>(self.PixelStorei.f)(pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PixelTransferf(&self, pname: types::GLenum, param: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLfloat) -> ()>(self.PixelTransferf.f)(pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PixelTransferi(&self, pname: types::GLenum, param: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint) -> ()>(self.PixelTransferi.f)(pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PixelZoom(&self, xfactor: types::GLfloat, yfactor: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat) -> ()>(self.PixelZoom.f)(xfactor, yfactor) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PointParameterf(&self, pname: types::GLenum, param: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLfloat) -> ()>(self.PointParameterf.f)(pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PointParameterfv(&self, pname: types::GLenum, params: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLfloat) -> ()>(self.PointParameterfv.f)(pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PointParameteri(&self, pname: types::GLenum, param: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint) -> ()>(self.PointParameteri.f)(pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PointParameteriv(&self, pname: types::GLenum, params: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLint) -> ()>(self.PointParameteriv.f)(pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PointSize(&self, size: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat) -> ()>(self.PointSize.f)(size) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PolygonMode(&self, face: types::GLenum, mode: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum) -> ()>(self.PolygonMode.f)(face, mode) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PolygonOffset(&self, factor: types::GLfloat, units: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat) -> ()>(self.PolygonOffset.f)(factor, units) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PolygonStipple(&self, mask: *const types::GLubyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLubyte) -> ()>(self.PolygonStipple.f)(mask) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PopAttrib(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.PopAttrib.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PopClientAttrib(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.PopClientAttrib.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PopDebugGroup(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.PopDebugGroup.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PopDebugGroupKHR(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.PopDebugGroupKHR.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PopGroupMarkerEXT(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.PopGroupMarkerEXT.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PopMatrix(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.PopMatrix.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PopName(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.PopName.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PrimitiveRestartIndex(&self, index: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.PrimitiveRestartIndex.f)(index) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PrioritizeTextures(&self, n: types::GLsizei, textures: *const types::GLuint, priorities: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *const types::GLuint, *const types::GLfloat) -> ()>(self.PrioritizeTextures.f)(n, textures, priorities) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramBinary(&self, program: types::GLuint, binaryFormat: types::GLenum, binary: *const __gl_imports::raw::c_void, length: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *const __gl_imports::raw::c_void, types::GLsizei) -> ()>(self.ProgramBinary.f)(program, binaryFormat, binary, length) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramParameteri(&self, program: types::GLuint, pname: types::GLenum, value: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLint) -> ()>(self.ProgramParameteri.f)(program, pname, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniform1f(&self, program: types::GLuint, location: types::GLint, v0: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLfloat) -> ()>(self.ProgramUniform1f.f)(program, location, v0) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniform1fv(&self, program: types::GLuint, location: types::GLint, count: types::GLsizei, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLsizei, *const types::GLfloat) -> ()>(self.ProgramUniform1fv.f)(program, location, count, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniform1i(&self, program: types::GLuint, location: types::GLint, v0: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLint) -> ()>(self.ProgramUniform1i.f)(program, location, v0) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniform1iv(&self, program: types::GLuint, location: types::GLint, count: types::GLsizei, value: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLsizei, *const types::GLint) -> ()>(self.ProgramUniform1iv.f)(program, location, count, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniform1ui(&self, program: types::GLuint, location: types::GLint, v0: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLuint) -> ()>(self.ProgramUniform1ui.f)(program, location, v0) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniform1uiv(&self, program: types::GLuint, location: types::GLint, count: types::GLsizei, value: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLsizei, *const types::GLuint) -> ()>(self.ProgramUniform1uiv.f)(program, location, count, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniform2f(&self, program: types::GLuint, location: types::GLint, v0: types::GLfloat, v1: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLfloat, types::GLfloat) -> ()>(self.ProgramUniform2f.f)(program, location, v0, v1) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniform2fv(&self, program: types::GLuint, location: types::GLint, count: types::GLsizei, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLsizei, *const types::GLfloat) -> ()>(self.ProgramUniform2fv.f)(program, location, count, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniform2i(&self, program: types::GLuint, location: types::GLint, v0: types::GLint, v1: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLint, types::GLint) -> ()>(self.ProgramUniform2i.f)(program, location, v0, v1) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniform2iv(&self, program: types::GLuint, location: types::GLint, count: types::GLsizei, value: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLsizei, *const types::GLint) -> ()>(self.ProgramUniform2iv.f)(program, location, count, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniform2ui(&self, program: types::GLuint, location: types::GLint, v0: types::GLuint, v1: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLuint, types::GLuint) -> ()>(self.ProgramUniform2ui.f)(program, location, v0, v1) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniform2uiv(&self, program: types::GLuint, location: types::GLint, count: types::GLsizei, value: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLsizei, *const types::GLuint) -> ()>(self.ProgramUniform2uiv.f)(program, location, count, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniform3f(&self, program: types::GLuint, location: types::GLint, v0: types::GLfloat, v1: types::GLfloat, v2: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.ProgramUniform3f.f)(program, location, v0, v1, v2) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniform3fv(&self, program: types::GLuint, location: types::GLint, count: types::GLsizei, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLsizei, *const types::GLfloat) -> ()>(self.ProgramUniform3fv.f)(program, location, count, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniform3i(&self, program: types::GLuint, location: types::GLint, v0: types::GLint, v1: types::GLint, v2: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLint, types::GLint, types::GLint) -> ()>(self.ProgramUniform3i.f)(program, location, v0, v1, v2) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniform3iv(&self, program: types::GLuint, location: types::GLint, count: types::GLsizei, value: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLsizei, *const types::GLint) -> ()>(self.ProgramUniform3iv.f)(program, location, count, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniform3ui(&self, program: types::GLuint, location: types::GLint, v0: types::GLuint, v1: types::GLuint, v2: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLuint, types::GLuint, types::GLuint) -> ()>(self.ProgramUniform3ui.f)(program, location, v0, v1, v2) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniform3uiv(&self, program: types::GLuint, location: types::GLint, count: types::GLsizei, value: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLsizei, *const types::GLuint) -> ()>(self.ProgramUniform3uiv.f)(program, location, count, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniform4f(&self, program: types::GLuint, location: types::GLint, v0: types::GLfloat, v1: types::GLfloat, v2: types::GLfloat, v3: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLfloat, types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.ProgramUniform4f.f)(program, location, v0, v1, v2, v3) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniform4fv(&self, program: types::GLuint, location: types::GLint, count: types::GLsizei, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLsizei, *const types::GLfloat) -> ()>(self.ProgramUniform4fv.f)(program, location, count, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniform4i(&self, program: types::GLuint, location: types::GLint, v0: types::GLint, v1: types::GLint, v2: types::GLint, v3: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint) -> ()>(self.ProgramUniform4i.f)(program, location, v0, v1, v2, v3) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniform4iv(&self, program: types::GLuint, location: types::GLint, count: types::GLsizei, value: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLsizei, *const types::GLint) -> ()>(self.ProgramUniform4iv.f)(program, location, count, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniform4ui(&self, program: types::GLuint, location: types::GLint, v0: types::GLuint, v1: types::GLuint, v2: types::GLuint, v3: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLuint, types::GLuint, types::GLuint, types::GLuint) -> ()>(self.ProgramUniform4ui.f)(program, location, v0, v1, v2, v3) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniform4uiv(&self, program: types::GLuint, location: types::GLint, count: types::GLsizei, value: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLsizei, *const types::GLuint) -> ()>(self.ProgramUniform4uiv.f)(program, location, count, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniformMatrix2fv(&self, program: types::GLuint, location: types::GLint, count: types::GLsizei, transpose: types::GLboolean, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLsizei, types::GLboolean, *const types::GLfloat) -> ()>(self.ProgramUniformMatrix2fv.f)(program, location, count, transpose, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniformMatrix2x3fv(&self, program: types::GLuint, location: types::GLint, count: types::GLsizei, transpose: types::GLboolean, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLsizei, types::GLboolean, *const types::GLfloat) -> ()>(self.ProgramUniformMatrix2x3fv.f)(program, location, count, transpose, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniformMatrix2x4fv(&self, program: types::GLuint, location: types::GLint, count: types::GLsizei, transpose: types::GLboolean, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLsizei, types::GLboolean, *const types::GLfloat) -> ()>(self.ProgramUniformMatrix2x4fv.f)(program, location, count, transpose, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniformMatrix3fv(&self, program: types::GLuint, location: types::GLint, count: types::GLsizei, transpose: types::GLboolean, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLsizei, types::GLboolean, *const types::GLfloat) -> ()>(self.ProgramUniformMatrix3fv.f)(program, location, count, transpose, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniformMatrix3x2fv(&self, program: types::GLuint, location: types::GLint, count: types::GLsizei, transpose: types::GLboolean, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLsizei, types::GLboolean, *const types::GLfloat) -> ()>(self.ProgramUniformMatrix3x2fv.f)(program, location, count, transpose, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniformMatrix3x4fv(&self, program: types::GLuint, location: types::GLint, count: types::GLsizei, transpose: types::GLboolean, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLsizei, types::GLboolean, *const types::GLfloat) -> ()>(self.ProgramUniformMatrix3x4fv.f)(program, location, count, transpose, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniformMatrix4fv(&self, program: types::GLuint, location: types::GLint, count: types::GLsizei, transpose: types::GLboolean, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLsizei, types::GLboolean, *const types::GLfloat) -> ()>(self.ProgramUniformMatrix4fv.f)(program, location, count, transpose, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniformMatrix4x2fv(&self, program: types::GLuint, location: types::GLint, count: types::GLsizei, transpose: types::GLboolean, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLsizei, types::GLboolean, *const types::GLfloat) -> ()>(self.ProgramUniformMatrix4x2fv.f)(program, location, count, transpose, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProgramUniformMatrix4x3fv(&self, program: types::GLuint, location: types::GLint, count: types::GLsizei, transpose: types::GLboolean, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLsizei, types::GLboolean, *const types::GLfloat) -> ()>(self.ProgramUniformMatrix4x3fv.f)(program, location, count, transpose, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProvokingVertex(&self, mode: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.ProvokingVertex.f)(mode) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ProvokingVertexANGLE(&self, mode: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.ProvokingVertexANGLE.f)(mode) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PushAttrib(&self, mask: types::GLbitfield) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLbitfield) -> ()>(self.PushAttrib.f)(mask) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PushClientAttrib(&self, mask: types::GLbitfield) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLbitfield) -> ()>(self.PushClientAttrib.f)(mask) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PushDebugGroup(&self, source: types::GLenum, id: types::GLuint, length: types::GLsizei, message: *const types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint, types::GLsizei, *const types::GLchar) -> ()>(self.PushDebugGroup.f)(source, id, length, message) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PushDebugGroupKHR(&self, source: types::GLenum, id: types::GLuint, length: types::GLsizei, message: *const types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint, types::GLsizei, *const types::GLchar) -> ()>(self.PushDebugGroupKHR.f)(source, id, length, message) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PushGroupMarkerEXT(&self, length: types::GLsizei, marker: *const types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *const types::GLchar) -> ()>(self.PushGroupMarkerEXT.f)(length, marker) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PushMatrix(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.PushMatrix.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PushName(&self, name: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.PushName.f)(name) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn QueryCounter(&self, id: types::GLuint, target: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum) -> ()>(self.QueryCounter.f)(id, target) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn QueryCounterEXT(&self, id: types::GLuint, target: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum) -> ()>(self.QueryCounterEXT.f)(id, target) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RasterPos2d(&self, x: types::GLdouble, y: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble, types::GLdouble) -> ()>(self.RasterPos2d.f)(x, y) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RasterPos2dv(&self, v: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble) -> ()>(self.RasterPos2dv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RasterPos2f(&self, x: types::GLfloat, y: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat) -> ()>(self.RasterPos2f.f)(x, y) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RasterPos2fv(&self, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat) -> ()>(self.RasterPos2fv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RasterPos2i(&self, x: types::GLint, y: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint) -> ()>(self.RasterPos2i.f)(x, y) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RasterPos2iv(&self, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLint) -> ()>(self.RasterPos2iv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RasterPos2s(&self, x: types::GLshort, y: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLshort, types::GLshort) -> ()>(self.RasterPos2s.f)(x, y) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RasterPos2sv(&self, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLshort) -> ()>(self.RasterPos2sv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RasterPos3d(&self, x: types::GLdouble, y: types::GLdouble, z: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble, types::GLdouble, types::GLdouble) -> ()>(self.RasterPos3d.f)(x, y, z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RasterPos3dv(&self, v: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble) -> ()>(self.RasterPos3dv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RasterPos3f(&self, x: types::GLfloat, y: types::GLfloat, z: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.RasterPos3f.f)(x, y, z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RasterPos3fv(&self, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat) -> ()>(self.RasterPos3fv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RasterPos3i(&self, x: types::GLint, y: types::GLint, z: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint, types::GLint) -> ()>(self.RasterPos3i.f)(x, y, z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RasterPos3iv(&self, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLint) -> ()>(self.RasterPos3iv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RasterPos3s(&self, x: types::GLshort, y: types::GLshort, z: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLshort, types::GLshort, types::GLshort) -> ()>(self.RasterPos3s.f)(x, y, z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RasterPos3sv(&self, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLshort) -> ()>(self.RasterPos3sv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RasterPos4d(&self, x: types::GLdouble, y: types::GLdouble, z: types::GLdouble, w: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble, types::GLdouble, types::GLdouble, types::GLdouble) -> ()>(self.RasterPos4d.f)(x, y, z, w) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RasterPos4dv(&self, v: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble) -> ()>(self.RasterPos4dv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RasterPos4f(&self, x: types::GLfloat, y: types::GLfloat, z: types::GLfloat, w: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.RasterPos4f.f)(x, y, z, w) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RasterPos4fv(&self, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat) -> ()>(self.RasterPos4fv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RasterPos4i(&self, x: types::GLint, y: types::GLint, z: types::GLint, w: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint, types::GLint, types::GLint) -> ()>(self.RasterPos4i.f)(x, y, z, w) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RasterPos4iv(&self, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLint) -> ()>(self.RasterPos4iv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RasterPos4s(&self, x: types::GLshort, y: types::GLshort, z: types::GLshort, w: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLshort, types::GLshort, types::GLshort, types::GLshort) -> ()>(self.RasterPos4s.f)(x, y, z, w) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RasterPos4sv(&self, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLshort) -> ()>(self.RasterPos4sv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ReadBuffer(&self, src: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.ReadBuffer.f)(src) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ReadPixels(&self, x: types::GLint, y: types::GLint, width: types::GLsizei, height: types::GLsizei, format: types::GLenum, type_: types::GLenum, pixels: *mut __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint, types::GLsizei, types::GLsizei, types::GLenum, types::GLenum, *mut __gl_imports::raw::c_void) -> ()>(self.ReadPixels.f)(x, y, width, height, format, type_, pixels) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Rectd(&self, x1: types::GLdouble, y1: types::GLdouble, x2: types::GLdouble, y2: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble, types::GLdouble, types::GLdouble, types::GLdouble) -> ()>(self.Rectd.f)(x1, y1, x2, y2) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Rectdv(&self, v1: *const types::GLdouble, v2: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble, *const types::GLdouble) -> ()>(self.Rectdv.f)(v1, v2) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Rectf(&self, x1: types::GLfloat, y1: types::GLfloat, x2: types::GLfloat, y2: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.Rectf.f)(x1, y1, x2, y2) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Rectfv(&self, v1: *const types::GLfloat, v2: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat, *const types::GLfloat) -> ()>(self.Rectfv.f)(v1, v2) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Recti(&self, x1: types::GLint, y1: types::GLint, x2: types::GLint, y2: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint, types::GLint, types::GLint) -> ()>(self.Recti.f)(x1, y1, x2, y2) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Rectiv(&self, v1: *const types::GLint, v2: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLint, *const types::GLint) -> ()>(self.Rectiv.f)(v1, v2) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Rects(&self, x1: types::GLshort, y1: types::GLshort, x2: types::GLshort, y2: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLshort, types::GLshort, types::GLshort, types::GLshort) -> ()>(self.Rects.f)(x1, y1, x2, y2) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Rectsv(&self, v1: *const types::GLshort, v2: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLshort, *const types::GLshort) -> ()>(self.Rectsv.f)(v1, v2) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ReleaseShaderCompiler(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.ReleaseShaderCompiler.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RenderMode(&self, mode: types::GLenum) -> types::GLint { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> types::GLint>(self.RenderMode.f)(mode) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RenderbufferStorage(&self, target: types::GLenum, internalformat: types::GLenum, width: types::GLsizei, height: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLsizei, types::GLsizei) -> ()>(self.RenderbufferStorage.f)(target, internalformat, width, height) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RenderbufferStorageMultisample(&self, target: types::GLenum, samples: types::GLsizei, internalformat: types::GLenum, width: types::GLsizei, height: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, types::GLenum, types::GLsizei, types::GLsizei) -> ()>(self.RenderbufferStorageMultisample.f)(target, samples, internalformat, width, height) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ResumeTransformFeedback(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.ResumeTransformFeedback.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Rotated(&self, angle: types::GLdouble, x: types::GLdouble, y: types::GLdouble, z: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble, types::GLdouble, types::GLdouble, types::GLdouble) -> ()>(self.Rotated.f)(angle, x, y, z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Rotatef(&self, angle: types::GLfloat, x: types::GLfloat, y: types::GLfloat, z: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.Rotatef.f)(angle, x, y, z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SampleCoverage(&self, value: types::GLfloat, invert: types::GLboolean) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLboolean) -> ()>(self.SampleCoverage.f)(value, invert) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SampleMaski(&self, maskNumber: types::GLuint, mask: types::GLbitfield) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLbitfield) -> ()>(self.SampleMaski.f)(maskNumber, mask) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SamplerParameterIiv(&self, sampler: types::GLuint, pname: types::GLenum, param: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *const types::GLint) -> ()>(self.SamplerParameterIiv.f)(sampler, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SamplerParameterIuiv(&self, sampler: types::GLuint, pname: types::GLenum, param: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *const types::GLuint) -> ()>(self.SamplerParameterIuiv.f)(sampler, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SamplerParameterf(&self, sampler: types::GLuint, pname: types::GLenum, param: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLfloat) -> ()>(self.SamplerParameterf.f)(sampler, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SamplerParameterfv(&self, sampler: types::GLuint, pname: types::GLenum, param: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *const types::GLfloat) -> ()>(self.SamplerParameterfv.f)(sampler, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SamplerParameteri(&self, sampler: types::GLuint, pname: types::GLenum, param: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLint) -> ()>(self.SamplerParameteri.f)(sampler, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SamplerParameteriv(&self, sampler: types::GLuint, pname: types::GLenum, param: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *const types::GLint) -> ()>(self.SamplerParameteriv.f)(sampler, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Scaled(&self, x: types::GLdouble, y: types::GLdouble, z: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble, types::GLdouble, types::GLdouble) -> ()>(self.Scaled.f)(x, y, z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Scalef(&self, x: types::GLfloat, y: types::GLfloat, z: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.Scalef.f)(x, y, z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Scissor(&self, x: types::GLint, y: types::GLint, width: types::GLsizei, height: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint, types::GLsizei, types::GLsizei) -> ()>(self.Scissor.f)(x, y, width, height) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SecondaryColor3b(&self, red: types::GLbyte, green: types::GLbyte, blue: types::GLbyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLbyte, types::GLbyte, types::GLbyte) -> ()>(self.SecondaryColor3b.f)(red, green, blue) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SecondaryColor3bv(&self, v: *const types::GLbyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLbyte) -> ()>(self.SecondaryColor3bv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SecondaryColor3d(&self, red: types::GLdouble, green: types::GLdouble, blue: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble, types::GLdouble, types::GLdouble) -> ()>(self.SecondaryColor3d.f)(red, green, blue) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SecondaryColor3dv(&self, v: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble) -> ()>(self.SecondaryColor3dv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SecondaryColor3f(&self, red: types::GLfloat, green: types::GLfloat, blue: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.SecondaryColor3f.f)(red, green, blue) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SecondaryColor3fv(&self, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat) -> ()>(self.SecondaryColor3fv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SecondaryColor3i(&self, red: types::GLint, green: types::GLint, blue: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint, types::GLint) -> ()>(self.SecondaryColor3i.f)(red, green, blue) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SecondaryColor3iv(&self, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLint) -> ()>(self.SecondaryColor3iv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SecondaryColor3s(&self, red: types::GLshort, green: types::GLshort, blue: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLshort, types::GLshort, types::GLshort) -> ()>(self.SecondaryColor3s.f)(red, green, blue) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SecondaryColor3sv(&self, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLshort) -> ()>(self.SecondaryColor3sv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SecondaryColor3ub(&self, red: types::GLubyte, green: types::GLubyte, blue: types::GLubyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLubyte, types::GLubyte, types::GLubyte) -> ()>(self.SecondaryColor3ub.f)(red, green, blue) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SecondaryColor3ubv(&self, v: *const types::GLubyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLubyte) -> ()>(self.SecondaryColor3ubv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SecondaryColor3ui(&self, red: types::GLuint, green: types::GLuint, blue: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint, types::GLuint) -> ()>(self.SecondaryColor3ui.f)(red, green, blue) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SecondaryColor3uiv(&self, v: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLuint) -> ()>(self.SecondaryColor3uiv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SecondaryColor3us(&self, red: types::GLushort, green: types::GLushort, blue: types::GLushort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLushort, types::GLushort, types::GLushort) -> ()>(self.SecondaryColor3us.f)(red, green, blue) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SecondaryColor3usv(&self, v: *const types::GLushort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLushort) -> ()>(self.SecondaryColor3usv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SecondaryColorP3ui(&self, type_: types::GLenum, color: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint) -> ()>(self.SecondaryColorP3ui.f)(type_, color) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SecondaryColorP3uiv(&self, type_: types::GLenum, color: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLuint) -> ()>(self.SecondaryColorP3uiv.f)(type_, color) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SecondaryColorPointer(&self, size: types::GLint, type_: types::GLenum, stride: types::GLsizei, pointer: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLenum, types::GLsizei, *const __gl_imports::raw::c_void) -> ()>(self.SecondaryColorPointer.f)(size, type_, stride, pointer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SelectBuffer(&self, size: types::GLsizei, buffer: *mut types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *mut types::GLuint) -> ()>(self.SelectBuffer.f)(size, buffer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SetFenceAPPLE(&self, fence: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.SetFenceAPPLE.f)(fence) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ShadeModel(&self, mode: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.ShadeModel.f)(mode) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ShaderBinary(&self, count: types::GLsizei, shaders: *const types::GLuint, binaryformat: types::GLenum, binary: *const __gl_imports::raw::c_void, length: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *const types::GLuint, types::GLenum, *const __gl_imports::raw::c_void, types::GLsizei) -> ()>(self.ShaderBinary.f)(count, shaders, binaryformat, binary, length) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ShaderSource(&self, shader: types::GLuint, count: types::GLsizei, string: *const *const types::GLchar, length: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLsizei, *const *const types::GLchar, *const types::GLint) -> ()>(self.ShaderSource.f)(shader, count, string, length) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ShaderStorageBlockBinding(&self, program: types::GLuint, storageBlockIndex: types::GLuint, storageBlockBinding: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint, types::GLuint) -> ()>(self.ShaderStorageBlockBinding.f)(program, storageBlockIndex, storageBlockBinding) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn StencilFunc(&self, func: types::GLenum, ref_: types::GLint, mask: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLuint) -> ()>(self.StencilFunc.f)(func, ref_, mask) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn StencilFuncSeparate(&self, face: types::GLenum, func: types::GLenum, ref_: types::GLint, mask: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLint, types::GLuint) -> ()>(self.StencilFuncSeparate.f)(face, func, ref_, mask) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn StencilMask(&self, mask: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.StencilMask.f)(mask) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn StencilMaskSeparate(&self, face: types::GLenum, mask: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint) -> ()>(self.StencilMaskSeparate.f)(face, mask) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn StencilOp(&self, fail: types::GLenum, zfail: types::GLenum, zpass: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLenum) -> ()>(self.StencilOp.f)(fail, zfail, zpass) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn StencilOpSeparate(&self, face: types::GLenum, sfail: types::GLenum, dpfail: types::GLenum, dppass: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLenum, types::GLenum) -> ()>(self.StencilOpSeparate.f)(face, sfail, dpfail, dppass) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TestFenceAPPLE(&self, fence: types::GLuint) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> types::GLboolean>(self.TestFenceAPPLE.f)(fence) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TestObjectAPPLE(&self, object: types::GLenum, name: types::GLuint) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint) -> types::GLboolean>(self.TestObjectAPPLE.f)(object, name) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexBuffer(&self, target: types::GLenum, internalformat: types::GLenum, buffer: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLuint) -> ()>(self.TexBuffer.f)(target, internalformat, buffer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord1d(&self, s: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble) -> ()>(self.TexCoord1d.f)(s) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord1dv(&self, v: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble) -> ()>(self.TexCoord1dv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord1f(&self, s: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat) -> ()>(self.TexCoord1f.f)(s) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord1fv(&self, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat) -> ()>(self.TexCoord1fv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord1i(&self, s: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint) -> ()>(self.TexCoord1i.f)(s) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord1iv(&self, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLint) -> ()>(self.TexCoord1iv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord1s(&self, s: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLshort) -> ()>(self.TexCoord1s.f)(s) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord1sv(&self, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLshort) -> ()>(self.TexCoord1sv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord2d(&self, s: types::GLdouble, t: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble, types::GLdouble) -> ()>(self.TexCoord2d.f)(s, t) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord2dv(&self, v: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble) -> ()>(self.TexCoord2dv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord2f(&self, s: types::GLfloat, t: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat) -> ()>(self.TexCoord2f.f)(s, t) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord2fv(&self, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat) -> ()>(self.TexCoord2fv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord2i(&self, s: types::GLint, t: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint) -> ()>(self.TexCoord2i.f)(s, t) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord2iv(&self, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLint) -> ()>(self.TexCoord2iv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord2s(&self, s: types::GLshort, t: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLshort, types::GLshort) -> ()>(self.TexCoord2s.f)(s, t) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord2sv(&self, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLshort) -> ()>(self.TexCoord2sv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord3d(&self, s: types::GLdouble, t: types::GLdouble, r: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble, types::GLdouble, types::GLdouble) -> ()>(self.TexCoord3d.f)(s, t, r) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord3dv(&self, v: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble) -> ()>(self.TexCoord3dv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord3f(&self, s: types::GLfloat, t: types::GLfloat, r: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.TexCoord3f.f)(s, t, r) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord3fv(&self, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat) -> ()>(self.TexCoord3fv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord3i(&self, s: types::GLint, t: types::GLint, r: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint, types::GLint) -> ()>(self.TexCoord3i.f)(s, t, r) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord3iv(&self, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLint) -> ()>(self.TexCoord3iv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord3s(&self, s: types::GLshort, t: types::GLshort, r: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLshort, types::GLshort, types::GLshort) -> ()>(self.TexCoord3s.f)(s, t, r) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord3sv(&self, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLshort) -> ()>(self.TexCoord3sv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord4d(&self, s: types::GLdouble, t: types::GLdouble, r: types::GLdouble, q: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble, types::GLdouble, types::GLdouble, types::GLdouble) -> ()>(self.TexCoord4d.f)(s, t, r, q) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord4dv(&self, v: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble) -> ()>(self.TexCoord4dv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord4f(&self, s: types::GLfloat, t: types::GLfloat, r: types::GLfloat, q: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.TexCoord4f.f)(s, t, r, q) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord4fv(&self, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat) -> ()>(self.TexCoord4fv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord4i(&self, s: types::GLint, t: types::GLint, r: types::GLint, q: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint, types::GLint, types::GLint) -> ()>(self.TexCoord4i.f)(s, t, r, q) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord4iv(&self, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLint) -> ()>(self.TexCoord4iv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord4s(&self, s: types::GLshort, t: types::GLshort, r: types::GLshort, q: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLshort, types::GLshort, types::GLshort, types::GLshort) -> ()>(self.TexCoord4s.f)(s, t, r, q) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoord4sv(&self, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLshort) -> ()>(self.TexCoord4sv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoordP1ui(&self, type_: types::GLenum, coords: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint) -> ()>(self.TexCoordP1ui.f)(type_, coords) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoordP1uiv(&self, type_: types::GLenum, coords: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLuint) -> ()>(self.TexCoordP1uiv.f)(type_, coords) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoordP2ui(&self, type_: types::GLenum, coords: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint) -> ()>(self.TexCoordP2ui.f)(type_, coords) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoordP2uiv(&self, type_: types::GLenum, coords: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLuint) -> ()>(self.TexCoordP2uiv.f)(type_, coords) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoordP3ui(&self, type_: types::GLenum, coords: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint) -> ()>(self.TexCoordP3ui.f)(type_, coords) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoordP3uiv(&self, type_: types::GLenum, coords: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLuint) -> ()>(self.TexCoordP3uiv.f)(type_, coords) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoordP4ui(&self, type_: types::GLenum, coords: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint) -> ()>(self.TexCoordP4ui.f)(type_, coords) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoordP4uiv(&self, type_: types::GLenum, coords: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLuint) -> ()>(self.TexCoordP4uiv.f)(type_, coords) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexCoordPointer(&self, size: types::GLint, type_: types::GLenum, stride: types::GLsizei, pointer: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLenum, types::GLsizei, *const __gl_imports::raw::c_void) -> ()>(self.TexCoordPointer.f)(size, type_, stride, pointer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexEnvf(&self, target: types::GLenum, pname: types::GLenum, param: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLfloat) -> ()>(self.TexEnvf.f)(target, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexEnvfv(&self, target: types::GLenum, pname: types::GLenum, params: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *const types::GLfloat) -> ()>(self.TexEnvfv.f)(target, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexEnvi(&self, target: types::GLenum, pname: types::GLenum, param: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLint) -> ()>(self.TexEnvi.f)(target, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexEnviv(&self, target: types::GLenum, pname: types::GLenum, params: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *const types::GLint) -> ()>(self.TexEnviv.f)(target, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexGend(&self, coord: types::GLenum, pname: types::GLenum, param: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLdouble) -> ()>(self.TexGend.f)(coord, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexGendv(&self, coord: types::GLenum, pname: types::GLenum, params: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *const types::GLdouble) -> ()>(self.TexGendv.f)(coord, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexGenf(&self, coord: types::GLenum, pname: types::GLenum, param: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLfloat) -> ()>(self.TexGenf.f)(coord, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexGenfv(&self, coord: types::GLenum, pname: types::GLenum, params: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *const types::GLfloat) -> ()>(self.TexGenfv.f)(coord, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexGeni(&self, coord: types::GLenum, pname: types::GLenum, param: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLint) -> ()>(self.TexGeni.f)(coord, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexGeniv(&self, coord: types::GLenum, pname: types::GLenum, params: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *const types::GLint) -> ()>(self.TexGeniv.f)(coord, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexImage1D(&self, target: types::GLenum, level: types::GLint, internalformat: types::GLint, width: types::GLsizei, border: types::GLint, format: types::GLenum, type_: types::GLenum, pixels: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLint, types::GLsizei, types::GLint, types::GLenum, types::GLenum, *const __gl_imports::raw::c_void) -> ()>(self.TexImage1D.f)(target, level, internalformat, width, border, format, type_, pixels) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexImage2D(&self, target: types::GLenum, level: types::GLint, internalformat: types::GLint, width: types::GLsizei, height: types::GLsizei, border: types::GLint, format: types::GLenum, type_: types::GLenum, pixels: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLint, types::GLsizei, types::GLsizei, types::GLint, types::GLenum, types::GLenum, *const __gl_imports::raw::c_void) -> ()>(self.TexImage2D.f)(target, level, internalformat, width, height, border, format, type_, pixels) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexImage2DMultisample(&self, target: types::GLenum, samples: types::GLsizei, internalformat: types::GLenum, width: types::GLsizei, height: types::GLsizei, fixedsamplelocations: types::GLboolean) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, types::GLenum, types::GLsizei, types::GLsizei, types::GLboolean) -> ()>(self.TexImage2DMultisample.f)(target, samples, internalformat, width, height, fixedsamplelocations) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexImage3D(&self, target: types::GLenum, level: types::GLint, internalformat: types::GLint, width: types::GLsizei, height: types::GLsizei, depth: types::GLsizei, border: types::GLint, format: types::GLenum, type_: types::GLenum, pixels: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLint, types::GLsizei, types::GLsizei, types::GLsizei, types::GLint, types::GLenum, types::GLenum, *const __gl_imports::raw::c_void) -> ()>(self.TexImage3D.f)(target, level, internalformat, width, height, depth, border, format, type_, pixels) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexImage3DMultisample(&self, target: types::GLenum, samples: types::GLsizei, internalformat: types::GLenum, width: types::GLsizei, height: types::GLsizei, depth: types::GLsizei, fixedsamplelocations: types::GLboolean) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, types::GLenum, types::GLsizei, types::GLsizei, types::GLsizei, types::GLboolean) -> ()>(self.TexImage3DMultisample.f)(target, samples, internalformat, width, height, depth, fixedsamplelocations) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexParameterIiv(&self, target: types::GLenum, pname: types::GLenum, params: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *const types::GLint) -> ()>(self.TexParameterIiv.f)(target, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexParameterIuiv(&self, target: types::GLenum, pname: types::GLenum, params: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *const types::GLuint) -> ()>(self.TexParameterIuiv.f)(target, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexParameterf(&self, target: types::GLenum, pname: types::GLenum, param: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLfloat) -> ()>(self.TexParameterf.f)(target, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexParameterfv(&self, target: types::GLenum, pname: types::GLenum, params: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *const types::GLfloat) -> ()>(self.TexParameterfv.f)(target, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexParameteri(&self, target: types::GLenum, pname: types::GLenum, param: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLint) -> ()>(self.TexParameteri.f)(target, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexParameteriv(&self, target: types::GLenum, pname: types::GLenum, params: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *const types::GLint) -> ()>(self.TexParameteriv.f)(target, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexStorage1D(&self, target: types::GLenum, levels: types::GLsizei, internalformat: types::GLenum, width: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, types::GLenum, types::GLsizei) -> ()>(self.TexStorage1D.f)(target, levels, internalformat, width) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexStorage1DEXT(&self, target: types::GLenum, levels: types::GLsizei, internalformat: types::GLenum, width: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, types::GLenum, types::GLsizei) -> ()>(self.TexStorage1DEXT.f)(target, levels, internalformat, width) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexStorage2D(&self, target: types::GLenum, levels: types::GLsizei, internalformat: types::GLenum, width: types::GLsizei, height: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, types::GLenum, types::GLsizei, types::GLsizei) -> ()>(self.TexStorage2D.f)(target, levels, internalformat, width, height) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexStorage2DEXT(&self, target: types::GLenum, levels: types::GLsizei, internalformat: types::GLenum, width: types::GLsizei, height: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, types::GLenum, types::GLsizei, types::GLsizei) -> ()>(self.TexStorage2DEXT.f)(target, levels, internalformat, width, height) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexStorage2DMultisample(&self, target: types::GLenum, samples: types::GLsizei, internalformat: types::GLenum, width: types::GLsizei, height: types::GLsizei, fixedsamplelocations: types::GLboolean) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, types::GLenum, types::GLsizei, types::GLsizei, types::GLboolean) -> ()>(self.TexStorage2DMultisample.f)(target, samples, internalformat, width, height, fixedsamplelocations) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexStorage3D(&self, target: types::GLenum, levels: types::GLsizei, internalformat: types::GLenum, width: types::GLsizei, height: types::GLsizei, depth: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, types::GLenum, types::GLsizei, types::GLsizei, types::GLsizei) -> ()>(self.TexStorage3D.f)(target, levels, internalformat, width, height, depth) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexStorage3DEXT(&self, target: types::GLenum, levels: types::GLsizei, internalformat: types::GLenum, width: types::GLsizei, height: types::GLsizei, depth: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, types::GLenum, types::GLsizei, types::GLsizei, types::GLsizei) -> ()>(self.TexStorage3DEXT.f)(target, levels, internalformat, width, height, depth) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexSubImage1D(&self, target: types::GLenum, level: types::GLint, xoffset: types::GLint, width: types::GLsizei, format: types::GLenum, type_: types::GLenum, pixels: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLint, types::GLsizei, types::GLenum, types::GLenum, *const __gl_imports::raw::c_void) -> ()>(self.TexSubImage1D.f)(target, level, xoffset, width, format, type_, pixels) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexSubImage2D(&self, target: types::GLenum, level: types::GLint, xoffset: types::GLint, yoffset: types::GLint, width: types::GLsizei, height: types::GLsizei, format: types::GLenum, type_: types::GLenum, pixels: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLint, types::GLint, types::GLsizei, types::GLsizei, types::GLenum, types::GLenum, *const __gl_imports::raw::c_void) -> ()>(self.TexSubImage2D.f)(target, level, xoffset, yoffset, width, height, format, type_, pixels) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexSubImage3D(&self, target: types::GLenum, level: types::GLint, xoffset: types::GLint, yoffset: types::GLint, zoffset: types::GLint, width: types::GLsizei, height: types::GLsizei, depth: types::GLsizei, format: types::GLenum, type_: types::GLenum, pixels: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLint, types::GLint, types::GLint, types::GLsizei, types::GLsizei, types::GLsizei, types::GLenum, types::GLenum, *const __gl_imports::raw::c_void) -> ()>(self.TexSubImage3D.f)(target, level, xoffset, yoffset, zoffset, width, height, depth, format, type_, pixels) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TextureRangeAPPLE(&self, target: types::GLenum, length: types::GLsizei, pointer: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, *const __gl_imports::raw::c_void) -> ()>(self.TextureRangeAPPLE.f)(target, length, pointer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TextureStorage1DEXT(&self, texture: types::GLuint, target: types::GLenum, levels: types::GLsizei, internalformat: types::GLenum, width: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLsizei, types::GLenum, types::GLsizei) -> ()>(self.TextureStorage1DEXT.f)(texture, target, levels, internalformat, width) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TextureStorage2DEXT(&self, texture: types::GLuint, target: types::GLenum, levels: types::GLsizei, internalformat: types::GLenum, width: types::GLsizei, height: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLsizei, types::GLenum, types::GLsizei, types::GLsizei) -> ()>(self.TextureStorage2DEXT.f)(texture, target, levels, internalformat, width, height) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TextureStorage3DEXT(&self, texture: types::GLuint, target: types::GLenum, levels: types::GLsizei, internalformat: types::GLenum, width: types::GLsizei, height: types::GLsizei, depth: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLsizei, types::GLenum, types::GLsizei, types::GLsizei, types::GLsizei) -> ()>(self.TextureStorage3DEXT.f)(texture, target, levels, internalformat, width, height, depth) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TransformFeedbackVaryings(&self, program: types::GLuint, count: types::GLsizei, varyings: *const *const types::GLchar, bufferMode: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLsizei, *const *const types::GLchar, types::GLenum) -> ()>(self.TransformFeedbackVaryings.f)(program, count, varyings, bufferMode) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Translated(&self, x: types::GLdouble, y: types::GLdouble, z: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble, types::GLdouble, types::GLdouble) -> ()>(self.Translated.f)(x, y, z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Translatef(&self, x: types::GLfloat, y: types::GLfloat, z: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.Translatef.f)(x, y, z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Uniform1f(&self, location: types::GLint, v0: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLfloat) -> ()>(self.Uniform1f.f)(location, v0) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Uniform1fv(&self, location: types::GLint, count: types::GLsizei, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLsizei, *const types::GLfloat) -> ()>(self.Uniform1fv.f)(location, count, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Uniform1i(&self, location: types::GLint, v0: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint) -> ()>(self.Uniform1i.f)(location, v0) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Uniform1iv(&self, location: types::GLint, count: types::GLsizei, value: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLsizei, *const types::GLint) -> ()>(self.Uniform1iv.f)(location, count, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Uniform1ui(&self, location: types::GLint, v0: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLuint) -> ()>(self.Uniform1ui.f)(location, v0) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Uniform1uiv(&self, location: types::GLint, count: types::GLsizei, value: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLsizei, *const types::GLuint) -> ()>(self.Uniform1uiv.f)(location, count, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Uniform2f(&self, location: types::GLint, v0: types::GLfloat, v1: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLfloat, types::GLfloat) -> ()>(self.Uniform2f.f)(location, v0, v1) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Uniform2fv(&self, location: types::GLint, count: types::GLsizei, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLsizei, *const types::GLfloat) -> ()>(self.Uniform2fv.f)(location, count, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Uniform2i(&self, location: types::GLint, v0: types::GLint, v1: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint, types::GLint) -> ()>(self.Uniform2i.f)(location, v0, v1) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Uniform2iv(&self, location: types::GLint, count: types::GLsizei, value: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLsizei, *const types::GLint) -> ()>(self.Uniform2iv.f)(location, count, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Uniform2ui(&self, location: types::GLint, v0: types::GLuint, v1: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLuint, types::GLuint) -> ()>(self.Uniform2ui.f)(location, v0, v1) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Uniform2uiv(&self, location: types::GLint, count: types::GLsizei, value: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLsizei, *const types::GLuint) -> ()>(self.Uniform2uiv.f)(location, count, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Uniform3f(&self, location: types::GLint, v0: types::GLfloat, v1: types::GLfloat, v2: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.Uniform3f.f)(location, v0, v1, v2) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Uniform3fv(&self, location: types::GLint, count: types::GLsizei, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLsizei, *const types::GLfloat) -> ()>(self.Uniform3fv.f)(location, count, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Uniform3i(&self, location: types::GLint, v0: types::GLint, v1: types::GLint, v2: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint, types::GLint, types::GLint) -> ()>(self.Uniform3i.f)(location, v0, v1, v2) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Uniform3iv(&self, location: types::GLint, count: types::GLsizei, value: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLsizei, *const types::GLint) -> ()>(self.Uniform3iv.f)(location, count, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Uniform3ui(&self, location: types::GLint, v0: types::GLuint, v1: types::GLuint, v2: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLuint, types::GLuint, types::GLuint) -> ()>(self.Uniform3ui.f)(location, v0, v1, v2) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Uniform3uiv(&self, location: types::GLint, count: types::GLsizei, value: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLsizei, *const types::GLuint) -> ()>(self.Uniform3uiv.f)(location, count, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Uniform4f(&self, location: types::GLint, v0: types::GLfloat, v1: types::GLfloat, v2: types::GLfloat, v3: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLfloat, types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.Uniform4f.f)(location, v0, v1, v2, v3) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Uniform4fv(&self, location: types::GLint, count: types::GLsizei, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLsizei, *const types::GLfloat) -> ()>(self.Uniform4fv.f)(location, count, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Uniform4i(&self, location: types::GLint, v0: types::GLint, v1: types::GLint, v2: types::GLint, v3: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint, types::GLint, types::GLint, types::GLint) -> ()>(self.Uniform4i.f)(location, v0, v1, v2, v3) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Uniform4iv(&self, location: types::GLint, count: types::GLsizei, value: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLsizei, *const types::GLint) -> ()>(self.Uniform4iv.f)(location, count, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Uniform4ui(&self, location: types::GLint, v0: types::GLuint, v1: types::GLuint, v2: types::GLuint, v3: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLuint, types::GLuint, types::GLuint, types::GLuint) -> ()>(self.Uniform4ui.f)(location, v0, v1, v2, v3) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Uniform4uiv(&self, location: types::GLint, count: types::GLsizei, value: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLsizei, *const types::GLuint) -> ()>(self.Uniform4uiv.f)(location, count, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn UniformBlockBinding(&self, program: types::GLuint, uniformBlockIndex: types::GLuint, uniformBlockBinding: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint, types::GLuint) -> ()>(self.UniformBlockBinding.f)(program, uniformBlockIndex, uniformBlockBinding) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn UniformMatrix2fv(&self, location: types::GLint, count: types::GLsizei, transpose: types::GLboolean, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLsizei, types::GLboolean, *const types::GLfloat) -> ()>(self.UniformMatrix2fv.f)(location, count, transpose, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn UniformMatrix2x3fv(&self, location: types::GLint, count: types::GLsizei, transpose: types::GLboolean, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLsizei, types::GLboolean, *const types::GLfloat) -> ()>(self.UniformMatrix2x3fv.f)(location, count, transpose, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn UniformMatrix2x4fv(&self, location: types::GLint, count: types::GLsizei, transpose: types::GLboolean, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLsizei, types::GLboolean, *const types::GLfloat) -> ()>(self.UniformMatrix2x4fv.f)(location, count, transpose, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn UniformMatrix3fv(&self, location: types::GLint, count: types::GLsizei, transpose: types::GLboolean, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLsizei, types::GLboolean, *const types::GLfloat) -> ()>(self.UniformMatrix3fv.f)(location, count, transpose, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn UniformMatrix3x2fv(&self, location: types::GLint, count: types::GLsizei, transpose: types::GLboolean, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLsizei, types::GLboolean, *const types::GLfloat) -> ()>(self.UniformMatrix3x2fv.f)(location, count, transpose, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn UniformMatrix3x4fv(&self, location: types::GLint, count: types::GLsizei, transpose: types::GLboolean, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLsizei, types::GLboolean, *const types::GLfloat) -> ()>(self.UniformMatrix3x4fv.f)(location, count, transpose, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn UniformMatrix4fv(&self, location: types::GLint, count: types::GLsizei, transpose: types::GLboolean, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLsizei, types::GLboolean, *const types::GLfloat) -> ()>(self.UniformMatrix4fv.f)(location, count, transpose, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn UniformMatrix4x2fv(&self, location: types::GLint, count: types::GLsizei, transpose: types::GLboolean, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLsizei, types::GLboolean, *const types::GLfloat) -> ()>(self.UniformMatrix4x2fv.f)(location, count, transpose, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn UniformMatrix4x3fv(&self, location: types::GLint, count: types::GLsizei, transpose: types::GLboolean, value: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLsizei, types::GLboolean, *const types::GLfloat) -> ()>(self.UniformMatrix4x3fv.f)(location, count, transpose, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn UnmapBuffer(&self, target: types::GLenum) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> types::GLboolean>(self.UnmapBuffer.f)(target) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn UseProgram(&self, program: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.UseProgram.f)(program) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn UseProgramStages(&self, pipeline: types::GLuint, stages: types::GLbitfield, program: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLbitfield, types::GLuint) -> ()>(self.UseProgramStages.f)(pipeline, stages, program) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ValidateProgram(&self, program: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.ValidateProgram.f)(program) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ValidateProgramPipeline(&self, pipeline: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.ValidateProgramPipeline.f)(pipeline) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Vertex2d(&self, x: types::GLdouble, y: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble, types::GLdouble) -> ()>(self.Vertex2d.f)(x, y) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Vertex2dv(&self, v: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble) -> ()>(self.Vertex2dv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Vertex2f(&self, x: types::GLfloat, y: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat) -> ()>(self.Vertex2f.f)(x, y) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Vertex2fv(&self, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat) -> ()>(self.Vertex2fv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Vertex2i(&self, x: types::GLint, y: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint) -> ()>(self.Vertex2i.f)(x, y) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Vertex2iv(&self, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLint) -> ()>(self.Vertex2iv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Vertex2s(&self, x: types::GLshort, y: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLshort, types::GLshort) -> ()>(self.Vertex2s.f)(x, y) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Vertex2sv(&self, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLshort) -> ()>(self.Vertex2sv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Vertex3d(&self, x: types::GLdouble, y: types::GLdouble, z: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble, types::GLdouble, types::GLdouble) -> ()>(self.Vertex3d.f)(x, y, z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Vertex3dv(&self, v: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble) -> ()>(self.Vertex3dv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Vertex3f(&self, x: types::GLfloat, y: types::GLfloat, z: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.Vertex3f.f)(x, y, z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Vertex3fv(&self, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat) -> ()>(self.Vertex3fv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Vertex3i(&self, x: types::GLint, y: types::GLint, z: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint, types::GLint) -> ()>(self.Vertex3i.f)(x, y, z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Vertex3iv(&self, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLint) -> ()>(self.Vertex3iv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Vertex3s(&self, x: types::GLshort, y: types::GLshort, z: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLshort, types::GLshort, types::GLshort) -> ()>(self.Vertex3s.f)(x, y, z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Vertex3sv(&self, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLshort) -> ()>(self.Vertex3sv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Vertex4d(&self, x: types::GLdouble, y: types::GLdouble, z: types::GLdouble, w: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble, types::GLdouble, types::GLdouble, types::GLdouble) -> ()>(self.Vertex4d.f)(x, y, z, w) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Vertex4dv(&self, v: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble) -> ()>(self.Vertex4dv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Vertex4f(&self, x: types::GLfloat, y: types::GLfloat, z: types::GLfloat, w: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.Vertex4f.f)(x, y, z, w) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Vertex4fv(&self, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat) -> ()>(self.Vertex4fv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Vertex4i(&self, x: types::GLint, y: types::GLint, z: types::GLint, w: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint, types::GLint, types::GLint) -> ()>(self.Vertex4i.f)(x, y, z, w) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Vertex4iv(&self, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLint) -> ()>(self.Vertex4iv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Vertex4s(&self, x: types::GLshort, y: types::GLshort, z: types::GLshort, w: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLshort, types::GLshort, types::GLshort, types::GLshort) -> ()>(self.Vertex4s.f)(x, y, z, w) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Vertex4sv(&self, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLshort) -> ()>(self.Vertex4sv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib1d(&self, index: types::GLuint, x: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLdouble) -> ()>(self.VertexAttrib1d.f)(index, x) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib1dv(&self, index: types::GLuint, v: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLdouble) -> ()>(self.VertexAttrib1dv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib1f(&self, index: types::GLuint, x: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLfloat) -> ()>(self.VertexAttrib1f.f)(index, x) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib1fv(&self, index: types::GLuint, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLfloat) -> ()>(self.VertexAttrib1fv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib1s(&self, index: types::GLuint, x: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLshort) -> ()>(self.VertexAttrib1s.f)(index, x) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib1sv(&self, index: types::GLuint, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLshort) -> ()>(self.VertexAttrib1sv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib2d(&self, index: types::GLuint, x: types::GLdouble, y: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLdouble, types::GLdouble) -> ()>(self.VertexAttrib2d.f)(index, x, y) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib2dv(&self, index: types::GLuint, v: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLdouble) -> ()>(self.VertexAttrib2dv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib2f(&self, index: types::GLuint, x: types::GLfloat, y: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLfloat, types::GLfloat) -> ()>(self.VertexAttrib2f.f)(index, x, y) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib2fv(&self, index: types::GLuint, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLfloat) -> ()>(self.VertexAttrib2fv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib2s(&self, index: types::GLuint, x: types::GLshort, y: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLshort, types::GLshort) -> ()>(self.VertexAttrib2s.f)(index, x, y) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib2sv(&self, index: types::GLuint, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLshort) -> ()>(self.VertexAttrib2sv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib3d(&self, index: types::GLuint, x: types::GLdouble, y: types::GLdouble, z: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLdouble, types::GLdouble, types::GLdouble) -> ()>(self.VertexAttrib3d.f)(index, x, y, z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib3dv(&self, index: types::GLuint, v: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLdouble) -> ()>(self.VertexAttrib3dv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib3f(&self, index: types::GLuint, x: types::GLfloat, y: types::GLfloat, z: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.VertexAttrib3f.f)(index, x, y, z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib3fv(&self, index: types::GLuint, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLfloat) -> ()>(self.VertexAttrib3fv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib3s(&self, index: types::GLuint, x: types::GLshort, y: types::GLshort, z: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLshort, types::GLshort, types::GLshort) -> ()>(self.VertexAttrib3s.f)(index, x, y, z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib3sv(&self, index: types::GLuint, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLshort) -> ()>(self.VertexAttrib3sv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib4Nbv(&self, index: types::GLuint, v: *const types::GLbyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLbyte) -> ()>(self.VertexAttrib4Nbv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib4Niv(&self, index: types::GLuint, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLint) -> ()>(self.VertexAttrib4Niv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib4Nsv(&self, index: types::GLuint, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLshort) -> ()>(self.VertexAttrib4Nsv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib4Nub(&self, index: types::GLuint, x: types::GLubyte, y: types::GLubyte, z: types::GLubyte, w: types::GLubyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLubyte, types::GLubyte, types::GLubyte, types::GLubyte) -> ()>(self.VertexAttrib4Nub.f)(index, x, y, z, w) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib4Nubv(&self, index: types::GLuint, v: *const types::GLubyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLubyte) -> ()>(self.VertexAttrib4Nubv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib4Nuiv(&self, index: types::GLuint, v: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLuint) -> ()>(self.VertexAttrib4Nuiv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib4Nusv(&self, index: types::GLuint, v: *const types::GLushort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLushort) -> ()>(self.VertexAttrib4Nusv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib4bv(&self, index: types::GLuint, v: *const types::GLbyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLbyte) -> ()>(self.VertexAttrib4bv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib4d(&self, index: types::GLuint, x: types::GLdouble, y: types::GLdouble, z: types::GLdouble, w: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLdouble, types::GLdouble, types::GLdouble, types::GLdouble) -> ()>(self.VertexAttrib4d.f)(index, x, y, z, w) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib4dv(&self, index: types::GLuint, v: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLdouble) -> ()>(self.VertexAttrib4dv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib4f(&self, index: types::GLuint, x: types::GLfloat, y: types::GLfloat, z: types::GLfloat, w: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLfloat, types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.VertexAttrib4f.f)(index, x, y, z, w) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib4fv(&self, index: types::GLuint, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLfloat) -> ()>(self.VertexAttrib4fv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib4iv(&self, index: types::GLuint, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLint) -> ()>(self.VertexAttrib4iv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib4s(&self, index: types::GLuint, x: types::GLshort, y: types::GLshort, z: types::GLshort, w: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLshort, types::GLshort, types::GLshort, types::GLshort) -> ()>(self.VertexAttrib4s.f)(index, x, y, z, w) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib4sv(&self, index: types::GLuint, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLshort) -> ()>(self.VertexAttrib4sv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib4ubv(&self, index: types::GLuint, v: *const types::GLubyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLubyte) -> ()>(self.VertexAttrib4ubv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib4uiv(&self, index: types::GLuint, v: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLuint) -> ()>(self.VertexAttrib4uiv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib4usv(&self, index: types::GLuint, v: *const types::GLushort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLushort) -> ()>(self.VertexAttrib4usv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribBinding(&self, attribindex: types::GLuint, bindingindex: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint) -> ()>(self.VertexAttribBinding.f)(attribindex, bindingindex) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribDivisor(&self, index: types::GLuint, divisor: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint) -> ()>(self.VertexAttribDivisor.f)(index, divisor) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribFormat(&self, attribindex: types::GLuint, size: types::GLint, type_: types::GLenum, normalized: types::GLboolean, relativeoffset: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLenum, types::GLboolean, types::GLuint) -> ()>(self.VertexAttribFormat.f)(attribindex, size, type_, normalized, relativeoffset) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribI1i(&self, index: types::GLuint, x: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint) -> ()>(self.VertexAttribI1i.f)(index, x) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribI1iv(&self, index: types::GLuint, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLint) -> ()>(self.VertexAttribI1iv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribI1ui(&self, index: types::GLuint, x: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint) -> ()>(self.VertexAttribI1ui.f)(index, x) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribI1uiv(&self, index: types::GLuint, v: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLuint) -> ()>(self.VertexAttribI1uiv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribI2i(&self, index: types::GLuint, x: types::GLint, y: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLint) -> ()>(self.VertexAttribI2i.f)(index, x, y) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribI2iv(&self, index: types::GLuint, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLint) -> ()>(self.VertexAttribI2iv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribI2ui(&self, index: types::GLuint, x: types::GLuint, y: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint, types::GLuint) -> ()>(self.VertexAttribI2ui.f)(index, x, y) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribI2uiv(&self, index: types::GLuint, v: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLuint) -> ()>(self.VertexAttribI2uiv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribI3i(&self, index: types::GLuint, x: types::GLint, y: types::GLint, z: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLint, types::GLint) -> ()>(self.VertexAttribI3i.f)(index, x, y, z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribI3iv(&self, index: types::GLuint, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLint) -> ()>(self.VertexAttribI3iv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribI3ui(&self, index: types::GLuint, x: types::GLuint, y: types::GLuint, z: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint, types::GLuint, types::GLuint) -> ()>(self.VertexAttribI3ui.f)(index, x, y, z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribI3uiv(&self, index: types::GLuint, v: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLuint) -> ()>(self.VertexAttribI3uiv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribI4bv(&self, index: types::GLuint, v: *const types::GLbyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLbyte) -> ()>(self.VertexAttribI4bv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribI4i(&self, index: types::GLuint, x: types::GLint, y: types::GLint, z: types::GLint, w: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLint, types::GLint, types::GLint) -> ()>(self.VertexAttribI4i.f)(index, x, y, z, w) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribI4iv(&self, index: types::GLuint, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLint) -> ()>(self.VertexAttribI4iv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribI4sv(&self, index: types::GLuint, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLshort) -> ()>(self.VertexAttribI4sv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribI4ubv(&self, index: types::GLuint, v: *const types::GLubyte) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLubyte) -> ()>(self.VertexAttribI4ubv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribI4ui(&self, index: types::GLuint, x: types::GLuint, y: types::GLuint, z: types::GLuint, w: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint, types::GLuint, types::GLuint, types::GLuint) -> ()>(self.VertexAttribI4ui.f)(index, x, y, z, w) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribI4uiv(&self, index: types::GLuint, v: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLuint) -> ()>(self.VertexAttribI4uiv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribI4usv(&self, index: types::GLuint, v: *const types::GLushort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLushort) -> ()>(self.VertexAttribI4usv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribIFormat(&self, attribindex: types::GLuint, size: types::GLint, type_: types::GLenum, relativeoffset: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLenum, types::GLuint) -> ()>(self.VertexAttribIFormat.f)(attribindex, size, type_, relativeoffset) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribIPointer(&self, index: types::GLuint, size: types::GLint, type_: types::GLenum, stride: types::GLsizei, pointer: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLenum, types::GLsizei, *const __gl_imports::raw::c_void) -> ()>(self.VertexAttribIPointer.f)(index, size, type_, stride, pointer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribP1ui(&self, index: types::GLuint, type_: types::GLenum, normalized: types::GLboolean, value: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLboolean, types::GLuint) -> ()>(self.VertexAttribP1ui.f)(index, type_, normalized, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribP1uiv(&self, index: types::GLuint, type_: types::GLenum, normalized: types::GLboolean, value: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLboolean, *const types::GLuint) -> ()>(self.VertexAttribP1uiv.f)(index, type_, normalized, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribP2ui(&self, index: types::GLuint, type_: types::GLenum, normalized: types::GLboolean, value: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLboolean, types::GLuint) -> ()>(self.VertexAttribP2ui.f)(index, type_, normalized, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribP2uiv(&self, index: types::GLuint, type_: types::GLenum, normalized: types::GLboolean, value: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLboolean, *const types::GLuint) -> ()>(self.VertexAttribP2uiv.f)(index, type_, normalized, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribP3ui(&self, index: types::GLuint, type_: types::GLenum, normalized: types::GLboolean, value: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLboolean, types::GLuint) -> ()>(self.VertexAttribP3ui.f)(index, type_, normalized, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribP3uiv(&self, index: types::GLuint, type_: types::GLenum, normalized: types::GLboolean, value: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLboolean, *const types::GLuint) -> ()>(self.VertexAttribP3uiv.f)(index, type_, normalized, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribP4ui(&self, index: types::GLuint, type_: types::GLenum, normalized: types::GLboolean, value: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLboolean, types::GLuint) -> ()>(self.VertexAttribP4ui.f)(index, type_, normalized, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribP4uiv(&self, index: types::GLuint, type_: types::GLenum, normalized: types::GLboolean, value: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLboolean, *const types::GLuint) -> ()>(self.VertexAttribP4uiv.f)(index, type_, normalized, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribPointer(&self, index: types::GLuint, size: types::GLint, type_: types::GLenum, normalized: types::GLboolean, stride: types::GLsizei, pointer: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLenum, types::GLboolean, types::GLsizei, *const __gl_imports::raw::c_void) -> ()>(self.VertexAttribPointer.f)(index, size, type_, normalized, stride, pointer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexBindingDivisor(&self, bindingindex: types::GLuint, divisor: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint) -> ()>(self.VertexBindingDivisor.f)(bindingindex, divisor) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexP2ui(&self, type_: types::GLenum, value: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint) -> ()>(self.VertexP2ui.f)(type_, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexP2uiv(&self, type_: types::GLenum, value: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLuint) -> ()>(self.VertexP2uiv.f)(type_, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexP3ui(&self, type_: types::GLenum, value: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint) -> ()>(self.VertexP3ui.f)(type_, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexP3uiv(&self, type_: types::GLenum, value: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLuint) -> ()>(self.VertexP3uiv.f)(type_, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexP4ui(&self, type_: types::GLenum, value: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint) -> ()>(self.VertexP4ui.f)(type_, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexP4uiv(&self, type_: types::GLenum, value: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const types::GLuint) -> ()>(self.VertexP4uiv.f)(type_, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexPointer(&self, size: types::GLint, type_: types::GLenum, stride: types::GLsizei, pointer: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLenum, types::GLsizei, *const __gl_imports::raw::c_void) -> ()>(self.VertexPointer.f)(size, type_, stride, pointer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Viewport(&self, x: types::GLint, y: types::GLint, width: types::GLsizei, height: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint, types::GLsizei, types::GLsizei) -> ()>(self.Viewport.f)(x, y, width, height) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn WaitSync(&self, sync: types::GLsync, flags: types::GLbitfield, timeout: types::GLuint64) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsync, types::GLbitfield, types::GLuint64) -> ()>(self.WaitSync.f)(sync, flags, timeout) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn WindowPos2d(&self, x: types::GLdouble, y: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble, types::GLdouble) -> ()>(self.WindowPos2d.f)(x, y) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn WindowPos2dv(&self, v: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble) -> ()>(self.WindowPos2dv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn WindowPos2f(&self, x: types::GLfloat, y: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat) -> ()>(self.WindowPos2f.f)(x, y) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn WindowPos2fv(&self, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat) -> ()>(self.WindowPos2fv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn WindowPos2i(&self, x: types::GLint, y: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint) -> ()>(self.WindowPos2i.f)(x, y) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn WindowPos2iv(&self, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLint) -> ()>(self.WindowPos2iv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn WindowPos2s(&self, x: types::GLshort, y: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLshort, types::GLshort) -> ()>(self.WindowPos2s.f)(x, y) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn WindowPos2sv(&self, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLshort) -> ()>(self.WindowPos2sv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn WindowPos3d(&self, x: types::GLdouble, y: types::GLdouble, z: types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLdouble, types::GLdouble, types::GLdouble) -> ()>(self.WindowPos3d.f)(x, y, z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn WindowPos3dv(&self, v: *const types::GLdouble) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLdouble) -> ()>(self.WindowPos3dv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn WindowPos3f(&self, x: types::GLfloat, y: types::GLfloat, z: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.WindowPos3f.f)(x, y, z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn WindowPos3fv(&self, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLfloat) -> ()>(self.WindowPos3fv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn WindowPos3i(&self, x: types::GLint, y: types::GLint, z: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint, types::GLint) -> ()>(self.WindowPos3i.f)(x, y, z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn WindowPos3iv(&self, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLint) -> ()>(self.WindowPos3iv.f)(v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn WindowPos3s(&self, x: types::GLshort, y: types::GLshort, z: types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLshort, types::GLshort, types::GLshort) -> ()>(self.WindowPos3s.f)(x, y, z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn WindowPos3sv(&self, v: *const types::GLshort) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const types::GLshort) -> ()>(self.WindowPos3sv.f)(v) }
}

        unsafe impl __gl_imports::Send for Gl {}
