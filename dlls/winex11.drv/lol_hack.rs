#![feature(nonzero)]

extern crate core;

mod query_renderer;
mod wine;

use core::nonzero::NonZero;
use std::borrow::Borrow;
use std::env;
use std::ffi::CStr;
use std::os::raw::*;
use std::slice;
use query_renderer::{ ProfileVersion, QueryRenderer };

#[allow(dead_code)]
mod glx_c {
    use std::os::raw::*;
    pub const GLX_CONTEXT_PROFILE_MASK_ARB: c_int = 0x9126;
    pub const GLX_CONTEXT_MAJOR_VERSION_ARB: c_int = 0x2091;
    pub const GLX_CONTEXT_MINOR_VERSION_ARB: c_int = 0x2092;
    pub const GLX_CONTEXT_CORE_PROFILE_BIT_ARB: c_int = 0b1;
    pub const GLX_CONTEXT_COMPATIBILITY_PROFILE_BIT_ARB: c_int = 0b10;
}

use glx_c::*;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Attrib {
    pub key: NonZero<c_int>,
    pub value: c_int,
}

impl Attrib {
    #[inline(always)]
    fn key(self) -> c_int {
        self.key.get()
    }

    #[inline(always)]
    fn set(&mut self, value: c_int) {
        self.value = value;
    }
}

pub struct AttribList<'a>(Option<&'a mut Option<Attrib>>);

impl<'a> AttribList<'a> {
    #[inline(always)]
    fn get(mut self, key: c_int) -> Option<&'a mut Attrib> {
        self.find(|attr| attr.key() == key)
    }

    #[inline(always)]
    fn set(self, key: c_int, value: c_int) -> bool {
        self.get(key).map(|a| a.set(value)).is_some()
    }
}

impl<'a> Iterator for AttribList<'a> {
    type Item = &'a mut Attrib;

    fn next(&mut self) -> Option<&'a mut Attrib> {
        let ret = self.0.take();
        ret.and_then(|v| {
            if v.is_some() {
                self.0 = unsafe {
                    Some(&mut slice::from_raw_parts_mut(v as *mut Option<Attrib>, 2)[1])
                };
            }
            v.as_mut()
        })
    }
}

trait FilterOption<T> {
    fn filter_opt<F: FnOnce(&T) -> bool>(self, f: F) -> Option<T>;
}

impl<T> FilterOption<T> for Option<T> {
    #[inline(always)]
    fn filter_opt<F: FnOnce(&T) -> bool>(self, f: F) -> Option<T> {
        self.and_then(|v| if f(&v) {
            Some(v)
        } else {
            None
        })
    }
}

trait OrDefault<T> {
    fn or_default(self) -> T;
}

impl<T: Default> OrDefault<T> for Option<T> {
    fn or_default(self) -> T {
        self.unwrap_or_else(Default::default)
    }
}

enum HackMethod {
    GiveCompat,
    GiveCore,
}

impl HackMethod {
    fn get() -> HackMethod {
        env::var("WINE_X11DRV_LOL_HACK_METHOD").ok().and_then(|s| match s.as_str() {
            "compat" => Some(HackMethod::GiveCompat),
            "core" => Some(HackMethod::GiveCore),
            _ => None,
        }).or_default()
    }
}

impl Default for HackMethod {
    fn default() -> HackMethod {
        HackMethod::GiveCompat
    }
}

#[no_mangle]
pub unsafe extern "C" fn glxdrv_apply_version_hack(attribs: &mut Option<Attrib>) {
    let major = AttribList(Some(attribs)).get(GLX_CONTEXT_MAJOR_VERSION_ARB)
        .map(|attr| attr.value);
    let minor = AttribList(Some(attribs)).get(GLX_CONTEXT_MINOR_VERSION_ARB)
        .map(|attr| attr.value);
    let profile_version = match (major, minor) {
        (Some(major), Some(minor)) => Some(ProfileVersion {
            major: major,
            minor: minor,
        }),
        _ => None,
    };
    let qr_ext = QueryRenderer::load();
    let profile = AttribList(Some(attribs))
        .get(GLX_CONTEXT_PROFILE_MASK_ARB)
        .map(|attr| attr.value)
        .filter_opt(|v| (v & GLX_CONTEXT_COMPATIBILITY_PROFILE_BIT_ARB) != 0);
    let mesa_override = env::var("MESA_GL_VERSION_OVERRIDE");
    //wine::trace(format!("MESA_GL_VERSION_OVERRIDE = {:?}", &mesa_override));
    //wine::trace(format!("QueryRenderer: {:?}", qr_ext.is_some()));
    profile_version
        .and_then(|pv| qr_ext.map(|qr| (qr.max_compatibility_profile_version(), pv)))
        .map(|(qr, pv)| if mesa_override.is_ok() {
            let qr = ProfileVersion::new(3, 0);
            (qr, pv)
        } else {
            (qr, pv)
        }).filter_opt(|&(qr, pv)| qr < pv)
        .and_then(|(qr, _)| profile.map(|profile| (qr, profile)))
        .map(|(qr, _)| {
            match HackMethod::get() {
                HackMethod::GiveCompat => {
                    AttribList(Some(attribs))
                        .set(GLX_CONTEXT_MAJOR_VERSION_ARB, qr.major);
                    AttribList(Some(attribs))
                        .set(GLX_CONTEXT_MINOR_VERSION_ARB, qr.minor);
                },
                HackMethod::GiveCore => {
                    AttribList(Some(attribs))
                        .set(GLX_CONTEXT_PROFILE_MASK_ARB, GLX_CONTEXT_CORE_PROFILE_BIT_ARB);
                },
            }
        });
}

#[no_mangle]
pub extern "C" fn glxdrv_attrib_list_get(attrib: c_int, attribs: AttribList) -> c_int {
    attribs.get(attrib)
        .map(|attr| attr.value)
        .unwrap_or(0)
}

fn has_extension(list: &str, ext: &str) -> bool {
    list
        .split_whitespace()
        .find(|name| name.eq(&ext))
        .is_some()
}

#[no_mangle]
pub extern "C" fn glxdrv_has_extension(list: *const c_char, ext: *const c_char) -> c_int {
    let list = unsafe {
        CStr::from_ptr(list).to_str().unwrap()
    };
    let ext = unsafe {
        CStr::from_ptr(ext).to_str().unwrap()
    };
    has_extension(list, ext) as c_int
}

#[no_mangle]
pub extern "C" fn glxdrv_init_lol() {
    match wine::current_exe().ok().as_ref().and_then(|s| s.split("\\").last()) {
        Some("League of Legends.exe") => {
            env::set_var("MESA_GL_VERSION_OVERRIDE", "3.2COMPAT");
        },
        name => wine::trace(format!("Executable: {:?}", name)),
    }
}

#[cfg(test)]
mod tests {
    use std::mem;
    use super::*;

    #[test]
    fn option_attrib_size() {
        assert_eq!(mem::size_of::<Attrib>(), mem::size_of::<Option<Attrib>>());
        assert_eq!(mem::size_of::<Option<Attrib>>(), 2 * mem::size_of::<c_int>());
    }

    #[test]
    fn attrib_list_size() {
        assert_eq!(mem::size_of::<AttribList<'static>>(), mem::size_of::<usize>());
    }

    #[test]
    fn attriblist_odd_bounds() {
        let attribs_data: &mut [c_int] = &mut [1, 2, 3, 4, 0];
        let head = &mut attribs_data[0];
        let len = AttribList(Some(unsafe { mem::transmute(head) })).count();
        assert_eq!(len, 2);
    }
}
