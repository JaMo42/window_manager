use super::display::ToXDisplay;
use super::window_builder::{WindowAttributes, WindowBuilder};
use super::*;

#[derive(Copy, Clone)]
pub struct Window {
  handle: XWindow,
  // If this is stored as a pointer the window cannot be sent between threads
  display: usize,
}

impl Window {
  fn display(&self) -> XDisplay {
    self.display as XDisplay
  }

  pub const fn uninit() -> Self {
    Self {
      display: 0,
      handle: XNone,
    }
  }

  pub fn from_handle<D: ToXDisplay>(display: &D, handle: XWindow) -> Self {
    Self {
      display: display.to_xdisplay() as usize,
      handle,
    }
  }

  pub fn builder(display: &Display) -> WindowBuilder {
    WindowBuilder::new(display)
  }

  pub fn destroy(&self) {
    unsafe {
      XDestroyWindow(self.display(), self.handle);
    }
  }

  pub fn is_none(&self) -> bool {
    self.handle == XNone
  }

  pub fn is_some(&self) -> bool {
    self.handle != XNone
  }

  pub fn kill_client(&self) {
    unsafe {
      XKillClient(self.display(), self.handle);
    }
  }

  pub fn handle(&self) -> XWindow {
    self.handle
  }

  pub fn raise(&self) {
    unsafe {
      XRaiseWindow(self.display(), self.handle);
    }
  }

  pub fn lower(&self) {
    unsafe {
      XLowerWindow(self.display(), self.handle);
    }
  }

  pub fn map(&self) {
    unsafe {
      XMapWindow(self.display(), self.handle);
    }
  }

  pub fn map_raised(&self) {
    unsafe {
      XMapRaised(self.display(), self.handle);
    }
  }

  pub fn map_subwindows(&self) {
    unsafe {
      XMapSubwindows(self.display(), self.handle);
    }
  }

  pub fn unmap(&self) {
    unsafe {
      XUnmapWindow(self.display(), self.handle);
    }
  }

  pub fn change_attributes(&self, f: fn(&mut WindowAttributes)) {
    let mut builder = WindowAttributes::new();
    f(&mut builder);
    let (mut attributes, valuemask) = builder.build();
    unsafe {
      XChangeWindowAttributes(self.display(), self.handle, valuemask, &mut attributes);
    }
  }

  pub fn change_event_mask(&mut self, mask: i64) {
    let mut wa: XSetWindowAttributes = unsafe { std::mem::MaybeUninit::zeroed().assume_init() };
    wa.event_mask = mask;
    unsafe {
      XChangeWindowAttributes(self.display(), self.handle, CWEventMask, &mut wa);
    }
  }

  pub fn set_border_width(&self, width: u32) {
    unsafe {
      XSetWindowBorderWidth(self.display(), self.handle, width);
    }
  }

  pub fn reparent<W: ToXWindow>(&self, parent: W, x: c_int, y: c_int) {
    unsafe {
      XReparentWindow(self.display(), self.handle, parent.to_xwindow(), x, y);
    }
  }

  pub fn save_context(&self, context: XContext, value: XPointer) {
    unsafe {
      XSaveContext(self.display(), self.handle, context, value);
    }
  }

  pub fn find_context(&self, context: XContext) -> Option<XPointer> {
    unsafe {
      let mut data: XPointer = std::ptr::null_mut();
      if XFindContext(self.display(), self.handle, context, &mut data) != 0 || data.is_null() {
        None
      } else {
        Some(data)
      }
    }
  }

  pub fn delete_context(&self, context: XContext) {
    unsafe {
      XDeleteContext(self.display(), self.handle, context);
    }
  }

