use std::cmp::max;
use rand::Rng;
use x11::xlib::*;
use super::core::*;
use super::config::*;
use super::action;
use super::property::{Net};
use super::*;

macro_rules! ignore_meta_window {
  ($win:expr) => {
    if meta_windows.contains (&$win) {
      return;
    }
  }
}


unsafe fn win2client<'a> (window: Window) -> Option<&'a mut Client> {
  for ws in workspaces.iter_mut () {
    for c in ws.iter_mut () {
      if c.window == window {
        return Some (c);
      }
    }
  }
  None
}


pub unsafe fn button_press (event: &XButtonEvent) {
  if event.subwindow == X_NONE {
    return;
  }
  ignore_meta_window! (event.subwindow);
  mouse = (Some (*event), get_window_geometry (event.subwindow));
  workspaces[active_workspace].focus (event.subwindow);
}


pub unsafe fn motion (event: &XButtonEvent) {
  if let Some (s) = mouse.0 {
    if s.subwindow == X_NONE {
      return;
    }
    ignore_meta_window! (s.subwindow);
    let xd = event.x_root - s.x_root;
    let yd = event.y_root - s.y_root;
    if s.button == 1 {
      // Move
      XMoveResizeWindow (
        display,
        s.subwindow,
        mouse.1.x + xd,
        mouse.1.y + yd,
        mouse.1.w as c_uint,
        mouse.1.h as c_uint
      );
    }
    else if s.button == 3 {
      // Resize
      XMoveResizeWindow (
        display,
        s.subwindow,
        mouse.1.x,
        mouse.1.y,
        max (1, mouse.1.w as c_int + xd) as c_uint,
        max (1, mouse.1.h as c_int + yd) as c_uint
      );
    }
  }
}


pub unsafe fn button_release (event: &XButtonEvent) {
  if let Some (s) = mouse.0 {
    if s.subwindow != X_NONE {
      ignore_meta_window! (s.subwindow);
      if s.button == 1 && s.state == (*config).modifier | MOD_SHIFT {
        action::move_snap (
          win2client (s.subwindow).unwrap (),
          event.x_root as u32,
          event.y_root as u32
        );
      }
      else {
        // Commit the new window geometry into the client
        let c = &mut win2client (s.subwindow).unwrap ();
        c.geometry = get_window_geometry (s.subwindow);
        c.prev_geometry = c.geometry;
      }
    }
  }
  mouse.0 = None;
}


pub unsafe fn key_press (event: &XKeyEvent) {
  let action = (*config).get (event.keycode, event.state);
  match action {
    Action::WM (f) => {
      if let Some (client) = focused_client! () {
        f (client);
      }
    },
    Action::WS (f, workspace_index, requires_window) => {
      let maybe_client = focused_client! ();
      if *requires_window && maybe_client.is_none () {
        return;
      }
      f (*workspace_index, maybe_client);
    },
    Action::Launch (cmd) => {
      run_process (cmd);
    },
    Action::Generic (f) => {
      f ();
    }
  }
}


pub unsafe fn map_request (event: &XMapRequestEvent) {
  for ws in workspaces.iter () {
    for c in ws.iter () {
      if c.window == event.window {
        XMapWindow (display, event.window);
        return;
      }
    }
  }
  // New client
  let window = event.window;
  let name = window_title (window);
  if name == "window_manager_bar" || (*config).meta_window_names.contains (&name) {
    meta_windows.push (window);
    XSelectInput (display, window, NoEventMask);
    XUngrabKey (display, AnyKey, AnyModifier, window);
    XMapWindow (display, window);
    log::info! ("New meta window: {} ({})", name, window);
  }
  else {
    // Give client random position inside window area
    let mut rng = rand::thread_rng ();
    let mut c = Client::new (window);
    let mut g = c.geometry;
    if g.w < window_area.w {
      let max_x = (window_area.w - g.w) as i32 + window_area.x;
      g.x = rng.gen_range (window_area.x..=max_x);
    }
    else {
      g.x = window_area.x;
      g.w = window_area.w;
    }
    if g.h < window_area.h {
      let max_y = (window_area.h - g.h) as i32 + window_area.y;
      g.y = rng.gen_range (window_area.y..=max_y);
    }
    else {
      g.y = window_area.y;
      g.h = window_area.h;
    }
    c.move_and_resize (g);
    workspaces[active_workspace].push (c);
    property::append (root, Net::ClientList, XA_WINDOW, 32, &window, 1);
    log::info! ("Mapped new client: {} ({})", name, window);
  }
}


