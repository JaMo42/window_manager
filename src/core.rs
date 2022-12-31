use crate::bar::Bar;
use crate::config::Config;
use crate::draw::Drawing_Context;
use crate::geometry::Geometry;
use crate::workspace::Workspace;
use crate::x::{Display, Window, XNone};
use std::os::raw::*;
use x11::xlib::*;

macro_rules! c_str {
  ($s:expr) => {
    std::ffi::CString::new($s).unwrap().as_ptr()
  };
}

macro_rules! string_from_ptr {
  ($ptr:expr) => {
    std::ffi::CStr::from_ptr($ptr).to_str().unwrap().to_owned()
  };
}

macro_rules! zeroed {
  () => {
    std::mem::MaybeUninit::zeroed().assume_init()
  };
}

macro_rules! my_panic {
  ($msg:expr) => {{
    log::error! ("\x1b[91mPANIC: {}\x1b[0m", $msg);
    panic! ($msg);
  }};

  ($($args:tt)*) => {{
    let msg = format! ($($args)*);
    log::error! ("\x1b[91mPANIC: {}\x1b[0m", msg);
    panic! ($($args)*);
  }};
}

macro_rules! log_error {
  ($result:expr) => {
    if let Err(error) = $result {
      log::error!("{}", error);
    }
  };

  ($result:expr, $what:expr) => {
    if let Err(error) = $result {
      log::error!("{}: {}", $what, error);
    }
  };
}

pub const MOD_WIN: c_uint = Mod4Mask;
pub const MOD_ALT: c_uint = Mod1Mask;
pub const MOD_SHIFT: c_uint = ShiftMask;
pub const MOD_CTRL: c_uint = ControlMask;
pub static mut numlock_mask: c_uint = 0;

/// Used to handle session manager messages from the mainloop
pub const SessionManagerEvent: i32 = LASTEvent + 1;

pub const SNAP_NONE: u8 = 0x0;
pub const SNAP_LEFT: u8 = 0x1;
pub const SNAP_RIGHT: u8 = 0x2;
pub const SNAP_TOP: u8 = 0x4;
pub const SNAP_BOTTOM: u8 = 0x8;
pub const SNAP_MAXIMIZED: u8 = 0x10;

pub const MOUSE_MOVE_RESIZE_RATE: u64 = 1000 / 30;

#[repr(usize)]
#[derive(PartialEq, Eq)]
pub enum Window_Kind {
  Root,
  Client,
  Frame,
  Frame_Button,
  Status_Bar,
  Notification,
  Meta_Or_Unmanaged,
  Tray_Client,
  Dock,
  Dock_Item,
  Dock_Show,
  Context_Menu,
  Split_Handle,
}

pub static mut display: Display = Display::uninit();
pub static mut root: Window = Window::uninit();
pub static mut workspaces: Vec<Workspace> = Vec::new();
pub static mut active_workspace: usize = 0;
pub static mut running: bool = false;
pub static mut quit_reason: String = String::new();
// Need to store as pointer since it contains a HashMap
pub static mut config: *const Config = std::ptr::null_mut();
pub static mut screen_size: Geometry = Geometry::new();
pub static mut mouse_held: c_uint = 0;
// Windows we do not create clients for and that ignore workspaces (status bars)
pub static mut meta_windows: Vec<Window> = Vec::new();
pub static mut draw: *mut Drawing_Context = std::ptr::null_mut();
pub static mut bar: Bar = Bar::new();
pub static mut wm_context: XContext = XNone as XContext;
pub static mut wm_winkind_context: XContext = XNone as XContext;
