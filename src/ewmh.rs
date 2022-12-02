use super::action;
use super::client::{Client, Client_Geometry};
use super::core::*;
use super::event::{mouse_move, mouse_resize};
use super::geometry::Geometry;
use super::property::{self, atom, Net, WM};
use crate::x::Window;
use libc::c_long;
use x11::xlib::*;

// https://specifications.freedesktop.org/wm-spec/wm-spec-1.3.html#idm46435610090352
const _NET_WM_MOVERESIZE_SIZE_TOPLEFT: c_long = 0;
const _NET_WM_MOVERESIZE_SIZE_TOP: c_long = 1;
const _NET_WM_MOVERESIZE_SIZE_TOPRIGHT: c_long = 2;
const _NET_WM_MOVERESIZE_SIZE_RIGHT: c_long = 3;
const _NET_WM_MOVERESIZE_SIZE_BOTTOMRIGHT: c_long = 4;
const _NET_WM_MOVERESIZE_SIZE_BOTTOM: c_long = 5;
const _NET_WM_MOVERESIZE_SIZE_BOTTOMLEFT: c_long = 6;
const _NET_WM_MOVERESIZE_SIZE_LEFT: c_long = 7;
const _NET_WM_MOVERESIZE_MOVE: c_long = 8;

pub unsafe fn set_window_type (window: Window, type_: Net) {
  property::set (
    window,
    Net::WMWindowType,
    XA_ATOM,
    32,
    &property::atom (type_),
    1,
  );
}

unsafe fn wm_change_state (client: &mut Client, state: c_long) {
  const NormalState: c_long = 1;
  const IconicState: c_long = 3;
  if state == NormalState {
    if client.workspace == active_workspace {
      workspaces[active_workspace].focus (client.window)
    } else {
      // `client.unminimize` would map it
      client.is_minimized = false;
      set_net_wm_state (client, &[]);
    }
  } else if state == IconicState {
    action::minimize (client);
  }
}

pub unsafe fn set_net_wm_state (client: &mut Client, atoms: &[Atom]) {
  property::set (
    client.window,
    Net::WMState,
    XA_ATOM,
    32,
    atoms.as_ptr (),
    atoms.len () as i32,
  );
}

unsafe fn net_wm_state (client: &mut Client, event: &XClientMessageEvent) {
  let data = event.data.as_longs ();
  macro_rules! new_state {
    ($member:ident) => {
      data[0] == 1 || (data[0] == 2 && !client.$member)
    };
  }
  if data[1] as Atom == atom (Net::WMStateFullscreen)
    || data[2] as Atom == atom (Net::WMStateFullscreen)
  {
    // _NET_WM_STATE_FULLSCREEN
    client.set_fullscreen (new_state! (is_fullscreen));
  }
  if data[1] as Atom == atom (Net::WMStateDemandsAttention)
    || data[2] as Atom == atom (Net::WMStateDemandsAttention)
  {
    // _NET_WM_STATE_DEMANDS_ATTENTION
    {
      // Don't set if already focused
      let f = focused_client! ();
      if f.is_some () && *f.unwrap () == *client {
        return;
      }
    }
    client.set_urgency (new_state! (is_urgent));
  }
  // Horizontal and Vertical maximization are both treated as maximizing in
  // both directions
  if data[1] as Atom == atom (Net::WMStateMaximizedHorz)
    || data[2] as Atom == atom (Net::WMStateMaximizedHorz)
    || data[1] as Atom == atom (Net::WMStateMaximizedVert)
    || data[2] as Atom == atom (Net::WMStateMaximizedVert)
  {
    if data[0] == 1 || (data[0] == 2 && (client.snap_state & SNAP_MAXIMIZED) != SNAP_MAXIMIZED) {
      action::snap (client, SNAP_MAXIMIZED);
      set_net_wm_state (
        client,
        &[
          atom (Net::WMStateMaximizedHorz),
          atom (Net::WMStateMaximizedVert),
        ],
      );
    } else {
      client.unsnap ();
      set_net_wm_state (client, &[]);
    }
    workspaces[active_workspace].focus (client.window);
  }
}

