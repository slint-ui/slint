
        mod __gl_imports {
            pub use std::mem;
            pub use std::marker::Send;
            pub use std::os::raw;
        }
    

        pub mod types {
            #![allow(non_camel_case_types, non_snake_case, dead_code, missing_copy_implementations)]
    
// platform-specific aliases are unknown
// IMPORTANT: these are alises to the same level of the bindings
// the values must be defined by the user
#[allow(dead_code)]
pub type khronos_utime_nanoseconds_t = super::khronos_utime_nanoseconds_t;
#[allow(dead_code)]
pub type khronos_uint64_t = super::khronos_uint64_t;
#[allow(dead_code)]
pub type khronos_ssize_t = super::khronos_ssize_t;
pub type EGLNativeDisplayType = super::EGLNativeDisplayType;
#[allow(dead_code)]
pub type EGLNativePixmapType = super::EGLNativePixmapType;
#[allow(dead_code)]
pub type EGLNativeWindowType = super::EGLNativeWindowType;
pub type EGLint = super::EGLint;
#[allow(dead_code)]
pub type NativeDisplayType = super::NativeDisplayType;
#[allow(dead_code)]
pub type NativePixmapType = super::NativePixmapType;
#[allow(dead_code)]
pub type NativeWindowType = super::NativeWindowType;

// EGL alises
pub type Bool = EGLBoolean; // TODO: not sure
pub type EGLBoolean = super::__gl_imports::raw::c_uint;
pub type EGLenum = super::__gl_imports::raw::c_uint;
pub type EGLAttribKHR = isize;
pub type EGLAttrib = isize;
pub type EGLConfig = *const super::__gl_imports::raw::c_void;
pub type EGLContext = *const super::__gl_imports::raw::c_void;
pub type EGLDeviceEXT = *const super::__gl_imports::raw::c_void;
pub type EGLDisplay = *const super::__gl_imports::raw::c_void;
pub type EGLSurface = *const super::__gl_imports::raw::c_void;
pub type EGLClientBuffer = *const super::__gl_imports::raw::c_void;
pub enum __eglMustCastToProperFunctionPointerType_fn {}
pub type __eglMustCastToProperFunctionPointerType =
    *mut __eglMustCastToProperFunctionPointerType_fn;
pub type EGLImageKHR = *const super::__gl_imports::raw::c_void;
pub type EGLImage = *const super::__gl_imports::raw::c_void;
pub type EGLOutputLayerEXT = *const super::__gl_imports::raw::c_void;
pub type EGLOutputPortEXT = *const super::__gl_imports::raw::c_void;
pub type EGLSyncKHR = *const super::__gl_imports::raw::c_void;
pub type EGLSync = *const super::__gl_imports::raw::c_void;
pub type EGLTimeKHR = khronos_utime_nanoseconds_t;
pub type EGLTime = khronos_utime_nanoseconds_t;
pub type EGLSyncNV = *const super::__gl_imports::raw::c_void;
pub type EGLTimeNV = khronos_utime_nanoseconds_t;
pub type EGLuint64NV = khronos_utime_nanoseconds_t;
pub type EGLStreamKHR = *const super::__gl_imports::raw::c_void;
pub type EGLuint64KHR = khronos_uint64_t;
pub type EGLNativeFileDescriptorKHR = super::__gl_imports::raw::c_int;
pub type EGLsizeiANDROID = khronos_ssize_t;
pub type EGLSetBlobFuncANDROID = extern "system" fn(*const super::__gl_imports::raw::c_void,
                                                    EGLsizeiANDROID,
                                                    *const super::__gl_imports::raw::c_void,
                                                    EGLsizeiANDROID)
                                                    -> ();
pub type EGLGetBlobFuncANDROID = extern "system" fn(*const super::__gl_imports::raw::c_void,
                                                    EGLsizeiANDROID,
                                                    *mut super::__gl_imports::raw::c_void,
                                                    EGLsizeiANDROID)
                                                    -> EGLsizeiANDROID;

#[repr(C)]
pub struct EGLClientPixmapHI {
    pData: *const super::__gl_imports::raw::c_void,
    iWidth: EGLint,
    iHeight: EGLint,
    iStride: EGLint,
}

}
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA_FORMAT: types::EGLenum = 0x3088;
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA_FORMAT_NONPRE: types::EGLenum = 0x308B;
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA_FORMAT_PRE: types::EGLenum = 0x308C;
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA_MASK_SIZE: types::EGLenum = 0x303E;
#[allow(dead_code, non_upper_case_globals)] pub const ALPHA_SIZE: types::EGLenum = 0x3021;
#[allow(dead_code, non_upper_case_globals)] pub const BACK_BUFFER: types::EGLenum = 0x3084;
#[allow(dead_code, non_upper_case_globals)] pub const BAD_ACCESS: types::EGLenum = 0x3002;
#[allow(dead_code, non_upper_case_globals)] pub const BAD_ALLOC: types::EGLenum = 0x3003;
#[allow(dead_code, non_upper_case_globals)] pub const BAD_ATTRIBUTE: types::EGLenum = 0x3004;
#[allow(dead_code, non_upper_case_globals)] pub const BAD_CONFIG: types::EGLenum = 0x3005;
#[allow(dead_code, non_upper_case_globals)] pub const BAD_CONTEXT: types::EGLenum = 0x3006;
#[allow(dead_code, non_upper_case_globals)] pub const BAD_CURRENT_SURFACE: types::EGLenum = 0x3007;
#[allow(dead_code, non_upper_case_globals)] pub const BAD_DEVICE_EXT: types::EGLenum = 0x322B;
#[allow(dead_code, non_upper_case_globals)] pub const BAD_DISPLAY: types::EGLenum = 0x3008;
#[allow(dead_code, non_upper_case_globals)] pub const BAD_MATCH: types::EGLenum = 0x3009;
#[allow(dead_code, non_upper_case_globals)] pub const BAD_NATIVE_PIXMAP: types::EGLenum = 0x300A;
#[allow(dead_code, non_upper_case_globals)] pub const BAD_NATIVE_WINDOW: types::EGLenum = 0x300B;
#[allow(dead_code, non_upper_case_globals)] pub const BAD_PARAMETER: types::EGLenum = 0x300C;
#[allow(dead_code, non_upper_case_globals)] pub const BAD_SURFACE: types::EGLenum = 0x300D;
#[allow(dead_code, non_upper_case_globals)] pub const BIND_TO_TEXTURE_RGB: types::EGLenum = 0x3039;
#[allow(dead_code, non_upper_case_globals)] pub const BIND_TO_TEXTURE_RGBA: types::EGLenum = 0x303A;
#[allow(dead_code, non_upper_case_globals)] pub const BLUE_SIZE: types::EGLenum = 0x3022;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_AGE_EXT: types::EGLenum = 0x313D;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_DESTROYED: types::EGLenum = 0x3095;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_PRESERVED: types::EGLenum = 0x3094;
#[allow(dead_code, non_upper_case_globals)] pub const BUFFER_SIZE: types::EGLenum = 0x3020;
#[allow(dead_code, non_upper_case_globals)] pub const CLIENT_APIS: types::EGLenum = 0x308D;
#[allow(dead_code, non_upper_case_globals)] pub const CL_EVENT_HANDLE: types::EGLenum = 0x309C;
#[allow(dead_code, non_upper_case_globals)] pub const COLORSPACE: types::EGLenum = 0x3087;
#[allow(dead_code, non_upper_case_globals)] pub const COLORSPACE_LINEAR: types::EGLenum = 0x308A;
#[allow(dead_code, non_upper_case_globals)] pub const COLORSPACE_sRGB: types::EGLenum = 0x3089;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_BUFFER_TYPE: types::EGLenum = 0x303F;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_COMPONENT_TYPE_EXT: types::EGLenum = 0x3339;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_COMPONENT_TYPE_FIXED_EXT: types::EGLenum = 0x333A;
#[allow(dead_code, non_upper_case_globals)] pub const COLOR_COMPONENT_TYPE_FLOAT_EXT: types::EGLenum = 0x333B;
#[allow(dead_code, non_upper_case_globals)] pub const CONDITION_SATISFIED: types::EGLenum = 0x30F6;
#[allow(dead_code, non_upper_case_globals)] pub const CONFIG_CAVEAT: types::EGLenum = 0x3027;
#[allow(dead_code, non_upper_case_globals)] pub const CONFIG_ID: types::EGLenum = 0x3028;
#[allow(dead_code, non_upper_case_globals)] pub const CONFORMANT: types::EGLenum = 0x3042;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_CLIENT_TYPE: types::EGLenum = 0x3097;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_CLIENT_VERSION: types::EGLenum = 0x3098;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_FLAGS_KHR: types::EGLenum = 0x30FC;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_LOST: types::EGLenum = 0x300E;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_MAJOR_VERSION: types::EGLenum = 0x3098;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_MAJOR_VERSION_KHR: types::EGLenum = 0x3098;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_MINOR_VERSION: types::EGLenum = 0x30FB;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_MINOR_VERSION_KHR: types::EGLenum = 0x30FB;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_OPENGL_COMPATIBILITY_PROFILE_BIT: types::EGLenum = 0x00000002;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_OPENGL_COMPATIBILITY_PROFILE_BIT_KHR: types::EGLenum = 0x00000002;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_OPENGL_CORE_PROFILE_BIT: types::EGLenum = 0x00000001;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_OPENGL_CORE_PROFILE_BIT_KHR: types::EGLenum = 0x00000001;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_OPENGL_DEBUG: types::EGLenum = 0x31B0;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_OPENGL_DEBUG_BIT_KHR: types::EGLenum = 0x00000001;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_OPENGL_FORWARD_COMPATIBLE: types::EGLenum = 0x31B1;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_OPENGL_FORWARD_COMPATIBLE_BIT_KHR: types::EGLenum = 0x00000002;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_OPENGL_NO_ERROR_KHR: types::EGLenum = 0x31B3;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_OPENGL_PROFILE_MASK: types::EGLenum = 0x30FD;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_OPENGL_PROFILE_MASK_KHR: types::EGLenum = 0x30FD;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_OPENGL_RESET_NOTIFICATION_STRATEGY: types::EGLenum = 0x31BD;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_OPENGL_RESET_NOTIFICATION_STRATEGY_EXT: types::EGLenum = 0x3138;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_OPENGL_RESET_NOTIFICATION_STRATEGY_KHR: types::EGLenum = 0x31BD;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_OPENGL_ROBUST_ACCESS: types::EGLenum = 0x31B2;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_OPENGL_ROBUST_ACCESS_BIT_KHR: types::EGLenum = 0x00000004;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_OPENGL_ROBUST_ACCESS_EXT: types::EGLenum = 0x30BF;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_PRIORITY_HIGH_IMG: types::EGLenum = 0x3101;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_PRIORITY_LEVEL_IMG: types::EGLenum = 0x3100;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_PRIORITY_LOW_IMG: types::EGLenum = 0x3103;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_PRIORITY_MEDIUM_IMG: types::EGLenum = 0x3102;
#[allow(dead_code, non_upper_case_globals)] pub const CONTEXT_PRIORITY_REALTIME_NV: types::EGLenum = 0x3357;
#[allow(dead_code, non_upper_case_globals)] pub const CORE_NATIVE_ENGINE: types::EGLenum = 0x305B;
#[allow(dead_code, non_upper_case_globals)] pub const DEFAULT_DISPLAY: types::EGLNativeDisplayType = 0 as types::EGLNativeDisplayType;
#[allow(dead_code, non_upper_case_globals)] pub const DEPTH_SIZE: types::EGLenum = 0x3025;
#[allow(dead_code, non_upper_case_globals)] pub const DEVICE_EXT: types::EGLenum = 0x322C;
#[allow(dead_code, non_upper_case_globals)] pub const DISPLAY_SCALING: types::EGLenum = 10000;
#[allow(dead_code, non_upper_case_globals)] pub const DONT_CARE: types::EGLint = -1 as types::EGLint;
#[allow(dead_code, non_upper_case_globals)] pub const DRAW: types::EGLenum = 0x3059;
#[allow(dead_code, non_upper_case_globals)] pub const DRM_DEVICE_FILE_EXT: types::EGLenum = 0x3233;
#[allow(dead_code, non_upper_case_globals)] pub const DRM_MASTER_FD_EXT: types::EGLenum = 0x333C;
#[allow(dead_code, non_upper_case_globals)] pub const EXTENSIONS: types::EGLenum = 0x3055;
#[allow(dead_code, non_upper_case_globals)] pub const FALSE: types::EGLBoolean = 0;
#[allow(dead_code, non_upper_case_globals)] pub const FOREVER: types::EGLuint64KHR = 0xFFFFFFFFFFFFFFFF;
#[allow(dead_code, non_upper_case_globals)] pub const GL_COLORSPACE: types::EGLenum = 0x309D;
#[allow(dead_code, non_upper_case_globals)] pub const GL_COLORSPACE_LINEAR: types::EGLenum = 0x308A;
#[allow(dead_code, non_upper_case_globals)] pub const GL_COLORSPACE_SRGB: types::EGLenum = 0x3089;
#[allow(dead_code, non_upper_case_globals)] pub const GL_RENDERBUFFER: types::EGLenum = 0x30B9;
#[allow(dead_code, non_upper_case_globals)] pub const GL_TEXTURE_2D: types::EGLenum = 0x30B1;
#[allow(dead_code, non_upper_case_globals)] pub const GL_TEXTURE_3D: types::EGLenum = 0x30B2;
#[allow(dead_code, non_upper_case_globals)] pub const GL_TEXTURE_CUBE_MAP_NEGATIVE_X: types::EGLenum = 0x30B4;
#[allow(dead_code, non_upper_case_globals)] pub const GL_TEXTURE_CUBE_MAP_NEGATIVE_Y: types::EGLenum = 0x30B6;
#[allow(dead_code, non_upper_case_globals)] pub const GL_TEXTURE_CUBE_MAP_NEGATIVE_Z: types::EGLenum = 0x30B8;
#[allow(dead_code, non_upper_case_globals)] pub const GL_TEXTURE_CUBE_MAP_POSITIVE_X: types::EGLenum = 0x30B3;
#[allow(dead_code, non_upper_case_globals)] pub const GL_TEXTURE_CUBE_MAP_POSITIVE_Y: types::EGLenum = 0x30B5;
#[allow(dead_code, non_upper_case_globals)] pub const GL_TEXTURE_CUBE_MAP_POSITIVE_Z: types::EGLenum = 0x30B7;
#[allow(dead_code, non_upper_case_globals)] pub const GL_TEXTURE_LEVEL: types::EGLenum = 0x30BC;
#[allow(dead_code, non_upper_case_globals)] pub const GL_TEXTURE_ZOFFSET: types::EGLenum = 0x30BD;
#[allow(dead_code, non_upper_case_globals)] pub const GREEN_SIZE: types::EGLenum = 0x3023;
#[allow(dead_code, non_upper_case_globals)] pub const HEIGHT: types::EGLenum = 0x3056;
#[allow(dead_code, non_upper_case_globals)] pub const HORIZONTAL_RESOLUTION: types::EGLenum = 0x3090;
#[allow(dead_code, non_upper_case_globals)] pub const IMAGE_PRESERVED: types::EGLenum = 0x30D2;
#[allow(dead_code, non_upper_case_globals)] pub const IMAGE_PRESERVED_KHR: types::EGLenum = 0x30D2;
#[allow(dead_code, non_upper_case_globals)] pub const LARGEST_PBUFFER: types::EGLenum = 0x3058;
#[allow(dead_code, non_upper_case_globals)] pub const LEVEL: types::EGLenum = 0x3029;
#[allow(dead_code, non_upper_case_globals)] pub const LOSE_CONTEXT_ON_RESET: types::EGLenum = 0x31BF;
#[allow(dead_code, non_upper_case_globals)] pub const LOSE_CONTEXT_ON_RESET_EXT: types::EGLenum = 0x31BF;
#[allow(dead_code, non_upper_case_globals)] pub const LOSE_CONTEXT_ON_RESET_KHR: types::EGLenum = 0x31BF;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE_BUFFER: types::EGLenum = 0x308F;
#[allow(dead_code, non_upper_case_globals)] pub const LUMINANCE_SIZE: types::EGLenum = 0x303D;
#[allow(dead_code, non_upper_case_globals)] pub const MATCH_NATIVE_PIXMAP: types::EGLenum = 0x3041;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_PBUFFER_HEIGHT: types::EGLenum = 0x302A;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_PBUFFER_PIXELS: types::EGLenum = 0x302B;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_PBUFFER_WIDTH: types::EGLenum = 0x302C;
#[allow(dead_code, non_upper_case_globals)] pub const MAX_SWAP_INTERVAL: types::EGLenum = 0x303C;
#[allow(dead_code, non_upper_case_globals)] pub const MIN_SWAP_INTERVAL: types::EGLenum = 0x303B;
#[allow(dead_code, non_upper_case_globals)] pub const MIPMAP_LEVEL: types::EGLenum = 0x3083;
#[allow(dead_code, non_upper_case_globals)] pub const MIPMAP_TEXTURE: types::EGLenum = 0x3082;
#[allow(dead_code, non_upper_case_globals)] pub const MULTISAMPLE_RESOLVE: types::EGLenum = 0x3099;
#[allow(dead_code, non_upper_case_globals)] pub const MULTISAMPLE_RESOLVE_BOX: types::EGLenum = 0x309B;
#[allow(dead_code, non_upper_case_globals)] pub const MULTISAMPLE_RESOLVE_BOX_BIT: types::EGLenum = 0x0200;
#[allow(dead_code, non_upper_case_globals)] pub const MULTISAMPLE_RESOLVE_DEFAULT: types::EGLenum = 0x309A;
#[allow(dead_code, non_upper_case_globals)] pub const NATIVE_RENDERABLE: types::EGLenum = 0x302D;
#[allow(dead_code, non_upper_case_globals)] pub const NATIVE_VISUAL_ID: types::EGLenum = 0x302E;
#[allow(dead_code, non_upper_case_globals)] pub const NATIVE_VISUAL_TYPE: types::EGLenum = 0x302F;
#[allow(dead_code, non_upper_case_globals)] pub const NONE: types::EGLenum = 0x3038;
#[allow(dead_code, non_upper_case_globals)] pub const NON_CONFORMANT_CONFIG: types::EGLenum = 0x3051;
#[allow(dead_code, non_upper_case_globals)] pub const NOT_INITIALIZED: types::EGLenum = 0x3001;
#[allow(dead_code, non_upper_case_globals)] pub const NO_CONTEXT: types::EGLContext = 0 as types::EGLContext;
#[allow(dead_code, non_upper_case_globals)] pub const NO_DEVICE_EXT: types::EGLDeviceEXT = 0 as types::EGLDeviceEXT;
#[allow(dead_code, non_upper_case_globals)] pub const NO_DISPLAY: types::EGLDisplay = 0 as types::EGLDisplay;
#[allow(dead_code, non_upper_case_globals)] pub const NO_IMAGE: types::EGLImage = 0 as types::EGLImage;
#[allow(dead_code, non_upper_case_globals)] pub const NO_IMAGE_KHR: types::EGLImageKHR = 0 as types::EGLImageKHR;
#[allow(dead_code, non_upper_case_globals)] pub const NO_NATIVE_FENCE_FD_ANDROID: types::EGLint = -1;
#[allow(dead_code, non_upper_case_globals)] pub const NO_RESET_NOTIFICATION: types::EGLenum = 0x31BE;
#[allow(dead_code, non_upper_case_globals)] pub const NO_RESET_NOTIFICATION_EXT: types::EGLenum = 0x31BE;
#[allow(dead_code, non_upper_case_globals)] pub const NO_RESET_NOTIFICATION_KHR: types::EGLenum = 0x31BE;
#[allow(dead_code, non_upper_case_globals)] pub const NO_SURFACE: types::EGLSurface = 0 as types::EGLSurface;
#[allow(dead_code, non_upper_case_globals)] pub const NO_SYNC: types::EGLSync = 0 as types::EGLSync;
#[allow(dead_code, non_upper_case_globals)] pub const NO_TEXTURE: types::EGLenum = 0x305C;
#[allow(dead_code, non_upper_case_globals)] pub const OPENGL_API: types::EGLenum = 0x30A2;
#[allow(dead_code, non_upper_case_globals)] pub const OPENGL_BIT: types::EGLenum = 0x0008;
#[allow(dead_code, non_upper_case_globals)] pub const OPENGL_ES2_BIT: types::EGLenum = 0x0004;
#[allow(dead_code, non_upper_case_globals)] pub const OPENGL_ES3_BIT: types::EGLenum = 0x00000040;
#[allow(dead_code, non_upper_case_globals)] pub const OPENGL_ES3_BIT_KHR: types::EGLenum = 0x00000040;
#[allow(dead_code, non_upper_case_globals)] pub const OPENGL_ES_API: types::EGLenum = 0x30A0;
#[allow(dead_code, non_upper_case_globals)] pub const OPENGL_ES_BIT: types::EGLenum = 0x0001;
#[allow(dead_code, non_upper_case_globals)] pub const OPENVG_API: types::EGLenum = 0x30A1;
#[allow(dead_code, non_upper_case_globals)] pub const OPENVG_BIT: types::EGLenum = 0x0002;
#[allow(dead_code, non_upper_case_globals)] pub const OPENVG_IMAGE: types::EGLenum = 0x3096;
#[allow(dead_code, non_upper_case_globals)] pub const PBUFFER_BIT: types::EGLenum = 0x0001;
#[allow(dead_code, non_upper_case_globals)] pub const PIXEL_ASPECT_RATIO: types::EGLenum = 0x3092;
#[allow(dead_code, non_upper_case_globals)] pub const PIXMAP_BIT: types::EGLenum = 0x0002;
#[allow(dead_code, non_upper_case_globals)] pub const PLATFORM_ANDROID_KHR: types::EGLenum = 0x3141;
#[allow(dead_code, non_upper_case_globals)] pub const PLATFORM_DEVICE_EXT: types::EGLenum = 0x313F;
#[allow(dead_code, non_upper_case_globals)] pub const PLATFORM_GBM_KHR: types::EGLenum = 0x31D7;
#[allow(dead_code, non_upper_case_globals)] pub const PLATFORM_GBM_MESA: types::EGLenum = 0x31D7;
#[allow(dead_code, non_upper_case_globals)] pub const PLATFORM_WAYLAND_EXT: types::EGLenum = 0x31D8;
#[allow(dead_code, non_upper_case_globals)] pub const PLATFORM_WAYLAND_KHR: types::EGLenum = 0x31D8;
#[allow(dead_code, non_upper_case_globals)] pub const PLATFORM_X11_EXT: types::EGLenum = 0x31D5;
#[allow(dead_code, non_upper_case_globals)] pub const PLATFORM_X11_KHR: types::EGLenum = 0x31D5;
#[allow(dead_code, non_upper_case_globals)] pub const PLATFORM_X11_SCREEN_EXT: types::EGLenum = 0x31D6;
#[allow(dead_code, non_upper_case_globals)] pub const PLATFORM_X11_SCREEN_KHR: types::EGLenum = 0x31D6;
#[allow(dead_code, non_upper_case_globals)] pub const READ: types::EGLenum = 0x305A;
#[allow(dead_code, non_upper_case_globals)] pub const RED_SIZE: types::EGLenum = 0x3024;
#[allow(dead_code, non_upper_case_globals)] pub const RENDERABLE_TYPE: types::EGLenum = 0x3040;
#[allow(dead_code, non_upper_case_globals)] pub const RENDER_BUFFER: types::EGLenum = 0x3086;
#[allow(dead_code, non_upper_case_globals)] pub const RGB_BUFFER: types::EGLenum = 0x308E;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLES: types::EGLenum = 0x3031;
#[allow(dead_code, non_upper_case_globals)] pub const SAMPLE_BUFFERS: types::EGLenum = 0x3032;
#[allow(dead_code, non_upper_case_globals)] pub const SIGNALED: types::EGLenum = 0x30F2;
#[allow(dead_code, non_upper_case_globals)] pub const SINGLE_BUFFER: types::EGLenum = 0x3085;
#[allow(dead_code, non_upper_case_globals)] pub const SLOW_CONFIG: types::EGLenum = 0x3050;
#[allow(dead_code, non_upper_case_globals)] pub const STENCIL_SIZE: types::EGLenum = 0x3026;
#[allow(dead_code, non_upper_case_globals)] pub const SUCCESS: types::EGLenum = 0x3000;
#[allow(dead_code, non_upper_case_globals)] pub const SURFACE_TYPE: types::EGLenum = 0x3033;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_BEHAVIOR: types::EGLenum = 0x3093;
#[allow(dead_code, non_upper_case_globals)] pub const SWAP_BEHAVIOR_PRESERVED_BIT: types::EGLenum = 0x0400;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_CL_EVENT: types::EGLenum = 0x30FE;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_CL_EVENT_COMPLETE: types::EGLenum = 0x30FF;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_CONDITION: types::EGLenum = 0x30F8;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_CONDITION_KHR: types::EGLenum = 0x30F8;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_FENCE: types::EGLenum = 0x30F9;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_FENCE_KHR: types::EGLenum = 0x30F9;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_FLUSH_COMMANDS_BIT: types::EGLenum = 0x0001;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_NATIVE_FENCE_ANDROID: types::EGLenum = 0x3144;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_NATIVE_FENCE_FD_ANDROID: types::EGLenum = 0x3145;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_NATIVE_FENCE_SIGNALED_ANDROID: types::EGLenum = 0x3146;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_PRIOR_COMMANDS_COMPLETE: types::EGLenum = 0x30F0;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_PRIOR_COMMANDS_COMPLETE_KHR: types::EGLenum = 0x30F0;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_STATUS: types::EGLenum = 0x30F1;
#[allow(dead_code, non_upper_case_globals)] pub const SYNC_TYPE: types::EGLenum = 0x30F7;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_2D: types::EGLenum = 0x305F;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_FORMAT: types::EGLenum = 0x3080;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_RGB: types::EGLenum = 0x305D;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_RGBA: types::EGLenum = 0x305E;
#[allow(dead_code, non_upper_case_globals)] pub const TEXTURE_TARGET: types::EGLenum = 0x3081;
#[allow(dead_code, non_upper_case_globals)] pub const TIMEOUT_EXPIRED: types::EGLenum = 0x30F5;
#[allow(dead_code, non_upper_case_globals)] pub const TRACK_REFERENCES_KHR: types::EGLenum = 0x3352;
#[allow(dead_code, non_upper_case_globals)] pub const TRANSPARENT_BLUE_VALUE: types::EGLenum = 0x3035;
#[allow(dead_code, non_upper_case_globals)] pub const TRANSPARENT_GREEN_VALUE: types::EGLenum = 0x3036;
#[allow(dead_code, non_upper_case_globals)] pub const TRANSPARENT_RED_VALUE: types::EGLenum = 0x3037;
#[allow(dead_code, non_upper_case_globals)] pub const TRANSPARENT_RGB: types::EGLenum = 0x3052;
#[allow(dead_code, non_upper_case_globals)] pub const TRANSPARENT_TYPE: types::EGLenum = 0x3034;
#[allow(dead_code, non_upper_case_globals)] pub const TRUE: types::EGLBoolean = 1;
#[allow(dead_code, non_upper_case_globals)] pub const UNKNOWN: types::EGLint = -1 as types::EGLint;
#[allow(dead_code, non_upper_case_globals)] pub const UNSIGNALED: types::EGLenum = 0x30F3;
#[allow(dead_code, non_upper_case_globals)] pub const VENDOR: types::EGLenum = 0x3053;
#[allow(dead_code, non_upper_case_globals)] pub const VERSION: types::EGLenum = 0x3054;
#[allow(dead_code, non_upper_case_globals)] pub const VERTICAL_RESOLUTION: types::EGLenum = 0x3091;
#[allow(dead_code, non_upper_case_globals)] pub const VG_ALPHA_FORMAT: types::EGLenum = 0x3088;
#[allow(dead_code, non_upper_case_globals)] pub const VG_ALPHA_FORMAT_NONPRE: types::EGLenum = 0x308B;
#[allow(dead_code, non_upper_case_globals)] pub const VG_ALPHA_FORMAT_PRE: types::EGLenum = 0x308C;
#[allow(dead_code, non_upper_case_globals)] pub const VG_ALPHA_FORMAT_PRE_BIT: types::EGLenum = 0x0040;
#[allow(dead_code, non_upper_case_globals)] pub const VG_COLORSPACE: types::EGLenum = 0x3087;
#[allow(dead_code, non_upper_case_globals)] pub const VG_COLORSPACE_LINEAR: types::EGLenum = 0x308A;
#[allow(dead_code, non_upper_case_globals)] pub const VG_COLORSPACE_LINEAR_BIT: types::EGLenum = 0x0020;
#[allow(dead_code, non_upper_case_globals)] pub const VG_COLORSPACE_sRGB: types::EGLenum = 0x3089;
#[allow(dead_code, non_upper_case_globals)] pub const WIDTH: types::EGLenum = 0x3057;
#[allow(dead_code, non_upper_case_globals)] pub const WINDOW_BIT: types::EGLenum = 0x0004;

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
            panic!("egl function was not loaded")
        }

        #[allow(non_camel_case_types, non_snake_case, dead_code)]
        #[derive(Clone)]
        pub struct Egl {
pub BindAPI: FnPtr,
pub BindTexImage: FnPtr,
pub ChooseConfig: FnPtr,
/// Fallbacks: ClientWaitSyncKHR
pub ClientWaitSync: FnPtr,
pub ClientWaitSyncKHR: FnPtr,
pub CopyBuffers: FnPtr,
pub CreateContext: FnPtr,
pub CreateImage: FnPtr,
pub CreateImageKHR: FnPtr,
pub CreatePbufferFromClientBuffer: FnPtr,
pub CreatePbufferSurface: FnPtr,
pub CreatePixmapSurface: FnPtr,
pub CreatePlatformPixmapSurface: FnPtr,
pub CreatePlatformPixmapSurfaceEXT: FnPtr,
pub CreatePlatformWindowSurface: FnPtr,
pub CreatePlatformWindowSurfaceEXT: FnPtr,
/// Fallbacks: CreateSync64KHR
pub CreateSync: FnPtr,
pub CreateSyncKHR: FnPtr,
pub CreateWindowSurface: FnPtr,
pub DestroyContext: FnPtr,
/// Fallbacks: DestroyImageKHR
pub DestroyImage: FnPtr,
pub DestroyImageKHR: FnPtr,
pub DestroySurface: FnPtr,
/// Fallbacks: DestroySyncKHR
pub DestroySync: FnPtr,
pub DestroySyncKHR: FnPtr,
pub DupNativeFenceFDANDROID: FnPtr,
pub GetConfigAttrib: FnPtr,
pub GetConfigs: FnPtr,
pub GetCurrentContext: FnPtr,
pub GetCurrentDisplay: FnPtr,
pub GetCurrentSurface: FnPtr,
pub GetDisplay: FnPtr,
pub GetError: FnPtr,
pub GetPlatformDisplay: FnPtr,
pub GetPlatformDisplayEXT: FnPtr,
pub GetProcAddress: FnPtr,
pub GetSyncAttrib: FnPtr,
pub GetSyncAttribKHR: FnPtr,
pub Initialize: FnPtr,
pub MakeCurrent: FnPtr,
pub QueryAPI: FnPtr,
pub QueryContext: FnPtr,
pub QueryDeviceAttribEXT: FnPtr,
pub QueryDeviceStringEXT: FnPtr,
pub QueryDevicesEXT: FnPtr,
pub QueryDisplayAttribEXT: FnPtr,
/// Fallbacks: QueryDisplayAttribEXT, QueryDisplayAttribNV
pub QueryDisplayAttribKHR: FnPtr,
pub QueryString: FnPtr,
pub QuerySurface: FnPtr,
pub ReleaseTexImage: FnPtr,
pub ReleaseThread: FnPtr,
pub SurfaceAttrib: FnPtr,
pub SwapBuffers: FnPtr,
pub SwapBuffersWithDamageEXT: FnPtr,
pub SwapBuffersWithDamageKHR: FnPtr,
pub SwapInterval: FnPtr,
pub Terminate: FnPtr,
pub WaitClient: FnPtr,
pub WaitGL: FnPtr,
pub WaitNative: FnPtr,
pub WaitSync: FnPtr,
pub WaitSyncKHR: FnPtr,
_priv: ()
}
impl Egl {
            /// Load each OpenGL symbol using a custom load function. This allows for the
            /// use of functions like `glfwGetProcAddress` or `SDL_GL_GetProcAddress`.
            ///
            /// ~~~ignore
            /// let gl = Gl::load_with(|s| glfw.get_proc_address(s));
            /// ~~~
            #[allow(dead_code, unused_variables)]
            pub fn load_with<F>(mut loadfn: F) -> Egl where F: FnMut(&'static str) -> *const __gl_imports::raw::c_void {
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
                Egl {
BindAPI: FnPtr::new(metaloadfn("eglBindAPI", &[])),
BindTexImage: FnPtr::new(metaloadfn("eglBindTexImage", &[])),
ChooseConfig: FnPtr::new(metaloadfn("eglChooseConfig", &[])),
ClientWaitSync: FnPtr::new(metaloadfn("eglClientWaitSync", &["eglClientWaitSyncKHR"])),
ClientWaitSyncKHR: FnPtr::new(metaloadfn("eglClientWaitSyncKHR", &[])),
CopyBuffers: FnPtr::new(metaloadfn("eglCopyBuffers", &[])),
CreateContext: FnPtr::new(metaloadfn("eglCreateContext", &[])),
CreateImage: FnPtr::new(metaloadfn("eglCreateImage", &[])),
CreateImageKHR: FnPtr::new(metaloadfn("eglCreateImageKHR", &[])),
CreatePbufferFromClientBuffer: FnPtr::new(metaloadfn("eglCreatePbufferFromClientBuffer", &[])),
CreatePbufferSurface: FnPtr::new(metaloadfn("eglCreatePbufferSurface", &[])),
CreatePixmapSurface: FnPtr::new(metaloadfn("eglCreatePixmapSurface", &[])),
CreatePlatformPixmapSurface: FnPtr::new(metaloadfn("eglCreatePlatformPixmapSurface", &[])),
CreatePlatformPixmapSurfaceEXT: FnPtr::new(metaloadfn("eglCreatePlatformPixmapSurfaceEXT", &[])),
CreatePlatformWindowSurface: FnPtr::new(metaloadfn("eglCreatePlatformWindowSurface", &[])),
CreatePlatformWindowSurfaceEXT: FnPtr::new(metaloadfn("eglCreatePlatformWindowSurfaceEXT", &[])),
CreateSync: FnPtr::new(metaloadfn("eglCreateSync", &["eglCreateSync64KHR"])),
CreateSyncKHR: FnPtr::new(metaloadfn("eglCreateSyncKHR", &[])),
CreateWindowSurface: FnPtr::new(metaloadfn("eglCreateWindowSurface", &[])),
DestroyContext: FnPtr::new(metaloadfn("eglDestroyContext", &[])),
DestroyImage: FnPtr::new(metaloadfn("eglDestroyImage", &["eglDestroyImageKHR"])),
DestroyImageKHR: FnPtr::new(metaloadfn("eglDestroyImageKHR", &[])),
DestroySurface: FnPtr::new(metaloadfn("eglDestroySurface", &[])),
DestroySync: FnPtr::new(metaloadfn("eglDestroySync", &["eglDestroySyncKHR"])),
DestroySyncKHR: FnPtr::new(metaloadfn("eglDestroySyncKHR", &[])),
DupNativeFenceFDANDROID: FnPtr::new(metaloadfn("eglDupNativeFenceFDANDROID", &[])),
GetConfigAttrib: FnPtr::new(metaloadfn("eglGetConfigAttrib", &[])),
GetConfigs: FnPtr::new(metaloadfn("eglGetConfigs", &[])),
GetCurrentContext: FnPtr::new(metaloadfn("eglGetCurrentContext", &[])),
GetCurrentDisplay: FnPtr::new(metaloadfn("eglGetCurrentDisplay", &[])),
GetCurrentSurface: FnPtr::new(metaloadfn("eglGetCurrentSurface", &[])),
GetDisplay: FnPtr::new(metaloadfn("eglGetDisplay", &[])),
GetError: FnPtr::new(metaloadfn("eglGetError", &[])),
GetPlatformDisplay: FnPtr::new(metaloadfn("eglGetPlatformDisplay", &[])),
GetPlatformDisplayEXT: FnPtr::new(metaloadfn("eglGetPlatformDisplayEXT", &[])),
GetProcAddress: FnPtr::new(metaloadfn("eglGetProcAddress", &[])),
GetSyncAttrib: FnPtr::new(metaloadfn("eglGetSyncAttrib", &[])),
GetSyncAttribKHR: FnPtr::new(metaloadfn("eglGetSyncAttribKHR", &[])),
Initialize: FnPtr::new(metaloadfn("eglInitialize", &[])),
MakeCurrent: FnPtr::new(metaloadfn("eglMakeCurrent", &[])),
QueryAPI: FnPtr::new(metaloadfn("eglQueryAPI", &[])),
QueryContext: FnPtr::new(metaloadfn("eglQueryContext", &[])),
QueryDeviceAttribEXT: FnPtr::new(metaloadfn("eglQueryDeviceAttribEXT", &[])),
QueryDeviceStringEXT: FnPtr::new(metaloadfn("eglQueryDeviceStringEXT", &[])),
QueryDevicesEXT: FnPtr::new(metaloadfn("eglQueryDevicesEXT", &[])),
QueryDisplayAttribEXT: FnPtr::new(metaloadfn("eglQueryDisplayAttribEXT", &[])),
QueryDisplayAttribKHR: FnPtr::new(metaloadfn("eglQueryDisplayAttribKHR", &["eglQueryDisplayAttribEXT", "eglQueryDisplayAttribNV"])),
QueryString: FnPtr::new(metaloadfn("eglQueryString", &[])),
QuerySurface: FnPtr::new(metaloadfn("eglQuerySurface", &[])),
ReleaseTexImage: FnPtr::new(metaloadfn("eglReleaseTexImage", &[])),
ReleaseThread: FnPtr::new(metaloadfn("eglReleaseThread", &[])),
SurfaceAttrib: FnPtr::new(metaloadfn("eglSurfaceAttrib", &[])),
SwapBuffers: FnPtr::new(metaloadfn("eglSwapBuffers", &[])),
SwapBuffersWithDamageEXT: FnPtr::new(metaloadfn("eglSwapBuffersWithDamageEXT", &[])),
SwapBuffersWithDamageKHR: FnPtr::new(metaloadfn("eglSwapBuffersWithDamageKHR", &[])),
SwapInterval: FnPtr::new(metaloadfn("eglSwapInterval", &[])),
Terminate: FnPtr::new(metaloadfn("eglTerminate", &[])),
WaitClient: FnPtr::new(metaloadfn("eglWaitClient", &[])),
WaitGL: FnPtr::new(metaloadfn("eglWaitGL", &[])),
WaitNative: FnPtr::new(metaloadfn("eglWaitNative", &[])),
WaitSync: FnPtr::new(metaloadfn("eglWaitSync", &[])),
WaitSyncKHR: FnPtr::new(metaloadfn("eglWaitSyncKHR", &[])),
_priv: ()
}
        }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BindAPI(&self, api: types::EGLenum) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLenum) -> types::EGLBoolean>(self.BindAPI.f)(api) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn BindTexImage(&self, dpy: types::EGLDisplay, surface: types::EGLSurface, buffer: types::EGLint) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLSurface, types::EGLint) -> types::EGLBoolean>(self.BindTexImage.f)(dpy, surface, buffer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ChooseConfig(&self, dpy: types::EGLDisplay, attrib_list: *const types::EGLint, configs: *mut types::EGLConfig, config_size: types::EGLint, num_config: *mut types::EGLint) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, *const types::EGLint, *mut types::EGLConfig, types::EGLint, *mut types::EGLint) -> types::EGLBoolean>(self.ChooseConfig.f)(dpy, attrib_list, configs, config_size, num_config) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ClientWaitSync(&self, dpy: types::EGLDisplay, sync: types::EGLSync, flags: types::EGLint, timeout: types::EGLTime) -> types::EGLint { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLSync, types::EGLint, types::EGLTime) -> types::EGLint>(self.ClientWaitSync.f)(dpy, sync, flags, timeout) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ClientWaitSyncKHR(&self, dpy: types::EGLDisplay, sync: types::EGLSyncKHR, flags: types::EGLint, timeout: types::EGLTimeKHR) -> types::EGLint { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLSyncKHR, types::EGLint, types::EGLTimeKHR) -> types::EGLint>(self.ClientWaitSyncKHR.f)(dpy, sync, flags, timeout) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CopyBuffers(&self, dpy: types::EGLDisplay, surface: types::EGLSurface, target: types::EGLNativePixmapType) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLSurface, types::EGLNativePixmapType) -> types::EGLBoolean>(self.CopyBuffers.f)(dpy, surface, target) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CreateContext(&self, dpy: types::EGLDisplay, config: types::EGLConfig, share_context: types::EGLContext, attrib_list: *const types::EGLint) -> types::EGLContext { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLConfig, types::EGLContext, *const types::EGLint) -> types::EGLContext>(self.CreateContext.f)(dpy, config, share_context, attrib_list) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CreateImage(&self, dpy: types::EGLDisplay, ctx: types::EGLContext, target: types::EGLenum, buffer: types::EGLClientBuffer, attrib_list: *const types::EGLAttrib) -> types::EGLImage { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLContext, types::EGLenum, types::EGLClientBuffer, *const types::EGLAttrib) -> types::EGLImage>(self.CreateImage.f)(dpy, ctx, target, buffer, attrib_list) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CreateImageKHR(&self, dpy: types::EGLDisplay, ctx: types::EGLContext, target: types::EGLenum, buffer: types::EGLClientBuffer, attrib_list: *const types::EGLint) -> types::EGLImageKHR { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLContext, types::EGLenum, types::EGLClientBuffer, *const types::EGLint) -> types::EGLImageKHR>(self.CreateImageKHR.f)(dpy, ctx, target, buffer, attrib_list) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CreatePbufferFromClientBuffer(&self, dpy: types::EGLDisplay, buftype: types::EGLenum, buffer: types::EGLClientBuffer, config: types::EGLConfig, attrib_list: *const types::EGLint) -> types::EGLSurface { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLenum, types::EGLClientBuffer, types::EGLConfig, *const types::EGLint) -> types::EGLSurface>(self.CreatePbufferFromClientBuffer.f)(dpy, buftype, buffer, config, attrib_list) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CreatePbufferSurface(&self, dpy: types::EGLDisplay, config: types::EGLConfig, attrib_list: *const types::EGLint) -> types::EGLSurface { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLConfig, *const types::EGLint) -> types::EGLSurface>(self.CreatePbufferSurface.f)(dpy, config, attrib_list) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CreatePixmapSurface(&self, dpy: types::EGLDisplay, config: types::EGLConfig, pixmap: types::EGLNativePixmapType, attrib_list: *const types::EGLint) -> types::EGLSurface { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLConfig, types::EGLNativePixmapType, *const types::EGLint) -> types::EGLSurface>(self.CreatePixmapSurface.f)(dpy, config, pixmap, attrib_list) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CreatePlatformPixmapSurface(&self, dpy: types::EGLDisplay, config: types::EGLConfig, native_pixmap: *mut __gl_imports::raw::c_void, attrib_list: *const types::EGLAttrib) -> types::EGLSurface { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLConfig, *mut __gl_imports::raw::c_void, *const types::EGLAttrib) -> types::EGLSurface>(self.CreatePlatformPixmapSurface.f)(dpy, config, native_pixmap, attrib_list) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CreatePlatformPixmapSurfaceEXT(&self, dpy: types::EGLDisplay, config: types::EGLConfig, native_pixmap: *mut __gl_imports::raw::c_void, attrib_list: *const types::EGLint) -> types::EGLSurface { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLConfig, *mut __gl_imports::raw::c_void, *const types::EGLint) -> types::EGLSurface>(self.CreatePlatformPixmapSurfaceEXT.f)(dpy, config, native_pixmap, attrib_list) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CreatePlatformWindowSurface(&self, dpy: types::EGLDisplay, config: types::EGLConfig, native_window: *mut __gl_imports::raw::c_void, attrib_list: *const types::EGLAttrib) -> types::EGLSurface { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLConfig, *mut __gl_imports::raw::c_void, *const types::EGLAttrib) -> types::EGLSurface>(self.CreatePlatformWindowSurface.f)(dpy, config, native_window, attrib_list) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CreatePlatformWindowSurfaceEXT(&self, dpy: types::EGLDisplay, config: types::EGLConfig, native_window: *mut __gl_imports::raw::c_void, attrib_list: *const types::EGLint) -> types::EGLSurface { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLConfig, *mut __gl_imports::raw::c_void, *const types::EGLint) -> types::EGLSurface>(self.CreatePlatformWindowSurfaceEXT.f)(dpy, config, native_window, attrib_list) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CreateSync(&self, dpy: types::EGLDisplay, type_: types::EGLenum, attrib_list: *const types::EGLAttrib) -> types::EGLSync { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLenum, *const types::EGLAttrib) -> types::EGLSync>(self.CreateSync.f)(dpy, type_, attrib_list) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CreateSyncKHR(&self, dpy: types::EGLDisplay, type_: types::EGLenum, attrib_list: *const types::EGLint) -> types::EGLSyncKHR { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLenum, *const types::EGLint) -> types::EGLSyncKHR>(self.CreateSyncKHR.f)(dpy, type_, attrib_list) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn CreateWindowSurface(&self, dpy: types::EGLDisplay, config: types::EGLConfig, win: types::EGLNativeWindowType, attrib_list: *const types::EGLint) -> types::EGLSurface { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLConfig, types::EGLNativeWindowType, *const types::EGLint) -> types::EGLSurface>(self.CreateWindowSurface.f)(dpy, config, win, attrib_list) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DestroyContext(&self, dpy: types::EGLDisplay, ctx: types::EGLContext) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLContext) -> types::EGLBoolean>(self.DestroyContext.f)(dpy, ctx) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DestroyImage(&self, dpy: types::EGLDisplay, image: types::EGLImage) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLImage) -> types::EGLBoolean>(self.DestroyImage.f)(dpy, image) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DestroyImageKHR(&self, dpy: types::EGLDisplay, image: types::EGLImageKHR) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLImageKHR) -> types::EGLBoolean>(self.DestroyImageKHR.f)(dpy, image) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DestroySurface(&self, dpy: types::EGLDisplay, surface: types::EGLSurface) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLSurface) -> types::EGLBoolean>(self.DestroySurface.f)(dpy, surface) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DestroySync(&self, dpy: types::EGLDisplay, sync: types::EGLSync) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLSync) -> types::EGLBoolean>(self.DestroySync.f)(dpy, sync) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DestroySyncKHR(&self, dpy: types::EGLDisplay, sync: types::EGLSyncKHR) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLSyncKHR) -> types::EGLBoolean>(self.DestroySyncKHR.f)(dpy, sync) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn DupNativeFenceFDANDROID(&self, dpy: types::EGLDisplay, sync: types::EGLSyncKHR) -> types::EGLint { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLSyncKHR) -> types::EGLint>(self.DupNativeFenceFDANDROID.f)(dpy, sync) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetConfigAttrib(&self, dpy: types::EGLDisplay, config: types::EGLConfig, attribute: types::EGLint, value: *mut types::EGLint) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLConfig, types::EGLint, *mut types::EGLint) -> types::EGLBoolean>(self.GetConfigAttrib.f)(dpy, config, attribute, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetConfigs(&self, dpy: types::EGLDisplay, configs: *mut types::EGLConfig, config_size: types::EGLint, num_config: *mut types::EGLint) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, *mut types::EGLConfig, types::EGLint, *mut types::EGLint) -> types::EGLBoolean>(self.GetConfigs.f)(dpy, configs, config_size, num_config) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetCurrentContext(&self, ) -> types::EGLContext { __gl_imports::mem::transmute::<_, extern "system" fn() -> types::EGLContext>(self.GetCurrentContext.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetCurrentDisplay(&self, ) -> types::EGLDisplay { __gl_imports::mem::transmute::<_, extern "system" fn() -> types::EGLDisplay>(self.GetCurrentDisplay.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetCurrentSurface(&self, readdraw: types::EGLint) -> types::EGLSurface { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLint) -> types::EGLSurface>(self.GetCurrentSurface.f)(readdraw) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetDisplay(&self, display_id: types::EGLNativeDisplayType) -> types::EGLDisplay { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLNativeDisplayType) -> types::EGLDisplay>(self.GetDisplay.f)(display_id) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetError(&self, ) -> types::EGLint { __gl_imports::mem::transmute::<_, extern "system" fn() -> types::EGLint>(self.GetError.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetPlatformDisplay(&self, platform: types::EGLenum, native_display: *mut __gl_imports::raw::c_void, attrib_list: *const types::EGLAttrib) -> types::EGLDisplay { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLenum, *mut __gl_imports::raw::c_void, *const types::EGLAttrib) -> types::EGLDisplay>(self.GetPlatformDisplay.f)(platform, native_display, attrib_list) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetPlatformDisplayEXT(&self, platform: types::EGLenum, native_display: *mut __gl_imports::raw::c_void, attrib_list: *const types::EGLint) -> types::EGLDisplay { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLenum, *mut __gl_imports::raw::c_void, *const types::EGLint) -> types::EGLDisplay>(self.GetPlatformDisplayEXT.f)(platform, native_display, attrib_list) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetProcAddress(&self, procname: *const __gl_imports::raw::c_char) -> types::__eglMustCastToProperFunctionPointerType { __gl_imports::mem::transmute::<_, extern "system" fn(*const __gl_imports::raw::c_char) -> types::__eglMustCastToProperFunctionPointerType>(self.GetProcAddress.f)(procname) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetSyncAttrib(&self, dpy: types::EGLDisplay, sync: types::EGLSync, attribute: types::EGLint, value: *mut types::EGLAttrib) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLSync, types::EGLint, *mut types::EGLAttrib) -> types::EGLBoolean>(self.GetSyncAttrib.f)(dpy, sync, attribute, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn GetSyncAttribKHR(&self, dpy: types::EGLDisplay, sync: types::EGLSyncKHR, attribute: types::EGLint, value: *mut types::EGLint) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLSyncKHR, types::EGLint, *mut types::EGLint) -> types::EGLBoolean>(self.GetSyncAttribKHR.f)(dpy, sync, attribute, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Initialize(&self, dpy: types::EGLDisplay, major: *mut types::EGLint, minor: *mut types::EGLint) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, *mut types::EGLint, *mut types::EGLint) -> types::EGLBoolean>(self.Initialize.f)(dpy, major, minor) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn MakeCurrent(&self, dpy: types::EGLDisplay, draw: types::EGLSurface, read: types::EGLSurface, ctx: types::EGLContext) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLSurface, types::EGLSurface, types::EGLContext) -> types::EGLBoolean>(self.MakeCurrent.f)(dpy, draw, read, ctx) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn QueryAPI(&self, ) -> types::EGLenum { __gl_imports::mem::transmute::<_, extern "system" fn() -> types::EGLenum>(self.QueryAPI.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn QueryContext(&self, dpy: types::EGLDisplay, ctx: types::EGLContext, attribute: types::EGLint, value: *mut types::EGLint) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLContext, types::EGLint, *mut types::EGLint) -> types::EGLBoolean>(self.QueryContext.f)(dpy, ctx, attribute, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn QueryDeviceAttribEXT(&self, device: types::EGLDeviceEXT, attribute: types::EGLint, value: *mut types::EGLAttrib) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDeviceEXT, types::EGLint, *mut types::EGLAttrib) -> types::EGLBoolean>(self.QueryDeviceAttribEXT.f)(device, attribute, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn QueryDeviceStringEXT(&self, device: types::EGLDeviceEXT, name: types::EGLint) -> *const __gl_imports::raw::c_char { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDeviceEXT, types::EGLint) -> *const __gl_imports::raw::c_char>(self.QueryDeviceStringEXT.f)(device, name) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn QueryDevicesEXT(&self, max_devices: types::EGLint, devices: *mut types::EGLDeviceEXT, num_devices: *mut types::EGLint) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLint, *mut types::EGLDeviceEXT, *mut types::EGLint) -> types::EGLBoolean>(self.QueryDevicesEXT.f)(max_devices, devices, num_devices) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn QueryDisplayAttribEXT(&self, dpy: types::EGLDisplay, attribute: types::EGLint, value: *mut types::EGLAttrib) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLint, *mut types::EGLAttrib) -> types::EGLBoolean>(self.QueryDisplayAttribEXT.f)(dpy, attribute, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn QueryDisplayAttribKHR(&self, dpy: types::EGLDisplay, name: types::EGLint, value: *mut types::EGLAttrib) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLint, *mut types::EGLAttrib) -> types::EGLBoolean>(self.QueryDisplayAttribKHR.f)(dpy, name, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn QueryString(&self, dpy: types::EGLDisplay, name: types::EGLint) -> *const __gl_imports::raw::c_char { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLint) -> *const __gl_imports::raw::c_char>(self.QueryString.f)(dpy, name) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn QuerySurface(&self, dpy: types::EGLDisplay, surface: types::EGLSurface, attribute: types::EGLint, value: *mut types::EGLint) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLSurface, types::EGLint, *mut types::EGLint) -> types::EGLBoolean>(self.QuerySurface.f)(dpy, surface, attribute, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ReleaseTexImage(&self, dpy: types::EGLDisplay, surface: types::EGLSurface, buffer: types::EGLint) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLSurface, types::EGLint) -> types::EGLBoolean>(self.ReleaseTexImage.f)(dpy, surface, buffer) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn ReleaseThread(&self, ) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn() -> types::EGLBoolean>(self.ReleaseThread.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SurfaceAttrib(&self, dpy: types::EGLDisplay, surface: types::EGLSurface, attribute: types::EGLint, value: types::EGLint) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLSurface, types::EGLint, types::EGLint) -> types::EGLBoolean>(self.SurfaceAttrib.f)(dpy, surface, attribute, value) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SwapBuffers(&self, dpy: types::EGLDisplay, surface: types::EGLSurface) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLSurface) -> types::EGLBoolean>(self.SwapBuffers.f)(dpy, surface) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SwapBuffersWithDamageEXT(&self, dpy: types::EGLDisplay, surface: types::EGLSurface, rects: *mut types::EGLint, n_rects: types::EGLint) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLSurface, *mut types::EGLint, types::EGLint) -> types::EGLBoolean>(self.SwapBuffersWithDamageEXT.f)(dpy, surface, rects, n_rects) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SwapBuffersWithDamageKHR(&self, dpy: types::EGLDisplay, surface: types::EGLSurface, rects: *mut types::EGLint, n_rects: types::EGLint) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLSurface, *mut types::EGLint, types::EGLint) -> types::EGLBoolean>(self.SwapBuffersWithDamageKHR.f)(dpy, surface, rects, n_rects) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn SwapInterval(&self, dpy: types::EGLDisplay, interval: types::EGLint) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLint) -> types::EGLBoolean>(self.SwapInterval.f)(dpy, interval) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn Terminate(&self, dpy: types::EGLDisplay) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay) -> types::EGLBoolean>(self.Terminate.f)(dpy) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn WaitClient(&self, ) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn() -> types::EGLBoolean>(self.WaitClient.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn WaitGL(&self, ) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn() -> types::EGLBoolean>(self.WaitGL.f)() }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn WaitNative(&self, engine: types::EGLint) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLint) -> types::EGLBoolean>(self.WaitNative.f)(engine) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn WaitSync(&self, dpy: types::EGLDisplay, sync: types::EGLSync, flags: types::EGLint) -> types::EGLBoolean { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLSync, types::EGLint) -> types::EGLBoolean>(self.WaitSync.f)(dpy, sync, flags) }
#[allow(non_snake_case, unused_variables, dead_code)]
            #[inline] pub unsafe fn WaitSyncKHR(&self, dpy: types::EGLDisplay, sync: types::EGLSyncKHR, flags: types::EGLint) -> types::EGLint { __gl_imports::mem::transmute::<_, extern "system" fn(types::EGLDisplay, types::EGLSyncKHR, types::EGLint) -> types::EGLint>(self.WaitSyncKHR.f)(dpy, sync, flags) }
}

        unsafe impl __gl_imports::Send for Egl {}
