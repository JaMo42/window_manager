use std::os::raw::*;
use rand::{prelude::ThreadRng, Rng};
use x11::xlib::*;
use crate::core::*;
use crate::property;
use crate::client::Client;

#[derive(Clone, Copy, Debug)]
pub struct Geometry {
  pub x: c_int,
  pub y: c_int,
  pub w: c_uint,
  pub h: c_uint
}

impl Geometry {
  pub const fn new () -> Self {
    Geometry { x: 0, y: 0, w: 0, h: 0 }
  }

  pub fn from_parts (x: c_int, y: c_int, w: c_uint, h: c_uint) -> Self {
    Geometry { x, y, w, h }
  }

  pub fn expand (&mut self, by: i32) -> &mut Self {
    self.x -= by;
    self.y -= by;
    let by2 = by << 1;
    if by >= 0 {
      self.w += by2 as u32;
      self.h += by2 as u32;
    }
    else {
      self.w -= -by2 as u32;
      self.h -= -by2 as u32;
    }
    self
  }

  pub unsafe fn clamp (&mut self, parent: &Geometry) {
    if self.x < parent.x {
      self.x = parent.x;
    }
    if self.y < parent.y {
      self.y = parent.y;
    }
    if self.w > parent.w {
      self.w = parent.w;
    }
    if self.h > parent.h {
      self.h = parent.h;
    }
  }

  pub unsafe fn center (&mut self, parent: &Geometry) -> &mut Self {
    self.x = parent.x + (parent.w as i32 - self.w as i32) / 2;
    self.y = parent.y + (parent.h as i32 - self.h as i32) / 2;
    self
  }

  pub unsafe fn center_inside (&mut self, parent: &Geometry) -> &mut Self{
    self.center (parent);
    self.clamp (parent);
    self
  }

  pub unsafe fn random_inside (&mut self, parent: &Geometry, rng: &mut ThreadRng) -> &mut Self {
    if self.w >= parent.w {
      self.w = parent.w;
      self.x = parent.x;
    }
    else {
      let max_x = (parent.w - self.w) as c_int + parent.x;
      self.x = rng.gen_range (parent.x..=max_x);
    }
    if self.h >= parent.h {
      self.h = parent.h;
      self.y = parent.y;
    }
    else {
      let max_y = (parent.h - self.h) as c_int + parent.y;
      self.y = rng.gen_range (parent.y..=max_y);
    }
    self
  }
}

pub struct Preview {
  window: Window,
  geometry: Geometry
}

impl Preview {
  pub unsafe fn create (initial_geometry: Geometry) -> Self {
    let mut vi: XVisualInfo = uninitialized! ();
    XMatchVisualInfo(display, XDefaultScreen(display), 32, TrueColor, &mut vi);
    let mut attributes: XSetWindowAttributes = uninitialized! ();
    attributes.override_redirect = X_TRUE;
    attributes.event_mask = ButtonPressMask|ButtonReleaseMask|PointerMotionMask;
    attributes.border_pixel = (*config).colors.selected.pixel;
    attributes.background_pixel = 0;
    attributes.colormap = XCreateColormap (display, root, vi.visual, AllocNone);
    let window = XCreateWindow(
      display,
      root,
      initial_geometry.x,
      initial_geometry.y,
      initial_geometry.w,
      initial_geometry.h,
      5,
      vi.depth,
      InputOutput as u32,
      vi.visual,
      CWEventMask|CWOverrideRedirect|CWBackPixel|CWBorderPixel|CWColormap,
      &mut attributes
    );
    let window_type_desktop = property::atom (property::Net::WMWindowTypeDesktop);
    property::set (
      window,
      property::Net::WMWindowType,
      XA_ATOM,
      32,
      &window_type_desktop,
      1
    );
    XClearWindow (display, window);
    XMapWindow (display, window);
    Preview {
      window,
      geometry: initial_geometry
    }
  }

  pub unsafe fn move_by (&mut self, x: i32, y: i32) {
    self.geometry.x += x;
    self.geometry.y += y;
  }

  pub unsafe fn resize_by (&mut self, w: i32, h: i32) {
    if w < 0 && (self.geometry.w - -w as u32) >= 160 {
      self.geometry.w -= (-w) as u32;
    }
    else {
      self.geometry.w += w as u32;
    }
    if h < 0 && (self.geometry.h - -h as u32) >= 90 {
      self.geometry.h -= (-h) as u32;
    }
    else {
      self.geometry.h += h as u32;
    }
  }

  pub unsafe fn update (&self) {
    XMoveResizeWindow (
      display,
      self.window,
      self.geometry.x,
      self.geometry.y,
      self.geometry.w,
      self.geometry.h
    );
    XClearWindow (display, self.window);
    XSync (display, X_FALSE);
  }

  pub unsafe fn finish (&mut self, client: &mut Client) {
    self.geometry.expand ((*config).border_width);
    XDestroyWindow (display, self.window);
    client.prev_geometry = self.geometry;
    client.snap_state = SNAP_NONE;
    client.move_and_resize (self.geometry);
  }
}
