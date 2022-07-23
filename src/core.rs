use std::os::raw::*;
use x11::xlib::*;
use super::config::Config;
use super::geometry::Geometry;
use super::workspace::Workspace;
use super::draw::Drawing_Context;
use super::bar::bar::Bar;

macro_rules! c_str {
  ($s:expr) => {
    CString::new ($s).unwrap ().as_ptr ()
  };
}

macro_rules! string_from_ptr {
  ($ptr:expr) => {
    std::ffi::CStr::from_ptr ($ptr).to_str ().unwrap ().to_owned ()
  }
}

macro_rules! uninitialized {
  () => {
    std::mem::MaybeUninit::uninit ().assume_init ()
  };
}

pub const X_FALSE: c_int = 0;
pub const X_TRUE: c_int = 1;
pub const X_NONE: c_ulong = 0;

pub const MOD_WIN: c_uint = Mod4Mask;
pub const MOD_ALT: c_uint = Mod1Mask;
pub const MOD_SHIFT: c_uint = ShiftMask;
pub const MOD_CTRL: c_uint = ControlMask;
pub static mut numlock_mask: c_uint = 0;

pub const SNAP_NONE: u8 = 0x0;
pub const SNAP_LEFT: u8 = 0x1;
pub const SNAP_RIGHT: u8 = 0x2;
pub const SNAP_TOP: u8 = 0x4;
pub const SNAP_BOTTOM: u8 = 0x8;
pub const SNAP_MAXIMIZED: u8 = 0x10;

pub static mut display: *mut Display = std::ptr::null_mut ();
pub static mut root: Window = X_NONE;
pub static mut workspaces: Vec<Workspace> = Vec::new ();
pub static mut active_workspace: usize = 0;
pub static mut running: bool = false;
// Need to store as pointer since it contains a HashMap
pub static mut config: *const Config = std::ptr::null_mut ();
pub static mut screen_size: Geometry = Geometry::new ();
pub static mut window_area: Geometry = Geometry::new ();
pub static mut mouse_held: c_uint = 0;
// Windows we do not create clients for and that ignore workspaces (status bars)
pub static mut meta_windows: Vec<Window> = Vec::new ();
pub static mut draw: *mut Drawing_Context = std::ptr::null_mut ();
pub static mut bar: Bar = Bar::new ();
