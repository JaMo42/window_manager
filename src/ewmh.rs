use libc::c_long;
use x11::xlib::*;
use super::core::*;
use super::property::{self, Net, WM, atom};
use super::client::Client;
use super::action;
use super::event::win2client;

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

unsafe fn wm_change_state (window: Window, state: c_long) {
  const NormalState: c_long = 1;
  const IconicState: c_long = 3;
  if let Some (client) = win2client (window) {
    if state == NormalState {
      if workspaces[active_workspace].contains (window) {
        workspaces[active_workspace].focus (window);
      }
    } else if state == IconicState {
      action::minimize (client);
    }
  }
}

/// Maybe handles a client message to a client window, returns whether the
/// message was handled or not.
pub unsafe fn client_message (client: &mut Client, event: &XClientMessageEvent) -> bool {
  if event.message_type == atom (Net::WMState) {
    // _NET_WM_STATE
    let data = event.data.as_longs ();
    macro_rules! new_state {
      ($member:ident) => { data[0] == 1 || (data[0] == 2 && !client.$member) }
    }
    if data[1] as Atom == atom (Net::WMStateFullscreen)
      || data[2] as Atom == atom (Net::WMStateFullscreen) {
      // _NET_WM_STATE_FULLSCREEN
      client.set_fullscreen (new_state! (is_fullscreen));
    }
    if data[1] as Atom == atom (Net::WMStateDemandsAttention)
      || data[2] as Atom == atom (Net::WMStateDemandsAttention) {
      // _NET_WM_STATE_DEMANDS_ATTENTION
      {
        // Don't set if already focused
        let f = focused_client! ();
        if f.is_some () && *f.unwrap () == *client {
          return true;
        }
      }
      client.set_urgency (new_state! (is_urgent));
    }
  }
  else if event.message_type == atom (Net::ActiveWindow) {
    log::debug! ("_NET_CURRENT_DESKTOP message to client");
    // This is what DWM uses for urgency
    {
      let f = focused_client! ();
      if f.is_some () && *f.unwrap () == *client {
        return true;
      }
    }
    if workspaces[active_workspace].contains (client.window) {
      workspaces[active_workspace].focus (client.window);
    } else {
      client.set_urgency (true);
    }
  }
  else if event.message_type == atom (WM::ChangeState) {
    wm_change_state (event.window, event.data.get_long (0));
  }
  false
}

/// Maybe handles a client message to the root window, returns whether the
/// message was handled or not.
pub unsafe fn root_message (event: &XClientMessageEvent) -> bool {
  if event.message_type == atom (Net::CurrentDesktop) {
    action::select_workspace (event.data.get_long (0) as usize, None);
  }
  false
}