pub unsafe fn enter (event: &XCrossingEvent) {
  if event.subwindow == X_NONE {
    return;
  }
  log::info! ("EnterNotify: {} ({})", window_title (event.subwindow), event.subwindow);
}


pub unsafe fn configure_request (event: &XConfigureRequestEvent) {
  XConfigureWindow (
    display, event.window, event.value_mask as u32,
    &mut XWindowChanges {
      x: event.x,
      y: event.y,
      width: event.width,
      height: event.height,
      border_width: event.border_width,
      sibling: event.above,
      stack_mode: event.detail
    }
  );
}

pub unsafe fn property_notify (_event: &XPropertyEvent) {
  log::trace! ("Event: property_notify");
}

pub unsafe fn unmap_notify (_event: &XUnmapEvent) {
  log::trace! ("Event: unmap_notify ");
}

pub unsafe fn configure_notify (_event: &XConfigureEvent) {
  log::trace! ("Event: configure_notify");
}

pub unsafe fn client_message (event: &XClientMessageEvent) {
  if let Some (mut client) = win2client (event.window) {
    log::debug! ("Client message: {}", event.message_type);
    if event.message_type == property::atom (Net::WMState) {
      // _NET_WM_STATE
      let data = event.data.as_longs ();
      if data[1] as Atom == property::atom (Net::WMStateFullscreen)
        || data[2] as Atom == property::atom (Net::WMStateFullscreen) {
        // _NET_WM_STATE_FULLSCREEN
        // TODO: for now uses the fullscreen snapping instead of actual fullscreen
        // (the snapping should probably be renamed to 'maximized')
        if data[0] == 1 || (data[0] == 2 && !client.is_snapped) {
          action::snap (&mut client, SNAP_FULLSCREEN);
          property::set (client.window, Net::WMState, XA_ATOM, 32,
            &property::atom (Net::WMStateFullscreen), 1);
        }
        else if client.is_snapped {
          property::set (client.window, Net::WMState, XA_ATOM, 32,
            std::ptr::null::<c_uchar> (), 0);
          client.unsnap ();
        }
      }
      if data[1] as Atom == property::atom (Net::WMStateDemandsAttention)
        || data[2] as Atom == property::atom (Net::WMStateDemandsAttention) {
        // _NET_WM_STATE_DEMANDS_ATTENTION
        client.set_urgency (data[0] == 1 || (data[0] == 2 && !client.is_urgent));
      }
    }
    else if event.message_type == property::atom (Net::ActiveWindow) {
      // This is what DWM uses for urgency
      {
        let f = focused_client! ();
        if f.is_some () && *f.unwrap () == *client {
          return;
        }
      }
      client.set_urgency (true);
    }
  }
}

pub unsafe fn mapping_notify (event: &XMappingEvent) {
  let mut ev = event.clone ();
  XRefreshKeyboardMapping (&mut ev);
  if ev.request == MappingKeyboard {
    grab_keys ();
  }
}

pub unsafe fn destroy_notify (event: &XDestroyWindowEvent) {
  let window = event.window;
  for ws_idx in 0..workspaces.len () {
    for client in workspaces[ws_idx].iter () {
      if client.window == window {
        workspaces[ws_idx].remove (client);
      }
    }
  }
  update_client_list ();
}

