use std::os::raw::*;
use std::ffi::CString;
use x11::xlib::*;
use super::core::*;

// TODO: _NET_NUMBER_OF_DESKTOPS, _NET_CURRENT_DESKTOP, _NET_DESKTOP_NAMES
//       These seem to be the best way to implement a external program that
//       prints desktop information for something like a polybar widget.
#[derive(Copy, Clone)]
#[repr(C)]
pub enum Net {
  Supported,
  ClientList,
  ActiveWindow,
  SupportingWMCheck,
  WMName,
  WMState,
  WMStateFullscreen,
  WMStateDemandsAttention,
  Last
}

#[derive(Copy, Clone)]
#[repr(C)]
pub enum WM {
  Protocols,
  DeleteWindow,
  TakeFocus,
  Last
}

pub trait Into_Atom {
  unsafe fn into_atom (&self) -> Atom;
}

impl Into_Atom for Net {
  unsafe fn into_atom (&self) -> Atom {
    net[*self as usize]
  }
}

impl Into_Atom for WM {
  unsafe fn into_atom (&self) -> Atom {
    wm[*self as usize]
  }
}

impl Into_Atom for Atom {
  unsafe fn into_atom (&self) -> Atom {
    *self
  }
}


pub static mut net: [Atom; Net::Last as usize] = [X_NONE; Net::Last as usize];
pub static mut wm: [Atom; WM::Last as usize] = [X_NONE; WM::Last as usize];

pub static mut wm_check_window: Window = X_NONE;


pub unsafe fn load_atoms () {
  macro_rules! N {
    ($property:expr, $name:expr) => {
      net[$property as usize] = XInternAtom (display, c_str! ($name), X_FALSE)
    }
  }
  macro_rules! W {
    ($property:expr, $name:expr) => {
      wm[$property as usize] = XInternAtom (display, c_str! ($name), X_FALSE)
    }
  }

  W! (WM::Protocols, "WM_PROTOCOLS");
  W! (WM::DeleteWindow, "WM_DELETE_WINDOW");
  W! (WM::TakeFocus, "WM_TAKE_FOCUS");
  W! (WM::DeleteWindow, "WM_DELETE_WINDOW");

  N! (Net::Supported, "_NET_SUPPORTED");
  N! (Net::ClientList, "_NET_CLIENT_LIST");
  N! (Net::ActiveWindow, "_NET_ACTIVE_WINDOW");
  N! (Net::SupportingWMCheck, "_NET_SUPPORTING_WM_CHECK");
  N! (Net::WMName, "_NET_WM_NAME");
  N! (Net::WMState, "_NET_WM_STATE");
  N! (Net::WMStateFullscreen, "_NET_WM_STATE_FULLSCREEN");
  N! (Net::WMStateDemandsAttention, "_NET_WM_STATE_DEMANDS_ATTENTION");

  log::debug! ("Net Properties: {:?}", net);
  log::debug! ("WM Properties: {:?}", wm);
}

pub unsafe fn init_set_root_properties () {
  const wm_name: &str = "window_manager";
  let utf8_string = XInternAtom (display, c_str! ("UTF8_STRING"), X_FALSE);

  wm_check_window = XCreateSimpleWindow (display, root, 0, 0, 1, 1, 0, 0, 0);
  set (wm_check_window, Net::SupportingWMCheck, XA_WINDOW, 32, &wm_check_window, 1);
  set (wm_check_window, Net::WMName, utf8_string, 8, c_str! (wm_name), wm_name.len () as i32);

  set (root, Net::SupportingWMCheck, XA_WINDOW, 32, &wm_check_window, 1);
  set (root, Net::Supported, XA_ATOM, 32, &net, Net::Last as i32);
  delete (root, Net::ActiveWindow);
  delete (root, Net::ClientList);
}

pub unsafe fn atom<P: Into_Atom> (property: P) -> Atom {
  property.into_atom ()
}

pub unsafe fn delete<P: Into_Atom> (window: Window, property: P) {
  XDeleteProperty (display, window, property.into_atom ());
}

pub unsafe fn set<P: Into_Atom, T> (
  window: Window, property: P, type_: Atom, format: c_int, data: *const T, n: c_int
) {
  XChangeProperty (
    display,
    window,
    property.into_atom (),
    type_,
    format,
    PropModeReplace,
    data as *const c_uchar,
    n
  );
}

pub unsafe fn append<P: Into_Atom, T> (
  window: Window, property: P, type_: Atom, format: c_int, data: *const T, n: c_int
) {
  XChangeProperty (
    display,
    window,
    property.into_atom (),
    type_,
    format,
    PropModeAppend,
    data as *const c_uchar,
    n
  );
}

pub unsafe fn get_string<P: Into_Atom> (window: Window, property: P) -> Option<String> {
  let mut a: Atom = X_NONE;
  let mut i: c_int = 0;
  let mut l: c_ulong = 0;
  let mut text: *mut c_uchar = std::ptr::null_mut ();
  if XGetWindowProperty (
    display, window, property.into_atom (), 0, 1000, X_FALSE, AnyPropertyType as u64,
    &mut a, &mut i, &mut l, &mut l, &mut text
  ) == Success as i32 {
    if a == X_NONE {
      None
    }
    else {
      let string = string_from_ptr! (text as *mut c_char);
      XFree (text as *mut c_void);
      Some (string)
    }
  }
  else {
    None
  }
}

