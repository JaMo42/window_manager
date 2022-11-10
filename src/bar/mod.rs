mod tray_client;
pub mod tray_manager;
mod widget;
mod xembed;

use crate::core::*;
use crate::cursor;
use crate::ewmh;
use crate::property;
use crate::{set_window_kind, set_window_opacity};
use std::ffi::CString;
use tray_manager::Tray_Manager;
use x11::xlib::*;

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
  mouse_widget: *mut dyn Widget,
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
      mouse_widget: widget::null_ptr (),
    }
  }

  pub unsafe fn create () -> Self {
    let screen = XDefaultScreen (display);
    let mut attributes: XSetWindowAttributes = uninitialized! ();
    attributes.override_redirect = X_TRUE;
    attributes.background_pixel = (*config).colors.bar_background.pixel;
    attributes.event_mask = ButtonPressMask | ExposureMask;
    attributes.cursor = cursor::normal;
    let mut class_hint = XClassHint {
      res_name: c_str! ("window_manager_bar") as *mut libc::c_char,
      res_class: c_str! ("window_manager_bar") as *mut libc::c_char,
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
      XDefaultVisual (display, screen),
      CWOverrideRedirect | CWBackPixel | CWEventMask | CWCursor,
      &mut attributes,
    );
    XSetClassHint (display, window, &mut class_hint);
    ewmh::set_window_type (window, property::Net::WMWindowTypeDock);
    set_window_opacity (window, (*config).bar_opacity);
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
      mouse_widget: widget::null_ptr (),
    }
  }

  /// Adds widgets
  pub unsafe fn build (&mut self) {
    macro_rules! push {
      ($target:expr, $widget:ident) => {
        if let Some (w) = widget::$widget::new () {
          $target.push (Box::new (w));
        } else {
          log::warn! ("Could not create widget: {}", stringify! ($widget));
        }
      };
    }
    push! (self.left_widgets, Workspaces);
    push! (self.right_widgets, DateTime);
    push! (self.right_widgets, Volume);
    push! (self.right_widgets, Battery);
  }

  pub unsafe fn draw (&mut self) {
    if !cfg! (feature = "bar") {
      return;
    }
    (*draw).select_font ((*config).bar_font.as_str ());

    // Left
    let mut x = 0;
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

  pub fn invalidate_widgets (&mut self) {
    for w in self
      .left_widgets
      .iter_mut ()
      .chain (self.right_widgets.iter_mut ())
    {
      w.invalidate ();
    }
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
    } else {
      (*self.mouse_widget).click (event);
    }
  }

  pub unsafe fn enter (&mut self, window: Window) {
    for w in self
      .left_widgets
      .iter_mut ()
      .chain (self.right_widgets.iter_mut ())
    {
      if w.window () == window {
        self.mouse_widget = w.as_mut ();
        w.enter ();
        return;
      }
    }
  }

  pub unsafe fn leave (&mut self, _window: Window) {
    (*self.mouse_widget).leave ();
    self.mouse_widget = widget::null_ptr ();
  }
}

impl Drop for Bar {
  fn drop (&mut self) {
    // TODO: same issue as with tooltip, display should already be closed here
    for w in self.left_widgets.iter () {
      unsafe {
        XDestroyWindow (display, w.window ());
      }
    }
    for w in self.right_widgets.iter () {
      unsafe {
        XDestroyWindow (display, w.window ());
      }
    }
  }
}

/// Redraws the bar if the "bar" feature is enabled
pub fn update () {
  // Note: bar.draw already does the feature check, this is mostly just a safe wrapper
  unsafe {
    bar.draw ();
  }
}
