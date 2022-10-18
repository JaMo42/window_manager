use std::os::raw::*;
use std::ffi::CString;
use x11::xlib::*;
use super::core::*;

#[macro_export]
macro_rules! set_cardinal {
  ($w:expr, $p:expr, $v:expr) => {
    let data = $v as c_uint;
    crate::property::set ($w, $p, XA_CARDINAL, 32, &data as *const c_uint as *const c_uchar, 1);
  }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub enum Net {
  Supported,
  ClientList,
  ActiveWindow,
  SupportingWMCheck,
  NumberOfDesktops,
  CurrentDesktop,
  WMName,
  WMState,
  WMStateFullscreen,
  WMStateDemandsAttention,
  WMWindowType,
  WMWindowTypeDialog,
  WMWindowTypeDock,
  WMWindowTypeDesktop,
  WMUserTime,
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
  unsafe fn into_atom (self) -> Atom;
}

impl Into_Atom for Net {
  unsafe fn into_atom (self) -> Atom {
    net[self as usize]
  }
}

impl Into_Atom for WM {
  unsafe fn into_atom (self) -> Atom {
    wm[self as usize]
  }
}

impl Into_Atom for Atom {
  unsafe fn into_atom (self) -> Atom {
    self
  }
}

pub struct Class_Hints {
  pub class: String,
  pub name: String
}

impl Class_Hints {
  pub unsafe fn new (window: Window) -> Option<Class_Hints> {
    let mut class_hints: XClassHint = uninitialized! ();
    if XGetClassHint (display, window, &mut class_hints) == 0 {
      None
    }
    else {
      Some (Class_Hints {
        class: string_from_ptr! (class_hints.res_class),
        name: string_from_ptr! (class_hints.res_name)
      })
    }
  }

  pub unsafe fn is_meta (&self) -> bool {
    (*config).meta_window_classes.contains (&self.class)
  }
}


pub static mut net: [Atom; Net::Last as usize] = [X_NONE; Net::Last as usize];
pub static mut wm: [Atom; WM::Last as usize] = [X_NONE; WM::Last as usize];

pub static mut wm_check_window: Window = X_NONE;


pub unsafe fn load_atoms () {
  macro_rules! W {
    ($property:ident, $name:expr) => {
      wm[WM::$property as usize] = XInternAtom (display, c_str! ($name), X_FALSE)
    }
  }
  macro_rules! N {
    ($property:ident, $name:expr) => {
      net[Net::$property as usize] = XInternAtom (display, c_str! ($name), X_FALSE)
    }
  }

  W! (Protocols, "WM_PROTOCOLS");
  W! (DeleteWindow, "WM_DELETE_WINDOW");
  W! (TakeFocus, "WM_TAKE_FOCUS");
  W! (DeleteWindow, "WM_DELETE_WINDOW");

  N! (Supported, "_NET_SUPPORTED");
  N! (ClientList, "_NET_CLIENT_LIST");
  N! (ActiveWindow, "_NET_ACTIVE_WINDOW");
  N! (SupportingWMCheck, "_NET_SUPPORTING_WM_CHECK");
  N! (NumberOfDesktops, "_NET_NUMBER_OF_DESKTOPS");
  N! (CurrentDesktop, "_NET_CURRENT_DESKTOP");
  N! (WMName, "_NET_WM_NAME");
  N! (WMState, "_NET_WM_STATE");
  N! (WMStateFullscreen, "_NET_WM_STATE_FULLSCREEN");
  N! (WMStateDemandsAttention, "_NET_WM_STATE_DEMANDS_ATTENTION");
  N! (WMWindowType, "_NET_WM_WINDOW_TYPE");
  N! (WMWindowTypeDialog, "_NET_WM_WINDOW_TYPE_DIALOG");
  N! (WMWindowTypeDock, "_NET_WM_WINDOW_TYPE_DOCK");
  N! (WMWindowTypeDesktop, "_NET_WM_WINDOW_TYPE_DESKTOP");
  N! (WMUserTime, "_NET_WM_USER_TIME");

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

  set_cardinal! (root, Net::NumberOfDesktops, workspaces.len ());
  set_cardinal! (root, Net::CurrentDesktop, active_workspace);
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

#[allow(dead_code)]
pub struct Property_Data {
  actual_type: Atom,
  format: c_int,
  nitems: c_ulong,
  bytes_after: c_ulong,
  data: *mut c_uchar
}

#[allow(dead_code)]
impl Property_Data {
  pub fn length (&self) -> usize {
    self.nitems as usize
  }

  pub unsafe fn value<T: Copy> (&self) -> T {
    *(self.data as *mut T)
  }

  pub unsafe fn value_at<T: Copy> (&self, idx: isize) -> T {
    *(self.data as *mut T).offset (idx)
  }

  #[allow(dead_code)]
  pub unsafe fn as_slice<T> (&self) -> &[T] {
    std::slice::from_raw_parts (self.data as *const T, self.nitems as usize)
  }

  pub unsafe fn as_string (&self) -> String {
    string_from_ptr! (self.data as *mut c_char)
  }
}

impl Drop for Property_Data {
  fn drop (&mut self) {
    unsafe { XFree (self.data as *mut c_void) };
  }
}

pub unsafe fn get<P: Into_Atom> (
  window: Window, property: P, offset: usize, length: usize, type_: Atom
) -> Option<Property_Data> {
  let mut actual_type: Atom = X_NONE;
  let mut format: c_int = 0;
  let mut nitems: c_ulong = 0;
  let mut bytes_after: c_ulong = 0;
  let mut data: *mut c_uchar = std::ptr::null_mut ();
  let status = XGetWindowProperty (
    display,
    window,
    property.into_atom (),
    offset as i64,
    length as i64,
    X_FALSE,
    type_,
    &mut actual_type,
    &mut format,
    &mut nitems,
    &mut bytes_after,
    &mut data
  );
  if status == Success as i32 && !data.is_null () {
    Some (Property_Data {
      actual_type,
      format,
      nitems,
      bytes_after,
      data
    })
  } else {
    None
  }
}

pub unsafe fn get_data_for_scalar<T, P: Into_Atom> (
  window: Window, property: P, type_: Atom, offset: usize
) -> Option<Property_Data> {
  let long_length = std::mem::size_of::<T> () / 4;
  let long_offset = (offset * std::mem::size_of::<T> ()) / 4;
  return get (window, property, long_offset, long_length, type_);
}

pub unsafe fn get_data_for_array<T, P: Into_Atom> (
  window: Window, property: P, type_: Atom, length: usize, offset: usize
) -> Option<Property_Data> {
  let long_length = (length * std::mem::size_of::<T> ()) / 4;
  let long_offset = (offset * std::mem::size_of::<T> ()) / 4;
  return get (window, property, long_offset, long_length, type_);
}

pub unsafe fn get_string<P: Into_Atom> (window: Window, property: P) -> Option<String> {
  const MAX_LENGTH: usize = 1024;
  get_data_for_array::<c_char, _> (
    window,
    property,
    AnyPropertyType as Atom,  // Could be XA_STRING or _XA_UTF8_STRING
    MAX_LENGTH,
    0
  )
  .map (|d| d.as_string ())
}

pub unsafe fn get_atom<P: Into_Atom> (window: Window, property: P) -> Atom {
  get_data_for_scalar::<Atom, _> (
    window,
    property,
    XA_ATOM,
    0
  )
  .map (|data| data.value ())
  .unwrap_or (X_NONE)
}
