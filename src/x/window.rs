use super::display::Into_Display;
use super::window_builder::{Window_Attributes, Window_Builder};
use super::*;

#[derive(Copy, Clone)]
pub struct Window {
  handle: XWindow,
  // If this is stored as a pointer the window cannot be sent between threads
  display: usize,
}

impl Window {
  fn display (&self) -> XDisplay {
    self.display as XDisplay
  }

  pub const fn uninit () -> Self {
    Self {
      display: 0,
      handle: XNone,
    }
  }

  pub fn from_handle<D: Into_Display> (display: &D, handle: XWindow) -> Self {
    Self {
      display: display.into_display () as usize,
      handle,
    }
  }

  pub fn builder (display: &Display) -> Window_Builder {
    Window_Builder::new (display)
  }

  pub fn destroy (&self) {
    unsafe {
      XDestroyWindow (self.display (), self.handle);
    }
  }

  pub fn is_none (&self) -> bool {
    self.handle == XNone
  }

  pub fn is_some (&self) -> bool {
    self.handle != XNone
  }

  pub fn kill_client (&self) {
    unsafe {
      XKillClient (self.display (), self.handle);
    }
  }

  pub fn handle (&self) -> XWindow {
    self.handle
  }

  pub fn raise (&self) {
    unsafe {
      XRaiseWindow (self.display (), self.handle);
    }
  }

  pub fn map (&self) {
    unsafe {
      XMapWindow (self.display (), self.handle);
    }
  }

  pub fn map_raised (&self) {
    unsafe {
      XMapRaised (self.display (), self.handle);
    }
  }

  pub fn map_subwindows (&self) {
    unsafe {
      XMapSubwindows (self.display (), self.handle);
    }
  }

  pub fn unmap (&self) {
    unsafe {
      XUnmapWindow (self.display (), self.handle);
    }
  }

  pub fn change_attributes (&self, f: fn (&mut Window_Attributes)) {
    let mut builder = Window_Attributes::new ();
    f (&mut builder);
    let (mut attributes, valuemask) = builder.build ();
    unsafe {
      XChangeWindowAttributes (self.display (), self.handle, valuemask, &mut attributes);
    }
  }

  pub fn change_event_mask (&mut self, mask: i64) {
    let mut wa: XSetWindowAttributes = unsafe { std::mem::MaybeUninit::zeroed ().assume_init () };
    wa.event_mask = mask;
    unsafe {
      XChangeWindowAttributes (self.display (), self.handle, CWEventMask, &mut wa);
    }
  }

  pub fn set_border_width (&self, width: u32) {
    unsafe {
      XSetWindowBorderWidth (self.display (), self.handle, width);
    }
  }

  pub fn reparent<W: Into_Window> (&self, parent: W, x: c_int, y: c_int) {
    unsafe {
      XReparentWindow (self.display (), self.handle, parent.into_window (), x, y);
    }
  }

  pub fn save_context (&self, context: XContext, value: XPointer) {
    unsafe {
      XSaveContext (self.display (), self.handle, context, value);
    }
  }

  pub fn find_context (&self, context: XContext) -> Option<XPointer> {
    unsafe {
      let mut data: XPointer = std::ptr::null_mut ();
      if XFindContext (self.display (), self.handle, context, &mut data) != 0 || data.is_null () {
        None
      } else {
        Some (data)
      }
    }
  }

  pub fn delete_context (&self, context: XContext) {
    unsafe {
      XDeleteContext (self.display (), self.handle, context);
    }
  }

  pub fn r#move (&self, x: i32, y: i32) {
    unsafe {
      XMoveWindow (self.display (), self.handle, x, y);
    }
  }

  pub fn resize (&self, w: u32, h: u32) {
    unsafe {
      XResizeWindow (self.display (), self.handle, w, h);
    }
  }

  pub fn move_and_resize (&self, x: i32, y: i32, w: u32, h: u32) {
    unsafe {
      XMoveResizeWindow (self.display (), self.handle, x, y, w, h);
    }
  }

  pub fn clear (&self) {
    unsafe {
      XClearWindow (self.display (), self.handle);
    }
  }

  pub fn set_background (&self, color: &crate::color::Color) {
    unsafe {
      XSetWindowBackground (self.display (), self.handle, color.pixel);
    }
  }

  pub fn get_attributes (&self) -> Option<XWindowAttributes> {
    unsafe {
      let mut wa = std::mem::MaybeUninit::zeroed ().assume_init ();
      if XGetWindowAttributes (self.display (), self.handle, &mut wa) != 0 {
        Some (wa)
      } else {
        None
      }
    }
  }

  pub fn send_event (&self, mut event: XEvent, mask: i64) -> bool {
    unsafe {
      XSendEvent (
        self.display (),
        self.handle,
        XFalse,
        mask,
        &mut event as *mut XEvent,
      ) != 0
    }
  }

  pub fn send_client_message<F> (&self, build: F) -> bool
  where
    F: Fn(&mut XClientMessageEvent),
  {
    unsafe {
      let mut event: XEvent = std::mem::MaybeUninit::zeroed ().assume_init ();
      let message = &mut event.client_message;
      message.type_ = ClientMessage;
      message.display = self.display ();
      message.window = self.handle;
      build (message);
      self.send_event (event, NoEventMask)
    }
  }

  pub fn send_configure_event<F> (&self, build: F) -> bool
  where
    F: Fn(&mut XConfigureEvent),
  {
    unsafe {
      let mut event: XEvent = std::mem::MaybeUninit::zeroed ().assume_init ();
      let configure = &mut event.configure;
      configure.type_ = ConfigureNotify;
      configure.display = self.display ();
      configure.window = self.handle;
      configure.event = self.handle;
      build (configure);
      self.send_event (event, StructureNotifyMask)
    }
  }
}

pub trait Into_Window {
  fn into_window (&self) -> XWindow;
}

impl Into_Window for Window {
  fn into_window (&self) -> XWindow {
    self.handle
  }
}

impl Into_Window for XWindow {
  fn into_window (&self) -> XWindow {
    *self
  }
}

impl PartialEq for Window {
  fn eq (&self, other: &Self) -> bool {
    self.handle == other.handle
  }
}

impl PartialEq<XWindow> for Window {
  fn eq (&self, other: &XWindow) -> bool {
    self.handle == *other
  }
}

impl Eq for Window {}

impl std::fmt::Display for Window {
  fn fmt (&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
    write! (f, "{}", self.handle)
  }
}
