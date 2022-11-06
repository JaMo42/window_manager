use x11::xlib::*;
use super::core::*;
use super::set_window_kind;
use super::geometry::Geometry;
use super::property::{self, Net};

pub struct Tooltip {
  window: Window,
  geometry: Geometry,
  active: bool
}

// As this is intended to be shown under the mouse cursor it only really makes
// sense to only have one, so it is stored globally so whoever uses it doesn't
// need to care about storing it.
pub static mut tooltip: Tooltip = Tooltip::new ();

impl Tooltip {
  const BORDER: u32 = 5;

  pub const fn new () -> Self {
    Self {
      window: X_NONE,
      geometry: Geometry::new (),
      active: false
    }
  }

  unsafe fn create (&mut self) {
    let mut attributes: XSetWindowAttributes = uninitialized! ();
    attributes.background_pixel = (*config).colors.bar_background.pixel;
    attributes.event_mask = ButtonPressMask | EnterWindowMask | LeaveWindowMask;
    self.window = XCreateWindow (
      display, root,
      0, 0, 10, 10,
      0,
      CopyFromParent,
      CopyFromParent as u32,
      CopyFromParent as *mut Visual,
      CWBackPixel|CWEventMask,
      &mut attributes
    );
    let window_type_dock = property::atom (Net::WMWindowTypeTooltip);
    property::set (
      self.window,
      property::Net::WMWindowType,
      XA_ATOM,
      32,
      &window_type_dock,
      1
    );
    set_window_kind (self.window, Window_Kind::Meta_Or_Unmanaged);
  }

  unsafe fn move_and_resize (&mut self, x: i32, y: i32, w: u32, h: u32) {
    self.geometry = Geometry::from_parts (x, y, w, h);
    self.geometry.clamp (&screen_size);
    XMoveResizeWindow (
      display, self.window,
      self.geometry.x, self.geometry.y,
      self.geometry.w, self.geometry.h
    );
  }

  pub unsafe fn show (&mut self, string: &str, x: i32, y: i32) {
    if self.window == X_NONE {
      self.create ();
    }
    if self.active {
      self.close ();
    }
    (*draw).select_font (&(*config).bar_font);
    let mut text = (*draw).text (string);
    let width = text.get_width () + 2 * Self::BORDER;
    // Add one to the height as the text width is based on the baseline and
    // I find this makes it look at bit better without looking uncentered.
    let height = text.get_height () + 2 * Self::BORDER + 1;
    self.move_and_resize (x, y, width, height);
    (*draw).fill_rect (0, 0, width, height, (*config).colors.bar_background);
    text = (*draw).text (string);
    text.at (Self::BORDER as i32, Self::BORDER as i32)
      .color ((*config).colors.bar_text)
      .draw ();
    XMapWindow (display, self.window);
    (*draw).render (self.window, 0, 0, width, height);
    self.active = true;
  }

  pub unsafe fn close (&mut self) {
    if self.active {
      XUnmapWindow (display, self.window);
      self.active = false;
    }
  }
}

impl Drop for Tooltip {
  fn drop (&mut self) {
    if self.window != X_NONE {
      // TODO: Isn't this called after main when the display is already closed?
      unsafe { XDestroyWindow (display, self.window); }
    }
  }
}
