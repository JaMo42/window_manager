use crate::as_static::AsStaticMut;

use super::config::*;
use super::context_menu;
use super::core::*;
use super::process;
use super::property::{atom, Net, Normal_Hints};
use super::*;
use x::{window::To_XWindow, XFalse, XNone};

pub const MOUSE_MASK: i64 = ButtonPressMask | ButtonReleaseMask | PointerMotionMask;

pub unsafe fn win2client<W: To_XWindow> (window: W) -> Option<&'static mut Client> {
  let mut data: XPointer = std::ptr::null_mut ();
  if window.to_xwindow () == XNone
    || window.to_xwindow () == root.handle ()
    || XFindContext (
      display.as_raw (),
      window.to_xwindow (),
      wm_context,
      &mut data,
    ) != 0
  {
    None
  } else if !data.is_null () {
    Some ((data as *mut Client).as_static_mut ())
  } else {
    None
  }
}

pub unsafe fn button_press (event: &XButtonEvent) {
  if context_menu::click (event) {
    return;
  }
  if is_kind (event.subwindow, Window_Kind::Meta_Or_Unmanaged) {
    return;
  }
  // Check for windows for which we have ButtonPressMask set
  if let Some (kind) = get_window_kind (event.window) {
    let mut handled = true;
    match kind {
      Window_Kind::Root => {
        handled = false;
      }
      Window_Kind::Status_Bar => {
        bar.click (event);
      }
      Window_Kind::Dock_Item => {
        dock::click_item (event);
      }
      // See `dock::create_show_window` as to why we need handle these
      Window_Kind::Dock_Show | Window_Kind::Dock => {}
      Window_Kind::Frame_Button => {
        if let Some (client) = win2client (event.window) {
          client.click (event.window);
        }
      }
      Window_Kind::Notification => {
        notifications::maybe_close (event.window);
      }
      _ => {
        log::warn! (
          "Ignoring click on window with ButtonPressMask: {}",
          event.window
        );
        handled = false;
      }
    }
    if handled {
      return;
    }
  }
  if is_kind (event.subwindow, Window_Kind::Status_Bar) {
    bar.click (event);
    return;
  }
  if let Some (client) = win2client (event.subwindow) {
    if event.button == Button1 && !client.may_move ()
      || event.button == Button3 && !client.may_resize ()
      || event.button == Button1 && client.click_frame (event.time)
    {
      return;
    }
  }
  if win2client (event.subwindow).is_some () {
    mouse_held = event.button;
    workspaces[active_workspace].focus (event.subwindow);
  }
}

pub unsafe fn button_relase () {
  mouse_held = 0;
}

