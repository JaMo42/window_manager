use x11::xlib::*;
use super::core::*;
use super::geometry::*;
use super::*;
use super::property::WM;

#[derive(Copy, Clone)]
pub struct Client {
  pub window: Window,
  pub geometry: Geometry,
  pub prev_geometry: Geometry,
  pub is_snapped: bool,
  pub is_urgent: bool,
  pub is_fullscreen: bool
}

impl Client {
  pub unsafe fn new (window: Window) -> Self {
    let mut wc: XWindowChanges = uninitialized! ();
    wc.border_width = (*config).border_width;
    XConfigureWindow (display, window, CWBorderWidth as u32, &mut wc);
    XMapWindow (display, window);
    let geometry = get_window_geometry (window);
    Client {
      window: window,
      geometry: geometry,
      prev_geometry: geometry,
      is_snapped: false,
      is_urgent: false,
      is_fullscreen: false
    }
  }

  pub unsafe fn move_and_resize (&mut self, target: Geometry) {
    self.geometry = target;
    if self.is_snapped {
      XMoveResizeWindow (
        display, self.window,
        target.x + (*config).gap as i32,
        target.y + (*config).gap as i32,
        target.w - (((*config).gap + (*config).border_width as u32) << 1),
        target.h - (((*config).gap + (*config).border_width as u32) << 1)
      );
    }
    else {
      XMoveResizeWindow (
        display, self.window,
        target.x, target.y,
        target.w, target.h
      );
    }
  }

  pub unsafe fn unsnap (&mut self) {
    self.is_snapped = false;
    self.move_and_resize (self.prev_geometry);
  }

  pub unsafe fn focus (&mut self) {
    if self.is_urgent {
      self.set_urgency (false);
    }
    focus_window (self.window);
    self.send_event (property::atom (WM::TakeFocus));
  }

  pub unsafe fn set_urgency (&mut self, urgency: bool) {
    if urgency == self.is_urgent {
      return;
    }
    self.is_urgent = urgency;
    if urgency {
      XSetWindowBorder (display, self.window, (*config).colors.urgent.pixel);
    }
    let hints = XGetWMHints (display, self.window);
    if !hints.is_null () {
      (*hints).flags = if urgency {
        (*hints).flags | XUrgencyHint
      } else {
        (*hints).flags & !XUrgencyHint
      };
      XSetWMHints (display, self.window, hints);
      XFree (hints as *mut c_void);
    }
  }

  pub unsafe fn update_hints (&mut self) {
    let hints = XGetWMHints (display, self.window);
    if !hints.is_null () {
      if let Some (focused) = focused_client! () {
        if *focused == *self && ((*hints).flags & XUrgencyHint) == 1 {
          // It's being made urgent but it's already the active window
          (*hints).flags &= !XUrgencyHint;
          XSetWMHints (display, self.window, hints);
        }
      }
      else {
        self.is_urgent = ((*hints).flags & XUrgencyHint) == 1;
      }
      XFree (hints as *mut c_void);
    }
  }

  pub unsafe fn send_event (&self, protocol: Atom) -> bool {
    let mut protocols: *mut Atom = std::ptr::null_mut ();
    let mut is_supported = false;
    let mut count: c_int = 0;
    if XGetWMProtocols (display, self.window, &mut protocols, &mut count) != 0 {
      for i in 0..count {
        is_supported = *protocols.add (i as usize) == protocol;
        if is_supported {
          break;
        }
      }
      XFree (protocols as *mut c_void);
    }
    if is_supported {
      let mut event: XEvent = uninitialized! ();
      event.type_ = ClientMessage;
      event.client_message.window = self.window;
      event.client_message.message_type = property::atom (WM::Protocols);
      event.client_message.format = 32;
      event.client_message.data.set_long (0, protocol as i64);
      event.client_message.data.set_long (1, CurrentTime as i64);
      XSendEvent (display, self.window, X_FALSE, NoEventMask, &mut event) != 0
    }
    else {
      false
    }
  }

  pub unsafe fn set_fullscreen (&mut self, state: bool) {
    if state == self.is_fullscreen {
      return;
    }
    self.is_fullscreen = state;
    if state {
      property::set (self.window, Net::WMState, XA_ATOM, 32,
        &property::atom (Net::WMStateFullscreen), 1);
      self.is_snapped = false;
      let border = (*config).border_width;
      self.move_and_resize (Geometry::from_parts (
        -border, -border,
        screen_size.w, screen_size.h
      ));
      XRaiseWindow (display, self.window);
    }
    else {
      property::set (self.window, Net::WMState, XA_ATOM, 32,
        std::ptr::null::<c_uchar> (), 0);
      self.move_and_resize (self.prev_geometry);
    }
  }
}

impl PartialEq for Client {
  fn eq (&self, other: &Self) -> bool {
    self.window == other.window
  }
}

impl std::fmt::Display for Client {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
    write! (f, "'{}' ({})", unsafe { window_title (self.window) }, self.window)
  }
}

