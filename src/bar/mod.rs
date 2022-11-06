mod xembed;
mod tray_client;
pub mod tray_manager;
mod widget;

use x11::xlib::*;
use std::ffi::CString;
use libc::{c_char, c_uchar, c_uint};
use crate::core::*;
use crate::set_window_kind;
use crate::cursor;
use crate::property;
use tray_manager::Tray_Manager;

use self::widget::Widget;

pub static mut tray: Tray_Manager = Tray_Manager::new ();

pub struct Bar {
  pub width: u32,
  pub height: u32,
  pub window: Window,
  last_scroll_time: Time,
  left_widgets: Vec<Box<dyn Widget>>,
  right_widgets: Vec<Box<dyn Widget>>,
  // The widget under the mouse, gets set on enter/leave events we don't need
  // to resolve it for clicks
  mouse_widget: *mut dyn Widget
}

impl Bar {
  /// Space between widgets
  pub const WIDGET_GAP: i32 = 10;
  /// Space between rightmost widget and system tray
  pub const RIGHT_GAP: u32 = 5;

  pub const fn new () -> Self {
    Self {
      width: 0,
      height: 0,
      window: X_NONE,
      last_scroll_time: 0,
      left_widgets: Vec::new (),
      right_widgets: Vec::new (),
      mouse_widget: widget::null_ptr ()
    }
  }

  pub unsafe fn create () -> Self {
    let screen = XDefaultScreen (display);
    let mut attributes: XSetWindowAttributes = uninitialized! ();
    attributes.override_redirect = X_TRUE;
    attributes.background_pixel = (*config).colors.bar_background.pixel;
    attributes.event_mask = ButtonPressMask|ExposureMask;
    attributes.cursor = cursor::normal;
    let mut class_hint = XClassHint {
      res_name: c_str! ("window_manager_bar") as *mut c_char,
      res_class: c_str! ("window_manager_bar") as *mut c_char
    };
    let width = screen_size.w as u32;
    let height = (*config).bar_height.get (Some (&(*config).bar_font));
    let window = XCreateWindow (
      display,
      root,
      0,
      0,
      width,
      height,
      0,
      XDefaultDepth (display, screen),
      CopyFromParent as u32,
      XDefaultVisual(display, screen),
      CWOverrideRedirect|CWBackPixel|CWEventMask|CWCursor,
      &mut attributes
    );
    XSetClassHint (display, window, &mut class_hint);
    let window_type_dock = property::atom (property::Net::WMWindowTypeDock);
    property::set (
      window,
      property::Net::WMWindowType,
      XA_ATOM,
      32,
      &window_type_dock,
      1
    );
    if (*config).bar_opacity != 100 {
      let atom = XInternAtom (display, c_str! ("_NET_WM_WINDOW_OPACITY"), X_FALSE);
      let value = 42949672u32 * (*config).bar_opacity as u32;
      set_cardinal! (window, atom, value);
    }
    // We don't want to interact with the blank part, instead the widgets
    // use `Window_Kind::Status_Bar`.
    set_window_kind (window, Window_Kind::Meta_Or_Unmanaged);
    XMapRaised (display, window);
    Self {
      width,
      height,
      window,
      last_scroll_time: 0,
      left_widgets: Vec::new (),
      right_widgets: Vec::new (),
      mouse_widget: widget::null_ptr ()
    }
  }

  /// Adds widgets
  pub unsafe fn build (&mut self) {
    self.left_widgets.push (
      Box::new (widget::Workspace_Widget::new ())
    );
    self.right_widgets.push (
      Box::new (widget::DateTime::new ())
    );
    if let Some (_) = crate::platform::get_volume_info () {
      // If this fails it's probably because `amixer` is not available
      self.right_widgets.push (
        Box::new (widget::Volume::new ())
      );
    }
    // TODO: also detect if battery information is available
    self.right_widgets.push (
      Box::new (widget::Battery::new ())
    );
  }

  pub unsafe fn draw (&mut self) {
    if !cfg! (feature="bar") { return; }
    (*draw).select_font((*config).bar_font.as_str ());

    // Left
    let mut x;
    x = 0;
    for w in self.left_widgets.iter_mut () {
      let width = w.update (self.height, Self::WIDGET_GAP as u32);
      XMoveWindow (display, w.window (), x, 0);
      x += width as i32;
      x += Self::WIDGET_GAP;
    }
    let mid_x = x;

    // Right
    x = (self.width - Self::RIGHT_GAP) as i32;
    let mut first = true;
    for w in self.right_widgets.iter_mut () {
      if first {
        let width = w.update (self.height, Self::RIGHT_GAP);
        x -= width as i32;
        XMoveWindow (display, w.window (), x, 0);
        first = false;
      } else {
        let width = w.update (self.height, Self::WIDGET_GAP as u32);
        x -= width as i32;
        x -= Self::WIDGET_GAP;
        XMoveWindow (display, w.window (), x, 0);
      }
    }
    let mid_width = (x - mid_x) as u32;

    XMoveResizeWindow (display, self.window, mid_x, 0, mid_width, self.height);
    XClearWindow (display, self.window);
    XSync (display, X_FALSE);
  }

  pub unsafe fn resize (&mut self, width: u32) {
    XResizeWindow (display, self.window, width, self.height);
    self.width = width;
    self.draw ();
  }

  pub unsafe fn click (&mut self, window: Window, event: &XButtonEvent) {
    // Limit scroll speed
    // We just do this for all button types as it doesn't matter for normal
    // button presses.
    if (event.time - self.last_scroll_time) <= (1000 / 10) {
      return;
    }
    self.last_scroll_time = event.time;
    if self.mouse_widget.is_null () {
      log::debug! ("MOUSE WIDGET IS NULL");
    } else if (*self.mouse_widget).window () != window {
      log::debug! ("MOUSE WIDGET HAS DIFFERENT WINDOW");
    }else {
      (*self.mouse_widget).click (event);
    }
  }

  pub unsafe fn enter (&mut self, window: Window) {
    for w in self.left_widgets.iter_mut ().chain (self.right_widgets.iter_mut ()) {
      if w.window () == window {
        self.mouse_widget = w.as_mut ();
        w.enter ();
        return;
      }
    }
  }

  pub unsafe fn leave (&mut self, window: Window) {
    for w in self.left_widgets.iter_mut ().chain (self.right_widgets.iter_mut ()) {
      if w.window () == window {
        w.leave ();
        self.mouse_widget = widget::null_ptr ();
        return;
      }
    }
  }
}

impl Drop for Bar {
  fn drop (&mut self) {
    // TODO: same issue as with tooltip, display should already be closed here
    for w in self.left_widgets.iter () {
      unsafe { XDestroyWindow (display, w.window ()); }
    }
    for w in self.right_widgets.iter () {
      unsafe { XDestroyWindow (display, w.window ()); }
    }
  }
}

/// Redraws the bar if the "bar" feature is enabled
pub fn update () {
  // Note: bar.draw already does the feature check, this is mostly just a safe wrapper
  unsafe { bar.draw (); }
}
