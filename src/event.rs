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
  let client: &mut Client = if let Some (c) = win2client (window) {
    c
  }
  else {
    return;
  };
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
    if client.is_snapped () { client.prev_geometry } else { client.geometry }
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
        preview.move_by (motion.x - mouse_x, motion.y - mouse_y);
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
  #[allow(unused_unsafe)]  // `clean_mods` contains an unsafe block
  if clean_mods! (state) == (*config).modifier | MOD_SHIFT {
    action::move_snap (client, mouse_x as u32, mouse_y as u32);
  }
  else {
    preview.finish (client);
  }
}


unsafe fn mouse_resize (window: Window) {
  let client = if let Some (c) = win2client (window) {
    c
  }
  else {
    return;
  };
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
  let mut prev_x = start_x;
  let mut prev_y = start_y;
  let mut preview = geometry::Preview::create (
    if client.is_snapped () { client.prev_geometry } else { client.geometry }
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
        preview.resize_by (motion.x - prev_x, motion.y - prev_y);
        preview.update ();
        prev_x = motion.x;
        prev_y = motion.y;
      }
      ButtonRelease => break,
      _ => {}
    }
  }
  XUngrabPointer (display, CurrentTime);
  preview.finish (client);
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
    let mut wa: XWindowAttributes = uninitialized !();
    if XGetWindowAttributes (display, window, &mut wa) == 0
      || wa.override_redirect != X_FALSE {
      log::info! ("ignoring window with override_redirect: {}", window);
      return;
    }
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
        g.center (&trans.geometry)
          .clamp (&window_area);
      }
    }
    if !has_trans_client {
      let mut rng = rand::thread_rng ();
      g.random_inside (&window_area, &mut rng);
    }
    c.prev_geometry = c.geometry;
    c.move_and_resize (g);
    c.configure ();
    // Add client
    if target_workspace == active_workspace {
      c.map ();
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
  }
}


pub unsafe fn configure_request (event: &XConfigureRequestEvent) {
  if let Some (client) = win2client (event.window) {
    if event.value_mask & CWBorderWidth as u64 != 0 {
      return;
    }
    if event.value_mask & CWX as u64 != 0 {
      client.geometry.x = event.x;
    }
    if event.value_mask & CWY as u64 != 0 {
      client.geometry.y = event.y;
    }
    if event.value_mask & CWWidth as u64 != 0 {
      client.geometry.w = event.width as u32;
    }
    if event.value_mask & CWHeight as u64 != 0 {
      client.geometry.h = event.height as u32;
    }
    if (event.value_mask & (CWX|CWY) as u64 != 0)
      && (event.value_mask & (CWWidth|CWHeight) as u64 == 0) {
      client.configure ();
    }
    if !client.is_snapped() {
      client.prev_geometry = *client.geometry.clone ().expand ((*config).border_width);
    }
    client.set_position_and_size (client.geometry);
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
  }
  bar.draw ();
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
  let mut ev = *event;
  XRefreshKeyboardMapping (&mut ev);
  if ev.request == MappingKeyboard {
    grab_keys ();
  }
}

pub unsafe fn destroy_notify (event: &XDestroyWindowEvent) {
  let window = event.window;
  XGrabServer (display);
  for workspace in &mut workspaces {
    if workspace.contains (window) {
      let c = workspace.remove (&Client::dummy (window));
      XSelectInput (display, c.frame, X_NONE as i64);
      XDestroyWindow (display, c.frame);
      update_client_list ();
      break;
    }
  }
  XUngrabServer (display);
}

pub unsafe fn expose (event: &XExposeEvent) {
  if event.count == 0 {
    bar.draw ();
  }
}
