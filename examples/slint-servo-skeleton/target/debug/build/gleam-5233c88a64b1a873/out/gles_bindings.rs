
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
#[allow(dead_code, non_upper_case_globals)] pub const ALIASED_LINE_WIDTH_RANGE: types::GLenum = 0x846E;
#[allow(dead_code, non_upper_case_globals)] pub const ALIASED_POINT_SIZE_RANGE: types::GLenum = 0x846D;
#[allow(dead_code, non_upper_case_globals)] pub const ALL_BARRIER_BITS: types::GLenum = 0xFFFFFFFF;
#[allow(dead_code, non_upper_case_globals)] pub const ALL_SHADER_BITS: types::GLenum = 0xFFFFFFFF;
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA: types::GLenum = 0x1906;
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA16F_EXT: types::GLenum = 0x881C;
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA32F_EXT: types::GLenum = 0x8816;
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA8_EXT: types::GLenum = 0x803C;
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA_BITS: types::GLenum = 0x0D55;
#[allow(dead_code, non_upper_case_globals)] pub const ALREADY_SIGNALED: types::GLenum = 0x911A;
#[allow(dead_code, non_upper_case_globals)] pub const ALWAYS: types::GLenum = 0x0207;
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
#[allow(dead_code, non_upper_case_globals)] pub const BACK: types::GLenum = 0x0405;
#[allow(dead_code, non_upper_case_globals)] pub const BGRA8_EXT: types::GLenum = 0x93A1;
#[allow(dead_code, non_upper_case_globals)] pub const BGRA_EXT: types::GLenum = 0x80E1;
#[allow(dead_code, non_upper_case_globals)] pub const BLEND: types::GLenum = 0x0BE2;
#[allow(dead_code, non_upper_case_globals)] pub const BLEND_ADVANCED_COHERENT_KHR: types::GLenum = 0x9285;
#[allow(dead_code, non_upper_case_globals)] pub const BLEND_COLOR: types::GLenum = 0x8005;
#[allow(dead_code, non_upper_case_globals)] pub const BLEND_DST_ALPHA: types::GLenum = 0x80CA;
#[allow(dead_code, non_upper_case_globals)] pub const BLEND_DST_RGB: types::GLenum = 0x80C8;
#[allow(dead_code, non_upper_case_globals)] pub const BLEND_EQUATION: types::GLenum = 0x8009;
#[allow(dead_code, non_upper_case_globals)] pub const BLEND_EQUATION_ALPHA: types::GLenum = 0x883D;
#[allow(dead_code, non_upper_case_globals)] pub const BLEND_EQUATION_RGB: types::GLenum = 0x8009;
#[allow(dead_code, non_upper_case_globals)] pub const BLEND_SRC_ALPHA: types::GLenum = 0x80CB;
#[allow(dead_code, non_upper_case_globals)] pub const BLEND_SRC_RGB: types::GLenum = 0x80C9;
#[allow(dead_code, non_upper_case_globals)] pub const BLOCK_INDEX: types::GLenum = 0x92FD;
#[allow(dead_code, non_upper_case_globals)] pub const BLUE: types::GLenum = 0x1905;
#[allow(dead_code, non_upper_case_globals)] pub const BLUE_BITS: types::GLenum = 0x0D54;
#[allow(dead_code, non_upper_case_globals)] pub const BOOL: types::GLenum = 0x8B56;
#[allow(dead_code, non_upper_case_globals)] pub const BOOL_VEC2: types::GLenum = 0x8B57;
#[allow(dead_code, non_upper_case_globals)] pub const BOOL_VEC3: types::GLenum = 0x8B58;
#[allow(dead_code, non_upper_case_globals)] pub const BOOL_VEC4: types::GLenum = 0x8B59;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER: types::GLenum = 0x82E0;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_ACCESS_FLAGS: types::GLenum = 0x911F;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_BINDING: types::GLenum = 0x9302;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_DATA_SIZE: types::GLenum = 0x9303;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_IMMUTABLE_STORAGE_EXT: types::GLenum = 0x821F;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_KHR: types::GLenum = 0x82E0;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_MAPPED: types::GLenum = 0x88BC;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_MAP_LENGTH: types::GLenum = 0x9120;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_MAP_OFFSET: types::GLenum = 0x9121;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_MAP_POINTER: types::GLenum = 0x88BD;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_SIZE: types::GLenum = 0x8764;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_STORAGE_FLAGS_EXT: types::GLenum = 0x8220;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_UPDATE_BARRIER_BIT: types::GLenum = 0x00000200;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_USAGE: types::GLenum = 0x8765;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_VARIABLE: types::GLenum = 0x92E5;
#[allow(dead_code, non_upper_case_globals)] pub const BYTE: types::GLenum = 0x1400;
#[allow(dead_code, non_upper_case_globals)] pub const CCW: types::GLenum = 0x0901;
#[allow(dead_code, non_upper_case_globals)] pub const CLAMP_TO_EDGE: types::GLenum = 0x812F;
#[allow(dead_code, non_upper_case_globals)] pub const CLIENT_MAPPED_BUFFER_BARRIER_BIT_EXT: types::GLenum = 0x00004000;
#[allow(dead_code, non_upper_case_globals)] pub const CLIENT_STORAGE_BIT_EXT: types::GLenum = 0x0200;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR: types::GLenum = 0x1800;
#[allow(dead_code, non_upper_case_globals)] pub const COLORBURN_KHR: types::GLenum = 0x929A;
#[allow(dead_code, non_upper_case_globals)] pub const COLORDODGE_KHR: types::GLenum = 0x9299;
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
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_WRITEMASK: types::GLenum = 0x0C23;
#[allow(dead_code, non_upper_case_globals)] pub const COMMAND_BARRIER_BIT: types::GLenum = 0x00000040;
#[allow(dead_code, non_upper_case_globals)] pub const COMPARE_REF_TO_TEXTURE: types::GLenum = 0x884E;
#[allow(dead_code, non_upper_case_globals)] pub const COMPILE_STATUS: types::GLenum = 0x8B81;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_R11_EAC: types::GLenum = 0x9270;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_RG11_EAC: types::GLenum = 0x9272;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_RGB8_ETC2: types::GLenum = 0x9274;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_RGB8_PUNCHTHROUGH_ALPHA1_ETC2: types::GLenum = 0x9276;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_RGBA8_ETC2_EAC: types::GLenum = 0x9278;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_SIGNED_R11_EAC: types::GLenum = 0x9271;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_SIGNED_RG11_EAC: types::GLenum = 0x9273;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_SRGB8_ALPHA8_ETC2_EAC: types::GLenum = 0x9279;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_SRGB8_ETC2: types::GLenum = 0x9275;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_SRGB8_PUNCHTHROUGH_ALPHA1_ETC2: types::GLenum = 0x9277;
#[allow(dead_code, non_upper_case_globals)] pub const COMPRESSED_TEXTURE_FORMATS: types::GLenum = 0x86A3;
#[allow(dead_code, non_upper_case_globals)] pub const COMPUTE_SHADER: types::GLenum = 0x91B9;
#[allow(dead_code, non_upper_case_globals)] pub const COMPUTE_SHADER_BIT: types::GLenum = 0x00000020;
#[allow(dead_code, non_upper_case_globals)] pub const COMPUTE_WORK_GROUP_SIZE: types::GLenum = 0x8267;
#[allow(dead_code, non_upper_case_globals)] pub const CONDITION_SATISFIED: types::GLenum = 0x911C;
#[allow(dead_code, non_upper_case_globals)] pub const CONSTANT_ALPHA: types::GLenum = 0x8003;
#[allow(dead_code, non_upper_case_globals)] pub const CONSTANT_COLOR: types::GLenum = 0x8001;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_FLAG_DEBUG_BIT: types::GLenum = 0x00000002;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_FLAG_DEBUG_BIT_KHR: types::GLenum = 0x00000002;
#[allow(dead_code, non_upper_case_globals)] pub const COPY_READ_BUFFER: types::GLenum = 0x8F36;
#[allow(dead_code, non_upper_case_globals)] pub const COPY_READ_BUFFER_BINDING: types::GLenum = 0x8F36;
#[allow(dead_code, non_upper_case_globals)] pub const COPY_WRITE_BUFFER: types::GLenum = 0x8F37;
#[allow(dead_code, non_upper_case_globals)] pub const COPY_WRITE_BUFFER_BINDING: types::GLenum = 0x8F37;
#[allow(dead_code, non_upper_case_globals)] pub const CULL_FACE: types::GLenum = 0x0B44;
#[allow(dead_code, non_upper_case_globals)] pub const CULL_FACE_MODE: types::GLenum = 0x0B45;
#[allow(dead_code, non_upper_case_globals)] pub const CURRENT_PROGRAM: types::GLenum = 0x8B8D;
#[allow(dead_code, non_upper_case_globals)] pub const CURRENT_QUERY: types::GLenum = 0x8865;
#[allow(dead_code, non_upper_case_globals)] pub const CURRENT_QUERY_EXT: types::GLenum = 0x8865;
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
#[allow(dead_code, non_upper_case_globals)] pub const DECR: types::GLenum = 0x1E03;
#[allow(dead_code, non_upper_case_globals)] pub const DECR_WRAP: types::GLenum = 0x8508;
#[allow(dead_code, non_upper_case_globals)] pub const DELETE_STATUS: types::GLenum = 0x8B80;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH: types::GLenum = 0x1801;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH24_STENCIL8: types::GLenum = 0x88F0;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH32F_STENCIL8: types::GLenum = 0x8CAD;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_ATTACHMENT: types::GLenum = 0x8D00;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_BITS: types::GLenum = 0x0D56;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_BUFFER_BIT: types::GLenum = 0x00000100;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_CLEAR_VALUE: types::GLenum = 0x0B73;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_COMPONENT: types::GLenum = 0x1902;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_COMPONENT16: types::GLenum = 0x81A5;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_COMPONENT24: types::GLenum = 0x81A6;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_COMPONENT32F: types::GLenum = 0x8CAC;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_FUNC: types::GLenum = 0x0B74;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_RANGE: types::GLenum = 0x0B70;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_STENCIL: types::GLenum = 0x84F9;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_STENCIL_ATTACHMENT: types::GLenum = 0x821A;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_STENCIL_TEXTURE_MODE: types::GLenum = 0x90EA;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_TEST: types::GLenum = 0x0B71;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_WRITEMASK: types::GLenum = 0x0B72;
#[allow(dead_code, non_upper_case_globals)] pub const DIFFERENCE_KHR: types::GLenum = 0x929E;
#[allow(dead_code, non_upper_case_globals)] pub const DISPATCH_INDIRECT_BUFFER: types::GLenum = 0x90EE;
#[allow(dead_code, non_upper_case_globals)] pub const DISPATCH_INDIRECT_BUFFER_BINDING: types::GLenum = 0x90EF;
#[allow(dead_code, non_upper_case_globals)] pub const DISPLAY_LIST: types::GLenum = 0x82E7;
#[allow(dead_code, non_upper_case_globals)] pub const DITHER: types::GLenum = 0x0BD0;
#[allow(dead_code, non_upper_case_globals)] pub const DONT_CARE: types::GLenum = 0x1100;
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
#[allow(dead_code, non_upper_case_globals)] pub const DST_ALPHA: types::GLenum = 0x0304;
#[allow(dead_code, non_upper_case_globals)] pub const DST_COLOR: types::GLenum = 0x0306;
#[allow(dead_code, non_upper_case_globals)] pub const DYNAMIC_COPY: types::GLenum = 0x88EA;
#[allow(dead_code, non_upper_case_globals)] pub const DYNAMIC_DRAW: types::GLenum = 0x88E8;
#[allow(dead_code, non_upper_case_globals)] pub const DYNAMIC_READ: types::GLenum = 0x88E9;
#[allow(dead_code, non_upper_case_globals)] pub const DYNAMIC_STORAGE_BIT_EXT: types::GLenum = 0x0100;
#[allow(dead_code, non_upper_case_globals)] pub const ELEMENT_ARRAY_BARRIER_BIT: types::GLenum = 0x00000002;
#[allow(dead_code, non_upper_case_globals)] pub const ELEMENT_ARRAY_BUFFER: types::GLenum = 0x8893;
#[allow(dead_code, non_upper_case_globals)] pub const ELEMENT_ARRAY_BUFFER_BINDING: types::GLenum = 0x8895;
#[allow(dead_code, non_upper_case_globals)] pub const EQUAL: types::GLenum = 0x0202;
#[allow(dead_code, non_upper_case_globals)] pub const EXCLUSION_KHR: types::GLenum = 0x92A0;
#[allow(dead_code, non_upper_case_globals)] pub const EXTENSIONS: types::GLenum = 0x1F03;
#[allow(dead_code, non_upper_case_globals)] pub const FALSE: types::GLboolean = 0;
#[allow(dead_code, non_upper_case_globals)] pub const FASTEST: types::GLenum = 0x1101;
#[allow(dead_code, non_upper_case_globals)] pub const FIXED: types::GLenum = 0x140C;
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
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_INCOMPLETE_MISSING_ATTACHMENT: types::GLenum = 0x8CD7;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_INCOMPLETE_MULTISAMPLE: types::GLenum = 0x8D56;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_UNDEFINED: types::GLenum = 0x8219;
#[allow(dead_code, non_upper_case_globals)] pub const FRAMEBUFFER_UNSUPPORTED: types::GLenum = 0x8CDD;
#[allow(dead_code, non_upper_case_globals)] pub const FRONT: types::GLenum = 0x0404;
#[allow(dead_code, non_upper_case_globals)] pub const FRONT_AND_BACK: types::GLenum = 0x0408;
#[allow(dead_code, non_upper_case_globals)] pub const FRONT_FACE: types::GLenum = 0x0B46;
#[allow(dead_code, non_upper_case_globals)] pub const FUNC_ADD: types::GLenum = 0x8006;
#[allow(dead_code, non_upper_case_globals)] pub const FUNC_REVERSE_SUBTRACT: types::GLenum = 0x800B;
#[allow(dead_code, non_upper_case_globals)] pub const FUNC_SUBTRACT: types::GLenum = 0x800A;
#[allow(dead_code, non_upper_case_globals)] pub const GENERATE_MIPMAP_HINT: types::GLenum = 0x8192;
#[allow(dead_code, non_upper_case_globals)] pub const GEQUAL: types::GLenum = 0x0206;
#[allow(dead_code, non_upper_case_globals)] pub const GPU_DISJOINT_EXT: types::GLenum = 0x8FBB;
#[allow(dead_code, non_upper_case_globals)] pub const GREATER: types::GLenum = 0x0204;
#[allow(dead_code, non_upper_case_globals)] pub const GREEN: types::GLenum = 0x1904;
#[allow(dead_code, non_upper_case_globals)] pub const GREEN_BITS: types::GLenum = 0x0D53;
#[allow(dead_code, non_upper_case_globals)] pub const HALF_FLOAT: types::GLenum = 0x140B;
#[allow(dead_code, non_upper_case_globals)] pub const HALF_FLOAT_OES: types::GLenum = 0x8D61;
#[allow(dead_code, non_upper_case_globals)] pub const HARDLIGHT_KHR: types::GLenum = 0x929B;
#[allow(dead_code, non_upper_case_globals)] pub const HIGH_FLOAT: types::GLenum = 0x8DF2;
#[allow(dead_code, non_upper_case_globals)] pub const HIGH_INT: types::GLenum = 0x8DF5;
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
#[allow(dead_code, non_upper_case_globals)] pub const INFO_LOG_LENGTH: types::GLenum = 0x8B84;
#[allow(dead_code, non_upper_case_globals)] pub const INT: types::GLenum = 0x1404;
#[allow(dead_code, non_upper_case_globals)] pub const INTERLEAVED_ATTRIBS: types::GLenum = 0x8C8C;
#[allow(dead_code, non_upper_case_globals)] pub const INT_2_10_10_10_REV: types::GLenum = 0x8D9F;
#[allow(dead_code, non_upper_case_globals)] pub const INT_IMAGE_2D: types::GLenum = 0x9058;
#[allow(dead_code, non_upper_case_globals)] pub const INT_IMAGE_2D_ARRAY: types::GLenum = 0x905E;
#[allow(dead_code, non_upper_case_globals)] pub const INT_IMAGE_3D: types::GLenum = 0x9059;
#[allow(dead_code, non_upper_case_globals)] pub const INT_IMAGE_CUBE: types::GLenum = 0x905B;
#[allow(dead_code, non_upper_case_globals)] pub const INT_SAMPLER_2D: types::GLenum = 0x8DCA;
#[allow(dead_code, non_upper_case_globals)] pub const INT_SAMPLER_2D_ARRAY: types::GLenum = 0x8DCF;
#[allow(dead_code, non_upper_case_globals)] pub const INT_SAMPLER_2D_MULTISAMPLE: types::GLenum = 0x9109;
#[allow(dead_code, non_upper_case_globals)] pub const INT_SAMPLER_3D: types::GLenum = 0x8DCB;
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
#[allow(dead_code, non_upper_case_globals)] pub const LEQUAL: types::GLenum = 0x0203;
#[allow(dead_code, non_upper_case_globals)] pub const LESS: types::GLenum = 0x0201;
#[allow(dead_code, non_upper_case_globals)] pub const LIGHTEN_KHR: types::GLenum = 0x9298;
#[allow(dead_code, non_upper_case_globals)] pub const LINEAR: types::GLenum = 0x2601;
#[allow(dead_code, non_upper_case_globals)] pub const LINEAR_MIPMAP_LINEAR: types::GLenum = 0x2703;
#[allow(dead_code, non_upper_case_globals)] pub const LINEAR_MIPMAP_NEAREST: types::GLenum = 0x2701;
#[allow(dead_code, non_upper_case_globals)] pub const LINES: types::GLenum = 0x0001;
#[allow(dead_code, non_upper_case_globals)] pub const LINE_LOOP: types::GLenum = 0x0002;
#[allow(dead_code, non_upper_case_globals)] pub const LINE_STRIP: types::GLenum = 0x0003;
#[allow(dead_code, non_upper_case_globals)] pub const LINE_WIDTH: types::GLenum = 0x0B21;
#[allow(dead_code, non_upper_case_globals)] pub const LINK_STATUS: types::GLenum = 0x8B82;
#[allow(dead_code, non_upper_case_globals)] pub const LOCATION: types::GLenum = 0x930E;
#[allow(dead_code, non_upper_case_globals)] pub const LOW_FLOAT: types::GLenum = 0x8DF0;
#[allow(dead_code, non_upper_case_globals)] pub const LOW_INT: types::GLenum = 0x8DF3;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE: types::GLenum = 0x1909;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE16F_EXT: types::GLenum = 0x881E;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE32F_EXT: types::GLenum = 0x8818;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE8_ALPHA8_EXT: types::GLenum = 0x8045;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE8_EXT: types::GLenum = 0x8040;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE_ALPHA: types::GLenum = 0x190A;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE_ALPHA16F_EXT: types::GLenum = 0x881F;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE_ALPHA32F_EXT: types::GLenum = 0x8819;
#[allow(dead_code, non_upper_case_globals)] pub const MAJOR_VERSION: types::GLenum = 0x821B;
#[allow(dead_code, non_upper_case_globals)] pub const MAP_COHERENT_BIT_EXT: types::GLenum = 0x0080;
#[allow(dead_code, non_upper_case_globals)] pub const MAP_FLUSH_EXPLICIT_BIT: types::GLenum = 0x0010;
#[allow(dead_code, non_upper_case_globals)] pub const MAP_INVALIDATE_BUFFER_BIT: types::GLenum = 0x0008;
#[allow(dead_code, non_upper_case_globals)] pub const MAP_INVALIDATE_RANGE_BIT: types::GLenum = 0x0004;
#[allow(dead_code, non_upper_case_globals)] pub const MAP_PERSISTENT_BIT_EXT: types::GLenum = 0x0040;
#[allow(dead_code, non_upper_case_globals)] pub const MAP_READ_BIT: types::GLenum = 0x0001;
#[allow(dead_code, non_upper_case_globals)] pub const MAP_UNSYNCHRONIZED_BIT: types::GLenum = 0x0020;
#[allow(dead_code, non_upper_case_globals)] pub const MAP_WRITE_BIT: types::GLenum = 0x0002;
#[allow(dead_code, non_upper_case_globals)] pub const MATRIX_STRIDE: types::GLenum = 0x92FF;
#[allow(dead_code, non_upper_case_globals)] pub const MAX: types::GLenum = 0x8008;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_3D_TEXTURE_SIZE: types::GLenum = 0x8073;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_ARRAY_TEXTURE_LAYERS: types::GLenum = 0x88FF;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_ATOMIC_COUNTER_BUFFER_BINDINGS: types::GLenum = 0x92DC;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_ATOMIC_COUNTER_BUFFER_SIZE: types::GLenum = 0x92D8;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COLOR_ATTACHMENTS: types::GLenum = 0x8CDF;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COLOR_TEXTURE_SAMPLES: types::GLenum = 0x910E;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMBINED_ATOMIC_COUNTERS: types::GLenum = 0x92D7;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMBINED_ATOMIC_COUNTER_BUFFERS: types::GLenum = 0x92D1;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMBINED_COMPUTE_UNIFORM_COMPONENTS: types::GLenum = 0x8266;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMBINED_FRAGMENT_UNIFORM_COMPONENTS: types::GLenum = 0x8A33;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_COMBINED_IMAGE_UNIFORMS: types::GLenum = 0x90CF;
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
#[allow(dead_code, non_upper_case_globals)] pub const MAX_ELEMENTS_INDICES: types::GLenum = 0x80E9;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_ELEMENTS_VERTICES: types::GLenum = 0x80E8;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_ELEMENT_INDEX: types::GLenum = 0x8D6B;
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
#[allow(dead_code, non_upper_case_globals)] pub const MAX_IMAGE_UNITS: types::GLenum = 0x8F38;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_INTEGER_SAMPLES: types::GLenum = 0x9110;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_LABEL_LENGTH: types::GLenum = 0x82E8;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_LABEL_LENGTH_KHR: types::GLenum = 0x82E8;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_NAME_LENGTH: types::GLenum = 0x92F6;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_NUM_ACTIVE_VARIABLES: types::GLenum = 0x92F7;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_PROGRAM_TEXEL_OFFSET: types::GLenum = 0x8905;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_PROGRAM_TEXTURE_GATHER_OFFSET: types::GLenum = 0x8E5F;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_RENDERBUFFER_SIZE: types::GLenum = 0x84E8;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_SAMPLES: types::GLenum = 0x8D57;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_SAMPLE_MASK_WORDS: types::GLenum = 0x8E59;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_SERVER_WAIT_TIMEOUT: types::GLenum = 0x9111;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_SHADER_PIXEL_LOCAL_STORAGE_FAST_SIZE_EXT: types::GLenum = 0x8F63;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_SHADER_PIXEL_LOCAL_STORAGE_SIZE_EXT: types::GLenum = 0x8F67;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_SHADER_STORAGE_BLOCK_SIZE: types::GLenum = 0x90DE;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_SHADER_STORAGE_BUFFER_BINDINGS: types::GLenum = 0x90DD;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_TEXTURE_IMAGE_UNITS: types::GLenum = 0x8872;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_TEXTURE_LOD_BIAS: types::GLenum = 0x84FD;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_TEXTURE_MAX_ANISOTROPY_EXT: types::GLenum = 0x84FF;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_TEXTURE_SIZE: types::GLenum = 0x0D33;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_TRANSFORM_FEEDBACK_INTERLEAVED_COMPONENTS: types::GLenum = 0x8C8A;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_TRANSFORM_FEEDBACK_SEPARATE_ATTRIBS: types::GLenum = 0x8C8B;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_TRANSFORM_FEEDBACK_SEPARATE_COMPONENTS: types::GLenum = 0x8C80;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_UNIFORM_BLOCK_SIZE: types::GLenum = 0x8A30;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_UNIFORM_BUFFER_BINDINGS: types::GLenum = 0x8A2F;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_UNIFORM_LOCATIONS: types::GLenum = 0x826E;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_VARYING_COMPONENTS: types::GLenum = 0x8B4B;
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
#[allow(dead_code, non_upper_case_globals)] pub const MULTIPLY_KHR: types::GLenum = 0x9294;
#[allow(dead_code, non_upper_case_globals)] pub const NAME_LENGTH: types::GLenum = 0x92F9;
#[allow(dead_code, non_upper_case_globals)] pub const NEAREST: types::GLenum = 0x2600;
#[allow(dead_code, non_upper_case_globals)] pub const NEAREST_MIPMAP_LINEAR: types::GLenum = 0x2702;
#[allow(dead_code, non_upper_case_globals)] pub const NEAREST_MIPMAP_NEAREST: types::GLenum = 0x2700;
#[allow(dead_code, non_upper_case_globals)] pub const NEVER: types::GLenum = 0x0200;
#[allow(dead_code, non_upper_case_globals)] pub const NICEST: types::GLenum = 0x1102;
#[allow(dead_code, non_upper_case_globals)] pub const NONE: types::GLenum = 0;
#[allow(dead_code, non_upper_case_globals)] pub const NOTEQUAL: types::GLenum = 0x0205;
#[allow(dead_code, non_upper_case_globals)] pub const NO_ERROR: types::GLenum = 0;
#[allow(dead_code, non_upper_case_globals)] pub const NUM_ACTIVE_VARIABLES: types::GLenum = 0x9304;
#[allow(dead_code, non_upper_case_globals)] pub const NUM_COMPRESSED_TEXTURE_FORMATS: types::GLenum = 0x86A2;
#[allow(dead_code, non_upper_case_globals)] pub const NUM_EXTENSIONS: types::GLenum = 0x821D;
#[allow(dead_code, non_upper_case_globals)] pub const NUM_PROGRAM_BINARY_FORMATS: types::GLenum = 0x87FE;
#[allow(dead_code, non_upper_case_globals)] pub const NUM_SAMPLE_COUNTS: types::GLenum = 0x9380;
#[allow(dead_code, non_upper_case_globals)] pub const NUM_SHADER_BINARY_FORMATS: types::GLenum = 0x8DF9;
#[allow(dead_code, non_upper_case_globals)] pub const OBJECT_TYPE: types::GLenum = 0x9112;
#[allow(dead_code, non_upper_case_globals)] pub const OFFSET: types::GLenum = 0x92FC;
#[allow(dead_code, non_upper_case_globals)] pub const ONE: types::GLenum = 1;
#[allow(dead_code, non_upper_case_globals)] pub const ONE_MINUS_CONSTANT_ALPHA: types::GLenum = 0x8004;
#[allow(dead_code, non_upper_case_globals)] pub const ONE_MINUS_CONSTANT_COLOR: types::GLenum = 0x8002;
#[allow(dead_code, non_upper_case_globals)] pub const ONE_MINUS_DST_ALPHA: types::GLenum = 0x0305;
#[allow(dead_code, non_upper_case_globals)] pub const ONE_MINUS_DST_COLOR: types::GLenum = 0x0307;
#[allow(dead_code, non_upper_case_globals)] pub const ONE_MINUS_SRC_ALPHA: types::GLenum = 0x0303;
#[allow(dead_code, non_upper_case_globals)] pub const ONE_MINUS_SRC_COLOR: types::GLenum = 0x0301;
#[allow(dead_code, non_upper_case_globals)] pub const OUT_OF_MEMORY: types::GLenum = 0x0505;
#[allow(dead_code, non_upper_case_globals)] pub const OVERLAY_KHR: types::GLenum = 0x9296;
#[allow(dead_code, non_upper_case_globals)] pub const PACK_ALIGNMENT: types::GLenum = 0x0D05;
#[allow(dead_code, non_upper_case_globals)] pub const PACK_ROW_LENGTH: types::GLenum = 0x0D02;
#[allow(dead_code, non_upper_case_globals)] pub const PACK_SKIP_PIXELS: types::GLenum = 0x0D04;
#[allow(dead_code, non_upper_case_globals)] pub const PACK_SKIP_ROWS: types::GLenum = 0x0D03;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_BUFFER_BARRIER_BIT: types::GLenum = 0x00000080;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_PACK_BUFFER: types::GLenum = 0x88EB;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_PACK_BUFFER_BINDING: types::GLenum = 0x88ED;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_UNPACK_BUFFER: types::GLenum = 0x88EC;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_UNPACK_BUFFER_BINDING: types::GLenum = 0x88EF;
#[allow(dead_code, non_upper_case_globals)] pub const POINTS: types::GLenum = 0x0000;
#[allow(dead_code, non_upper_case_globals)] pub const POLYGON_OFFSET_FACTOR: types::GLenum = 0x8038;
#[allow(dead_code, non_upper_case_globals)] pub const POLYGON_OFFSET_FILL: types::GLenum = 0x8037;
#[allow(dead_code, non_upper_case_globals)] pub const POLYGON_OFFSET_UNITS: types::GLenum = 0x2A00;
#[allow(dead_code, non_upper_case_globals)] pub const PRIMITIVE_RESTART_FIXED_INDEX: types::GLenum = 0x8D69;
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
#[allow(dead_code, non_upper_case_globals)] pub const PROGRAM_SEPARABLE: types::GLenum = 0x8258;
#[allow(dead_code, non_upper_case_globals)] pub const QUERY: types::GLenum = 0x82E3;
#[allow(dead_code, non_upper_case_globals)] pub const QUERY_COUNTER_BITS_EXT: types::GLenum = 0x8864;
#[allow(dead_code, non_upper_case_globals)] pub const QUERY_KHR: types::GLenum = 0x82E3;
#[allow(dead_code, non_upper_case_globals)] pub const QUERY_RESULT: types::GLenum = 0x8866;
#[allow(dead_code, non_upper_case_globals)] pub const QUERY_RESULT_AVAILABLE: types::GLenum = 0x8867;
#[allow(dead_code, non_upper_case_globals)] pub const QUERY_RESULT_AVAILABLE_EXT: types::GLenum = 0x8867;
#[allow(dead_code, non_upper_case_globals)] pub const QUERY_RESULT_EXT: types::GLenum = 0x8866;
#[allow(dead_code, non_upper_case_globals)] pub const R11F_G11F_B10F: types::GLenum = 0x8C3A;
#[allow(dead_code, non_upper_case_globals)] pub const R16F: types::GLenum = 0x822D;
#[allow(dead_code, non_upper_case_globals)] pub const R16F_EXT: types::GLenum = 0x822D;
#[allow(dead_code, non_upper_case_globals)] pub const R16I: types::GLenum = 0x8233;
#[allow(dead_code, non_upper_case_globals)] pub const R16UI: types::GLenum = 0x8234;
#[allow(dead_code, non_upper_case_globals)] pub const R32F: types::GLenum = 0x822E;
#[allow(dead_code, non_upper_case_globals)] pub const R32F_EXT: types::GLenum = 0x822E;
#[allow(dead_code, non_upper_case_globals)] pub const R32I: types::GLenum = 0x8235;
#[allow(dead_code, non_upper_case_globals)] pub const R32UI: types::GLenum = 0x8236;
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
#[allow(dead_code, non_upper_case_globals)] pub const RED_BITS: types::GLenum = 0x0D52;
#[allow(dead_code, non_upper_case_globals)] pub const RED_INTEGER: types::GLenum = 0x8D94;
#[allow(dead_code, non_upper_case_globals)] pub const REFERENCED_BY_COMPUTE_SHADER: types::GLenum = 0x930B;
#[allow(dead_code, non_upper_case_globals)] pub const REFERENCED_BY_FRAGMENT_SHADER: types::GLenum = 0x930A;
#[allow(dead_code, non_upper_case_globals)] pub const REFERENCED_BY_VERTEX_SHADER: types::GLenum = 0x9306;
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
#[allow(dead_code, non_upper_case_globals)] pub const REPEAT: types::GLenum = 0x2901;
#[allow(dead_code, non_upper_case_globals)] pub const REPLACE: types::GLenum = 0x1E01;
#[allow(dead_code, non_upper_case_globals)] pub const REQUIRED_TEXTURE_IMAGE_UNITS_OES: types::GLenum = 0x8D68;
#[allow(dead_code, non_upper_case_globals)] pub const RG: types::GLenum = 0x8227;
#[allow(dead_code, non_upper_case_globals)] pub const RG16F: types::GLenum = 0x822F;
#[allow(dead_code, non_upper_case_globals)] pub const RG16F_EXT: types::GLenum = 0x822F;
#[allow(dead_code, non_upper_case_globals)] pub const RG16I: types::GLenum = 0x8239;
#[allow(dead_code, non_upper_case_globals)] pub const RG16UI: types::GLenum = 0x823A;
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
#[allow(dead_code, non_upper_case_globals)] pub const RGB10_A2: types::GLenum = 0x8059;
#[allow(dead_code, non_upper_case_globals)] pub const RGB10_A2UI: types::GLenum = 0x906F;
#[allow(dead_code, non_upper_case_globals)] pub const RGB10_A2_EXT: types::GLenum = 0x8059;
#[allow(dead_code, non_upper_case_globals)] pub const RGB10_EXT: types::GLenum = 0x8052;
#[allow(dead_code, non_upper_case_globals)] pub const RGB16F: types::GLenum = 0x881B;
#[allow(dead_code, non_upper_case_globals)] pub const RGB16F_EXT: types::GLenum = 0x881B;
#[allow(dead_code, non_upper_case_globals)] pub const RGB16I: types::GLenum = 0x8D89;
#[allow(dead_code, non_upper_case_globals)] pub const RGB16UI: types::GLenum = 0x8D77;
#[allow(dead_code, non_upper_case_globals)] pub const RGB32F: types::GLenum = 0x8815;
#[allow(dead_code, non_upper_case_globals)] pub const RGB32F_EXT: types::GLenum = 0x8815;
#[allow(dead_code, non_upper_case_globals)] pub const RGB32I: types::GLenum = 0x8D83;
#[allow(dead_code, non_upper_case_globals)] pub const RGB32UI: types::GLenum = 0x8D71;
#[allow(dead_code, non_upper_case_globals)] pub const RGB565: types::GLenum = 0x8D62;
#[allow(dead_code, non_upper_case_globals)] pub const RGB5_A1: types::GLenum = 0x8057;
#[allow(dead_code, non_upper_case_globals)] pub const RGB8: types::GLenum = 0x8051;
#[allow(dead_code, non_upper_case_globals)] pub const RGB8I: types::GLenum = 0x8D8F;
#[allow(dead_code, non_upper_case_globals)] pub const RGB8UI: types::GLenum = 0x8D7D;
#[allow(dead_code, non_upper_case_globals)] pub const RGB8_SNORM: types::GLenum = 0x8F96;
#[allow(dead_code, non_upper_case_globals)] pub const RGB9_E5: types::GLenum = 0x8C3D;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA: types::GLenum = 0x1908;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA16F: types::GLenum = 0x881A;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA16F_EXT: types::GLenum = 0x881A;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA16I: types::GLenum = 0x8D88;
#[allow(dead_code, non_upper_case_globals)] pub const RGBA16UI: types::GLenum = 0x8D76;
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
#[allow(dead_code, non_upper_case_globals)] pub const RGB_INTEGER: types::GLenum = 0x8D98;
#[allow(dead_code, non_upper_case_globals)] pub const RG_INTEGER: types::GLenum = 0x8228;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER: types::GLenum = 0x82E6;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_2D: types::GLenum = 0x8B5E;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_2D_ARRAY: types::GLenum = 0x8DC1;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_2D_ARRAY_SHADOW: types::GLenum = 0x8DC4;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_2D_MULTISAMPLE: types::GLenum = 0x9108;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_2D_SHADOW: types::GLenum = 0x8B62;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_3D: types::GLenum = 0x8B5F;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_BINDING: types::GLenum = 0x8919;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_CUBE: types::GLenum = 0x8B60;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_CUBE_SHADOW: types::GLenum = 0x8DC5;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_EXTERNAL_OES: types::GLenum = 0x8D66;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLER_KHR: types::GLenum = 0x82E6;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLES: types::GLenum = 0x80A9;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLE_ALPHA_TO_COVERAGE: types::GLenum = 0x809E;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLE_BUFFERS: types::GLenum = 0x80A8;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLE_COVERAGE: types::GLenum = 0x80A0;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLE_COVERAGE_INVERT: types::GLenum = 0x80AB;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLE_COVERAGE_VALUE: types::GLenum = 0x80AA;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLE_MASK: types::GLenum = 0x8E51;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLE_MASK_VALUE: types::GLenum = 0x8E52;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLE_POSITION: types::GLenum = 0x8E50;
#[allow(dead_code, non_upper_case_globals)] pub const SCISSOR_BOX: types::GLenum = 0x0C10;
#[allow(dead_code, non_upper_case_globals)] pub const SCISSOR_TEST: types::GLenum = 0x0C11;
#[allow(dead_code, non_upper_case_globals)] pub const SCREEN_KHR: types::GLenum = 0x9295;
#[allow(dead_code, non_upper_case_globals)] pub const SEPARATE_ATTRIBS: types::GLenum = 0x8C8D;
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
#[allow(dead_code, non_upper_case_globals)] pub const SHADING_LANGUAGE_VERSION: types::GLenum = 0x8B8C;
#[allow(dead_code, non_upper_case_globals)] pub const SHORT: types::GLenum = 0x1402;
#[allow(dead_code, non_upper_case_globals)] pub const SIGNALED: types::GLenum = 0x9119;
#[allow(dead_code, non_upper_case_globals)] pub const SIGNED_NORMALIZED: types::GLenum = 0x8F9C;
#[allow(dead_code, non_upper_case_globals)] pub const SOFTLIGHT_KHR: types::GLenum = 0x929C;
#[allow(dead_code, non_upper_case_globals)] pub const SRC_ALPHA: types::GLenum = 0x0302;
#[allow(dead_code, non_upper_case_globals)] pub const SRC_ALPHA_SATURATE: types::GLenum = 0x0308;
#[allow(dead_code, non_upper_case_globals)] pub const SRC_COLOR: types::GLenum = 0x0300;
#[allow(dead_code, non_upper_case_globals)] pub const SRGB: types::GLenum = 0x8C40;
#[allow(dead_code, non_upper_case_globals)] pub const SRGB8: types::GLenum = 0x8C41;
#[allow(dead_code, non_upper_case_globals)] pub const SRGB8_ALPHA8: types::GLenum = 0x8C43;
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
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_INDEX8: types::GLenum = 0x8D48;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_PASS_DEPTH_FAIL: types::GLenum = 0x0B95;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_PASS_DEPTH_PASS: types::GLenum = 0x0B96;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_REF: types::GLenum = 0x0B97;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_TEST: types::GLenum = 0x0B90;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_VALUE_MASK: types::GLenum = 0x0B93;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_WRITEMASK: types::GLenum = 0x0B98;
#[allow(dead_code, non_upper_case_globals)] pub const STREAM_COPY: types::GLenum = 0x88E2;
#[allow(dead_code, non_upper_case_globals)] pub const STREAM_DRAW: types::GLenum = 0x88E0;
#[allow(dead_code, non_upper_case_globals)] pub const STREAM_READ: types::GLenum = 0x88E1;
#[allow(dead_code, non_upper_case_globals)] pub const SUBPIXEL_BITS: types::GLenum = 0x0D50;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_CONDITION: types::GLenum = 0x9113;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_FENCE: types::GLenum = 0x9116;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_FLAGS: types::GLenum = 0x9115;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_FLUSH_COMMANDS_BIT: types::GLenum = 0x00000001;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_GPU_COMMANDS_COMPLETE: types::GLenum = 0x9117;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_STATUS: types::GLenum = 0x9114;
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
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_2D: types::GLenum = 0x0DE1;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_2D_ARRAY: types::GLenum = 0x8C1A;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_2D_MULTISAMPLE: types::GLenum = 0x9100;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_3D: types::GLenum = 0x806F;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_ALPHA_SIZE: types::GLenum = 0x805F;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_ALPHA_TYPE: types::GLenum = 0x8C13;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BASE_LEVEL: types::GLenum = 0x813C;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BINDING_2D: types::GLenum = 0x8069;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BINDING_2D_ARRAY: types::GLenum = 0x8C1D;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BINDING_2D_MULTISAMPLE: types::GLenum = 0x9104;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BINDING_3D: types::GLenum = 0x806A;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BINDING_CUBE_MAP: types::GLenum = 0x8514;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BINDING_EXTERNAL_OES: types::GLenum = 0x8D67;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BLUE_SIZE: types::GLenum = 0x805E;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_BLUE_TYPE: types::GLenum = 0x8C12;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_COMPARE_FUNC: types::GLenum = 0x884D;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_COMPARE_MODE: types::GLenum = 0x884C;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_COMPRESSED: types::GLenum = 0x86A1;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_CUBE_MAP: types::GLenum = 0x8513;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_CUBE_MAP_NEGATIVE_X: types::GLenum = 0x8516;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_CUBE_MAP_NEGATIVE_Y: types::GLenum = 0x8518;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_CUBE_MAP_NEGATIVE_Z: types::GLenum = 0x851A;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_CUBE_MAP_POSITIVE_X: types::GLenum = 0x8515;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_CUBE_MAP_POSITIVE_Y: types::GLenum = 0x8517;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_CUBE_MAP_POSITIVE_Z: types::GLenum = 0x8519;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_DEPTH: types::GLenum = 0x8071;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_DEPTH_SIZE: types::GLenum = 0x884A;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_DEPTH_TYPE: types::GLenum = 0x8C16;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_EXTERNAL_OES: types::GLenum = 0x8D65;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_FETCH_BARRIER_BIT: types::GLenum = 0x00000008;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_FIXED_SAMPLE_LOCATIONS: types::GLenum = 0x9107;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_GREEN_SIZE: types::GLenum = 0x805D;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_GREEN_TYPE: types::GLenum = 0x8C11;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_HEIGHT: types::GLenum = 0x1001;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_IMMUTABLE_FORMAT: types::GLenum = 0x912F;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_IMMUTABLE_FORMAT_EXT: types::GLenum = 0x912F;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_IMMUTABLE_LEVELS: types::GLenum = 0x82DF;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_INTERNAL_FORMAT: types::GLenum = 0x1003;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_MAG_FILTER: types::GLenum = 0x2800;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_MAX_ANISOTROPY_EXT: types::GLenum = 0x84FE;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_MAX_LEVEL: types::GLenum = 0x813D;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_MAX_LOD: types::GLenum = 0x813B;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_MIN_FILTER: types::GLenum = 0x2801;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_MIN_LOD: types::GLenum = 0x813A;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_RED_SIZE: types::GLenum = 0x805C;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_RED_TYPE: types::GLenum = 0x8C10;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_SAMPLES: types::GLenum = 0x9106;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_SHARED_SIZE: types::GLenum = 0x8C3F;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_STENCIL_SIZE: types::GLenum = 0x88F1;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_SWIZZLE_A: types::GLenum = 0x8E45;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_SWIZZLE_B: types::GLenum = 0x8E44;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_SWIZZLE_G: types::GLenum = 0x8E43;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_SWIZZLE_R: types::GLenum = 0x8E42;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_UPDATE_BARRIER_BIT: types::GLenum = 0x00000100;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_USAGE_ANGLE: types::GLenum = 0x93A2;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_WIDTH: types::GLenum = 0x1000;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_WRAP_R: types::GLenum = 0x8072;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_WRAP_S: types::GLenum = 0x2802;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_WRAP_T: types::GLenum = 0x2803;
#[allow(dead_code, non_upper_case_globals)] pub const TIMEOUT_EXPIRED: types::GLenum = 0x911B;
#[allow(dead_code, non_upper_case_globals)] pub const TIMEOUT_IGNORED: types::GLuint64 = 0xFFFFFFFFFFFFFFFF;
#[allow(dead_code, non_upper_case_globals)] pub const TIMESTAMP_EXT: types::GLenum = 0x8E28;
#[allow(dead_code, non_upper_case_globals)] pub const TIME_ELAPSED_EXT: types::GLenum = 0x88BF;
#[allow(dead_code, non_upper_case_globals)] pub const TOP_LEVEL_ARRAY_SIZE: types::GLenum = 0x930C;
#[allow(dead_code, non_upper_case_globals)] pub const TOP_LEVEL_ARRAY_STRIDE: types::GLenum = 0x930D;
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
#[allow(dead_code, non_upper_case_globals)] pub const TRIANGLES: types::GLenum = 0x0004;
#[allow(dead_code, non_upper_case_globals)] pub const TRIANGLE_FAN: types::GLenum = 0x0006;
#[allow(dead_code, non_upper_case_globals)] pub const TRIANGLE_STRIP: types::GLenum = 0x0005;
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
#[allow(dead_code, non_upper_case_globals)] pub const UNPACK_IMAGE_HEIGHT: types::GLenum = 0x806E;
#[allow(dead_code, non_upper_case_globals)] pub const UNPACK_ROW_LENGTH: types::GLenum = 0x0CF2;
#[allow(dead_code, non_upper_case_globals)] pub const UNPACK_SKIP_IMAGES: types::GLenum = 0x806D;
#[allow(dead_code, non_upper_case_globals)] pub const UNPACK_SKIP_PIXELS: types::GLenum = 0x0CF4;
#[allow(dead_code, non_upper_case_globals)] pub const UNPACK_SKIP_ROWS: types::GLenum = 0x0CF3;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNALED: types::GLenum = 0x9118;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_BYTE: types::GLenum = 0x1401;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT: types::GLenum = 0x1405;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_10F_11F_11F_REV: types::GLenum = 0x8C3B;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_24_8: types::GLenum = 0x84FA;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_2_10_10_10_REV: types::GLenum = 0x8368;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_5_9_9_9_REV: types::GLenum = 0x8C3E;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_ATOMIC_COUNTER: types::GLenum = 0x92DB;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_IMAGE_2D: types::GLenum = 0x9063;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_IMAGE_2D_ARRAY: types::GLenum = 0x9069;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_IMAGE_3D: types::GLenum = 0x9064;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_IMAGE_CUBE: types::GLenum = 0x9066;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_SAMPLER_2D: types::GLenum = 0x8DD2;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_SAMPLER_2D_ARRAY: types::GLenum = 0x8DD7;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_SAMPLER_2D_MULTISAMPLE: types::GLenum = 0x910A;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_SAMPLER_3D: types::GLenum = 0x8DD3;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_SAMPLER_CUBE: types::GLenum = 0x8DD4;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_VEC2: types::GLenum = 0x8DC6;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_VEC3: types::GLenum = 0x8DC7;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_INT_VEC4: types::GLenum = 0x8DC8;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_NORMALIZED: types::GLenum = 0x8C17;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_SHORT: types::GLenum = 0x1403;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_SHORT_4_4_4_4: types::GLenum = 0x8033;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_SHORT_5_5_5_1: types::GLenum = 0x8034;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNED_SHORT_5_6_5: types::GLenum = 0x8363;
#[allow(dead_code, non_upper_case_globals)] pub const VALIDATE_STATUS: types::GLenum = 0x8B83;
#[allow(dead_code, non_upper_case_globals)] pub const VENDOR: types::GLenum = 0x1F00;
#[allow(dead_code, non_upper_case_globals)] pub const VERSION: types::GLenum = 0x1F02;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_ARRAY: types::GLenum = 0x8074;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_ARRAY_BINDING: types::GLenum = 0x85B5;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_ARRAY_KHR: types::GLenum = 0x8074;
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
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_SHADER: types::GLenum = 0x8B31;
#[allow(dead_code, non_upper_case_globals)] pub const VERTEX_SHADER_BIT: types::GLenum = 0x00000001;
#[allow(dead_code, non_upper_case_globals)] pub const VIEWPORT: types::GLenum = 0x0BA2;
#[allow(dead_code, non_upper_case_globals)] pub const WAIT_FAILED: types::GLenum = 0x911D;
#[allow(dead_code, non_upper_case_globals)] pub const WRITE_ONLY: types::GLenum = 0x88B9;
#[allow(dead_code, non_upper_case_globals)] pub const ZERO: types::GLenum = 0;

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
            panic!("gles2 function was not loaded")
        }

        #[allow(non_camel_case_types, non_snake_case, dead_code)]
        #[derive(Clone)]
        pub struct Gles2 {
pub ActiveShaderProgram: FnPtr,
/// Fallbacks: ActiveTextureARB
pub ActiveTexture: FnPtr,
/// Fallbacks: AttachObjectARB
pub AttachShader: FnPtr,
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
pub BindVertexBuffer: FnPtr,
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
pub BufferStorageEXT: FnPtr,
/// Fallbacks: BufferSubDataARB
pub BufferSubData: FnPtr,
/// Fallbacks: CheckFramebufferStatusEXT
pub CheckFramebufferStatus: FnPtr,
pub Clear: FnPtr,
pub ClearBufferfi: FnPtr,
pub ClearBufferfv: FnPtr,
pub ClearBufferiv: FnPtr,
pub ClearBufferuiv: FnPtr,
pub ClearColor: FnPtr,
/// Fallbacks: ClearDepthfOES
pub ClearDepthf: FnPtr,
pub ClearStencil: FnPtr,
/// Fallbacks: ClientWaitSyncAPPLE
pub ClientWaitSync: FnPtr,
pub ColorMask: FnPtr,
/// Fallbacks: CompileShaderARB
pub CompileShader: FnPtr,
/// Fallbacks: CompressedTexImage2DARB
pub CompressedTexImage2D: FnPtr,
/// Fallbacks: CompressedTexImage3DARB
pub CompressedTexImage3D: FnPtr,
/// Fallbacks: CompressedTexSubImage2DARB
pub CompressedTexSubImage2D: FnPtr,
/// Fallbacks: CompressedTexSubImage3DARB
pub CompressedTexSubImage3D: FnPtr,
/// Fallbacks: CopyBufferSubDataNV
pub CopyBufferSubData: FnPtr,
pub CopyImageSubDataEXT: FnPtr,
pub CopySubTexture3DANGLE: FnPtr,
pub CopySubTextureCHROMIUM: FnPtr,
/// Fallbacks: CopyTexImage2DEXT
pub CopyTexImage2D: FnPtr,
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
/// Fallbacks: DeleteFramebuffersEXT
pub DeleteFramebuffers: FnPtr,
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
pub DepthFunc: FnPtr,
pub DepthMask: FnPtr,
/// Fallbacks: DepthRangefOES
pub DepthRangef: FnPtr,
/// Fallbacks: DetachObjectARB
pub DetachShader: FnPtr,
pub Disable: FnPtr,
/// Fallbacks: DisableVertexAttribArrayARB
pub DisableVertexAttribArray: FnPtr,
pub DispatchCompute: FnPtr,
pub DispatchComputeIndirect: FnPtr,
/// Fallbacks: DrawArraysEXT
pub DrawArrays: FnPtr,
pub DrawArraysIndirect: FnPtr,
/// Fallbacks: DrawArraysInstancedANGLE, DrawArraysInstancedARB, DrawArraysInstancedEXT, DrawArraysInstancedNV
pub DrawArraysInstanced: FnPtr,
/// Fallbacks: DrawBuffersARB, DrawBuffersATI, DrawBuffersEXT
pub DrawBuffers: FnPtr,
pub DrawElements: FnPtr,
pub DrawElementsIndirect: FnPtr,
/// Fallbacks: DrawElementsInstancedANGLE, DrawElementsInstancedARB, DrawElementsInstancedEXT, DrawElementsInstancedNV
pub DrawElementsInstanced: FnPtr,
/// Fallbacks: DrawRangeElementsEXT
pub DrawRangeElements: FnPtr,
pub EGLImageTargetRenderbufferStorageOES: FnPtr,
pub EGLImageTargetTexture2DOES: FnPtr,
pub Enable: FnPtr,
/// Fallbacks: EnableVertexAttribArrayARB
pub EnableVertexAttribArray: FnPtr,
/// Fallbacks: EndQueryARB
pub EndQuery: FnPtr,
pub EndQueryEXT: FnPtr,
/// Fallbacks: EndTransformFeedbackEXT, EndTransformFeedbackNV
pub EndTransformFeedback: FnPtr,
/// Fallbacks: FenceSyncAPPLE
pub FenceSync: FnPtr,
pub Finish: FnPtr,
pub Flush: FnPtr,
/// Fallbacks: FlushMappedBufferRangeAPPLE, FlushMappedBufferRangeEXT
pub FlushMappedBufferRange: FnPtr,
pub FramebufferParameteri: FnPtr,
/// Fallbacks: FramebufferRenderbufferEXT
pub FramebufferRenderbuffer: FnPtr,
/// Fallbacks: FramebufferTexture2DEXT
pub FramebufferTexture2D: FnPtr,
/// Fallbacks: FramebufferTextureLayerARB, FramebufferTextureLayerEXT
pub FramebufferTextureLayer: FnPtr,
pub FrontFace: FnPtr,
/// Fallbacks: GenBuffersARB
pub GenBuffers: FnPtr,
/// Fallbacks: GenFramebuffersEXT
pub GenFramebuffers: FnPtr,
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
/// Fallbacks: GenerateMipmapEXT
pub GenerateMipmap: FnPtr,
/// Fallbacks: GetActiveAttribARB
pub GetActiveAttrib: FnPtr,
/// Fallbacks: GetActiveUniformARB
pub GetActiveUniform: FnPtr,
pub GetActiveUniformBlockName: FnPtr,
pub GetActiveUniformBlockiv: FnPtr,
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
/// Fallbacks: GetDebugMessageLogARB, GetDebugMessageLogKHR
pub GetDebugMessageLog: FnPtr,
pub GetDebugMessageLogKHR: FnPtr,
pub GetError: FnPtr,
pub GetFloatv: FnPtr,
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
/// Fallbacks: GetMultisamplefvNV
pub GetMultisamplefv: FnPtr,
/// Fallbacks: GetObjectLabelKHR
pub GetObjectLabel: FnPtr,
pub GetObjectLabelKHR: FnPtr,
/// Fallbacks: GetObjectPtrLabelKHR
pub GetObjectPtrLabel: FnPtr,
pub GetObjectPtrLabelKHR: FnPtr,
/// Fallbacks: GetPointervEXT, GetPointervKHR
pub GetPointerv: FnPtr,
pub GetPointervKHR: FnPtr,
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
pub GetQueryObjecti64vEXT: FnPtr,
pub GetQueryObjectivEXT: FnPtr,
pub GetQueryObjectui64vEXT: FnPtr,
/// Fallbacks: GetQueryObjectuivARB
pub GetQueryObjectuiv: FnPtr,
pub GetQueryObjectuivEXT: FnPtr,
/// Fallbacks: GetQueryivARB
pub GetQueryiv: FnPtr,
pub GetQueryivEXT: FnPtr,
/// Fallbacks: GetRenderbufferParameterivEXT
pub GetRenderbufferParameteriv: FnPtr,
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
pub GetTexLevelParameterfv: FnPtr,
pub GetTexLevelParameteriv: FnPtr,
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
/// Fallbacks: GetVertexAttribfvARB, GetVertexAttribfvNV
pub GetVertexAttribfv: FnPtr,
/// Fallbacks: GetVertexAttribivARB, GetVertexAttribivNV
pub GetVertexAttribiv: FnPtr,
pub Hint: FnPtr,
pub InsertEventMarkerEXT: FnPtr,
pub InvalidateFramebuffer: FnPtr,
pub InvalidateSubFramebuffer: FnPtr,
/// Fallbacks: IsBufferARB
pub IsBuffer: FnPtr,
pub IsEnabled: FnPtr,
/// Fallbacks: IsFramebufferEXT
pub IsFramebuffer: FnPtr,
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
pub LineWidth: FnPtr,
/// Fallbacks: LinkProgramARB
pub LinkProgram: FnPtr,
/// Fallbacks: MapBufferRangeEXT
pub MapBufferRange: FnPtr,
/// Fallbacks: MemoryBarrierEXT
pub MemoryBarrier: FnPtr,
pub MemoryBarrierByRegion: FnPtr,
/// Fallbacks: ObjectLabelKHR
pub ObjectLabel: FnPtr,
pub ObjectLabelKHR: FnPtr,
/// Fallbacks: ObjectPtrLabelKHR
pub ObjectPtrLabel: FnPtr,
pub ObjectPtrLabelKHR: FnPtr,
/// Fallbacks: PauseTransformFeedbackNV
pub PauseTransformFeedback: FnPtr,
pub PixelStorei: FnPtr,
pub PolygonOffset: FnPtr,
/// Fallbacks: PopDebugGroupKHR
pub PopDebugGroup: FnPtr,
pub PopDebugGroupKHR: FnPtr,
pub PopGroupMarkerEXT: FnPtr,
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
pub ProvokingVertexANGLE: FnPtr,
/// Fallbacks: PushDebugGroupKHR
pub PushDebugGroup: FnPtr,
pub PushDebugGroupKHR: FnPtr,
pub PushGroupMarkerEXT: FnPtr,
pub QueryCounterEXT: FnPtr,
pub ReadBuffer: FnPtr,
pub ReadPixels: FnPtr,
pub ReleaseShaderCompiler: FnPtr,
/// Fallbacks: RenderbufferStorageEXT
pub RenderbufferStorage: FnPtr,
/// Fallbacks: RenderbufferStorageMultisampleEXT, RenderbufferStorageMultisampleNV
pub RenderbufferStorageMultisample: FnPtr,
/// Fallbacks: ResumeTransformFeedbackNV
pub ResumeTransformFeedback: FnPtr,
/// Fallbacks: SampleCoverageARB
pub SampleCoverage: FnPtr,
pub SampleMaski: FnPtr,
pub SamplerParameterf: FnPtr,
pub SamplerParameterfv: FnPtr,
pub SamplerParameteri: FnPtr,
pub SamplerParameteriv: FnPtr,
pub Scissor: FnPtr,
pub ShaderBinary: FnPtr,
/// Fallbacks: ShaderSourceARB
pub ShaderSource: FnPtr,
pub StencilFunc: FnPtr,
pub StencilFuncSeparate: FnPtr,
pub StencilMask: FnPtr,
pub StencilMaskSeparate: FnPtr,
pub StencilOp: FnPtr,
/// Fallbacks: StencilOpSeparateATI
pub StencilOpSeparate: FnPtr,
pub TexImage2D: FnPtr,
/// Fallbacks: TexImage3DEXT
pub TexImage3D: FnPtr,
pub TexParameterf: FnPtr,
pub TexParameterfv: FnPtr,
pub TexParameteri: FnPtr,
pub TexParameteriv: FnPtr,
pub TexStorage1DEXT: FnPtr,
/// Fallbacks: TexStorage2DEXT
pub TexStorage2D: FnPtr,
pub TexStorage2DEXT: FnPtr,
pub TexStorage2DMultisample: FnPtr,
/// Fallbacks: TexStorage3DEXT
pub TexStorage3D: FnPtr,
pub TexStorage3DEXT: FnPtr,
/// Fallbacks: TexSubImage2DEXT
pub TexSubImage2D: FnPtr,
/// Fallbacks: TexSubImage3DEXT
pub TexSubImage3D: FnPtr,
pub TextureStorage1DEXT: FnPtr,
pub TextureStorage2DEXT: FnPtr,
pub TextureStorage3DEXT: FnPtr,
/// Fallbacks: TransformFeedbackVaryingsEXT
pub TransformFeedbackVaryings: FnPtr,
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
/// Fallbacks: VertexAttrib1fARB, VertexAttrib1fNV
pub VertexAttrib1f: FnPtr,
/// Fallbacks: VertexAttrib1fvARB, VertexAttrib1fvNV
pub VertexAttrib1fv: FnPtr,
/// Fallbacks: VertexAttrib2fARB, VertexAttrib2fNV
pub VertexAttrib2f: FnPtr,
/// Fallbacks: VertexAttrib2fvARB, VertexAttrib2fvNV
pub VertexAttrib2fv: FnPtr,
/// Fallbacks: VertexAttrib3fARB, VertexAttrib3fNV
pub VertexAttrib3f: FnPtr,
/// Fallbacks: VertexAttrib3fvARB, VertexAttrib3fvNV
pub VertexAttrib3fv: FnPtr,
/// Fallbacks: VertexAttrib4fARB, VertexAttrib4fNV
pub VertexAttrib4f: FnPtr,
/// Fallbacks: VertexAttrib4fvARB, VertexAttrib4fvNV
pub VertexAttrib4fv: FnPtr,
pub VertexAttribBinding: FnPtr,
/// Fallbacks: VertexAttribDivisorANGLE, VertexAttribDivisorARB, VertexAttribDivisorEXT, VertexAttribDivisorNV
pub VertexAttribDivisor: FnPtr,
pub VertexAttribFormat: FnPtr,
/// Fallbacks: VertexAttribI4iEXT
pub VertexAttribI4i: FnPtr,
/// Fallbacks: VertexAttribI4ivEXT
pub VertexAttribI4iv: FnPtr,
/// Fallbacks: VertexAttribI4uiEXT
pub VertexAttribI4ui: FnPtr,
/// Fallbacks: VertexAttribI4uivEXT
pub VertexAttribI4uiv: FnPtr,
pub VertexAttribIFormat: FnPtr,
/// Fallbacks: VertexAttribIPointerEXT
pub VertexAttribIPointer: FnPtr,
/// Fallbacks: VertexAttribPointerARB
pub VertexAttribPointer: FnPtr,
pub VertexBindingDivisor: FnPtr,
pub Viewport: FnPtr,
/// Fallbacks: WaitSyncAPPLE
pub WaitSync: FnPtr,
_priv: ()
}
impl Gles2 {
            /// Load each OpenGL symbol using a custom load function. This allows for the
            /// use of functions like `glfwGetProcAddress` or `SDL_GL_GetProcAddress`.
            ///
            /// ~~~ignore
            /// let gl = Gl::load_with(|s| glfw.get_proc_address(s));
            /// ~~~
            #[allow(dead_code, unused_variables)]
            pub fn load_with<F>(mut loadfn: F) -> Gles2 where F: FnMut(&'static str) -> *const __gl_imports::raw::c_void {
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
                Gles2 {
ActiveShaderProgram: FnPtr::new(metaloadfn("glActiveShaderProgram", &[])),
ActiveTexture: FnPtr::new(metaloadfn("glActiveTexture", &["glActiveTextureARB"])),
AttachShader: FnPtr::new(metaloadfn("glAttachShader", &["glAttachObjectARB"])),
BeginQuery: FnPtr::new(metaloadfn("glBeginQuery", &["glBeginQueryARB"])),
BeginQueryEXT: FnPtr::new(metaloadfn("glBeginQueryEXT", &[])),
BeginTransformFeedback: FnPtr::new(metaloadfn("glBeginTransformFeedback", &["glBeginTransformFeedbackEXT", "glBeginTransformFeedbackNV"])),
BindAttribLocation: FnPtr::new(metaloadfn("glBindAttribLocation", &["glBindAttribLocationARB"])),
BindBuffer: FnPtr::new(metaloadfn("glBindBuffer", &["glBindBufferARB"])),
BindBufferBase: FnPtr::new(metaloadfn("glBindBufferBase", &["glBindBufferBaseEXT", "glBindBufferBaseNV"])),
BindBufferRange: FnPtr::new(metaloadfn("glBindBufferRange", &["glBindBufferRangeEXT", "glBindBufferRangeNV"])),
BindFramebuffer: FnPtr::new(metaloadfn("glBindFramebuffer", &[])),
BindImageTexture: FnPtr::new(metaloadfn("glBindImageTexture", &[])),
BindProgramPipeline: FnPtr::new(metaloadfn("glBindProgramPipeline", &[])),
BindRenderbuffer: FnPtr::new(metaloadfn("glBindRenderbuffer", &[])),
BindSampler: FnPtr::new(metaloadfn("glBindSampler", &[])),
BindTexture: FnPtr::new(metaloadfn("glBindTexture", &["glBindTextureEXT"])),
BindTransformFeedback: FnPtr::new(metaloadfn("glBindTransformFeedback", &[])),
BindVertexArray: FnPtr::new(metaloadfn("glBindVertexArray", &["glBindVertexArrayOES"])),
BindVertexBuffer: FnPtr::new(metaloadfn("glBindVertexBuffer", &[])),
BlendBarrierKHR: FnPtr::new(metaloadfn("glBlendBarrierKHR", &[])),
BlendColor: FnPtr::new(metaloadfn("glBlendColor", &["glBlendColorEXT"])),
BlendEquation: FnPtr::new(metaloadfn("glBlendEquation", &["glBlendEquationEXT"])),
BlendEquationSeparate: FnPtr::new(metaloadfn("glBlendEquationSeparate", &["glBlendEquationSeparateEXT"])),
BlendFunc: FnPtr::new(metaloadfn("glBlendFunc", &[])),
BlendFuncSeparate: FnPtr::new(metaloadfn("glBlendFuncSeparate", &["glBlendFuncSeparateEXT", "glBlendFuncSeparateINGR"])),
BlitFramebuffer: FnPtr::new(metaloadfn("glBlitFramebuffer", &["glBlitFramebufferEXT", "glBlitFramebufferNV"])),
BufferData: FnPtr::new(metaloadfn("glBufferData", &["glBufferDataARB"])),
BufferStorageEXT: FnPtr::new(metaloadfn("glBufferStorageEXT", &[])),
BufferSubData: FnPtr::new(metaloadfn("glBufferSubData", &["glBufferSubDataARB"])),
CheckFramebufferStatus: FnPtr::new(metaloadfn("glCheckFramebufferStatus", &["glCheckFramebufferStatusEXT"])),
Clear: FnPtr::new(metaloadfn("glClear", &[])),
ClearBufferfi: FnPtr::new(metaloadfn("glClearBufferfi", &[])),
ClearBufferfv: FnPtr::new(metaloadfn("glClearBufferfv", &[])),
ClearBufferiv: FnPtr::new(metaloadfn("glClearBufferiv", &[])),
ClearBufferuiv: FnPtr::new(metaloadfn("glClearBufferuiv", &[])),
ClearColor: FnPtr::new(metaloadfn("glClearColor", &[])),
ClearDepthf: FnPtr::new(metaloadfn("glClearDepthf", &["glClearDepthfOES"])),
ClearStencil: FnPtr::new(metaloadfn("glClearStencil", &[])),
ClientWaitSync: FnPtr::new(metaloadfn("glClientWaitSync", &["glClientWaitSyncAPPLE"])),
ColorMask: FnPtr::new(metaloadfn("glColorMask", &[])),
CompileShader: FnPtr::new(metaloadfn("glCompileShader", &["glCompileShaderARB"])),
CompressedTexImage2D: FnPtr::new(metaloadfn("glCompressedTexImage2D", &["glCompressedTexImage2DARB"])),
CompressedTexImage3D: FnPtr::new(metaloadfn("glCompressedTexImage3D", &["glCompressedTexImage3DARB"])),
CompressedTexSubImage2D: FnPtr::new(metaloadfn("glCompressedTexSubImage2D", &["glCompressedTexSubImage2DARB"])),
CompressedTexSubImage3D: FnPtr::new(metaloadfn("glCompressedTexSubImage3D", &["glCompressedTexSubImage3DARB"])),
CopyBufferSubData: FnPtr::new(metaloadfn("glCopyBufferSubData", &["glCopyBufferSubDataNV"])),
CopyImageSubDataEXT: FnPtr::new(metaloadfn("glCopyImageSubDataEXT", &[])),
CopySubTexture3DANGLE: FnPtr::new(metaloadfn("glCopySubTexture3DANGLE", &[])),
CopySubTextureCHROMIUM: FnPtr::new(metaloadfn("glCopySubTextureCHROMIUM", &[])),
CopyTexImage2D: FnPtr::new(metaloadfn("glCopyTexImage2D", &["glCopyTexImage2DEXT"])),
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
DeleteFramebuffers: FnPtr::new(metaloadfn("glDeleteFramebuffers", &["glDeleteFramebuffersEXT"])),
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
DepthFunc: FnPtr::new(metaloadfn("glDepthFunc", &[])),
DepthMask: FnPtr::new(metaloadfn("glDepthMask", &[])),
DepthRangef: FnPtr::new(metaloadfn("glDepthRangef", &["glDepthRangefOES"])),
DetachShader: FnPtr::new(metaloadfn("glDetachShader", &["glDetachObjectARB"])),
Disable: FnPtr::new(metaloadfn("glDisable", &[])),
DisableVertexAttribArray: FnPtr::new(metaloadfn("glDisableVertexAttribArray", &["glDisableVertexAttribArrayARB"])),
DispatchCompute: FnPtr::new(metaloadfn("glDispatchCompute", &[])),
DispatchComputeIndirect: FnPtr::new(metaloadfn("glDispatchComputeIndirect", &[])),
DrawArrays: FnPtr::new(metaloadfn("glDrawArrays", &["glDrawArraysEXT"])),
DrawArraysIndirect: FnPtr::new(metaloadfn("glDrawArraysIndirect", &[])),
DrawArraysInstanced: FnPtr::new(metaloadfn("glDrawArraysInstanced", &["glDrawArraysInstancedANGLE", "glDrawArraysInstancedARB", "glDrawArraysInstancedEXT", "glDrawArraysInstancedNV"])),
DrawBuffers: FnPtr::new(metaloadfn("glDrawBuffers", &["glDrawBuffersARB", "glDrawBuffersATI", "glDrawBuffersEXT"])),
DrawElements: FnPtr::new(metaloadfn("glDrawElements", &[])),
DrawElementsIndirect: FnPtr::new(metaloadfn("glDrawElementsIndirect", &[])),
DrawElementsInstanced: FnPtr::new(metaloadfn("glDrawElementsInstanced", &["glDrawElementsInstancedANGLE", "glDrawElementsInstancedARB", "glDrawElementsInstancedEXT", "glDrawElementsInstancedNV"])),
DrawRangeElements: FnPtr::new(metaloadfn("glDrawRangeElements", &["glDrawRangeElementsEXT"])),
EGLImageTargetRenderbufferStorageOES: FnPtr::new(metaloadfn("glEGLImageTargetRenderbufferStorageOES", &[])),
EGLImageTargetTexture2DOES: FnPtr::new(metaloadfn("glEGLImageTargetTexture2DOES", &[])),
Enable: FnPtr::new(metaloadfn("glEnable", &[])),
EnableVertexAttribArray: FnPtr::new(metaloadfn("glEnableVertexAttribArray", &["glEnableVertexAttribArrayARB"])),
EndQuery: FnPtr::new(metaloadfn("glEndQuery", &["glEndQueryARB"])),
EndQueryEXT: FnPtr::new(metaloadfn("glEndQueryEXT", &[])),
EndTransformFeedback: FnPtr::new(metaloadfn("glEndTransformFeedback", &["glEndTransformFeedbackEXT", "glEndTransformFeedbackNV"])),
FenceSync: FnPtr::new(metaloadfn("glFenceSync", &["glFenceSyncAPPLE"])),
Finish: FnPtr::new(metaloadfn("glFinish", &[])),
Flush: FnPtr::new(metaloadfn("glFlush", &[])),
FlushMappedBufferRange: FnPtr::new(metaloadfn("glFlushMappedBufferRange", &["glFlushMappedBufferRangeAPPLE", "glFlushMappedBufferRangeEXT"])),
FramebufferParameteri: FnPtr::new(metaloadfn("glFramebufferParameteri", &[])),
FramebufferRenderbuffer: FnPtr::new(metaloadfn("glFramebufferRenderbuffer", &["glFramebufferRenderbufferEXT"])),
FramebufferTexture2D: FnPtr::new(metaloadfn("glFramebufferTexture2D", &["glFramebufferTexture2DEXT"])),
FramebufferTextureLayer: FnPtr::new(metaloadfn("glFramebufferTextureLayer", &["glFramebufferTextureLayerARB", "glFramebufferTextureLayerEXT"])),
FrontFace: FnPtr::new(metaloadfn("glFrontFace", &[])),
GenBuffers: FnPtr::new(metaloadfn("glGenBuffers", &["glGenBuffersARB"])),
GenFramebuffers: FnPtr::new(metaloadfn("glGenFramebuffers", &["glGenFramebuffersEXT"])),
GenProgramPipelines: FnPtr::new(metaloadfn("glGenProgramPipelines", &[])),
GenQueries: FnPtr::new(metaloadfn("glGenQueries", &["glGenQueriesARB"])),
GenQueriesEXT: FnPtr::new(metaloadfn("glGenQueriesEXT", &[])),
GenRenderbuffers: FnPtr::new(metaloadfn("glGenRenderbuffers", &["glGenRenderbuffersEXT"])),
GenSamplers: FnPtr::new(metaloadfn("glGenSamplers", &[])),
GenTextures: FnPtr::new(metaloadfn("glGenTextures", &[])),
GenTransformFeedbacks: FnPtr::new(metaloadfn("glGenTransformFeedbacks", &["glGenTransformFeedbacksNV"])),
GenVertexArrays: FnPtr::new(metaloadfn("glGenVertexArrays", &["glGenVertexArraysAPPLE", "glGenVertexArraysOES"])),
GenerateMipmap: FnPtr::new(metaloadfn("glGenerateMipmap", &["glGenerateMipmapEXT"])),
GetActiveAttrib: FnPtr::new(metaloadfn("glGetActiveAttrib", &["glGetActiveAttribARB"])),
GetActiveUniform: FnPtr::new(metaloadfn("glGetActiveUniform", &["glGetActiveUniformARB"])),
GetActiveUniformBlockName: FnPtr::new(metaloadfn("glGetActiveUniformBlockName", &[])),
GetActiveUniformBlockiv: FnPtr::new(metaloadfn("glGetActiveUniformBlockiv", &[])),
GetActiveUniformsiv: FnPtr::new(metaloadfn("glGetActiveUniformsiv", &[])),
GetAttachedShaders: FnPtr::new(metaloadfn("glGetAttachedShaders", &[])),
GetAttribLocation: FnPtr::new(metaloadfn("glGetAttribLocation", &["glGetAttribLocationARB"])),
GetBooleani_v: FnPtr::new(metaloadfn("glGetBooleani_v", &["glGetBooleanIndexedvEXT"])),
GetBooleanv: FnPtr::new(metaloadfn("glGetBooleanv", &[])),
GetBufferParameteri64v: FnPtr::new(metaloadfn("glGetBufferParameteri64v", &[])),
GetBufferParameteriv: FnPtr::new(metaloadfn("glGetBufferParameteriv", &["glGetBufferParameterivARB"])),
GetBufferPointerv: FnPtr::new(metaloadfn("glGetBufferPointerv", &["glGetBufferPointervARB", "glGetBufferPointervOES"])),
GetDebugMessageLog: FnPtr::new(metaloadfn("glGetDebugMessageLog", &["glGetDebugMessageLogARB", "glGetDebugMessageLogKHR"])),
GetDebugMessageLogKHR: FnPtr::new(metaloadfn("glGetDebugMessageLogKHR", &[])),
GetError: FnPtr::new(metaloadfn("glGetError", &[])),
GetFloatv: FnPtr::new(metaloadfn("glGetFloatv", &[])),
GetFragDataLocation: FnPtr::new(metaloadfn("glGetFragDataLocation", &["glGetFragDataLocationEXT"])),
GetFramebufferAttachmentParameteriv: FnPtr::new(metaloadfn("glGetFramebufferAttachmentParameteriv", &["glGetFramebufferAttachmentParameterivEXT"])),
GetFramebufferParameteriv: FnPtr::new(metaloadfn("glGetFramebufferParameteriv", &[])),
GetInteger64i_v: FnPtr::new(metaloadfn("glGetInteger64i_v", &[])),
GetInteger64v: FnPtr::new(metaloadfn("glGetInteger64v", &["glGetInteger64vAPPLE"])),
GetIntegeri_v: FnPtr::new(metaloadfn("glGetIntegeri_v", &["glGetIntegerIndexedvEXT"])),
GetIntegerv: FnPtr::new(metaloadfn("glGetIntegerv", &[])),
GetInternalformativ: FnPtr::new(metaloadfn("glGetInternalformativ", &[])),
GetMultisamplefv: FnPtr::new(metaloadfn("glGetMultisamplefv", &["glGetMultisamplefvNV"])),
GetObjectLabel: FnPtr::new(metaloadfn("glGetObjectLabel", &["glGetObjectLabelKHR"])),
GetObjectLabelKHR: FnPtr::new(metaloadfn("glGetObjectLabelKHR", &[])),
GetObjectPtrLabel: FnPtr::new(metaloadfn("glGetObjectPtrLabel", &["glGetObjectPtrLabelKHR"])),
GetObjectPtrLabelKHR: FnPtr::new(metaloadfn("glGetObjectPtrLabelKHR", &[])),
GetPointerv: FnPtr::new(metaloadfn("glGetPointerv", &["glGetPointervEXT", "glGetPointervKHR"])),
GetPointervKHR: FnPtr::new(metaloadfn("glGetPointervKHR", &[])),
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
GetQueryObjecti64vEXT: FnPtr::new(metaloadfn("glGetQueryObjecti64vEXT", &[])),
GetQueryObjectivEXT: FnPtr::new(metaloadfn("glGetQueryObjectivEXT", &[])),
GetQueryObjectui64vEXT: FnPtr::new(metaloadfn("glGetQueryObjectui64vEXT", &[])),
GetQueryObjectuiv: FnPtr::new(metaloadfn("glGetQueryObjectuiv", &["glGetQueryObjectuivARB"])),
GetQueryObjectuivEXT: FnPtr::new(metaloadfn("glGetQueryObjectuivEXT", &[])),
GetQueryiv: FnPtr::new(metaloadfn("glGetQueryiv", &["glGetQueryivARB"])),
GetQueryivEXT: FnPtr::new(metaloadfn("glGetQueryivEXT", &[])),
GetRenderbufferParameteriv: FnPtr::new(metaloadfn("glGetRenderbufferParameteriv", &["glGetRenderbufferParameterivEXT"])),
GetSamplerParameterfv: FnPtr::new(metaloadfn("glGetSamplerParameterfv", &[])),
GetSamplerParameteriv: FnPtr::new(metaloadfn("glGetSamplerParameteriv", &[])),
GetShaderInfoLog: FnPtr::new(metaloadfn("glGetShaderInfoLog", &[])),
GetShaderPrecisionFormat: FnPtr::new(metaloadfn("glGetShaderPrecisionFormat", &[])),
GetShaderSource: FnPtr::new(metaloadfn("glGetShaderSource", &["glGetShaderSourceARB"])),
GetShaderiv: FnPtr::new(metaloadfn("glGetShaderiv", &[])),
GetString: FnPtr::new(metaloadfn("glGetString", &[])),
GetStringi: FnPtr::new(metaloadfn("glGetStringi", &[])),
GetSynciv: FnPtr::new(metaloadfn("glGetSynciv", &["glGetSyncivAPPLE"])),
GetTexLevelParameterfv: FnPtr::new(metaloadfn("glGetTexLevelParameterfv", &[])),
GetTexLevelParameteriv: FnPtr::new(metaloadfn("glGetTexLevelParameteriv", &[])),
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
GetVertexAttribfv: FnPtr::new(metaloadfn("glGetVertexAttribfv", &["glGetVertexAttribfvARB", "glGetVertexAttribfvNV"])),
GetVertexAttribiv: FnPtr::new(metaloadfn("glGetVertexAttribiv", &["glGetVertexAttribivARB", "glGetVertexAttribivNV"])),
Hint: FnPtr::new(metaloadfn("glHint", &[])),
InsertEventMarkerEXT: FnPtr::new(metaloadfn("glInsertEventMarkerEXT", &[])),
InvalidateFramebuffer: FnPtr::new(metaloadfn("glInvalidateFramebuffer", &[])),
InvalidateSubFramebuffer: FnPtr::new(metaloadfn("glInvalidateSubFramebuffer", &[])),
IsBuffer: FnPtr::new(metaloadfn("glIsBuffer", &["glIsBufferARB"])),
IsEnabled: FnPtr::new(metaloadfn("glIsEnabled", &[])),
IsFramebuffer: FnPtr::new(metaloadfn("glIsFramebuffer", &["glIsFramebufferEXT"])),
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
LineWidth: FnPtr::new(metaloadfn("glLineWidth", &[])),
LinkProgram: FnPtr::new(metaloadfn("glLinkProgram", &["glLinkProgramARB"])),
MapBufferRange: FnPtr::new(metaloadfn("glMapBufferRange", &["glMapBufferRangeEXT"])),
MemoryBarrier: FnPtr::new(metaloadfn("glMemoryBarrier", &["glMemoryBarrierEXT"])),
MemoryBarrierByRegion: FnPtr::new(metaloadfn("glMemoryBarrierByRegion", &[])),
ObjectLabel: FnPtr::new(metaloadfn("glObjectLabel", &["glObjectLabelKHR"])),
ObjectLabelKHR: FnPtr::new(metaloadfn("glObjectLabelKHR", &[])),
ObjectPtrLabel: FnPtr::new(metaloadfn("glObjectPtrLabel", &["glObjectPtrLabelKHR"])),
ObjectPtrLabelKHR: FnPtr::new(metaloadfn("glObjectPtrLabelKHR", &[])),
PauseTransformFeedback: FnPtr::new(metaloadfn("glPauseTransformFeedback", &["glPauseTransformFeedbackNV"])),
PixelStorei: FnPtr::new(metaloadfn("glPixelStorei", &[])),
PolygonOffset: FnPtr::new(metaloadfn("glPolygonOffset", &[])),
PopDebugGroup: FnPtr::new(metaloadfn("glPopDebugGroup", &["glPopDebugGroupKHR"])),
PopDebugGroupKHR: FnPtr::new(metaloadfn("glPopDebugGroupKHR", &[])),
PopGroupMarkerEXT: FnPtr::new(metaloadfn("glPopGroupMarkerEXT", &[])),
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
ProvokingVertexANGLE: FnPtr::new(metaloadfn("glProvokingVertexANGLE", &[])),
PushDebugGroup: FnPtr::new(metaloadfn("glPushDebugGroup", &["glPushDebugGroupKHR"])),
PushDebugGroupKHR: FnPtr::new(metaloadfn("glPushDebugGroupKHR", &[])),
PushGroupMarkerEXT: FnPtr::new(metaloadfn("glPushGroupMarkerEXT", &[])),
QueryCounterEXT: FnPtr::new(metaloadfn("glQueryCounterEXT", &[])),
ReadBuffer: FnPtr::new(metaloadfn("glReadBuffer", &[])),
ReadPixels: FnPtr::new(metaloadfn("glReadPixels", &[])),
ReleaseShaderCompiler: FnPtr::new(metaloadfn("glReleaseShaderCompiler", &[])),
RenderbufferStorage: FnPtr::new(metaloadfn("glRenderbufferStorage", &["glRenderbufferStorageEXT"])),
RenderbufferStorageMultisample: FnPtr::new(metaloadfn("glRenderbufferStorageMultisample", &["glRenderbufferStorageMultisampleEXT", "glRenderbufferStorageMultisampleNV"])),
ResumeTransformFeedback: FnPtr::new(metaloadfn("glResumeTransformFeedback", &["glResumeTransformFeedbackNV"])),
SampleCoverage: FnPtr::new(metaloadfn("glSampleCoverage", &["glSampleCoverageARB"])),
SampleMaski: FnPtr::new(metaloadfn("glSampleMaski", &[])),
SamplerParameterf: FnPtr::new(metaloadfn("glSamplerParameterf", &[])),
SamplerParameterfv: FnPtr::new(metaloadfn("glSamplerParameterfv", &[])),
SamplerParameteri: FnPtr::new(metaloadfn("glSamplerParameteri", &[])),
SamplerParameteriv: FnPtr::new(metaloadfn("glSamplerParameteriv", &[])),
Scissor: FnPtr::new(metaloadfn("glScissor", &[])),
ShaderBinary: FnPtr::new(metaloadfn("glShaderBinary", &[])),
ShaderSource: FnPtr::new(metaloadfn("glShaderSource", &["glShaderSourceARB"])),
StencilFunc: FnPtr::new(metaloadfn("glStencilFunc", &[])),
StencilFuncSeparate: FnPtr::new(metaloadfn("glStencilFuncSeparate", &[])),
StencilMask: FnPtr::new(metaloadfn("glStencilMask", &[])),
StencilMaskSeparate: FnPtr::new(metaloadfn("glStencilMaskSeparate", &[])),
StencilOp: FnPtr::new(metaloadfn("glStencilOp", &[])),
StencilOpSeparate: FnPtr::new(metaloadfn("glStencilOpSeparate", &["glStencilOpSeparateATI"])),
TexImage2D: FnPtr::new(metaloadfn("glTexImage2D", &[])),
TexImage3D: FnPtr::new(metaloadfn("glTexImage3D", &["glTexImage3DEXT"])),
TexParameterf: FnPtr::new(metaloadfn("glTexParameterf", &[])),
TexParameterfv: FnPtr::new(metaloadfn("glTexParameterfv", &[])),
TexParameteri: FnPtr::new(metaloadfn("glTexParameteri", &[])),
TexParameteriv: FnPtr::new(metaloadfn("glTexParameteriv", &[])),
TexStorage1DEXT: FnPtr::new(metaloadfn("glTexStorage1DEXT", &[])),
TexStorage2D: FnPtr::new(metaloadfn("glTexStorage2D", &["glTexStorage2DEXT"])),
TexStorage2DEXT: FnPtr::new(metaloadfn("glTexStorage2DEXT", &[])),
TexStorage2DMultisample: FnPtr::new(metaloadfn("glTexStorage2DMultisample", &[])),
TexStorage3D: FnPtr::new(metaloadfn("glTexStorage3D", &["glTexStorage3DEXT"])),
TexStorage3DEXT: FnPtr::new(metaloadfn("glTexStorage3DEXT", &[])),
TexSubImage2D: FnPtr::new(metaloadfn("glTexSubImage2D", &["glTexSubImage2DEXT"])),
TexSubImage3D: FnPtr::new(metaloadfn("glTexSubImage3D", &["glTexSubImage3DEXT"])),
TextureStorage1DEXT: FnPtr::new(metaloadfn("glTextureStorage1DEXT", &[])),
TextureStorage2DEXT: FnPtr::new(metaloadfn("glTextureStorage2DEXT", &[])),
TextureStorage3DEXT: FnPtr::new(metaloadfn("glTextureStorage3DEXT", &[])),
TransformFeedbackVaryings: FnPtr::new(metaloadfn("glTransformFeedbackVaryings", &["glTransformFeedbackVaryingsEXT"])),
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
VertexAttrib1f: FnPtr::new(metaloadfn("glVertexAttrib1f", &["glVertexAttrib1fARB", "glVertexAttrib1fNV"])),
VertexAttrib1fv: FnPtr::new(metaloadfn("glVertexAttrib1fv", &["glVertexAttrib1fvARB", "glVertexAttrib1fvNV"])),
VertexAttrib2f: FnPtr::new(metaloadfn("glVertexAttrib2f", &["glVertexAttrib2fARB", "glVertexAttrib2fNV"])),
VertexAttrib2fv: FnPtr::new(metaloadfn("glVertexAttrib2fv", &["glVertexAttrib2fvARB", "glVertexAttrib2fvNV"])),
VertexAttrib3f: FnPtr::new(metaloadfn("glVertexAttrib3f", &["glVertexAttrib3fARB", "glVertexAttrib3fNV"])),
VertexAttrib3fv: FnPtr::new(metaloadfn("glVertexAttrib3fv", &["glVertexAttrib3fvARB", "glVertexAttrib3fvNV"])),
VertexAttrib4f: FnPtr::new(metaloadfn("glVertexAttrib4f", &["glVertexAttrib4fARB", "glVertexAttrib4fNV"])),
VertexAttrib4fv: FnPtr::new(metaloadfn("glVertexAttrib4fv", &["glVertexAttrib4fvARB", "glVertexAttrib4fvNV"])),
VertexAttribBinding: FnPtr::new(metaloadfn("glVertexAttribBinding", &[])),
VertexAttribDivisor: FnPtr::new(metaloadfn("glVertexAttribDivisor", &["glVertexAttribDivisorANGLE", "glVertexAttribDivisorARB", "glVertexAttribDivisorEXT", "glVertexAttribDivisorNV"])),
VertexAttribFormat: FnPtr::new(metaloadfn("glVertexAttribFormat", &[])),
VertexAttribI4i: FnPtr::new(metaloadfn("glVertexAttribI4i", &["glVertexAttribI4iEXT"])),
VertexAttribI4iv: FnPtr::new(metaloadfn("glVertexAttribI4iv", &["glVertexAttribI4ivEXT"])),
VertexAttribI4ui: FnPtr::new(metaloadfn("glVertexAttribI4ui", &["glVertexAttribI4uiEXT"])),
VertexAttribI4uiv: FnPtr::new(metaloadfn("glVertexAttribI4uiv", &["glVertexAttribI4uivEXT"])),
VertexAttribIFormat: FnPtr::new(metaloadfn("glVertexAttribIFormat", &[])),
VertexAttribIPointer: FnPtr::new(metaloadfn("glVertexAttribIPointer", &["glVertexAttribIPointerEXT"])),
VertexAttribPointer: FnPtr::new(metaloadfn("glVertexAttribPointer", &["glVertexAttribPointerARB"])),
VertexBindingDivisor: FnPtr::new(metaloadfn("glVertexBindingDivisor", &[])),
Viewport: FnPtr::new(metaloadfn("glViewport", &[])),
WaitSync: FnPtr::new(metaloadfn("glWaitSync", &["glWaitSyncAPPLE"])),
_priv: ()
}
        }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ActiveShaderProgram(&self, pipeline: types::GLuint, program: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint) -> ()>(self.ActiveShaderProgram.f)(pipeline, program) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ActiveTexture(&self, texture: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.ActiveTexture.f)(texture) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn AttachShader(&self, program: types::GLuint, shader: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint) -> ()>(self.AttachShader.f)(program, shader) }
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
            #[inline] pub unsafe fn BindVertexBuffer(&self, bindingindex: types::GLuint, buffer: types::GLuint, offset: types::GLintptr, stride: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint, types::GLintptr, types::GLsizei) -> ()>(self.BindVertexBuffer.f)(bindingindex, buffer, offset, stride) }
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
            #[inline] pub unsafe fn BufferStorageEXT(&self, target: types::GLenum, size: types::GLsizeiptr, data: *const __gl_imports::raw::c_void, flags: types::GLbitfield) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizeiptr, *const __gl_imports::raw::c_void, types::GLbitfield) -> ()>(self.BufferStorageEXT.f)(target, size, data, flags) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BufferSubData(&self, target: types::GLenum, offset: types::GLintptr, size: types::GLsizeiptr, data: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLintptr, types::GLsizeiptr, *const __gl_imports::raw::c_void) -> ()>(self.BufferSubData.f)(target, offset, size, data) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CheckFramebufferStatus(&self, target: types::GLenum) -> types::GLenum { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> types::GLenum>(self.CheckFramebufferStatus.f)(target) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Clear(&self, mask: types::GLbitfield) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLbitfield) -> ()>(self.Clear.f)(mask) }
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
            #[inline] pub unsafe fn ClearDepthf(&self, d: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat) -> ()>(self.ClearDepthf.f)(d) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ClearStencil(&self, s: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint) -> ()>(self.ClearStencil.f)(s) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ClientWaitSync(&self, sync: types::GLsync, flags: types::GLbitfield, timeout: types::GLuint64) -> types::GLenum { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsync, types::GLbitfield, types::GLuint64) -> types::GLenum>(self.ClientWaitSync.f)(sync, flags, timeout) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ColorMask(&self, red: types::GLboolean, green: types::GLboolean, blue: types::GLboolean, alpha: types::GLboolean) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLboolean, types::GLboolean, types::GLboolean, types::GLboolean) -> ()>(self.ColorMask.f)(red, green, blue, alpha) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CompileShader(&self, shader: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.CompileShader.f)(shader) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CompressedTexImage2D(&self, target: types::GLenum, level: types::GLint, internalformat: types::GLenum, width: types::GLsizei, height: types::GLsizei, border: types::GLint, imageSize: types::GLsizei, data: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLenum, types::GLsizei, types::GLsizei, types::GLint, types::GLsizei, *const __gl_imports::raw::c_void) -> ()>(self.CompressedTexImage2D.f)(target, level, internalformat, width, height, border, imageSize, data) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CompressedTexImage3D(&self, target: types::GLenum, level: types::GLint, internalformat: types::GLenum, width: types::GLsizei, height: types::GLsizei, depth: types::GLsizei, border: types::GLint, imageSize: types::GLsizei, data: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLenum, types::GLsizei, types::GLsizei, types::GLsizei, types::GLint, types::GLsizei, *const __gl_imports::raw::c_void) -> ()>(self.CompressedTexImage3D.f)(target, level, internalformat, width, height, depth, border, imageSize, data) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CompressedTexSubImage2D(&self, target: types::GLenum, level: types::GLint, xoffset: types::GLint, yoffset: types::GLint, width: types::GLsizei, height: types::GLsizei, format: types::GLenum, imageSize: types::GLsizei, data: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLint, types::GLint, types::GLsizei, types::GLsizei, types::GLenum, types::GLsizei, *const __gl_imports::raw::c_void) -> ()>(self.CompressedTexSubImage2D.f)(target, level, xoffset, yoffset, width, height, format, imageSize, data) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CompressedTexSubImage3D(&self, target: types::GLenum, level: types::GLint, xoffset: types::GLint, yoffset: types::GLint, zoffset: types::GLint, width: types::GLsizei, height: types::GLsizei, depth: types::GLsizei, format: types::GLenum, imageSize: types::GLsizei, data: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLint, types::GLint, types::GLint, types::GLsizei, types::GLsizei, types::GLsizei, types::GLenum, types::GLsizei, *const __gl_imports::raw::c_void) -> ()>(self.CompressedTexSubImage3D.f)(target, level, xoffset, yoffset, zoffset, width, height, depth, format, imageSize, data) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CopyBufferSubData(&self, readTarget: types::GLenum, writeTarget: types::GLenum, readOffset: types::GLintptr, writeOffset: types::GLintptr, size: types::GLsizeiptr) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLintptr, types::GLintptr, types::GLsizeiptr) -> ()>(self.CopyBufferSubData.f)(readTarget, writeTarget, readOffset, writeOffset, size) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CopyImageSubDataEXT(&self, srcName: types::GLuint, srcTarget: types::GLenum, srcLevel: types::GLint, srcX: types::GLint, srcY: types::GLint, srcZ: types::GLint, dstName: types::GLuint, dstTarget: types::GLenum, dstLevel: types::GLint, dstX: types::GLint, dstY: types::GLint, dstZ: types::GLint, srcWidth: types::GLsizei, srcHeight: types::GLsizei, srcDepth: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLint, types::GLint, types::GLint, types::GLint, types::GLuint, types::GLenum, types::GLint, types::GLint, types::GLint, types::GLint, types::GLsizei, types::GLsizei, types::GLsizei) -> ()>(self.CopyImageSubDataEXT.f)(srcName, srcTarget, srcLevel, srcX, srcY, srcZ, dstName, dstTarget, dstLevel, dstX, dstY, dstZ, srcWidth, srcHeight, srcDepth) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CopySubTexture3DANGLE(&self, sourceId: types::GLuint, sourceLevel: types::GLint, destTarget: types::GLenum, destId: types::GLuint, destLevel: types::GLint, xoffset: types::GLint, yoffset: types::GLint, zoffset: types::GLint, x: types::GLint, y: types::GLint, z: types::GLint, width: types::GLint, height: types::GLint, depth: types::GLint, unpackFlipY: types::GLboolean, unpackPremultiplyAlpha: types::GLboolean, unpackUnmultiplyAlpha: types::GLboolean) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLenum, types::GLuint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLboolean, types::GLboolean, types::GLboolean) -> ()>(self.CopySubTexture3DANGLE.f)(sourceId, sourceLevel, destTarget, destId, destLevel, xoffset, yoffset, zoffset, x, y, z, width, height, depth, unpackFlipY, unpackPremultiplyAlpha, unpackUnmultiplyAlpha) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CopySubTextureCHROMIUM(&self, sourceId: types::GLuint, sourceLevel: types::GLint, destTarget: types::GLenum, destId: types::GLuint, destLevel: types::GLint, xoffset: types::GLint, yoffset: types::GLint, x: types::GLint, y: types::GLint, width: types::GLint, height: types::GLint, unpackFlipY: types::GLboolean, unpackPremultiplyAlpha: types::GLboolean, unpackUnmultiplyAlpha: types::GLboolean) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLenum, types::GLuint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLint, types::GLboolean, types::GLboolean, types::GLboolean) -> ()>(self.CopySubTextureCHROMIUM.f)(sourceId, sourceLevel, destTarget, destId, destLevel, xoffset, yoffset, x, y, width, height, unpackFlipY, unpackPremultiplyAlpha, unpackUnmultiplyAlpha) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CopyTexImage2D(&self, target: types::GLenum, level: types::GLint, internalformat: types::GLenum, x: types::GLint, y: types::GLint, width: types::GLsizei, height: types::GLsizei, border: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLenum, types::GLint, types::GLint, types::GLsizei, types::GLsizei, types::GLint) -> ()>(self.CopyTexImage2D.f)(target, level, internalformat, x, y, width, height, border) }
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
            #[inline] pub unsafe fn DeleteFramebuffers(&self, n: types::GLsizei, framebuffers: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *const types::GLuint) -> ()>(self.DeleteFramebuffers.f)(n, framebuffers) }
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
            #[inline] pub unsafe fn DepthFunc(&self, func: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.DepthFunc.f)(func) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DepthMask(&self, flag: types::GLboolean) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLboolean) -> ()>(self.DepthMask.f)(flag) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DepthRangef(&self, n: types::GLfloat, f: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat) -> ()>(self.DepthRangef.f)(n, f) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DetachShader(&self, program: types::GLuint, shader: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint) -> ()>(self.DetachShader.f)(program, shader) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Disable(&self, cap: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.Disable.f)(cap) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DisableVertexAttribArray(&self, index: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.DisableVertexAttribArray.f)(index) }
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
            #[inline] pub unsafe fn DrawBuffers(&self, n: types::GLsizei, bufs: *const types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *const types::GLenum) -> ()>(self.DrawBuffers.f)(n, bufs) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DrawElements(&self, mode: types::GLenum, count: types::GLsizei, type_: types::GLenum, indices: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, types::GLenum, *const __gl_imports::raw::c_void) -> ()>(self.DrawElements.f)(mode, count, type_, indices) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DrawElementsIndirect(&self, mode: types::GLenum, type_: types::GLenum, indirect: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *const __gl_imports::raw::c_void) -> ()>(self.DrawElementsIndirect.f)(mode, type_, indirect) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DrawElementsInstanced(&self, mode: types::GLenum, count: types::GLsizei, type_: types::GLenum, indices: *const __gl_imports::raw::c_void, instancecount: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, types::GLenum, *const __gl_imports::raw::c_void, types::GLsizei) -> ()>(self.DrawElementsInstanced.f)(mode, count, type_, indices, instancecount) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DrawRangeElements(&self, mode: types::GLenum, start: types::GLuint, end: types::GLuint, count: types::GLsizei, type_: types::GLenum, indices: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint, types::GLuint, types::GLsizei, types::GLenum, *const __gl_imports::raw::c_void) -> ()>(self.DrawRangeElements.f)(mode, start, end, count, type_, indices) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EGLImageTargetRenderbufferStorageOES(&self, target: types::GLenum, image: types::GLeglImageOES) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLeglImageOES) -> ()>(self.EGLImageTargetRenderbufferStorageOES.f)(target, image) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EGLImageTargetTexture2DOES(&self, target: types::GLenum, image: types::GLeglImageOES) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLeglImageOES) -> ()>(self.EGLImageTargetTexture2DOES.f)(target, image) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Enable(&self, cap: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.Enable.f)(cap) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EnableVertexAttribArray(&self, index: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.EnableVertexAttribArray.f)(index) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EndQuery(&self, target: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.EndQuery.f)(target) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EndQueryEXT(&self, target: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.EndQueryEXT.f)(target) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn EndTransformFeedback(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.EndTransformFeedback.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FenceSync(&self, condition: types::GLenum, flags: types::GLbitfield) -> types::GLsync { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLbitfield) -> types::GLsync>(self.FenceSync.f)(condition, flags) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Finish(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.Finish.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Flush(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.Flush.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FlushMappedBufferRange(&self, target: types::GLenum, offset: types::GLintptr, length: types::GLsizeiptr) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLintptr, types::GLsizeiptr) -> ()>(self.FlushMappedBufferRange.f)(target, offset, length) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FramebufferParameteri(&self, target: types::GLenum, pname: types::GLenum, param: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLint) -> ()>(self.FramebufferParameteri.f)(target, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FramebufferRenderbuffer(&self, target: types::GLenum, attachment: types::GLenum, renderbuffertarget: types::GLenum, renderbuffer: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLenum, types::GLuint) -> ()>(self.FramebufferRenderbuffer.f)(target, attachment, renderbuffertarget, renderbuffer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FramebufferTexture2D(&self, target: types::GLenum, attachment: types::GLenum, textarget: types::GLenum, texture: types::GLuint, level: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLenum, types::GLuint, types::GLint) -> ()>(self.FramebufferTexture2D.f)(target, attachment, textarget, texture, level) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FramebufferTextureLayer(&self, target: types::GLenum, attachment: types::GLenum, texture: types::GLuint, level: types::GLint, layer: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLuint, types::GLint, types::GLint) -> ()>(self.FramebufferTextureLayer.f)(target, attachment, texture, level, layer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn FrontFace(&self, mode: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.FrontFace.f)(mode) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GenBuffers(&self, n: types::GLsizei, buffers: *mut types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *mut types::GLuint) -> ()>(self.GenBuffers.f)(n, buffers) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GenFramebuffers(&self, n: types::GLsizei, framebuffers: *mut types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *mut types::GLuint) -> ()>(self.GenFramebuffers.f)(n, framebuffers) }
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
            #[inline] pub unsafe fn GetDebugMessageLog(&self, count: types::GLuint, bufSize: types::GLsizei, sources: *mut types::GLenum, types: *mut types::GLenum, ids: *mut types::GLuint, severities: *mut types::GLenum, lengths: *mut types::GLsizei, messageLog: *mut types::GLchar) -> types::GLuint { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLsizei, *mut types::GLenum, *mut types::GLenum, *mut types::GLuint, *mut types::GLenum, *mut types::GLsizei, *mut types::GLchar) -> types::GLuint>(self.GetDebugMessageLog.f)(count, bufSize, sources, types, ids, severities, lengths, messageLog) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetDebugMessageLogKHR(&self, count: types::GLuint, bufSize: types::GLsizei, sources: *mut types::GLenum, types: *mut types::GLenum, ids: *mut types::GLuint, severities: *mut types::GLenum, lengths: *mut types::GLsizei, messageLog: *mut types::GLchar) -> types::GLuint { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLsizei, *mut types::GLenum, *mut types::GLenum, *mut types::GLuint, *mut types::GLenum, *mut types::GLsizei, *mut types::GLchar) -> types::GLuint>(self.GetDebugMessageLogKHR.f)(count, bufSize, sources, types, ids, severities, lengths, messageLog) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetError(&self, ) -> types::GLenum { __gl_imports::mem::transmute::<_, extern "system" fn() -> types::GLenum>(self.GetError.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetFloatv(&self, pname: types::GLenum, data: *mut types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *mut types::GLfloat) -> ()>(self.GetFloatv.f)(pname, data) }
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
            #[inline] pub unsafe fn GetPointerv(&self, pname: types::GLenum, params: *const *mut __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const *mut __gl_imports::raw::c_void) -> ()>(self.GetPointerv.f)(pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetPointervKHR(&self, pname: types::GLenum, params: *const *mut __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, *const *mut __gl_imports::raw::c_void) -> ()>(self.GetPointervKHR.f)(pname, params) }
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
            #[inline] pub unsafe fn GetQueryObjecti64vEXT(&self, id: types::GLuint, pname: types::GLenum, params: *mut types::GLint64) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *mut types::GLint64) -> ()>(self.GetQueryObjecti64vEXT.f)(id, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetQueryObjectivEXT(&self, id: types::GLuint, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *mut types::GLint) -> ()>(self.GetQueryObjectivEXT.f)(id, pname, params) }
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
            #[inline] pub unsafe fn GetTexLevelParameterfv(&self, target: types::GLenum, level: types::GLint, pname: types::GLenum, params: *mut types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLenum, *mut types::GLfloat) -> ()>(self.GetTexLevelParameterfv.f)(target, level, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetTexLevelParameteriv(&self, target: types::GLenum, level: types::GLint, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLenum, *mut types::GLint) -> ()>(self.GetTexLevelParameteriv.f)(target, level, pname, params) }
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
            #[inline] pub unsafe fn GetVertexAttribfv(&self, index: types::GLuint, pname: types::GLenum, params: *mut types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *mut types::GLfloat) -> ()>(self.GetVertexAttribfv.f)(index, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetVertexAttribiv(&self, index: types::GLuint, pname: types::GLenum, params: *mut types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *mut types::GLint) -> ()>(self.GetVertexAttribiv.f)(index, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Hint(&self, target: types::GLenum, mode: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum) -> ()>(self.Hint.f)(target, mode) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn InsertEventMarkerEXT(&self, length: types::GLsizei, marker: *const types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *const types::GLchar) -> ()>(self.InsertEventMarkerEXT.f)(length, marker) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn InvalidateFramebuffer(&self, target: types::GLenum, numAttachments: types::GLsizei, attachments: *const types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, *const types::GLenum) -> ()>(self.InvalidateFramebuffer.f)(target, numAttachments, attachments) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn InvalidateSubFramebuffer(&self, target: types::GLenum, numAttachments: types::GLsizei, attachments: *const types::GLenum, x: types::GLint, y: types::GLint, width: types::GLsizei, height: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, *const types::GLenum, types::GLint, types::GLint, types::GLsizei, types::GLsizei) -> ()>(self.InvalidateSubFramebuffer.f)(target, numAttachments, attachments, x, y, width, height) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn IsBuffer(&self, buffer: types::GLuint) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> types::GLboolean>(self.IsBuffer.f)(buffer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn IsEnabled(&self, cap: types::GLenum) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> types::GLboolean>(self.IsEnabled.f)(cap) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn IsFramebuffer(&self, framebuffer: types::GLuint) -> types::GLboolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> types::GLboolean>(self.IsFramebuffer.f)(framebuffer) }
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
            #[inline] pub unsafe fn LineWidth(&self, width: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat) -> ()>(self.LineWidth.f)(width) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn LinkProgram(&self, program: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint) -> ()>(self.LinkProgram.f)(program) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MapBufferRange(&self, target: types::GLenum, offset: types::GLintptr, length: types::GLsizeiptr, access: types::GLbitfield) -> *mut __gl_imports::raw::c_void { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLintptr, types::GLsizeiptr, types::GLbitfield) -> *mut __gl_imports::raw::c_void>(self.MapBufferRange.f)(target, offset, length, access) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MemoryBarrier(&self, barriers: types::GLbitfield) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLbitfield) -> ()>(self.MemoryBarrier.f)(barriers) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MemoryBarrierByRegion(&self, barriers: types::GLbitfield) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLbitfield) -> ()>(self.MemoryBarrierByRegion.f)(barriers) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ObjectLabel(&self, identifier: types::GLenum, name: types::GLuint, length: types::GLsizei, label: *const types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint, types::GLsizei, *const types::GLchar) -> ()>(self.ObjectLabel.f)(identifier, name, length, label) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ObjectLabelKHR(&self, identifier: types::GLenum, name: types::GLuint, length: types::GLsizei, label: *const types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint, types::GLsizei, *const types::GLchar) -> ()>(self.ObjectLabelKHR.f)(identifier, name, length, label) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ObjectPtrLabel(&self, ptr: *const __gl_imports::raw::c_void, length: types::GLsizei, label: *const types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const __gl_imports::raw::c_void, types::GLsizei, *const types::GLchar) -> ()>(self.ObjectPtrLabel.f)(ptr, length, label) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ObjectPtrLabelKHR(&self, ptr: *const __gl_imports::raw::c_void, length: types::GLsizei, label: *const types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(*const __gl_imports::raw::c_void, types::GLsizei, *const types::GLchar) -> ()>(self.ObjectPtrLabelKHR.f)(ptr, length, label) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PauseTransformFeedback(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.PauseTransformFeedback.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PixelStorei(&self, pname: types::GLenum, param: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint) -> ()>(self.PixelStorei.f)(pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PolygonOffset(&self, factor: types::GLfloat, units: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLfloat) -> ()>(self.PolygonOffset.f)(factor, units) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PopDebugGroup(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.PopDebugGroup.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PopDebugGroupKHR(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.PopDebugGroupKHR.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PopGroupMarkerEXT(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.PopGroupMarkerEXT.f)() }
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
            #[inline] pub unsafe fn ProvokingVertexANGLE(&self, mode: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.ProvokingVertexANGLE.f)(mode) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PushDebugGroup(&self, source: types::GLenum, id: types::GLuint, length: types::GLsizei, message: *const types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint, types::GLsizei, *const types::GLchar) -> ()>(self.PushDebugGroup.f)(source, id, length, message) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PushDebugGroupKHR(&self, source: types::GLenum, id: types::GLuint, length: types::GLsizei, message: *const types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLuint, types::GLsizei, *const types::GLchar) -> ()>(self.PushDebugGroupKHR.f)(source, id, length, message) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn PushGroupMarkerEXT(&self, length: types::GLsizei, marker: *const types::GLchar) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *const types::GLchar) -> ()>(self.PushGroupMarkerEXT.f)(length, marker) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn QueryCounterEXT(&self, id: types::GLuint, target: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum) -> ()>(self.QueryCounterEXT.f)(id, target) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ReadBuffer(&self, src: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum) -> ()>(self.ReadBuffer.f)(src) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ReadPixels(&self, x: types::GLint, y: types::GLint, width: types::GLsizei, height: types::GLsizei, format: types::GLenum, type_: types::GLenum, pixels: *mut __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint, types::GLsizei, types::GLsizei, types::GLenum, types::GLenum, *mut __gl_imports::raw::c_void) -> ()>(self.ReadPixels.f)(x, y, width, height, format, type_, pixels) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ReleaseShaderCompiler(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.ReleaseShaderCompiler.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RenderbufferStorage(&self, target: types::GLenum, internalformat: types::GLenum, width: types::GLsizei, height: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLsizei, types::GLsizei) -> ()>(self.RenderbufferStorage.f)(target, internalformat, width, height) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn RenderbufferStorageMultisample(&self, target: types::GLenum, samples: types::GLsizei, internalformat: types::GLenum, width: types::GLsizei, height: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLsizei, types::GLenum, types::GLsizei, types::GLsizei) -> ()>(self.RenderbufferStorageMultisample.f)(target, samples, internalformat, width, height) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ResumeTransformFeedback(&self, ) -> () { __gl_imports::mem::transmute::<_, extern "system" fn() -> ()>(self.ResumeTransformFeedback.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SampleCoverage(&self, value: types::GLfloat, invert: types::GLboolean) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLfloat, types::GLboolean) -> ()>(self.SampleCoverage.f)(value, invert) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SampleMaski(&self, maskNumber: types::GLuint, mask: types::GLbitfield) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLbitfield) -> ()>(self.SampleMaski.f)(maskNumber, mask) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SamplerParameterf(&self, sampler: types::GLuint, pname: types::GLenum, param: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLfloat) -> ()>(self.SamplerParameterf.f)(sampler, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SamplerParameterfv(&self, sampler: types::GLuint, pname: types::GLenum, param: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *const types::GLfloat) -> ()>(self.SamplerParameterfv.f)(sampler, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SamplerParameteri(&self, sampler: types::GLuint, pname: types::GLenum, param: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLint) -> ()>(self.SamplerParameteri.f)(sampler, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SamplerParameteriv(&self, sampler: types::GLuint, pname: types::GLenum, param: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, *const types::GLint) -> ()>(self.SamplerParameteriv.f)(sampler, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Scissor(&self, x: types::GLint, y: types::GLint, width: types::GLsizei, height: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint, types::GLsizei, types::GLsizei) -> ()>(self.Scissor.f)(x, y, width, height) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ShaderBinary(&self, count: types::GLsizei, shaders: *const types::GLuint, binaryformat: types::GLenum, binary: *const __gl_imports::raw::c_void, length: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsizei, *const types::GLuint, types::GLenum, *const __gl_imports::raw::c_void, types::GLsizei) -> ()>(self.ShaderBinary.f)(count, shaders, binaryformat, binary, length) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ShaderSource(&self, shader: types::GLuint, count: types::GLsizei, string: *const *const types::GLchar, length: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLsizei, *const *const types::GLchar, *const types::GLint) -> ()>(self.ShaderSource.f)(shader, count, string, length) }
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
            #[inline] pub unsafe fn TexImage2D(&self, target: types::GLenum, level: types::GLint, internalformat: types::GLint, width: types::GLsizei, height: types::GLsizei, border: types::GLint, format: types::GLenum, type_: types::GLenum, pixels: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLint, types::GLsizei, types::GLsizei, types::GLint, types::GLenum, types::GLenum, *const __gl_imports::raw::c_void) -> ()>(self.TexImage2D.f)(target, level, internalformat, width, height, border, format, type_, pixels) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexImage3D(&self, target: types::GLenum, level: types::GLint, internalformat: types::GLint, width: types::GLsizei, height: types::GLsizei, depth: types::GLsizei, border: types::GLint, format: types::GLenum, type_: types::GLenum, pixels: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLint, types::GLsizei, types::GLsizei, types::GLsizei, types::GLint, types::GLenum, types::GLenum, *const __gl_imports::raw::c_void) -> ()>(self.TexImage3D.f)(target, level, internalformat, width, height, depth, border, format, type_, pixels) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexParameterf(&self, target: types::GLenum, pname: types::GLenum, param: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLfloat) -> ()>(self.TexParameterf.f)(target, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexParameterfv(&self, target: types::GLenum, pname: types::GLenum, params: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *const types::GLfloat) -> ()>(self.TexParameterfv.f)(target, pname, params) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexParameteri(&self, target: types::GLenum, pname: types::GLenum, param: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, types::GLint) -> ()>(self.TexParameteri.f)(target, pname, param) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexParameteriv(&self, target: types::GLenum, pname: types::GLenum, params: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLenum, *const types::GLint) -> ()>(self.TexParameteriv.f)(target, pname, params) }
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
            #[inline] pub unsafe fn TexSubImage2D(&self, target: types::GLenum, level: types::GLint, xoffset: types::GLint, yoffset: types::GLint, width: types::GLsizei, height: types::GLsizei, format: types::GLenum, type_: types::GLenum, pixels: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLint, types::GLint, types::GLsizei, types::GLsizei, types::GLenum, types::GLenum, *const __gl_imports::raw::c_void) -> ()>(self.TexSubImage2D.f)(target, level, xoffset, yoffset, width, height, format, type_, pixels) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TexSubImage3D(&self, target: types::GLenum, level: types::GLint, xoffset: types::GLint, yoffset: types::GLint, zoffset: types::GLint, width: types::GLsizei, height: types::GLsizei, depth: types::GLsizei, format: types::GLenum, type_: types::GLenum, pixels: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLenum, types::GLint, types::GLint, types::GLint, types::GLint, types::GLsizei, types::GLsizei, types::GLsizei, types::GLenum, types::GLenum, *const __gl_imports::raw::c_void) -> ()>(self.TexSubImage3D.f)(target, level, xoffset, yoffset, zoffset, width, height, depth, format, type_, pixels) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TextureStorage1DEXT(&self, texture: types::GLuint, target: types::GLenum, levels: types::GLsizei, internalformat: types::GLenum, width: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLsizei, types::GLenum, types::GLsizei) -> ()>(self.TextureStorage1DEXT.f)(texture, target, levels, internalformat, width) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TextureStorage2DEXT(&self, texture: types::GLuint, target: types::GLenum, levels: types::GLsizei, internalformat: types::GLenum, width: types::GLsizei, height: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLsizei, types::GLenum, types::GLsizei, types::GLsizei) -> ()>(self.TextureStorage2DEXT.f)(texture, target, levels, internalformat, width, height) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TextureStorage3DEXT(&self, texture: types::GLuint, target: types::GLenum, levels: types::GLsizei, internalformat: types::GLenum, width: types::GLsizei, height: types::GLsizei, depth: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLenum, types::GLsizei, types::GLenum, types::GLsizei, types::GLsizei, types::GLsizei) -> ()>(self.TextureStorage3DEXT.f)(texture, target, levels, internalformat, width, height, depth) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn TransformFeedbackVaryings(&self, program: types::GLuint, count: types::GLsizei, varyings: *const *const types::GLchar, bufferMode: types::GLenum) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLsizei, *const *const types::GLchar, types::GLenum) -> ()>(self.TransformFeedbackVaryings.f)(program, count, varyings, bufferMode) }
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
            #[inline] pub unsafe fn VertexAttrib1f(&self, index: types::GLuint, x: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLfloat) -> ()>(self.VertexAttrib1f.f)(index, x) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib1fv(&self, index: types::GLuint, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLfloat) -> ()>(self.VertexAttrib1fv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib2f(&self, index: types::GLuint, x: types::GLfloat, y: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLfloat, types::GLfloat) -> ()>(self.VertexAttrib2f.f)(index, x, y) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib2fv(&self, index: types::GLuint, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLfloat) -> ()>(self.VertexAttrib2fv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib3f(&self, index: types::GLuint, x: types::GLfloat, y: types::GLfloat, z: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.VertexAttrib3f.f)(index, x, y, z) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib3fv(&self, index: types::GLuint, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLfloat) -> ()>(self.VertexAttrib3fv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib4f(&self, index: types::GLuint, x: types::GLfloat, y: types::GLfloat, z: types::GLfloat, w: types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLfloat, types::GLfloat, types::GLfloat, types::GLfloat) -> ()>(self.VertexAttrib4f.f)(index, x, y, z, w) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttrib4fv(&self, index: types::GLuint, v: *const types::GLfloat) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLfloat) -> ()>(self.VertexAttrib4fv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribBinding(&self, attribindex: types::GLuint, bindingindex: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint) -> ()>(self.VertexAttribBinding.f)(attribindex, bindingindex) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribDivisor(&self, index: types::GLuint, divisor: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint) -> ()>(self.VertexAttribDivisor.f)(index, divisor) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribFormat(&self, attribindex: types::GLuint, size: types::GLint, type_: types::GLenum, normalized: types::GLboolean, relativeoffset: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLenum, types::GLboolean, types::GLuint) -> ()>(self.VertexAttribFormat.f)(attribindex, size, type_, normalized, relativeoffset) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribI4i(&self, index: types::GLuint, x: types::GLint, y: types::GLint, z: types::GLint, w: types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLint, types::GLint, types::GLint) -> ()>(self.VertexAttribI4i.f)(index, x, y, z, w) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribI4iv(&self, index: types::GLuint, v: *const types::GLint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLint) -> ()>(self.VertexAttribI4iv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribI4ui(&self, index: types::GLuint, x: types::GLuint, y: types::GLuint, z: types::GLuint, w: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint, types::GLuint, types::GLuint, types::GLuint) -> ()>(self.VertexAttribI4ui.f)(index, x, y, z, w) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribI4uiv(&self, index: types::GLuint, v: *const types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, *const types::GLuint) -> ()>(self.VertexAttribI4uiv.f)(index, v) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribIFormat(&self, attribindex: types::GLuint, size: types::GLint, type_: types::GLenum, relativeoffset: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLenum, types::GLuint) -> ()>(self.VertexAttribIFormat.f)(attribindex, size, type_, relativeoffset) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribIPointer(&self, index: types::GLuint, size: types::GLint, type_: types::GLenum, stride: types::GLsizei, pointer: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLenum, types::GLsizei, *const __gl_imports::raw::c_void) -> ()>(self.VertexAttribIPointer.f)(index, size, type_, stride, pointer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexAttribPointer(&self, index: types::GLuint, size: types::GLint, type_: types::GLenum, normalized: types::GLboolean, stride: types::GLsizei, pointer: *const __gl_imports::raw::c_void) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLint, types::GLenum, types::GLboolean, types::GLsizei, *const __gl_imports::raw::c_void) -> ()>(self.VertexAttribPointer.f)(index, size, type_, normalized, stride, pointer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn VertexBindingDivisor(&self, bindingindex: types::GLuint, divisor: types::GLuint) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLuint, types::GLuint) -> ()>(self.VertexBindingDivisor.f)(bindingindex, divisor) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Viewport(&self, x: types::GLint, y: types::GLint, width: types::GLsizei, height: types::GLsizei) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLint, types::GLint, types::GLsizei, types::GLsizei) -> ()>(self.Viewport.f)(x, y, width, height) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn WaitSync(&self, sync: types::GLsync, flags: types::GLbitfield, timeout: types::GLuint64) -> () { __gl_imports::mem::transmute::<_, extern "system" fn(types::GLsync, types::GLbitfield, types::GLuint64) -> ()>(self.WaitSync.f)(sync, flags, timeout) }
}

        unsafe impl __gl_imports::Send for Gles2 {}
