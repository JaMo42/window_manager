use std::os::raw::*;

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
    if by >= 0 {
      self.w += by as u32;
      self.h += by as u32;
    }
    else {
      self.w -= -by as u32;
      self.h -= -by as u32;
    }
  }
}

