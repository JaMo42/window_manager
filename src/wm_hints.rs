use libc::c_void;
use x11::xlib::{XFree, XGetWMHints, XSetWMHints, XUrgencyHint, XWMHints};
use xcb::Xid;

use crate::x::Window;

#[derive(Debug)]
pub struct WmHints {
    inner: XWMHints,
}

impl WmHints {
    pub fn get(window: &Window) -> Option<Self> {
        let maybe_hints = unsafe {
            XGetWMHints(
                window.display().xlib_display(),
                window.resource_id() as x11::xlib::Window,
            )
        };
        if maybe_hints.is_null() {
            return None;
        }
        let this = Self {
            inner: unsafe { *maybe_hints },
        };
        unsafe { XFree(maybe_hints as *mut c_void) };
        Some(this)
    }

    pub fn is_urgent(&self) -> bool {
        self.inner.flags & XUrgencyHint == XUrgencyHint
    }

    pub fn set_urgent(&mut self, urgent: bool) {
        if urgent {
            self.inner.flags |= XUrgencyHint;
        } else {
            self.inner.flags &= !XUrgencyHint;
        }
    }

    pub fn set(&self, window: &Window) {
        unsafe {
            XSetWMHints(
                window.display().xlib_display(),
                window.resource_id() as x11::xlib::Window,
                // Doesn't actually need to be mutable but since it's a C
                // function it just is by default.
                &self.inner as *const XWMHints as *mut XWMHints,
            );
        }
    }
}

impl std::fmt::Display for WmHints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WmHints")
            .field("is_urgent", &self.is_urgent())
            .finish()
    }
}