  pub fn r#move(&self, x: i32, y: i32) {
    unsafe {
      XMoveWindow(self.display(), self.handle, x, y);
    }
  }

  pub fn resize(&self, w: u32, h: u32) {
    unsafe {
      XResizeWindow(self.display(), self.handle, w, h);
    }
  }

  pub fn move_and_resize(&self, x: i32, y: i32, w: u32, h: u32) {
    unsafe {
      XMoveResizeWindow(self.display(), self.handle, x, y, w, h);
    }
  }

  pub fn clear(&self) {
    unsafe {
      XClearWindow(self.display(), self.handle);
    }
  }

  pub fn set_background(&self, color: &crate::color::Color) {
    unsafe {
      XSetWindowBackground(self.display(), self.handle, color.pixel);
    }
  }

  pub fn get_attributes(&self) -> Option<XWindowAttributes> {
    unsafe {
      let mut wa = std::mem::MaybeUninit::zeroed().assume_init();
      if XGetWindowAttributes(self.display(), self.handle, &mut wa) != 0 {
        Some(wa)
      } else {
        None
      }
    }
  }

  pub fn send_event(&self, mut event: XEvent, mask: i64) -> bool {
    unsafe {
      XSendEvent(
        self.display(),
        self.handle,
        XFalse,
        mask,
        &mut event as *mut XEvent,
      ) != 0
    }
  }

  pub fn send_client_message<F>(&self, build: F) -> bool
  where
    F: Fn(&mut XClientMessageEvent),
  {
    unsafe {
      let mut event: XEvent = std::mem::MaybeUninit::zeroed().assume_init();
      event.type_ = ClientMessage;
      let message = &mut event.client_message;
      message.display = self.display();
      message.window = self.handle;
      build(message);
      self.send_event(event, NoEventMask)
    }
  }

  pub fn send_configure_event<F>(&self, build: F) -> bool
  where
    F: Fn(&mut XConfigureEvent),
  {
    unsafe {
      let mut event: XEvent = std::mem::MaybeUninit::zeroed().assume_init();
      event.type_ = ConfigureNotify;
      let configure = &mut event.configure;
      configure.display = self.display();
      configure.window = self.handle;
      configure.event = self.handle;
      build(configure);
      self.send_event(event, StructureNotifyMask)
    }
  }

  pub fn get_wm_hints(&self) -> *mut XWMHints {
    unsafe { XGetWMHints(self.display(), self.handle) }
  }

  pub fn set_wm_hints(&self, hints: *mut XWMHints) {
    unsafe {
      XSetWMHints(self.display(), self.handle, hints);
    }
  }

  pub fn get_wm_protocols(&self) -> Vec<Atom> {
    unsafe {
      let mut protocols: *mut Atom = std::ptr::null_mut();
      let mut count: i32 = 0;
      if XGetWMProtocols(self.display(), self.handle, &mut protocols, &mut count) != 0 {
        let result = std::slice::from_raw_parts(protocols, count as usize).to_vec();
        XFree(protocols as *mut c_void);
        result
      } else {
        Vec::new()
      }
    }
  }

  pub fn set_class_hint(&self, class: &str, name: &str) {
    unsafe {
      let class_cstr = std::ffi::CString::new(class).unwrap();
      let name_cstr = std::ffi::CString::new(name).unwrap();
      let mut h = XClassHint {
        res_class: class_cstr.as_ptr() as *mut i8,
        res_name: name_cstr.as_ptr() as *mut i8,
      };
      XSetClassHint(self.display(), self.handle, &mut h);
    }
  }
}

pub trait ToXWindow {
  fn to_xwindow(&self) -> XWindow;
}

impl ToXWindow for Window {
  fn to_xwindow(&self) -> XWindow {
    self.handle
  }
}

impl ToXWindow for XWindow {
  fn to_xwindow(&self) -> XWindow {
    *self
  }
}

impl PartialEq for Window {
  fn eq(&self, other: &Self) -> bool {
    self.handle == other.handle
  }
}

impl PartialEq<XWindow> for Window {
  fn eq(&self, other: &XWindow) -> bool {
    self.handle == *other
  }
}

impl Eq for Window {}

impl std::fmt::Display for Window {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
    write!(f, "{}", self.handle)
  }
}
