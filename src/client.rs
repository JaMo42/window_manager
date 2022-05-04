use x11::xlib::*;
use super::core::*;
use super::geometry::*;
use super::*;

#[derive(Copy, Clone)]
pub struct Client {
  pub window: Window,
  pub geometry: Geometry,
  pub prev_geometry: Geometry,
  pub is_snapped: bool,
}

impl Client {
  pub fn new (window: Window) -> Self {
    unsafe {
      let mut wc: XWindowChanges = uninitialized! ();
      wc.border_width = (*config).border_width;
      XConfigureWindow (display, window, CWBorderWidth as u32, &mut wc);
      XMapWindow (display, window);
      let geometry = get_window_geometry (window);
      Client {
        window: window,
        geometry: geometry,
        prev_geometry: geometry,
        is_snapped: false
      }
    }
  }

  pub unsafe fn move_and_resize (&mut self, target: Geometry) {
    self.geometry = target;
    if self.is_snapped {
      XMoveResizeWindow (
        display, self.window,
        target.x + (*config).gap as i32,
        target.y + (*config).gap as i32,
        target.w - (((*config).gap + (*config).border_width as u32) << 1),
        target.h - (((*config).gap + (*config).border_width as u32) << 1)
      );
    }
    else {
      XMoveResizeWindow (
        display, self.window,
        target.x, target.y,
        target.w, target.h
      );
    }
  }

  pub unsafe fn unsnap (&mut self) {
    self.is_snapped = false;
    self.move_and_resize (self.prev_geometry);
  }
}

