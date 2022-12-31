use crate::core::*;
use crate::geometry::Geometry;
use crate::x::{Window, XFalse, XNone};
use std::os::raw::*;
use x11::xlib::*;

#[macro_export]
macro_rules! set_cardinal {
  ($w:expr, $p:expr, $v:expr) => {
    let data = $v as c_uint;
    $crate::property::set(
      $w,
      $p,
      XA_CARDINAL,
      32,
      &data as *const c_uint as *const c_uchar,
      1,
    );
  };
}

#[derive(Copy, Clone)]
#[repr(C)]
pub enum Net {
  ActiveWindow,
  ClientList,
  CurrentDesktop,
  FrameExtents,
  MoveresizeWindow,
  NumberOfDesktops,
  Supported,
  SupportingWMCheck,
  SystemTrayOpcode,
  SystemTrayOrientation,
  SystemTrayS0,
  WMActionChangeDesktop,
  WMActionClose,
  WMActionFullscreen,
  WMActionMaximizeHorz,
  WMActionMaximizeVert,
  WMActionMove,
  WMActionResize,
  WMAllowedActions,
  WMMoveresize,
  WMName,
  WMState,
  WMStateDemandsAttention,
  WMStateFullscreen,
  WMStateHidden,
  WMStateMaximizedHorz,
  WMStateMaximizedVert,
  WMUserTime,
  WMWindowOpacity,
  WMWindowType,
  WMWindowTypeDesktop,
  WMWindowTypeDialog,
  WMWindowTypeDock,
  WMWindowTypeNotification,
  WMWindowTypePopupMenu,
  WMWindowTypeTooltip,
  Last,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub enum WM {
  ChangeState,
  Class,
  DeleteWindow,
  Protocols,
  TakeFocus,
  Last,
}

#[derive(Copy, Clone)]
#[repr(C)]
#[allow(clippy::enum_variant_names)]
pub enum XEmbed {
  XEmbed,
  Info,
  Last,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub enum Other {
  Manager,
  MotfifWMHints,
  GtkApplicationId,
  Last,
}

pub trait Into_Atom {
  unsafe fn into_atom(self) -> Atom;
}

impl Into_Atom for Net {
  unsafe fn into_atom(self) -> Atom {
    net[self as usize]
  }
}

impl Into_Atom for WM {
  unsafe fn into_atom(self) -> Atom {
    wm[self as usize]
  }
}

impl Into_Atom for XEmbed {
  unsafe fn into_atom(self) -> Atom {
    xembed[self as usize]
  }
}

impl Into_Atom for Other {
  unsafe fn into_atom(self) -> Atom {
    other[self as usize]
  }
}

impl Into_Atom for Atom {
  unsafe fn into_atom(self) -> Atom {
    self
  }
}

pub struct Class_Hints {
  pub class: String,
  pub name: String,
}

impl Class_Hints {
  pub unsafe fn new(window: Window) -> Option<Class_Hints> {
    let mut class_hints: XClassHint = zeroed!();
    if XGetClassHint(display.as_raw(), window.handle(), &mut class_hints) == 0 {
      None
    } else {
      let result = Some(Class_Hints {
        class: string_from_ptr!(class_hints.res_class),
        name: string_from_ptr!(class_hints.res_name),
      });
      XFree(class_hints.res_class as *mut c_void);
      XFree(class_hints.res_name as *mut c_void);
      result
    }
  }

