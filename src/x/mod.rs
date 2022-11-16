#![allow(dead_code)]
#![allow(unused_variables)]
use libc::*;
use std::ffi::CString;
use x11::xlib::*;

pub type XDisplay = *mut x11::xlib::Display;
pub type XWindow = x11::xlib::Window;
type Error_Handler = unsafe extern "C" fn (XDisplay, *mut XErrorEvent) -> i32;

pub const XNone: c_ulong = 0;
pub const XFalse: c_int = 0;
pub const XTrue: c_int = 1;

pub mod display;
pub mod window;
pub mod window_builder;

// Shadow xlib types with wrappers
pub use display::Display;
pub use window::Window;

pub fn set_error_handler (f: Error_Handler) -> Option<Error_Handler> {
  unsafe { XSetErrorHandler (Some (f)) }
}

pub fn unique_context () -> XContext {
  unsafe { XUniqueContext () }
}

pub fn string_to_keysym (string: &str) -> KeySym {
  unsafe {
    let c_str = CString::new (string).unwrap ();
    XStringToKeysym (c_str.as_ptr ())
  }
}
