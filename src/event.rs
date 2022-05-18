use std::cmp::max;
use rand::Rng;
use x11::xlib::*;
use super::core::*;
use super::config::*;
use super::action;
use super::property::{Net, atom, get_atom};
use super::*;

pub const MOUSE_MASK: i64 = ButtonPressMask|ButtonReleaseMask|PointerMotionMask;


unsafe fn win2client<'a> (window: Window) -> Option<&'a mut Client> {
  if window == X_NONE {
    return None;
  }
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
    if event.button == 1 && !client.may_move ()
      || event.button == 3 && !client.may_resize () {
      return;
    }
  }
  mouse_held = event.button;
  workspaces[active_workspace].focus (event.subwindow);
}


pub unsafe fn motion (event: &XButtonEvent) {
  if mouse_held != 0 {
    match mouse_held {
      1 => mouse_move (event.subwindow),
      3 => mouse_resize (event.subwindow),
      _ => {}
    }
    mouse_held = 0;
  }
  else {
    // Ignore all subsequent MotionNotify events
    let mut my_event: XEvent = uninitialized! ();
    loop {
      XNextEvent (display, &mut my_event);
      if my_event.type_ != MotionNotify {
        break;
      }
    }
    XPutBackEvent (display, &mut my_event);
  }
}


unsafe fn pointer_position () -> Option<(c_int, c_int)> {
  let mut x: c_int = 0;
  let mut y: c_int = 0;
  // Dummy values
  let mut i: c_int = 0;
  let mut u: c_uint = 0;
  let mut w: Window = X_NONE;
  if XQueryPointer (
    display, root, &mut w, &mut w, &mut x, &mut y, &mut i, &mut i, &mut u
  ) == X_TRUE {
    Some ((x, y))
  }
  else {
    None
  }
}


unsafe fn mouse_move (window: Window) {
  let client: &mut Client;
  if let Some (c) = win2client (window) {
    client = c;
  }
  else {
    return;
  }
  if XGrabPointer (
    display,
    root,
    X_FALSE,
    MOUSE_MASK as u32,
    GrabModeAsync,
    GrabModeAsync,
    X_NONE,
    cursor::moving,
    CurrentTime
  ) != GrabSuccess {
    return;
  }
  let start_x: c_int;
  let start_y: c_int;
  if let Some ((x, y)) = pointer_position () {
    start_x = x;
    start_y = y;
  }
  else {
    return;
  }
  let mut event: XEvent = uninitialized! ();
  let mut last_time: Time = 0;
  let client_x = client.geometry.x;
  let client_y = client.geometry.y;
  let mut mouse_x = 0;
  let mut mouse_y = 0;
  let mut state = 0;
  loop {
    XMaskEvent (display, MOUSE_MASK|SubstructureRedirectMask, &mut event);
    match event.type_ {
      ConfigureRequest => configure_request (&event.configure_request),
      MapRequest => map_request (&event.map_request),
      MotionNotify => {
        let motion = event.motion;
        // Only handle at 60 FPS
        if (motion.time - last_time) <= (1000 / 60) {
          continue;
        }
        last_time = motion.time;
        let new_x = client_x + (motion.x - start_x);
        let new_y = client_y + (motion.y - start_y);
        XMoveResizeWindow (
          display, window,
          new_x, new_y,
          client.prev_geometry.w, client.prev_geometry.h
        );
        mouse_x = motion.x_root;
        mouse_y = motion.y_root;
        state = motion.state;
      }
      ButtonRelease => break,
      _ => {}
    }
  }
  XUngrabPointer (display, CurrentTime);
  #[allow(unused_unsafe)]  // `clean_mods` contains an unsafe block
  if clean_mods! (state) == (*config).modifier | MOD_SHIFT {
    action::move_snap (client, mouse_x as u32, mouse_y as u32);
  }
  else {
    client.geometry = get_window_geometry (window);
    client.prev_geometry = client.geometry;
    client.is_snapped = false;
  }
}