  pub unsafe fn is_meta(&self) -> bool {
    (*config).meta_window_classes.contains(&self.class)
  }
}

pub static mut net: [Atom; Net::Last as usize] = [XNone; Net::Last as usize];
pub static mut wm: [Atom; WM::Last as usize] = [XNone; WM::Last as usize];
pub static mut xembed: [Atom; XEmbed::Last as usize] = [XNone; XEmbed::Last as usize];
pub static mut other: [Atom; Other::Last as usize] = [XNone; Other::Last as usize];

pub static mut wm_check_window: Window = Window::uninit();

pub unsafe fn load_atoms() {
  macro_rules! W {
    ($property:ident, $name:expr) => {
      wm[WM::$property as usize] = display.intern_atom($name)
    };
  }
  macro_rules! N {
    ($property:ident, $name:expr) => {
      net[Net::$property as usize] = display.intern_atom($name)
    };
  }

  W!(ChangeState, "WM_CHANGE_STATE");
  W!(Class, "WM_CLASS");
  W!(DeleteWindow, "WM_DELETE_WINDOW");
  W!(DeleteWindow, "WM_DELETE_WINDOW");
  W!(Protocols, "WM_PROTOCOLS");
  W!(TakeFocus, "WM_TAKE_FOCUS");

  N!(ActiveWindow, "_NET_ACTIVE_WINDOW");
  N!(ClientList, "_NET_CLIENT_LIST");
  N!(CurrentDesktop, "_NET_CURRENT_DESKTOP");
  N!(FrameExtents, "_NET_FRAME_EXTENTS");
  N!(MoveresizeWindow, "_NET_MOVERESIZE_WINDOW");
  N!(NumberOfDesktops, "_NET_NUMBER_OF_DESKTOPS");
  N!(Supported, "_NET_SUPPORTED");
  N!(SupportingWMCheck, "_NET_SUPPORTING_WM_CHECK");
  N!(SystemTrayOpcode, "_NET_SYSTEM_TRAY_OPCODE");
  N!(SystemTrayOrientation, "_NET_SYSTEM_TRAY_ORIENTATION");
  N!(SystemTrayS0, "_NET_SYSTEM_TRAY_S0");
  N!(WMActionChangeDesktop, "_NET_WM_ACTION_CHANGE_DESKTOP");
  N!(WMActionClose, "_NET_WM_ACTION_CLOSE");
  N!(WMActionFullscreen, "_NET_WM_ACTION_FULLSCREEN");
  N!(WMActionMaximizeHorz, "_NET_WM_ACTION_MAXIMIZE_HORZ");
  N!(WMActionMaximizeVert, "_NET_WM_ACTION_MAXIMIZE_VERT");
  N!(WMActionMove, "_NET_WM_ACTION_MOVE");
  N!(WMActionResize, "_NET_WM_ACTION_RESIZE");
  N!(WMAllowedActions, "_NET_WM_ALLOWED_ACTIONS");
  N!(WMMoveresize, "_NET_WM_MOVERESIZE");
  N!(WMName, "_NET_WM_NAME");
  N!(WMState, "_NET_WM_STATE");
  N!(WMStateDemandsAttention, "_NET_WM_STATE_DEMANDS_ATTENTION");
  N!(WMStateFullscreen, "_NET_WM_STATE_FULLSCREEN");
  N!(WMStateHidden, "_NET_WM_STATE_HIDDEN");
  N!(WMStateMaximizedHorz, "_NET_WM_STATE_MAXIMIZED_HORZ");
  N!(WMStateMaximizedVert, "_NET_WM_STATE_MAXIMIZED_VERT");
  N!(WMUserTime, "_NET_WM_USER_TIME");
  N!(WMWindowOpacity, "_NET_WM_WINDOW_OPACITY");
  N!(WMWindowType, "_NET_WM_WINDOW_TYPE");
  N!(WMWindowTypeDesktop, "_NET_WM_WINDOW_TYPE_DESKTOP");
  N!(WMWindowTypeDialog, "_NET_WM_WINDOW_TYPE_DIALOG");
  N!(WMWindowTypeDock, "_NET_WM_WINDOW_TYPE_DOCK");
  N!(WMWindowTypeNotification, "_NET_WM_WINDOW_TYPE_NOTIFICATION");
  N!(WMWindowTypePopupMenu, "_NET_WM_WINDOW_TYPE_POPUP_MENU");
  N!(WMWindowTypeTooltip, "_NET_WM_WINDOW_TYPE_TOOLTIP");

  xembed[XEmbed::XEmbed as usize] = display.intern_atom("_XEMBED");
  xembed[XEmbed::Info as usize] = display.intern_atom("_XEMBED_INFO");

  other[Other::Manager as usize] = display.intern_atom("MANAGER");
  other[Other::MotfifWMHints as usize] = display.intern_atom("_MOTIF_WM_HINTS");
  other[Other::GtkApplicationId as usize] = display.intern_atom("_GTK_APPLICATION_ID");
}

pub unsafe fn init_set_root_properties() {
  const wm_name: &str = "window_manager";
  let utf8_string = display.intern_atom("UTF8_STRING");

  wm_check_window = display.create_simple_window();

  set(
    wm_check_window,
    Net::SupportingWMCheck,
    XA_WINDOW,
    32,
    &wm_check_window,
    1,
  );
  set(
    wm_check_window,
    Net::WMName,
    utf8_string,
    8,
    c_str!(wm_name),
    wm_name.len() as i32,
  );

  set(
    root,
    Net::SupportingWMCheck,
    XA_WINDOW,
    32,
    &wm_check_window,
    1,
  );
  set(root, Net::Supported, XA_ATOM, 32, &net, Net::Last as i32);
  delete(root, Net::ActiveWindow);
  delete(root, Net::ClientList);

  set_cardinal!(root, Net::NumberOfDesktops, workspaces.len());
  set_cardinal!(root, Net::CurrentDesktop, active_workspace);
}

pub unsafe fn atom<P: Into_Atom>(property: P) -> Atom {
  property.into_atom()
}

pub unsafe fn delete<P: Into_Atom>(window: Window, property: P) {
  XDeleteProperty(display.as_raw(), window.handle(), property.into_atom());
}

pub unsafe fn set<P: Into_Atom, T>(
  window: Window,
  property: P,
  type_: Atom,
  format: c_int,
  data: *const T,
  n: c_int,
) {
  XChangeProperty(
    display.as_raw(),
    window.handle(),
    property.into_atom(),
    type_,
    format,
    PropModeReplace,
    data as *const c_uchar,
    n,
  );
}

pub unsafe fn append<P: Into_Atom, T>(
  window: Window,
  property: P,
  type_: Atom,
  format: c_int,
  data: *const T,
  n: c_int,
) {
  XChangeProperty(
    display.as_raw(),
    window.handle(),
    property.into_atom(),
    type_,
    format,
    PropModeAppend,
    data as *const c_uchar,
    n,
  );
}

#[allow(dead_code)]
pub struct Property_Data {
  actual_type: Atom,
  format: c_int,
  nitems: c_ulong,
  bytes_after: c_ulong,
  data: *mut c_uchar,
}

#[allow(dead_code)]
impl Property_Data {
  pub fn actual_type(&self) -> Atom {
    self.actual_type
  }

