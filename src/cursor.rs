use std::os::raw::*;
use x11::xlib::*;
use super::core::*;

pub static mut normal: Cursor = X_NONE;
pub static mut moving: Cursor = X_NONE;
pub static mut resizing: Cursor = X_NONE;
pub static mut resizing_horizontal: Cursor = X_NONE;
pub static mut resizing_vertical: Cursor = X_NONE;

unsafe fn create_cursor (shape: c_uint) -> Cursor {
  XCreateFontCursor (display, shape)
}

unsafe fn free_cursor (cursor: Cursor) {
  XFreeCursor (display, cursor);
}

pub unsafe fn load_cursors () {
  normal = create_cursor (68); //XC_left_ptr
  moving = create_cursor (52); //XC_fleur
  resizing = create_cursor (120); //XC_sizing
  resizing_horizontal = create_cursor (108);  //XC_sb_h_double_arrow
  resizing_vertical = create_cursor (116); //XC_sb_v_double_arrow
  let mut wa: XSetWindowAttributes = uninitialized! ();
  wa.cursor = normal;
  XChangeWindowAttributes (display, root, CWCursor, &mut wa);
}

pub unsafe fn free_cursors () {
  free_cursor (normal);
  free_cursor (moving);
  free_cursor (resizing);
}