pub unsafe fn motion (event: &XMotionEvent) {
  if mouse_held != 0 {
    if let Some (c) = win2client (event.subwindow) {
      let mut lock_width = false;
      let mut lock_height = false;
      if (event.state & (*config).modifier) == 0 {
        let g = c.client_geometry ();
        if event.x - g.x > 0 && event.y - g.y > 0 {
          let extra = i32::max (10 - decorated_frame_offset.x, 0);
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
  } else if is_kind (event.window, Window_Kind::Context_Menu) {
    crate::context_menu::motion (event);
  } else {
    // Ignore all subsequent MotionNotify events
    let mut my_event: XEvent = uninitialized! ();
    loop {
      display.next_event (&mut my_event);
      if my_event.type_ != MotionNotify {
        break;
      }
    }
    display.push_event (&mut my_event);
  }
}

pub unsafe fn mouse_move (client: &mut Client) {
  let _grab = if let Some (grab) = display.scoped_pointer_grab (MOUSE_MASK, cursor::moving) {
    grab
  } else {
    return;
  };
  let start_x: c_int;
  let start_y: c_int;
  if let Some ((x, y)) = display.query_pointer_position () {
    start_x = x;
    start_y = y;
  } else {
    return;
  }
  let mut event: XEvent = uninitialized! ();
  let mut last_time: Time = 0;
  let mut mouse_x = start_x;
  let mut mouse_y = start_y;
  let mut preview = geometry::Preview::create (if client.is_snapped () {
    client.saved_geometry ()
  } else {
    client.frame_geometry ()
  });
  loop {
    display.mask_event (MOUSE_MASK | SubstructureRedirectMask, &mut event);
    match event.type_ {
      ConfigureRequest => configure_request (&event.configure_request),
      MapRequest => map_request (&event.map_request),
      MotionNotify => {
        let motion = event.motion;
        if (motion.time - last_time) <= MOUSE_MOVE_RESIZE_RATE {
          continue;
        }
        last_time = motion.time;
        if motion.state & MOD_SHIFT == MOD_SHIFT {
          preview.snap (motion.x, motion.y);
        } else if !monitors::at (motion.x, motion.y)
          .window_area ()
          .contains (motion.x, motion.y)
        {
          preview.move_edge (motion.x, motion.y);
        } else {
          preview.move_by (motion.x - mouse_x, motion.y - mouse_y);
        }
        preview.update ();
        mouse_x = motion.x;
        mouse_y = motion.y;
      }
      ButtonRelease => break,
      _ => {}
    }
  }
  preview.finish (client);
}

pub unsafe fn mouse_resize (client: &mut Client, lock_width: bool, lock_height: bool) {
  let cursor = if lock_height {
    cursor::resizing_horizontal
  } else if lock_width {
    cursor::resizing_vertical
  } else {
    cursor::resizing
  };
  let _grab = if let Some (grab) = display.scoped_pointer_grab (MOUSE_MASK, cursor) {
    grab
  } else {
    return;
  };
  let start_x: c_int;
  let start_y: c_int;
  if let Some ((x, y)) = display.query_pointer_position () {
    start_x = x;
    start_y = y;
  } else {
    return;
  }
  let mut event: XEvent = uninitialized! ();
  let mut last_time: Time = 0;
  let mut prev_x = start_x;
  let mut prev_y = start_y;
  let mut dx = 0;
  let mut dy = 0;
  let width_mul = !lock_width as i32;
  let height_mul = !lock_height as i32;
  let mut preview = geometry::Preview::create (if client.is_snapped () {
    client.saved_geometry ()
  } else {
    client.frame_geometry ()
  });
  let normal_hints = Normal_Hints::get (client.window);
  loop {
    display.mask_event (MOUSE_MASK | SubstructureRedirectMask, &mut event);
    match event.type_ {
      ConfigureRequest => configure_request (&event.configure_request),
      MapRequest => map_request (&event.map_request),
      MotionNotify => {
        let motion = event.motion;
        if (motion.time - last_time) <= MOUSE_MOVE_RESIZE_RATE {
          continue;
        }
        last_time = motion.time;
        let mx = (motion.x - prev_x) * width_mul;
        let my = (motion.y - prev_y) * height_mul;
        dx += mx;
        dy += my;
        preview.resize_by (mx, my);
        if let Some (h) = normal_hints.as_ref () {
          // If resizing freely prefer the direction the mouse has moved more in
          let keep_height = lock_width || (!lock_height && dx > dy);
          preview.apply_normal_hints (h, keep_height);
        }
        preview.update ();
        prev_x = motion.x;
        prev_y = motion.y;
      }
      ButtonRelease => break,
      _ => {}
    }
  }
  preview.finish (client);
}

pub unsafe fn key_press (event: &XKeyEvent) {
  if is_kind (event.window, Window_Kind::Context_Menu) {
    context_menu::key_press (event);
  }
  if let Some (action) = (*config).get (event.keycode, event.state) {
    match action {
      Action::WM (f) => {
        if let Some (client) = focused_client! () {
          f (client);
        }
      }
      Action::WS (f, workspace_index, requires_window) => {
        let maybe_client = focused_client! ();
        if *requires_window && maybe_client.is_none () {
          return;
        }
        f (*workspace_index, maybe_client);
      }
      Action::Launch (cmd) => {
        if let Err (error) = process::run (
          &cmd
            .iter ()
            .map (std::ops::Deref::deref)
            .collect::<Vec<&str>> (),
        ) {
          log::error! ("Failed to run process '{}': {}", cmd.join (" "), error);
        }
      }
      Action::Generic (f) => {
        f ();
      }
    }
  }
}

pub unsafe fn map_request (event: &XMapRequestEvent) {
  if let Some (client) = win2client (event.window) {
    if client.workspace == active_workspace {
      client.map ();
      return;
    }
  }
  // New client
  let window = Window::from_handle (&display, event.window);
  let name = window_title (window);
  let maybe_class_hints = property::Class_Hints::new (window);
  if maybe_class_hints
    .as_ref ()
    .map (|h| h.is_meta ())
    .unwrap_or_else (|| name == "window_manager_bar")
  {
    meta_windows.push (window);
    set_window_kind (window, Window_Kind::Meta_Or_Unmanaged);
    window.map ();
    log::info! ("New meta window: {} ({})", name, window);
  } else {
    let _grab = display.scoped_grab ();
    let wa = window.get_attributes ();
    if wa.map (|a| a.override_redirect != XFalse).unwrap_or (false) {
      log::info! ("ignoring window with override_redirect: {}", window);
      return;
    }
    let mut c = Client::new (window);
    let mut g = c.client_geometry ();
    // Transient for
    let mut target_workspace = active_workspace;
    let mut trans_win = XNone;
    let mut has_trans_client = false;
    let mut window_area;
    if XGetTransientForHint (display.as_raw (), window.handle (), &mut trans_win) != 0 {
      if let Some (trans) = win2client (trans_win) {
        has_trans_client = true;
        target_workspace = trans.workspace;
        window_area = monitors::main ().window_area ();
        g.center (&trans.client_geometry ())
          .clamp (&window_area.get_client ());
      }
    }
    if !has_trans_client {
      window_area = monitors::main ().window_area ();
      if c.is_dialog {
        g.center_inside (window_area);
      } else {
        let mut rng = rand::thread_rng ();
        g.random_inside (&window_area.get_client (), &mut rng);
      }
    }
    // Set size
    c.move_and_resize (Client_Geometry::Client (g));
    c.save_geometry ();
    // Add client
    if target_workspace == active_workspace {
      c.map ();
      c.draw_border ();
    }
    dock::add_client (&mut c);
    workspaces[target_workspace].push (c);
    property::append (root, Net::ClientList, XA_WINDOW, 32, &window, 1);
    log::info! ("Mapped new client: '{}' ({})", name, window);
    if trans_win != XNone {
      log::info! (
        "    Transient for: '{}' ({})",
        window_title (Window::from_handle (&display, trans_win)),
        trans_win
      );
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
    if !client.is_snapped () {
      client.save_geometry ();
    }
    if (event.value_mask & (CWX | CWY) as u64 != 0)
      && (event.value_mask & (CWWidth | CWHeight) as u64 == 0)
    {
      client.configure ();
    }
  } else {
    XConfigureWindow (
      display.as_raw (),
      event.window,
      event.value_mask as u32,
      &mut XWindowChanges {
        x: event.x,
        y: event.y,
        width: event.width,
        height: event.height,
        border_width: 0,
        sibling: event.above,
        stack_mode: event.detail,
      },
    );
  }
  display.sync (false);
}

pub unsafe fn property_notify (event: &XPropertyEvent) {
  if event.state == PropertyDelete {
  } else if let Some (client) = win2client (event.window) {
    if event.atom == XA_WM_HINTS {
      client.update_hints ();
    } else if event.atom == XA_WM_NAME || event.atom == atom (Net::WMName) {
      client.set_title (&window_title (client.window));
    } else if event.atom == atom (Net::WMUserTime)
      && focused_client! ().map_or (true, |f| f.window != event.window)
    {
      if workspaces[active_workspace].contains (client.window) {
        workspaces[active_workspace].focus (client.window);
      } else {
        client.set_urgency (true);
      }
    }
  } else if event.atom == property::atom (property::XEmbed::Info) {
    bar::tray.property_notifty (event);
  }
}

pub unsafe fn client_message (event: &XClientMessageEvent) {
  if root == event.window {
    ewmh::root_message (event);
  } else if let Some (client) = win2client (event.window) {
    ewmh::client_message (client, event);
  } else if event.message_type == property::atom (Net::SystemTrayOpcode) {
    bar::tray.client_message (event);
  } else {
    log::debug! (
      "Unhandeled client message event: {} ({})",
      display.get_atom_name (event.message_type),
      event.message_type
    );
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
  let window = Window::from_handle (&display, event.window);
  if is_kind (window, Window_Kind::Tray_Client) {
    bar::tray.maybe_remove_client (event.window);
    return;
  }
  if let Some (client) = win2client (event.window) {
    dock::remove_client (client);
  }
  for workspace in &mut workspaces {
    if workspace.contains (window) {
      workspace.remove (&Client::dummy (window)).destroy ();
      update_client_list ();
      break;
    }
  }
}

pub unsafe fn expose (event: &XExposeEvent) {
  if event.count == 0 {
    if bar.window == event.window {
      bar.draw ();
    } else if bar::tray.window () == event.window {
      bar::tray.refresh ();
    }
  }
}

pub unsafe fn crossing (event: &XCrossingEvent) {
  if let Some (kind) = get_window_kind (event.window) {
    use Window_Kind::*;
    match kind {
      Frame_Button => {
        if let Some (client) = win2client (event.window) {
          for b in client.buttons_mut () {
            if b.window == event.window {
              b.draw (event.type_ == EnterNotify);
            }
          }
        }
      }
      Status_Bar => {
        if event.type_ == EnterNotify {
          bar.enter (event.window);
        } else {
          bar.leave (event.window);
        }
      }
      Dock => {
        dock::cross (event);
      }
      Dock_Item => {
        dock::cross_item (event);
      }
      Dock_Show => {
        dock::cross_show (event);
      }
      Context_Menu => {
        context_menu::cross (event);
      }
      _ => {}
    }
  }
}

pub unsafe fn map_notify (event: &XMapEvent) {
  bar::tray.map_notify (event);
}

pub unsafe fn unmap_notify (event: &XUnmapEvent) {
  bar::tray.unmap_notify (event);
}