  pub fn length(&self) -> usize {
    self.nitems as usize
  }

  pub unsafe fn value<T: Copy>(&self) -> T {
    *(self.data as *mut T)
  }

  pub unsafe fn value_at<T: Copy>(&self, idx: isize) -> T {
    *(self.data as *mut T).offset(idx)
  }

  #[allow(dead_code)]
  pub unsafe fn as_slice<T>(&self) -> &[T] {
    std::slice::from_raw_parts(self.data as *const T, self.nitems as usize)
  }

  pub unsafe fn as_string(&self) -> String {
    string_from_ptr!(self.data as *mut c_char)
  }
}

impl Drop for Property_Data {
  fn drop(&mut self) {
    unsafe { XFree(self.data as *mut c_void) };
  }
}

pub unsafe fn get<P: Into_Atom>(
  window: Window,
  property: P,
  offset: usize,
  length: usize,
  type_: Atom,
) -> Option<Property_Data> {
  let mut actual_type: Atom = XNone;
  let mut format: c_int = 0;
  let mut nitems: c_ulong = 0;
  let mut bytes_after: c_ulong = 0;
  let mut data: *mut c_uchar = std::ptr::null_mut();
  let status = XGetWindowProperty(
    display.as_raw(),
    window.handle(),
    property.into_atom(),
    offset as i64,
    length as i64,
    XFalse,
    type_,
    &mut actual_type,
    &mut format,
    &mut nitems,
    &mut bytes_after,
    &mut data,
  );
  if status == Success as i32 && !data.is_null() {
    Some(Property_Data {
      actual_type,
      format,
      nitems,
      bytes_after,
      data,
    })
  } else {
    None
  }
}

pub unsafe fn get_data_for_scalar<T, P: Into_Atom>(
  window: Window,
  property: P,
  type_: Atom,
  offset: usize,
) -> Option<Property_Data> {
  let long_length = std::mem::size_of::<T>() / 4;
  let long_offset = (offset * std::mem::size_of::<T>()) / 4;
  get(window, property, long_offset, long_length, type_)
}

pub unsafe fn get_data_for_array<T, P: Into_Atom>(
  window: Window,
  property: P,
  type_: Atom,
  length: usize,
  offset: usize,
) -> Option<Property_Data> {
  let long_length = (length * std::mem::size_of::<T>()) / 4;
  let long_offset = (offset * std::mem::size_of::<T>()) / 4;
  get(window, property, long_offset, long_length, type_)
}

pub unsafe fn get_string<P: Into_Atom>(window: Window, property: P) -> Option<String> {
  const MAX_LENGTH: usize = 1024;
  get_data_for_array::<c_char, _>(
    window,
    property,
    AnyPropertyType as Atom, // Could be XA_STRING or _XA_UTF8_STRING
    MAX_LENGTH,
    0,
  )
  .map(|d| d.as_string())
}

pub unsafe fn get_atom<P: Into_Atom>(window: Window, property: P) -> Atom {
  get_data_for_scalar::<Atom, _>(window, property, XA_ATOM, 0)
    .map(|data| data.value())
    .unwrap_or(XNone)
}

#[derive(Copy, Clone)]
pub struct Normal_Hints {
  min_size: Option<(u32, u32)>,
  max_size: Option<(u32, u32)>,
  resize_inc: Option<(u32, u32)>,
  aspect_ratio: Option<(f64, f64)>,
}

impl Normal_Hints {
  pub unsafe fn get(window: Window) -> Option<Normal_Hints> {
    let hints = XAllocSizeHints();
    macro_rules! get_field {
      ($field_1:ident, $field_2:ident, $flag:ident) => {
        if (*hints).flags & $flag == $flag {
          Some(((*hints).$field_1 as u32, (*hints).$field_2 as u32))
        } else {
          None
        }
      };
    }
    let mut _ignored = 0;
    if XGetWMNormalHints(display.as_raw(), window.handle(), hints, &mut _ignored) == 0 {
      XFree(hints as *mut c_void);
      return None;
    }
    let mut result = Normal_Hints {
      min_size: get_field!(min_width, min_height, PMinSize),
      max_size: get_field!(max_width, max_height, PMaxSize),
      resize_inc: get_field!(width_inc, height_inc, PResizeInc),
      aspect_ratio: None,
    };
    if (*hints).flags & PAspect == PAspect {
      result.aspect_ratio = Some((
        (*hints).min_aspect.x as f64 / (*hints).min_aspect.y as f64,
        (*hints).max_aspect.x as f64 / (*hints).max_aspect.y as f64,
      ));
    }
    XFree(hints as *mut c_void);
    Some(result)
  }

