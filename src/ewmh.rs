use x11::xlib::*;
use super::core::*;
use super::property::{self, Net};

pub unsafe fn set_window_type (window: Window, type_: Net) {
  property::set (
    window,
    Net::WMWindowType,
    XA_ATOM,
    32,
    &property::atom (type_),
    1
  );
}

/// Maybe handles a client message to a client window, returns whether the
/// message was handled or not.
pub unsafe fn client_message (event: &XClientMessageEvent) -> bool {
  false
}

/// Maybe handles a client message to the root window, returns whether the
/// message was handled or not.
pub unsafe fn root_message (event: &XClientMessageEvent) -> bool {
  false
}