unsafe fn net_wm_moveresize (client: &mut Client, event: &XClientMessageEvent) {
  //let x_root = event.data.get_long (0);
  //let y_root = event.data.get_long (1);
  let direction = event.data.get_long (2);
  //let button = event.data.get_long (3);
  //let source_indication = event.data.get_long (0);

  // Note: resizing from the left, top, or any corner that's not the bottom-right
  //       corner is kinda weird since the mouse_resize expects to resize in the
  //       bottom and/or right direction, we could just ignore them but it's
  //       probably nicer to have them anyways.

  if client.workspace == active_workspace {
    workspaces[active_workspace].focus (client.window);
  }

  if direction == _NET_WM_MOVERESIZE_MOVE && client.may_move () {
    mouse_move (client);
  } else if (direction == _NET_WM_MOVERESIZE_SIZE_LEFT
    || direction == _NET_WM_MOVERESIZE_SIZE_RIGHT)
    && client.may_resize ()
  {
    mouse_resize (client, false, true);
  } else if (direction == _NET_WM_MOVERESIZE_SIZE_TOP
    || direction == _NET_WM_MOVERESIZE_SIZE_BOTTOM)
    && client.may_resize ()
  {
    mouse_resize (client, true, false);
  } else if (direction == _NET_WM_MOVERESIZE_SIZE_TOPLEFT
    || direction == _NET_WM_MOVERESIZE_SIZE_TOPRIGHT
    || direction == _NET_WM_MOVERESIZE_SIZE_BOTTOMRIGHT
    || direction == _NET_WM_MOVERESIZE_SIZE_BOTTOMLEFT)
    && client.may_resize ()
  {
    mouse_resize (client, false, false);
  }
  // _NET_WM_MOVERESIZE_SIZE_KEYBOARD and _NET_WM_MOVERESIZE_MOVE_KEYBOARD are
  // not implemented.
}

unsafe fn net_moveresize_window (client: &mut Client, event: &XClientMessageEvent) {
  client.snap_state = SNAP_NONE;
  client.move_and_resize (Client_Geometry::Frame (Geometry::from_parts (
    event.data.get_long (1) as i32,
    event.data.get_long (2) as i32,
    event.data.get_long (3) as u32,
    event.data.get_long (4) as u32,
  )));
  client.save_geometry ();
  // Not sure if this should focus or not, for now I'll go without.
  //if client.workspace == active_workspace {
  //  workspaces[active_workspace].focus (client.window);
  //}
}

/// Maybe handles a client message to a client window, returns whether the
/// message was handled or not.
pub unsafe fn client_message (client: &mut Client, event: &XClientMessageEvent) -> bool {
  if event.message_type == atom (Net::WMState) {
    // _NET_WM_STATE
    net_wm_state (client, event);
  } else if event.message_type == atom (Net::ActiveWindow) {
    // This is what DWM uses for urgency
    {
      let f = focused_client! ();
      if f.is_some () && *f.unwrap () == *client {
        return true;
      }
    }
    if client.workspace == active_workspace {
      workspaces[active_workspace].focus (client.window);
    } else {
      client.set_urgency (true);
    }
  } else if event.message_type == atom (WM::ChangeState) {
    wm_change_state (client, event.data.get_long (0));
  } else if event.message_type == atom (Net::WMMoveresize) {
    net_wm_moveresize (client, event);
  } else if event.message_type == atom (Net::MoveresizeWindow) {
    net_moveresize_window (client, event);
  } else {
    return false;
  }
  true
}

/// Maybe handles a client message to the root window, returns whether the
/// message was handled or not.
pub unsafe fn root_message (event: &XClientMessageEvent) -> bool {
  if event.message_type == atom (Net::CurrentDesktop) {
    action::select_workspace (event.data.get_long (0) as usize, None);
  } else {
    return false;
  }
  true
}

pub unsafe fn set_allowed_actions (window: Window, may_resize: bool) {
  let mut actions = vec! [
    atom (Net::WMActionMove),
    atom (Net::WMActionClose),
    atom (Net::WMActionChangeDesktop),
  ];
  if may_resize {
    actions.push (atom (Net::WMActionResize));
    actions.push (atom (Net::WMActionMaximizeHorz));
    actions.push (atom (Net::WMActionMaximizeVert));
    actions.push (atom (Net::WMActionFullscreen));
  }
  property::set (
    window,
    Net::WMAllowedActions,
    XA_ATOM,
    32,
    actions.as_ptr (),
    actions.len () as i32,
  );
}
