use super::{window::To_XWindow, *};
use std::ffi::{CStr, CString};

pub struct Display {
  connection: XDisplay,
  screen: c_int,
  root: XWindow,
  width: u32,
  height: u32,
}

impl Display {
  pub const fn uninit () -> Self {
    Self {
      connection: std::ptr::null_mut (),
      screen: 0,
      root: XNone,
      width: 0,
      height: 0,
    }
  }

  pub fn connect (name: Option<&str>) -> Self {
    let connection;
    let root;
    let screen;
    let width;
    let height;

    unsafe {
      connection = XOpenDisplay (
        name
          .map (|s| s.as_ptr () as *const c_char)
          .unwrap_or (std::ptr::null ()),
      );
      if connection.is_null () {
        // TODO: print display name used
        panic! ("Could not open display");
      }
      root = XDefaultRootWindow (connection);
      screen = XDefaultScreen (connection);
      width = XDisplayWidth (connection, screen) as u32;
      height = XDisplayHeight (connection, screen) as u32;
    }

    Self {
      connection,
      screen,
      root,
      width,
      height,
    }
  }

  pub fn root (&self) -> XWindow {
    self.root
  }

  pub fn default_screen (&self) -> i32 {
    self.screen
  }

  pub fn width (&self) -> u32 {
    self.width
  }

  pub fn height (&self) -> u32 {
    self.height
  }

  pub fn size (&self) -> (u32, u32) {
    (self.width, self.height)
  }

  pub fn default_visual (&self) -> *mut Visual {
    unsafe { XDefaultVisual (self.connection, self.screen) }
  }

  pub fn default_colormap (&self) -> Colormap {
    unsafe { XDefaultColormap (self.connection, self.screen) }
  }

  pub fn default_depth (&self) -> u32 {
    unsafe { XDefaultDepth (self.connection, self.screen) as u32 }
  }

  pub fn close (&mut self) {
    if !self.connection.is_null () {
      unsafe {
        XCloseDisplay (self.connection);
      }
      self.connection = std::ptr::null_mut ();
    }
  }

  pub fn as_raw (&self) -> XDisplay {
    self.connection
  }

  pub fn flush (&self) {
    unsafe {
      XFlush (self.connection);
    }
  }

  pub fn sync (&self, discard_events: bool) {
    unsafe {
      XSync (self.connection, discard_events as i32);
    }
  }

  pub fn next_event (&self, event_out: &mut XEvent) {
    unsafe {
      XNextEvent (self.connection, event_out);
    }
  }

  pub fn mask_event (&self, mask: i64, event_out: &mut XEvent) {
    unsafe {
      XMaskEvent (self.connection, mask, event_out);
    }
  }

  pub fn push_event (&self, event: &mut XEvent) {
    unsafe {
      XPutBackEvent (self.connection, event);
    }
  }

  pub fn grab (&self) {
    unsafe {
      XGrabServer (self.connection);
    }
  }

  pub fn ungrab (&self) {
    unsafe {
      XUngrabServer (self.connection);
    }
  }

  /// Creates a `Scoped_Grab` for the display
  pub fn scoped_grab (&self) -> Scoped_Grab {
    Scoped_Grab::new (self.connection)
  }

  pub fn set_input_focus<W: To_XWindow> (&self, window: W) {
    unsafe {
      XSetInputFocus (
        self.connection,
        window.to_xwindow (),
        RevertToParent,
        CurrentTime,
      );
    }
  }

  pub fn get_modifier_mapping (&self) -> *mut XModifierKeymap {
    unsafe { XGetModifierMapping (self.connection) }
  }

  pub fn keysym_to_keycode (&self, sym: KeySym) -> u8 {
    unsafe { XKeysymToKeycode (self.connection, sym) }
  }

  pub fn grab_key (&self, code: u32, mods: u32) {
    unsafe {
      XGrabKey (
        self.connection,
        code as i32,
        mods,
        self.root,
        XTrue,
        GrabModeAsync,
        GrabModeAsync,
      );
    }
  }

  pub fn ungrab_key (&self, code: u32, mods: u32) {
    unsafe {
      XUngrabKey (self.connection, code as i32, mods, self.root);
    }
  }

  pub fn grab_keyboard (&self, window: Window) {
    unsafe {
      XGrabKeyboard (
        self.connection,
        window.handle (),
        XFalse,
        GrabModeAsync,
        GrabModeAsync,
        CurrentTime,
      );
    }
  }

  pub fn ungrab_keyboard (&self) {
    unsafe {
      XUngrabKeyboard (self.connection, CurrentTime);
    }
  }

  pub fn grab_button_for (&self, button: u32, mods: u32, window: impl To_XWindow) {
    unsafe {
      XGrabButton (
        self.connection,
        button,
        mods,
        window.to_xwindow (),
        XTrue,
        (ButtonPressMask | ButtonReleaseMask | PointerMotionMask) as u32,
        GrabModeAsync,
        GrabModeAsync,
        XNone,
        XNone,
      );
    }
  }