unsafe fn mouse_resize (window: Window) {
  let client: &mut Client;
  if let Some (c) = win2client (window) {
    client = c;
  }
  else {
    return;
  }
  if XGrabPointer (
    display,
    root,
    X_FALSE,
    MOUSE_MASK as u32,
    GrabModeAsync,
    GrabModeAsync,
    X_NONE,
    cursor::resizing,
    CurrentTime
  ) != GrabSuccess {
    return;
  }
  let start_x: c_int;
  let start_y: c_int;
  if let Some ((x, y)) = pointer_position () {
    start_x = x;
    start_y = y;
  }
  else {
    return;
  }
  let mut event: XEvent = uninitialized! ();
  let mut last_time: Time = 0;
  let client_w = client.geometry.w as i32;
  let client_h = client.geometry.h as i32;
  loop {
    XMaskEvent (display, MOUSE_MASK|SubstructureRedirectMask, &mut event);
    match event.type_ {
      ConfigureRequest => configure_request (&event.configure_request),
      MapRequest => map_request (&event.map_request),
      MotionNotify => {
        let motion = event.motion;
        // Only handle at 60 FPS
        if (motion.time - last_time) <= (1000 / 60) {
          continue;
        }
        last_time = motion.time;
        let new_w = max (10, client_w + (motion.x - start_x)) as u32;
        let new_h = max (10, client_h + (motion.y - start_y)) as u32;
        XMoveResizeWindow (
          display, window,
          client.geometry.x, client.geometry.y,
          new_w, new_h
        );
      }
      ButtonRelease => break,
      _ => {}
    }
  }
  XUngrabPointer (display, CurrentTime);
  client.geometry = get_window_geometry (window);
  client.prev_geometry = client.geometry;
  client.is_snapped = false;
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
  let class_hints = property::Class_Hints::new (window);
  if name == "window_manager_bar"
    || (*config).meta_window_classes.contains (&class_hints.class) {
    meta_windows.push (window);
    XMapWindow (display, window);
    log::info! ("New meta window: {} ({})", name, window);
  }
  else {
    let mut c = Client::new (window);
    let mut g = c.geometry;
    // Window type
    if get_atom (window, Net::WMWindowType) == atom (Net::WMWindowTypeDialog) {
      c.is_dialog = true;
    }
    // Transient for
    let mut target_workspace = active_workspace;
    let mut trans_win: Window = X_NONE;
    let mut has_trans_client: bool = false;
    if XGetTransientForHint (display, window, &mut trans_win) != 0 {
      if let Some (trans) = win2client (trans_win) {
        has_trans_client = true;
        target_workspace = trans.workspace;
        // Center inside parent
        g.x = trans.geometry.x + (trans.geometry.w as i32 - g.w as i32) / 2;
        g.y = trans.geometry.y + (trans.geometry.h as i32 - g.h as i32) / 2;
      }
    }
    if !has_trans_client {
      // Give client random position inside window area and clamp its size into
      // the window area
      let mut rng = rand::thread_rng ();
      if g.w < window_area.w {
        let max_x = (window_area.w - g.w) as c_int + window_area.x;
        g.x = rng.gen_range (window_area.x..=max_x);
      }
      else {
        g.x = window_area.x;
        g.w = window_area.w - ((*config).border_width << 1) as c_uint;
      }
      if g.h < window_area.h {
        let max_y = (window_area.h - g.h) as c_int + window_area.y;
        g.y = rng.gen_range (window_area.y..=max_y);
      }
      else {
        g.y = window_area.y;
        g.h = window_area.h - ((*config).border_width << 1) as c_uint;
      }
    }
    c.move_and_resize (g);
    c.prev_geometry = c.geometry;
    // Add client
    if target_workspace == active_workspace {
      XMapWindow (display, window);
    }
    workspaces[target_workspace].push (c);
    property::append (root, Net::ClientList, XA_WINDOW, 32, &window, 1);
    log::info! ("Mapped new client: '{}' ({})", name, window);
    if trans_win != X_NONE {
      log::info! ("    Transient for: '{}' ({})", window_title (trans_win), trans_win);
    }
    log::info! ("            Class: {}", class_hints.class);
    log::info! ("             Name: {}", class_hints.name);
  }
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

pub unsafe fn client_message (event: &XClientMessageEvent) {
  if let Some (client) = win2client (event.window) {
    log::debug! ("Client message: {}", event.message_type);
    // Something about just printing 'client' here sometimes just freezes the
    // entire program (TODO)
    log::debug! ("  Recipient: {}", client.window);
    log::debug! ("  Data (longs): {:?}", event.data.as_longs ());
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
            return;
          }
        }
        client.set_urgency (new_state! (is_urgent));
      }
    }
    else if event.message_type == atom (Net::ActiveWindow) {
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
    if event.message_type == atom (Net::CurrentDesktop) {
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

