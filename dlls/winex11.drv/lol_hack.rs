#![feature(nonzero)]

extern crate core;

mod query_renderer;

use core::nonzero::NonZero;
use std::env;
use std::ffi::CStr;
use std::os::raw::*;
use std::slice;
use query_renderer::{ ProfileVersion, QueryRenderer };

const GLX_CONTEXT_PROFILE_MASK_ARB: c_int = 0x9126;
const GLX_CONTEXT_MAJOR_VERSION_ARB: c_int = 0x2091;
const GLX_CONTEXT_MINOR_VERSION_ARB: c_int = 0x2092;
const GLX_CONTEXT_CORE_PROFILE_BIT_ARB: c_int = 0b1;
const GLX_CONTEXT_COMPATIBILITY_PROFILE_BIT_ARB: c_int = 0b10;

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
}

pub struct AttribList<'a>(Option<&'a mut Option<Attrib>>);

impl<'a> AttribList<'a> {
    #[inline(always)]
    fn get(mut self, key: c_int) -> Option<&'a mut Attrib> {
        self.find(|attr| attr.key() == key)
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
    let mut should_continue = profile_version
        .and_then(|pv| QueryRenderer::load().map(|qr| (qr, pv)))
        .map(|(qr, pv)| qr.max_compatibility_profile_version() < pv)
        .unwrap_or(true);
    should_continue = should_continue || env::var("WINE_X11DRV_OVERRIDE_LOL").is_ok();
    if !should_continue {
        return;
    }
    let profile = AttribList(Some(attribs))
        .get(GLX_CONTEXT_PROFILE_MASK_ARB)
        .filter_opt(|attr| (attr.value & GLX_CONTEXT_COMPATIBILITY_PROFILE_BIT_ARB) != 0);
    if let Some(profile) = profile {
        let mut set_core = || {
            profile.value = GLX_CONTEXT_CORE_PROFILE_BIT_ARB;
        };
        match (major, minor) {
            (Some(3), Some(minor)) if minor > 0 => set_core(),
            (Some(major), _) if major > 3 => set_core(),
            _ => {},
        }
    }
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
