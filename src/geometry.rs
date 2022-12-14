use super::error::fatal_error;
use crate::action::{move_snap_flags, snap, snap_geometry};
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

  pub fn offset_of (
    &self,
    x: i32,
    y: i32,
    x_static_threshhold: i32,
    y_static_threshhold: i32,
  ) -> Point_Offset {
    let x_inside = x - self.x;
    let y_inside = y - self.y;
    let x_offset = if x_inside > x_static_threshhold {
      Offset::Percent (x_inside as f32 / self.w as f32)
    } else {
      Offset::Static (x_inside)
    };
    let y_offset = if y_inside > y_static_threshhold {
      Offset::Percent (y_inside as f32 / self.h as f32)
    } else {
      Offset::Static (y_inside)
    };
    Point_Offset {
      x: x_offset,
      y: y_offset,
    }
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
  snap_flags: u8,
  final_geometry: Geometry,
  is_snapped: bool,
  did_finish: bool,
  workspace: usize,
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

  pub unsafe fn create (initial_geometry: Geometry, workspace: usize) -> Self {
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
    Preview {
      window,
      original_geometry: initial_geometry,
      geometry: initial_geometry,
      snap_geometry: Geometry::new (),
      snap_flags: SNAP_NONE,
      final_geometry: initial_geometry,
      is_snapped: false,
      did_finish: false,
      workspace,
    }
  }

  pub fn show (&self) {
    self.window.map ()
  }

  pub fn ensure_unsnapped (&mut self, mouse_x: i32, mouse_y: i32, offset: &Point_Offset) {
    if self.is_snapped {
      let (x_offset, y_offset) = offset.inside (&self.original_geometry);
      self.geometry.x = mouse_x - x_offset;
      self.geometry.y = mouse_y - y_offset;
      self.is_snapped = false;
    }
  }

  pub fn move_by (&mut self, x: i32, y: i32) {
    self.geometry.x += x;
    self.geometry.y += y;
    self.final_geometry = self.geometry;
  }

  pub unsafe fn move_edge (&mut self, x: i32, y: i32) {
    let monitor = monitors::at (x, y);
    let area = monitor.window_area ();
    let is_top = y < area.y;
    let is_bot = y > (area.y + area.h as i32);
    if x < area.x {
      self.snap_flags = SNAP_LEFT | (SNAP_TOP * is_top as u8) | (SNAP_BOTTOM * is_bot as u8);
    } else if x > (area.x + area.w as i32) {
      self.snap_flags = SNAP_RIGHT | (SNAP_TOP * is_top as u8) | (SNAP_BOTTOM * is_bot as u8);
    } else {
      // bottom maximizes as well, this way we don't need to return to
      // `mouse_move` and call `move_by` instead.
      self.snap_flags = SNAP_MAXIMIZED;
    }
    self.snap_geometry = snap_geometry (self.snap_flags, monitor, self.workspace);
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
    self.snap_flags = move_snap_flags (x as u32, y as u32);
    self.is_snapped = true;
    let monitor = {
      let (x, y) = self.geometry.center_point ();
      monitors::at (x, y)
    };
    self.snap_geometry = snap_geometry (self.snap_flags, monitor, self.workspace);
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
    if self.did_finish {
      return;
    }
    self.did_finish = true;
    self.window.destroy ();
    if self.is_snapped {
      if self.snap_flags == client.snap_state {
        return;
      }
      if client.snap_state == SNAP_NONE {
        client.save_geometry ();
      }
      snap (client, self.snap_flags);
    } else {
      if self.final_geometry == self.original_geometry {
        return;
      }
      if client.is_snapped () {
        workspaces[client.workspace].remove_snapped_client (client);
      }
      client.snap_state = SNAP_NONE;
      client.move_and_resize (Client_Geometry::Frame (self.final_geometry));
      client.save_geometry ();
      ewmh::set_net_wm_state (client, &[]);
    }
  }

  pub unsafe fn cancel (&mut self) {
    if self.did_finish {
      return;
    }
    self.did_finish = true;
    self.window.destroy ();
  }
}

pub enum Offset {
  Static (i32),
  Percent (f32),
}

pub struct Point_Offset {
  x: Offset,
  y: Offset,
}

impl Point_Offset {
  pub fn inside (&self, geometry: &Geometry) -> (i32, i32) {
    let x = match self.x {
      Offset::Static (offset) => offset,
      Offset::Percent (percent) => (geometry.w as f32 * percent) as i32,
    };
    let y = match self.y {
      Offset::Static (offset) => offset,
      Offset::Percent (percent) => (geometry.h as f32 * percent) as i32,
    };
    (x, y)
  }
}
