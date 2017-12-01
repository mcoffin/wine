#![allow(dead_code)]
use std::cmp::Ordering;
use std::ffi::CStr;
use std::mem;
use std::os::raw::*;

mod glx {
    use std::ffi::CStr;
    use std::os::raw::*;

    pub const RENDERER_OPENGL_CORE_PROFILE_VERSION_MESA: c_int = 0x818a;
    pub const RENDERER_OPENGL_COMPATIBILITY_PROFILE_VERSION_MESA: c_int = 0x818b;

    #[link(name = "GLX")]
    extern "C" {
        fn glXGetProcAddress(name: *const i8) -> Option<unsafe extern "system" fn()>;
    }

    #[inline(always)]
    pub fn get_proc_address(name: &CStr) -> Option<unsafe extern "system" fn()> {
        unsafe { glXGetProcAddress(name.as_ptr()) }
    }
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ProfileVersion {
    pub major: c_int,
    pub minor: c_int,
}

impl ProfileVersion {
    pub fn new(major: c_int, minor: c_int) -> ProfileVersion {
        ProfileVersion {
            major: major,
            minor: minor,
        }
    }
}

impl PartialOrd for ProfileVersion {
    fn partial_cmp(&self, other: &ProfileVersion) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ProfileVersion {
    fn cmp(&self, other: &ProfileVersion) -> Ordering {
        match self.major.cmp(&other.major) {
            Ordering::Equal => self.minor.cmp(&other.minor),
            ne => ne,
        }
    }
}

pub struct QueryRenderer {
    query_current_renderer_integer: unsafe extern "system" fn(attribute: c_int, value: *mut c_uint) -> c_int,
}

impl QueryRenderer {
    pub fn load() -> Option<QueryRenderer> {
        let p = glx::get_proc_address(unsafe { 
            CStr::from_bytes_with_nul_unchecked(b"glXQueryCurrentRendererIntegerMESA\0")
        });
        p.map(|fptr| QueryRenderer {
            query_current_renderer_integer: unsafe { mem::transmute(fptr) },
        })
    }

    pub fn max_core_profile_version(&self) -> ProfileVersion {
        let mut version = ProfileVersion::new(0, 0);
        unsafe {
            (self.query_current_renderer_integer)(glx::RENDERER_OPENGL_CORE_PROFILE_VERSION_MESA, mem::transmute(&mut version as *mut ProfileVersion));
        }
        version
    }

    pub fn max_compatibility_profile_version(&self) -> ProfileVersion {
        let mut version = ProfileVersion::new(0, 0);
        unsafe {
            (self.query_current_renderer_integer)(glx::RENDERER_OPENGL_COMPATIBILITY_PROFILE_VERSION_MESA, mem::transmute(&mut version as *mut ProfileVersion));
        }
        version
    }
}