  /// Applies the hints to the given geometry.
  /// If `keep_height` is `true` the width will be changed instead of the
  /// height when adjusting the aspect ratio, it has no effect on the other
  /// size limits.
  /// This only applies size constraints, not resize increments.
  pub fn constrain(&self, g_in: &Geometry, keep_height: bool) -> Geometry {
    let mut g = *g_in;
    if let Some((minw, minh)) = self.min_size {
      g.w = u32::max(g.w, minw);
      g.h = u32::max(g.h, minh);
    }
    if let Some((maxw, maxh)) = self.max_size {
      g.w = u32::min(g.w, maxw);
      g.h = u32::min(g.h, maxh);
    }
    if let Some((min_aspect, max_aspect)) = self.aspect_ratio {
      let in_ratio = g_in.w as f64 / g_in.h as f64;
      let mut correct = None;
      if in_ratio < min_aspect {
        correct = Some(min_aspect);
      } else if in_ratio > max_aspect {
        correct = Some(max_aspect);
      }
      if let Some(ratio) = correct {
        if keep_height {
          g.w = (g.h as f64 / (1.0 / ratio)).round() as u32;
        } else {
          g.h = (g.w as f64 / ratio).round() as u32
        }
      }
    }
    g
  }

  pub fn resize_inc(&self) -> Option<(i32, i32)> {
    // Storing it as u32 keeps the get_field! macro simpler
    self.resize_inc.map(|(w, h)| (w as i32, h as i32))
  }
}

pub const MWM_HINTS_DECORATIONS: c_ulong = 1 << 1;

// MotifWmHints
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct Motif_Hints {
  pub flags: c_ulong,
  pub functions: c_ulong,
  pub decorations: c_ulong,
  pub input_mode: c_long,
  pub status: c_ulong,
}

impl Motif_Hints {
  pub unsafe fn get(window: Window) -> Option<Self> {
    let atom = atom(Other::MotfifWMHints);
    get_data_for_scalar::<Motif_Hints, _>(window, atom, atom, 0).map(|data| data.value())
  }
}
