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
  pub is_urgent: bool
}

impl Client {
  pub unsafe fn new (window: Window) -> Self {
    let mut wc: XWindowChanges = uninitialized! ();
    wc.border_width = (*config).border_width;
    XConfigureWindow (display, window, CWBorderWidth as u32, &mut wc);
    XMapWindow (display, window);
    let geometry = get_window_geometry (window);
    Client {
      window: window,
      geometry: geometry,
      prev_geometry: geometry,
      is_snapped: false,
      is_urgent: false
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

  pub unsafe fn focus (&mut self) {
    if self.is_urgent {
      self.set_urgency (false);
    }
    focus_window (self.window);
  }

  pub unsafe fn set_urgency (&mut self, urgency: bool) {
    self.is_urgent = urgency;
    if urgency {
      XSetWindowBorder (display, self.window, (*config).colors.urgent.pixel);
    }
    let hints = XGetWMHints (display, self.window);
    if !hints.is_null () {
      (*hints).flags = if urgency {
        (*hints).flags | XUrgencyHint
      } else {
        (*hints).flags & !XUrgencyHint
      };
      XSetWMHints (display, self.window, hints);
      XFree (hints as *mut c_void);
    }

  }
}

impl PartialEq for Client {
  fn eq (&self, other: &Self) -> bool {
    self.window == other.window
  }
}

impl std::fmt::Display for Client {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
    write! (f, "{} ({})", unsafe { window_title (self.window) }, self.window)
  }
}

