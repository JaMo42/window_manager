use crate::core::*;
use crate::x::XNone;
use x11::xfixes::*;
use x11::xlib::*;

// https://code.woboq.org/gtk/include/X11/extensions/xfixeswire.h.html#_M/XFixesSelectionNotify
const XFixesSelectionNotify: i32 = 0;

// https://code.woboq.org/gtk/include/X11/extensions/xfixeswire.h.html#_M/XFixesSetSelectionOwnerNotifyMask
const XFixesSetSelectionOwnerNotifyMask: u64 = 1 << 0;

static mut hack_active: bool = false;

pub fn get_selection_notify_event_type() -> i32 {
  let mut event_base = 0;
  let mut error_base = 0;
  unsafe {
    if XFixesQueryExtension(display.as_raw(), &mut event_base, &mut error_base) == True {
      event_base + XFixesSelectionNotify
    } else {
      -1
    }
  }
}

pub unsafe fn listen() {
  let selection = display.intern_atom("XdndSelection");
  XFixesSelectSelectionInput(
    display.as_raw(),
    display.root(),
    selection,
    XFixesSetSelectionOwnerNotifyMask,
  );
}

pub fn selection_notify(event: &XFixesSelectionNotifyEvent) {
  if event.owner == XNone {
    unsafe { hack_end() };
  } else {
    unsafe { hack_start() };
  }
}

// FIXME:
// In theory supporting XDND should be easy, the frame windows receive the
// client messages and we just forward them to the client windows.
// But for some reason doing that just won't for me so for now it is supporting
// using this hack:
// When a windows takes ownership of the XdndSelection selection we just
// reparent all windows back onto the root window so they receive the messages
// directly and afterwards put them back into their frames.
//
// Some programs seem to not clear the selection (observed when dragging
// something from chromium onto itself), for this reason `hack_end` will also
// be called whenever the root window is clicked as an easy way to fix clients.

unsafe fn hack_start() {
  if hack_active {
    return;
  }
  log::trace!("XDND hack: reparenting all clients to root");
  for client in workspaces[active_workspace].iter().rev() {
    let g = client.client_geometry();
    client.frame.raise();
    client.window.reparent(root, g.x, g.y);
  }
  display.flush();
  hack_active = true;
}

unsafe fn hack_end() {
  if !hack_active {
    return;
  }
  log::trace!("XDND hack: reparenting all clients back into their frame");
  for client in workspaces[active_workspace].iter().rev() {
    let offset = client.frame_offset();
    client.window.reparent(client.frame, offset.x, offset.y);
  }
  workspaces[active_workspace].focus_client(0);
  display.flush();
  hack_active = false;
}

pub fn ensure_hack_stopped() {
  unsafe { hack_end() };
}
