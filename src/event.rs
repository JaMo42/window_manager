use x11::xlib::*;
use super::core::*;
use super::config::*;
use super::action;
use super::property::{Net, atom, get_atom};
use super::*;

pub const MOUSE_MASK: i64 = ButtonPressMask|ButtonReleaseMask|PointerMotionMask;


unsafe fn win2client (window: Window) -> Option<&'static mut Client> {
  let mut data: XPointer = std::ptr::null_mut ();
  if window == X_NONE || window == root
    || XFindContext (display, window, wm_context, &mut data) != 0 {
    None
  }
  else if !data.is_null () {
    Some (&mut *(data as *mut Client))
  }
  else {
    None
  }
}


pub unsafe fn button_press (event: &XButtonEvent) {
  if cfg! (feature = "bar") {
    // Meta key is ignored when clicking on bar
    if event.window == bar.window || event.subwindow == bar.window {
      bar.button_press (event);
      return;
    }
  }
  if event.subwindow == X_NONE {
    if let Some (client) = win2client (event.window) {
      client.click (event.window);
    }
    return;
  }
  if meta_windows.contains (&event.subwindow) {
    return;
  }
  if let Some (client) = win2client (event.subwindow) {
    if event.button == Button1 && !client.may_move ()
      || event.button == Button3 && !client.may_resize () {
      return;
    }
  }
  mouse_held = event.button;
  workspaces[active_workspace].focus (event.subwindow);
}


pub unsafe fn button_relase () {
  mouse_held = 0;
}


pub unsafe fn motion (event: &XButtonEvent) {
  if mouse_held != 0 {
    if let Some (c) = win2client (event.subwindow) {
      let mut lock_width = false;
      let mut lock_height = false;
      if (event.state & (*config).modifier) == 0 {
        let g = c.client_geometry ();
        if event.x - g.x > 0 && event.y - g.y > 0 {
          let extra = i32::max (10 - frame_offset.x, 0);
          if event.x - g.x + extra < g.w as i32 {
            lock_width = true;
          } else if event.y - g.y + extra < g.h as i32 {
            lock_height = true;
          }
          mouse_held = Button3;
        } else {
          mouse_held = Button1;
        }
      }
      match mouse_held {
        Button1 => mouse_move (c),
        Button3 => mouse_resize (c, lock_width, lock_height),
        _ => {}
      }
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


unsafe fn mouse_move (client: &mut Client) {
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
  let mut mouse_x = start_x;
  let mut mouse_y = start_y;
  let mut state = 0;
  let mut preview = geometry::Preview::create (
    if client.is_snapped () {
      client.saved_geometry ()
    } else {
      client.frame_geometry ()
    }
  );
  loop {
    XMaskEvent (display, MOUSE_MASK|SubstructureRedirectMask, &mut event);
    match event.type_ {
      ConfigureRequest => configure_request (&event.configure_request),
      MapRequest => map_request (&event.map_request),
      MotionNotify => {
        let motion = event.motion;
        // Only handle at 60 FPS
        if (motion.time - last_time) <= MOUSE_MOVE_RESIZE_RATE {
          continue;
        }
        last_time = motion.time;
        if state & MOD_SHIFT == MOD_SHIFT {
          preview.snap (motion.x, motion.y);
        }
        else {
          preview.move_by (motion.x - mouse_x, motion.y - mouse_y);
        }
        preview.update ();
        mouse_x = motion.x;
        mouse_y = motion.y;
        state = motion.state;
      }
      ButtonRelease => break,
      _ => {}
    }
  }
  XUngrabPointer (display, CurrentTime);
  preview.finish (client, state & MOD_SHIFT == MOD_SHIFT);
}


unsafe fn mouse_resize (client: &mut Client, lock_width: bool, lock_height: bool) {
  let cursor = if lock_height {
    cursor::resizing_horizontal
  } else if lock_width {
    cursor::resizing_vertical
  } else {
    cursor::resizing
  };
  if XGrabPointer (
    display,
    root,
    X_FALSE,
    MOUSE_MASK as u32,
    GrabModeAsync,
    GrabModeAsync,
    X_NONE,
    cursor,
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
  let mut prev_x = start_x;
  let mut prev_y = start_y;
  let width_mul = !lock_width as i32;
  let height_mul = !lock_height as i32;
  let mut preview = geometry::Preview::create (
    if client.is_snapped () {
      client.saved_geometry ()
    } else {
      client.frame_geometry ()
    }
  );
  loop {
    XMaskEvent (display, MOUSE_MASK|SubstructureRedirectMask, &mut event);
    match event.type_ {
      ConfigureRequest => configure_request (&event.configure_request),
      MapRequest => map_request (&event.map_request),
      MotionNotify => {
        let motion = event.motion;
        // Only handle at 60 FPS
        if (motion.time - last_time) <= MOUSE_MOVE_RESIZE_RATE {
          continue;
        }
        last_time = motion.time;
        preview.resize_by (
          (motion.x - prev_x) * width_mul,
          (motion.y - prev_y) * height_mul
        );
        preview.update ();
        prev_x = motion.x;
        prev_y = motion.y;
      }
      ButtonRelease => break,
      _ => {}
    }
  }
  XUngrabPointer (display, CurrentTime);
  preview.finish (client, false);
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
        run_process (cmd);
      },
      Action::Generic (f) => {
        f ();
      }
    }
  }
}


pub unsafe fn map_request (event: &XMapRequestEvent) {
  // TODO: should only check active workspace?
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
  let maybe_class_hints = property::Class_Hints::new (window);
  if maybe_class_hints.is_some () && maybe_class_hints.as_ref ().unwrap ().is_meta ()
    || name == "window_manager_bar" {
    meta_windows.push (window);
    XMapWindow (display, window);
    log::info! ("New meta window: {} ({})", name, window);
  }
  else {
    XGrabServer (display);
    let mut wa: XWindowAttributes = uninitialized !();
    if XGetWindowAttributes (display, window, &mut wa) == 0
      || wa.override_redirect != X_FALSE {
      log::info! ("ignoring window with override_redirect: {}", window);
      return;
    }
    let mut c = Client::new (window);
    let mut g = c.client_geometry ();
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
        g.center (&trans.client_geometry ())
          .clamp (&window_area.get_client (&frame_offset));
      }
    }
    if !has_trans_client {
      let mut rng = rand::thread_rng ();
      g.random_inside (&window_area.get_client (&frame_offset), &mut rng);
    }
    c.move_and_resize (Client_Geometry::Client (g));
    c.save_geometry ();
    // Add client
    if target_workspace == active_workspace {
      c.map ();
      c.draw_border ();
    }
    workspaces[target_workspace].push (c);
    property::append (root, Net::ClientList, XA_WINDOW, 32, &window, 1);
    log::info! ("Mapped new client: '{}' ({})", name, window);
    if trans_win != X_NONE {
      log::info! ("    Transient for: '{}' ({})", window_title (trans_win), trans_win);
    }
    if let Some (class_hints) = maybe_class_hints {
      log::info! ("            Class: {}", class_hints.class);
      log::info! ("             Name: {}", class_hints.name);
    }
    XUngrabServer (display);
  }
}


