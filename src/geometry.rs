use std::os::raw::*;
use rand::{prelude::ThreadRng, Rng};

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

  pub fn expand (&mut self, by: i32) {
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

  pub unsafe fn center (&mut self, parent: &Geometry) {
    self.x = parent.x + (parent.w as i32 - self.w as i32) / 2;
    self.y = parent.y + (parent.h as i32 - self.h as i32) / 2;
  }

  pub unsafe fn center_inside (&mut self, parent: &Geometry) {
    self.center (parent);
    self.clamp (parent);
  }

  pub unsafe fn random_inside (&mut self, parent: &Geometry, rng: &mut ThreadRng) {
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
  }
}
