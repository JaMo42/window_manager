use super::error::fatal_error;
use crate::action::{move_snap_flags, snap_geometry};
use crate::client::{decorated_frame_offset, Client, Client_Geometry};
use crate::core::*;
use crate::ewmh;
use crate::monitors;
use crate::mouse_passthrough;
use crate::property;
use crate::x::Window;
use rand::{prelude::ThreadRng, Rng};
use std::os::raw::*;
use x11::xlib::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
  pub x: c_int,
  pub y: c_int,
  pub w: c_uint,
  pub h: c_uint,
}

impl Geometry {
  pub const fn new () -> Self {
    Geometry {
      x: 0,
      y: 0,
      w: 0,
      h: 0,
    }
  }

  pub const fn from_parts (x: c_int, y: c_int, w: c_uint, h: c_uint) -> Self {
    Geometry { x, y, w, h }
  }

  pub fn expand (&mut self, by: i32) -> &mut Self {
    self.x -= by;
    self.y -= by;
    let by2 = by << 1;
    if by >= 0 {
      self.w += by2 as u32;
      self.h += by2 as u32;
    } else {
      self.w -= -by2 as u32;
      self.h -= -by2 as u32;
    }
    self
  }

  pub fn clamp (&mut self, parent: &Geometry) {
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

  pub fn center (&mut self, parent: &Geometry) -> &mut Self {
    self.x = parent.x + (parent.w as i32 - self.w as i32) / 2;
    self.y = parent.y + (parent.h as i32 - self.h as i32) / 2;
    self
  }

  pub fn center_inside (&mut self, parent: &Geometry) -> &mut Self {
    self.center (parent);
    self.clamp (parent);
    self
  }

  pub fn random_inside (&mut self, parent: &Geometry, rng: &mut ThreadRng) -> &mut Self {
    if self.w >= parent.w {
      self.w = parent.w;
      self.x = parent.x;
    } else {
      let max_x = (parent.w - self.w) as c_int + parent.x;
      self.x = rng.gen_range (parent.x..=max_x);
    }
    if self.h >= parent.h {
      self.h = parent.h;
      self.y = parent.y;
    } else {
      let max_y = (parent.h - self.h) as c_int + parent.y;
      self.y = rng.gen_range (parent.y..=max_y);
    }
    self
  }

  // Returns the geometry of a window frame around a window with this geometry
  pub unsafe fn get_frame (&self) -> Geometry {
    Geometry::from_parts (
      self.x - decorated_frame_offset.x,
      self.y - decorated_frame_offset.y,
      self.w + decorated_frame_offset.w,
      self.h + decorated_frame_offset.h,
    )
  }

  // Inverse of `get_frame`
  pub unsafe fn get_client (&self) -> Geometry {
    Geometry::from_parts (
      self.x + decorated_frame_offset.x,
      self.y + decorated_frame_offset.y,
      self.w - decorated_frame_offset.w,
      self.h - decorated_frame_offset.h,
    )
  }

  pub fn contains (&self, x: i32, y: i32) -> bool {
    x >= self.x && x < (self.x + self.w as i32) && y >= self.y && y < (self.y + self.h as i32)
  }

  pub fn center_point (&self) -> (i32, i32) {
    (self.x + self.w as i32 / 2, self.y + self.h as i32 / 2)
  }
}

impl std::fmt::Display for Geometry {
  fn fmt (&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
    write! (f, "{}x{}+{}+{}", self.w, self.h, self.x, self.y)
  }
}

pub struct Preview {
  window: Window,
  original_geometry: Geometry,
  geometry: Geometry,
  snap_geometry: Geometry,
  final_geometry: Geometry,
  is_snapped: bool,
}

impl Preview {
  const MIN_WIDTH: u32 = 3 * 160;
  const MIN_HEIGHT: u32 = 3 * 90;
  const BORDER_WIDTH: c_int = 5;
  // Only apply resize increment if resize amount is larger than this value.
  // (if this is 0 it becomes very hard to not resize should the user change
  // their mind about resizing, if it's too large it may become impossible to
  // resize by a single increment)
  const RESIZE_INCREMENT_THRESHHOLD: i32 = 5;

  pub unsafe fn create (initial_geometry: Geometry) -> Self {
    let vi = display
      .match_visual_info (32, TrueColor)
      .unwrap_or_else (|| {
        fatal_error ("Could not create colormap");
      });
    let colormap = display.create_colormap (vi.visual, AllocNone);
    let window = Window::builder (&display)
      .position (
        initial_geometry.x - Self::BORDER_WIDTH,
        initial_geometry.y - Self::BORDER_WIDTH,
      )
      .size (initial_geometry.w, initial_geometry.h)
      .border_width (Self::BORDER_WIDTH as c_uint)
      .depth (vi.depth)
      .visual (vi.visual)
      .attributes (|attributes| {
        attributes
          .override_redirect (true)
          .border_pixel ((*config).colors.selected.pixel)
          .background_pixel (0);
      })
      .colormap (colormap)
      .build ();
    mouse_passthrough (window);
    ewmh::set_window_type (window, property::Net::WMWindowTypeDesktop);
    window.clear ();
    window.map ();
    Preview {
      window,
      original_geometry: initial_geometry,
      geometry: initial_geometry,
      snap_geometry: Geometry::new (),
      final_geometry: initial_geometry,
      is_snapped: false,
    }
  }

