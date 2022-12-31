use super::xembed;
use crate::core::*;
use crate::property::{self, WM};
use crate::set_window_kind;
use crate::x::{Window, XWindow};
use x11::xlib::*;

pub struct Tray_Client {
  window: Window,
  xembed_info: xembed::Info,
  size: u32,
  is_mapped: bool,
}

impl Tray_Client {
  pub fn new(window: XWindow, size: u32) -> Self {
    let window = Window::from_handle(unsafe { &display }, window);
    unsafe {
      set_window_kind(window, Window_Kind::Tray_Client);
    }
    Self {
      window,
      xembed_info: xembed::Info::new(),
      size,
      is_mapped: false,
    }
  }

  pub fn window(&self) -> Window {
    self.window
  }

  pub fn xembed_info(&self) -> &xembed::Info {
    &self.xembed_info
  }

  pub unsafe fn set_position(&self, x: i32, y: i32) {
    self.window.r#move(x, y);
    self.configure(x, y);
  }

  pub unsafe fn configure(&self, x: i32, y: i32) {
    XConfigureWindow(
      display.as_raw(),
      self.window.handle(),
      (CWX | CWY | CWWidth | CWHeight) as u32,
      &mut XWindowChanges {
        x,
        y,
        width: self.size as i32,
        height: self.size as i32,
        border_width: 0,
        sibling: 0,
        stack_mode: 0,
      },
    );
  }

  pub unsafe fn query_xembed_info(&mut self) {
    self.xembed_info.query(self.window);
  }

  pub unsafe fn update_mapped_state(&mut self) {
    if self.xembed_info.is_mapped() {
      self.window.map_raised();
      self.is_mapped = true;
    } else {
      self.window.unmap();
      self.is_mapped = false;
    }
  }

  pub unsafe fn class(&self) -> String {
    if let Some(prop) = property::get_string(self.window, WM::Class) {
      prop
    } else {
      log::warn!("No WM_CLASS on tray icon {}", self.window);
      format!("{}", self.window)
    }
  }

  pub fn is_mapped(&self) -> bool {
    self.is_mapped
  }

  pub fn set_mapped(&mut self, state: bool) {
    self.is_mapped = state;
  }
}
