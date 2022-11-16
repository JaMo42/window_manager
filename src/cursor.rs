use super::core::*;
use crate::x::XNone;
use std::os::raw::*;
use x11::xlib::*;

pub static mut normal: Cursor = XNone;
pub static mut moving: Cursor = XNone;
pub static mut resizing: Cursor = XNone;
pub static mut resizing_horizontal: Cursor = XNone;
pub static mut resizing_vertical: Cursor = XNone;

unsafe fn create_cursor (shape: c_uint) -> Cursor {
  display.create_font_cursor (shape)
}

unsafe fn free_cursor (cursor: Cursor) {
  display.free_cursor (cursor);
}

pub unsafe fn load_cursors () {
  normal = create_cursor (68); //XC_left_ptr
  moving = create_cursor (52); //XC_fleur
  resizing = create_cursor (120); //XC_sizing
  resizing_horizontal = create_cursor (108); //XC_sb_h_double_arrow
  resizing_vertical = create_cursor (116); //XC_sb_v_double_arrow
  root.change_attributes (|a| {
    a.cursor (normal);
  });
}

pub unsafe fn free_cursors () {
  free_cursor (normal);
  free_cursor (moving);
  free_cursor (resizing);
}
