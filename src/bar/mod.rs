use x11::xlib::*;
use std::ffi::CString;
use libc::{c_char, c_uchar, c_uint};
use crate::core::*;
use crate::cursor;
use crate::property;
use crate::action::select_workspace;
use crate::window_title;


pub struct Bar {
  pub width: u32,
  pub height: u32,
  pub window: Window,
  last_scroll_time: Time
}

impl Bar {
  pub const fn new () -> Self {
    Self {
      width: 0,
      height: 0,
      window: X_NONE,
      last_scroll_time: 0
    }
  }

  pub unsafe fn create () -> Self {
    let screen = XDefaultScreen (display);
    let mut wa: XSetWindowAttributes = uninitialized! ();
    wa.override_redirect = X_TRUE;
    wa.background_pixmap = ParentRelative as u64;
    wa.event_mask = ButtonPressMask|ExposureMask;
    let mut class_hint = XClassHint {
      res_name: c_str! ("window_manager_bar") as *mut c_char,
      res_class: c_str! ("window_manager_bar") as *mut c_char
    };
    let width = screen_size.w as u32;
    // TODO: based on config; font_size + something
    let height = 36;
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
      CWOverrideRedirect|CWBackPixmap|CWEventMask, &mut wa
    );
    XDefineCursor (display, window, cursor::normal);
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
    XMapRaised (display, window);
    Self {
      width,
      height,
      window,
      last_scroll_time: 0
    }
  }

  pub unsafe fn draw (&self) {
    if !cfg! (feature="bar") { return; }
    (*draw).select_font((*config).bar_font.as_str ());
    (*draw).rect (0, 0, bar.width, bar.height, (*config).colors.bar_background.pixel, true);
    // ==== LEFT ====
    // Workspaces
    for (idx, workspace) in workspaces.iter ().enumerate () {
      (*draw).rect (
        (idx as u32 * self.height) as i32, 0,
        self.height, self.height,
        if workspace.has_urgent () {
          (*config).colors.bar_urgent_workspace.pixel
        }
        else if idx == active_workspace {
          (*config).colors.bar_active_workspace.pixel
        } else {
          (*config).colors.bar_workspace.pixel
        },
        true
      );
      (*draw).text_in_rect (
        (idx as u32 * self.height) as i32, 0,
        self.height as i32, self.height as i32,
        format! ("{}", idx+1).as_str (),
        if workspace.has_urgent () {
          (*config).colors.bar_urgent_workspace_text
        }
        else if idx == active_workspace {
          (*config).colors.bar_active_workspace_text
        } else {
          (*config).colors.bar_text
        },
        true,
        true
      );
    }
    // Active window title
    if let Some (f) = focused_client! () {
      let title = window_title (f.window);
      (*draw).text_in_rect(
        (self.height * workspaces.len () as u32 + 20) as i32,
        0,
        0,
        self.height as i32,
        title.as_str (),
        (*config).colors.bar_text,
        true,
        false
      );
    }
    // ==== RIGHT ====
    // Time
    let now= chrono::Local::now ();
    let time_text = format! ("{}", now.format ("%a %b %e %T %Y"));
    let x = (*draw).text_right(
      (self.width - 10) as i32,
      self.height as i32,
      time_text.as_str (),
      (*config).colors.bar_text
    );
    // Battery
    let power_supply = "BAT0";
    let mut capacity = std::fs::read_to_string (
      format! ("/sys/class/power_supply/{}/capacity", power_supply)
    ).expect("Could not read battery status");
    capacity.pop ();
    let mut status = std::fs::read_to_string (
      format! ("/sys/class/power_supply/{}/status", power_supply)
    ).expect("Could not read battery status");
    status.pop ();
    let battery_text = format! ("{}:{}%({})", power_supply, capacity, status);
    (*draw).text_right (
      x - 20,
      self.height as i32,
      battery_text.as_str (),
      (*config).colors.bar_text
    );

    (*draw).render (bar.window, 0, 0, bar.width, bar.height);
  }

  pub unsafe fn button_press (&mut self, event: &XButtonEvent) {
    // Limit scroll speed
    // We just do this for all button types as it doesn't matter for normal
    // button presses.
    if (event.time - self.last_scroll_time) <= (1000 / 10) {
      return;
    }
    self.last_scroll_time = event.time;
    // Ignore clicks outside of workspace widget
    if event.x <= (self.height * workspaces.len () as u32) as i32 {
      if event.button == Button1 || event.button == Button2 || event.button == Button3 {
        // Left/Middle/Right click selects workspace under cursor
        select_workspace (event.x as usize / self.height as usize, None);
      }
      else if event.button == Button5 {
        // Scrolling up selects the next workspace
        select_workspace ((active_workspace + 1) % workspaces.len (), None)
      }
      else if event.button == Button4 {
        // Scrolling down selects the previous workspace
        if active_workspace == 0 {
          select_workspace (workspaces.len () - 1, None);
        }
        else {
          select_workspace (active_workspace - 1, None);
        }
      }
    }
  }
}