  pub fn grab_button (&self, button: u32, mods: u32) {
    self.grab_button_for (button, mods, self.root);
  }

  pub fn ungrab_button_for (&self, button: u32, mods: u32, window: impl To_XWindow) {
    unsafe {
      XUngrabButton (self.connection, button, mods, window.to_xwindow ());
    }
  }

  pub fn ungrab_button (&self, button: u32, mods: u32) {
    self.ungrab_button_for (button, mods, self.root);
  }

  pub fn grab_pointer (&self, mask: i64, cursor: Cursor) -> bool {
    unsafe {
      XGrabPointer (
        self.connection,
        self.root,
        XFalse,
        mask as u32,
        GrabModeAsync,
        GrabModeAsync,
        XNone,
        cursor,
        CurrentTime,
      ) == GrabSuccess
    }
  }

  pub fn ungrab_pointer (&self) {
    unsafe {
      XUngrabPointer (self.connection, CurrentTime);
    }
  }

  pub fn allow_events (&self, mode: i32) {
    unsafe {
      XAllowEvents (self.connection, mode, CurrentTime);
    }
  }

  pub fn scoped_pointer_grab (&self, mask: i64, cursor: Cursor) -> Option<Scoped_Pointer_Grab> {
    if self.grab_pointer (mask, cursor) {
      Some (Scoped_Pointer_Grab {
        display: self.connection,
      })
    } else {
      None
    }
  }

  pub fn query_pointer_position (&self) -> Option<(i32, i32)> {
    let mut x: c_int = 0;
    let mut y: c_int = 0;
    // Dummy values
    let mut i: c_int = 0;
    let mut u: c_uint = 0;
    let mut w: XWindow = XNone;
    if unsafe {
      XQueryPointer (
        self.connection,
        self.root,
        &mut w,
        &mut w,
        &mut x,
        &mut y,
        &mut i,
        &mut i,
        &mut u,
      )
    } == XTrue
    {
      Some ((x, y))
    } else {
      None
    }
  }

  pub fn intern_atom (&self, name: &str) -> Atom {
    unsafe {
      let cstr = CString::new (name).unwrap ();
      XInternAtom (self.connection, cstr.as_ptr (), XFalse)
    }
  }

  pub fn get_atom_name (&self, atom: Atom) -> String {
    unsafe {
      CStr::from_ptr (XGetAtomName (self.connection, atom))
        .to_str ()
        .unwrap ()
        .to_owned ()
    }
  }

  pub fn create_simple_window (&self) -> Window {
    Window::from_handle (&self.connection, unsafe {
      XCreateSimpleWindow (self.connection, self.root, 0, 0, 1, 1, 0, 0, 0)
    })
  }

  pub fn map_window (&self, window: XWindow) {
    unsafe {
      XMapWindow (self.connection, window);
    }
  }

  pub fn get_selection_owner (&self, selection: Atom) -> XWindow {
    unsafe { XGetSelectionOwner (self.connection, selection) }
  }

  pub fn set_selection_ownder<W: To_XWindow> (&self, selection: Atom, owner: W) {
    unsafe {
      XSetSelectionOwner (self.connection, selection, owner.to_xwindow (), CurrentTime);
    }
  }

  pub fn create_font_cursor (&self, shape: u32) -> Cursor {
    unsafe { XCreateFontCursor (self.connection, shape) }
  }

  pub fn free_cursor (&self, cursor: Cursor) {
    unsafe {
      XFreeCursor (self.connection, cursor);
    }
  }

  pub fn match_visual_info (&self, depth: i32, class: i32) -> Option<XVisualInfo> {
    unsafe {
      let mut vi: XVisualInfo = std::mem::MaybeUninit::zeroed ().assume_init ();
      if XMatchVisualInfo (self.connection, self.screen, depth, class, &mut vi) != 0 {
        Some (vi)
      } else {
        None
      }
    }
  }

  pub fn create_colormap (&self, visual: *mut Visual, alloc: i32) -> Colormap {
    unsafe { XCreateColormap (self.connection, self.root, visual, alloc) }
  }
}

pub trait To_XDisplay {
  fn to_xdisplay (&self) -> XDisplay;
}

impl To_XDisplay for Display {
  fn to_xdisplay (&self) -> XDisplay {
    self.as_raw ()
  }
}

impl To_XDisplay for XDisplay {
  fn to_xdisplay (&self) -> XDisplay {
    *self
  }
}

/// Grabs the display when created, ungrabs it when dropped
pub struct Scoped_Grab {
  display: XDisplay,
}

impl Scoped_Grab {
  fn new (display: XDisplay) -> Self {
    unsafe {
      XGrabServer (display);
    }
    Self { display }
  }
}

impl Drop for Scoped_Grab {
  fn drop (&mut self) {
    unsafe {
      XUngrabServer (self.display);
    }
  }
}

pub struct Scoped_Pointer_Grab {
  display: XDisplay,
}

impl Drop for Scoped_Pointer_Grab {
  fn drop (&mut self) {
    unsafe {
      XUngrabPointer (self.display, CurrentTime);
    }
  }
}
