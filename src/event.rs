use std::cmp::max;
use rand::Rng;
use x11::xlib::*;
use super::core::*;
use super::config::*;
use super::action;
use super::property::{Net};
use super::*;

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
  if meta_windows.contains (&event.subwindow) {
    return;
  }
  if let Some (client) = win2client (event.subwindow) {
    if client.is_fullscreen {
      return;
    }
  }
  mouse = (Some (*event), get_window_geometry (event.subwindow));
  workspaces[active_workspace].focus (event.subwindow);
}


pub unsafe fn motion (event: &XButtonEvent) {
  if let Some (s) = mouse.0 {
    if s.subwindow == X_NONE {
      return;
    }
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
  if let Some (action) = (*config).get (event.keycode, event.state) {
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
        run_process (cmd.clone ());
      },
      Action::Generic (f) => {
        f ();
      }
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
    XMapWindow (display, window);
    log::info! ("New meta window: {} ({})", name, window);
  }
  else {
    // Give client random position inside window area and clamp its size into
    // the window area
    let mut rng = rand::thread_rng ();
    let mut c = Client::new (window);
    let mut g = c.geometry;
    if g.w < window_area.w {
      let max_x = (window_area.w - g.w) as i32 + window_area.x;
      g.x = rng.gen_range (window_area.x..=max_x);
    }
    else {
      g.x = window_area.x;
      g.w = window_area.w - ((*config).border_width << 1) as u32;
    }
    if g.h < window_area.h {
      let max_y = (window_area.h - g.h) as i32 + window_area.y;
      g.y = rng.gen_range (window_area.y..=max_y);
    }
    else {
      g.y = window_area.y;
      g.h = window_area.h - ((*config).border_width << 1) as u32;
    }
    c.move_and_resize (g);
    workspaces[active_workspace].push (c);
    property::append (root, Net::ClientList, XA_WINDOW, 32, &window, 1);
    log::info! ("Mapped new client: '{}' ({})", name, window);
  }
}


pub unsafe fn enter (event: &XCrossingEvent) {
  if event.subwindow == X_NONE {
    return;
  }
  log::info! ("EnterNotify: '{}' ({})", window_title (event.subwindow), event.subwindow);
}


pub unsafe fn configure_request (event: &XConfigureRequestEvent) {
  log::trace! ("configure_request: '{}' ({})", window_title (event.window), event.window);
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

pub unsafe fn property_notify (event: &XPropertyEvent) {
  log::trace! ("property_notify");
  if event.state == PropertyDelete {
    return;
  }
  else if let Some (client) = win2client (event.window) {
    match event.atom {
      XA_WM_HINTS => { client.update_hints (); }
      _ => {}
    }
  }
}

pub unsafe fn unmap_notify (_event: &XUnmapEvent) {
  log::trace! ("unmap_notify ");
}

pub unsafe fn configure_notify (event: &XConfigureEvent) {
  log::trace! ("configure_notify: '{}' ({})", window_title (event.window), event.window);
}

pub unsafe fn client_message (event: &XClientMessageEvent) {
  if let Some (client) = win2client (event.window) {
    log::debug! ("Client message: {}", event.message_type);
    log::debug! ("  Recipient: {}", client);
    log::debug! ("  Data (longs): {:?}", event.data.as_longs ());
    if event.message_type == property::atom (Net::WMState) {
      // _NET_WM_STATE
      let data = event.data.as_longs ();
      macro_rules! new_state {
        ($member:ident) => {
          data[0] == 1 || (data[0] == 2 && !client.$member)
        }
      }
      if data[1] as Atom == property::atom (Net::WMStateFullscreen)
        || data[2] as Atom == property::atom (Net::WMStateFullscreen) {
        // _NET_WM_STATE_FULLSCREEN
        client.set_fullscreen (new_state! (is_fullscreen));
      }
      if data[1] as Atom == property::atom (Net::WMStateDemandsAttention)
        || data[2] as Atom == property::atom (Net::WMStateDemandsAttention) {
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
  else if event.window == root {
    log::debug! ("Client message: {}", event.message_type);
    log::debug! ("  Recipient: <root> ({})", event.window);
    log::debug! ("  Data (longs): {:?}", event.data.as_longs ());
    if event.message_type == property::atom (Net::CurrentDesktop) {
      action::select_workspace (event.data.get_long (0) as usize, None);
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