  pub fn move_by (&mut self, x: i32, y: i32) {
    self.geometry.x += x;
    self.geometry.y += y;
    self.is_snapped = false;
    self.final_geometry = self.geometry;
  }

  pub unsafe fn move_edge (&mut self, x: i32, y: i32) {
    let mon = monitors::at (x, y).window_area ();
    let is_top = y < mon.y;
    let is_bot = y > (mon.y + mon.h as i32);
    if x < mon.x {
      let flags = SNAP_LEFT | (SNAP_TOP * is_top as u8) | (SNAP_BOTTOM * is_bot as u8);
      self.snap_geometry = snap_geometry (flags, mon);
    } else if x > (mon.x + mon.w as i32) {
      let flags = SNAP_RIGHT | (SNAP_TOP * is_top as u8) | (SNAP_BOTTOM * is_bot as u8);
      self.snap_geometry = snap_geometry (flags, mon);
    } else {
      // bottom maximizes as well, this way we don't need to return to
      // `mouse_move` and call `move_by` instead.
      self.snap_geometry = snap_geometry (SNAP_MAXIMIZED, mon);
    }
    self.is_snapped = true;
  }

  pub fn resize_by (&mut self, w: i32, h: i32) {
    if w < 0 {
      let ww = -w as u32;
      if self.geometry.w > ww && self.geometry.w - ww >= Self::MIN_WIDTH {
        self.geometry.w -= ww as u32;
      }
    } else {
      self.geometry.w += w as u32;
    }
    if h < 0 {
      let hh = -h as u32;
      if self.geometry.h > hh && self.geometry.h - hh >= Self::MIN_HEIGHT {
        self.geometry.h -= hh;
      }
    } else {
      self.geometry.h += h as u32;
    }
    self.final_geometry = self.geometry;
  }

  pub unsafe fn snap (&mut self, x: i32, y: i32) {
    let flags = move_snap_flags (x as u32, y as u32);
    self.is_snapped = true;
    let window_area = {
      let (x, y) = self.geometry.center_point ();
      monitors::at (x, y).window_area ()
    };
    self.snap_geometry = snap_geometry (flags, window_area);
  }

  pub unsafe fn apply_normal_hints (&mut self, hints: &property::Normal_Hints, keep_height: bool) {
    let g;
    // Apply resize increment
    if let Some ((winc, hinc)) = hints.resize_inc () {
      let mut dw = self.geometry.w as i32 - self.original_geometry.w as i32;
      let mut dh = self.geometry.h as i32 - self.original_geometry.h as i32;
      if dw < -Self::RESIZE_INCREMENT_THRESHHOLD {
        dw = (dw - winc + 1) / winc * winc;
      } else if dw > Self::RESIZE_INCREMENT_THRESHHOLD {
        dw = (dw + winc - 1) / winc * winc;
      } else {
        dw = 0;
      }
      if dh < -Self::RESIZE_INCREMENT_THRESHHOLD {
        dh = (dh - hinc + 1) / hinc * hinc;
      } else if dh > Self::RESIZE_INCREMENT_THRESHHOLD {
        dh = (dh + hinc - 1) / hinc * hinc;
      } else {
        dh = 0;
      }
      g = Geometry::from_parts (
        self.geometry.x,
        self.geometry.y,
        (self.original_geometry.w as i32 + dw) as u32,
        (self.original_geometry.h as i32 + dh) as u32,
      );
    } else {
      g = self.geometry;
    }
    // Apply size constraints
    self.final_geometry = hints.constrain (&g.get_client (), keep_height).get_frame ();
  }

  pub unsafe fn update (&self) {
    let g = if self.is_snapped {
      let mut gg = self.snap_geometry;
      gg.w -= 2 * (*config).gap;
      gg.h -= 2 * (*config).gap;
      gg
    } else {
      let mut gg = self.final_geometry;
      gg.x -= Preview::BORDER_WIDTH;
      gg.y -= Preview::BORDER_WIDTH;
      gg
    };
    self.window.move_and_resize (g.x, g.y, g.w, g.h);
    self.window.clear ();
    display.sync (false);
  }

  pub unsafe fn finish (&mut self, client: &mut Client) {
    self.window.destroy ();
    if self.is_snapped {
      let flags = move_snap_flags (self.geometry.x as u32, self.geometry.y as u32);
      if flags == client.snap_state {
        return;
      }
      client.save_geometry ();
      client.snap_state = flags;
      client.move_and_resize (Client_Geometry::Snap (self.snap_geometry));
    } else {
      if self.final_geometry == self.original_geometry {
        return;
      }
      client.modify_saved_geometry (|g| *g = self.final_geometry);
      client.snap_state = 1; // Any non-zero value so unsnap does the resizing for us
      client.unsnap ();
      client.save_geometry ();
    }
  }
}