pub unsafe fn configure_request (event: &XConfigureRequestEvent) {
  if let Some (client) = win2client (event.window) {
    if event.value_mask & CWBorderWidth as u64 != 0 {
      return;
    }
    let mut g = client.client_geometry ();
    if event.value_mask & CWX as u64 != 0 {
      g.x = event.x;
    }
    if event.value_mask & CWY as u64 != 0 {
      g.y = event.y;
    }
    if event.value_mask & CWWidth as u64 != 0 {
      g.w = event.width as u32;
    }
    if event.value_mask & CWHeight as u64 != 0 {
      g.h = event.height as u32;
    }
    client.move_and_resize (Client_Geometry::Client (g));
    if !client.is_snapped() {
      client.save_geometry ();
    }
    if (event.value_mask & (CWX|CWY) as u64 != 0)
      && (event.value_mask & (CWWidth|CWHeight) as u64 == 0) {
      client.configure ();
    }
  }
  else {
    XConfigureWindow (
      display, event.window, event.value_mask as u32,
      &mut XWindowChanges {
        x: event.x,
        y: event.y,
        width: event.width,
        height: event.height,
        border_width: 0,
        sibling: event.above,
        stack_mode: event.detail
      }
    );
  }
  XSync (display, X_FALSE);
}

pub unsafe fn property_notify (event: &XPropertyEvent) {
  if event.state == PropertyDelete {
  }
  else if let Some (client) = win2client (event.window) {
    if event.atom == XA_WM_HINTS {
      client.update_hints ();
    }
    else if event.atom == XA_WM_NAME || event.atom == atom (Net::WMName) {
      client.set_title (&window_title (client.window));
    }
    else if event.atom == atom (Net::WMUserTime)
      && focused_client! ().map_or (true, |f| f.window != event.window)
    {
      if workspaces[active_workspace].contains (client.window) {
        workspaces[active_workspace].focus (client.window);
      }
      else {
        client.set_urgency (true);
      }
    }
  } else if event.atom == property::atom (property::XEmbed::Info) {
    bar::tray.property_notifty (event);
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
      if workspaces[active_workspace].contains (client.window) {
        workspaces[active_workspace].focus (client.window);
      }
      else {
        client.set_urgency (true);
      }
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
  else if event.message_type == property::atom (Net::SystemTrayOpcode) {
    log::debug! ("Client message: {}", event.message_type);
    log::debug! ("  Recipient: Tray ({})", event.window);
    bar::tray.client_message (event);
  }
  else {
    log::debug! ("Unhandeled client message: {}", event.message_type);
    log::debug! ("  Recipient: {}", event.window);
    log::debug! ("  Data (longs): {:?}", event.data.as_longs ());
  }
}

pub unsafe fn mapping_notify (event: &XMappingEvent) {
  let mut ev = *event;
  XRefreshKeyboardMapping (&mut ev);
  if ev.request == MappingKeyboard {
    grab_keys ();
  }
}

pub unsafe fn destroy_notify (event: &XDestroyWindowEvent) {
  let window = event.window;
  for workspace in &mut workspaces {
    if workspace.contains (window) {
      workspace.remove (&Client::dummy (window))
        .destroy ();
      update_client_list ();
      break;
    }
  }
  if bar::tray.maybe_remove_client (window) {
    return;
  }
}

pub unsafe fn expose (event: &XExposeEvent) {
  if event.count == 0 {
    if event.window == bar.window {
      bar.draw ();
    } else if event.window == bar::tray.window () {
      bar::tray.refresh ();
    }
  }
}

pub unsafe fn crossing (event: &XCrossingEvent) {
  if let Some (client) = win2client (event.window) {
    for b in client.buttons_mut () {
      if b.window == event.window {
        b.draw (event.type_ == EnterNotify);
      }
    }
  }
}

pub unsafe fn map_notify (event: &XMapEvent) {
  bar::tray.map_notify (event);
}

pub unsafe fn unmap_notify (event: &XUnmapEvent) {
  bar::tray.unmap_notify (event);
}
