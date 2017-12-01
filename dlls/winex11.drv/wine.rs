#![allow(dead_code)]

use std::borrow::Borrow;
use std::ffi::{ CStr, CString };
use std::os::raw::*;

extern "C" {
    fn glxdrv_current_exe(size: c_int, buffer: *mut c_char) -> u32;
    fn glxdrv_trace(s: *const i8);
}

pub fn current_exe() -> Result<String, u32> {
    let mut buf: [c_char; 1024] = [0; 1024];
    let ret = unsafe {
        glxdrv_current_exe(buf.len() as c_int, buf.as_mut_ptr())
    };
    if ret == 0 || ret == buf.len() as u32 {
        return Err(ret)
    } else {
        let s = unsafe { CStr::from_ptr(buf.as_ptr()) };
        Ok(s.to_owned().into_string().unwrap())
    }
}

#[inline(always)]
pub fn trace_native(s: &CStr) {
    unsafe {
        glxdrv_trace(s.as_ptr())
    }
}

pub fn trace<S: AsRef<str>>(s: S) {
    let s = CString::new(s.as_ref()).unwrap();
    trace_native(s.borrow());
}
