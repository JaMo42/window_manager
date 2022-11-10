use crate::core::*;
use crate::property::{self, XEmbed};
use libc::c_long;
use x11::xlib::*;

const VERSION: u32 = 5;

const EMBEDDED_NOTIFY: c_long = 0;

const FLAG_MAPPED: u32 = 1 << 0;

pub struct Info {
  version: u32,
  flags: u32,
}

impl Info {
  pub const fn new () -> Self {
    Self {
      version: 0,
      flags: 0,
    }
  }

  pub unsafe fn query (&mut self, window: Window) -> bool {
    // TODO: this generates a lot of BadWindow errors for some reason
    if let Some (data) = property::get_data_for_array::<u32, _> (
      window,
      XEmbed::Info,
      property::atom (XEmbed::Info),
      2,
      0,
    ) {
      self.version = data.value_at (0);
      self.flags = data.value_at (0);
      true
    } else {
      false
    }
  }

  pub fn version (&self) -> u32 {
    self.version
  }

  pub fn is_mapped (&self) -> bool {
    (self.flags & FLAG_MAPPED) == FLAG_MAPPED
  }
}

pub unsafe fn send_message (recipient: Window, msg: c_long, d1: c_long, d2: c_long, d3: c_long) {
  let mut event: XEvent = std::mem::zeroed ();
  let message = &mut event.client_message;
  message.window = recipient;
  message.message_type = property::atom (XEmbed::XEmbed);
  message.format = 32;
  message.data.set_long (0, CurrentTime as c_long);
  message.data.set_long (1, msg);
  message.data.set_long (2, d1);
  message.data.set_long (3, d2);
  message.data.set_long (4, d3);
  XSendEvent (display, recipient, X_FALSE, NoEventMask, &mut event);
  XSync (display, X_FALSE);
}

pub unsafe fn embed (window: Window, parent: Window, version: u32) {
  send_message (
    window,
    EMBEDDED_NOTIFY,
    0,
    parent as c_long,
    u32::min (version, VERSION) as i64,
  );
}
