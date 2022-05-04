use std::cmp::max;
use rand::Rng;
use x11::xlib::*;
use super::core::*;
use super::config::*;
use super::action;
use super::*;

macro_rules! ignore_meta_window {
  ($win:expr) => {
    if meta_windows.contains (&$win) {
      return;
    }
  }
}


pub unsafe fn button_press (event: &XButtonEvent) {
  log::trace! ("Event: button_press");
  if event.subwindow == X_NONE {
    return;
  }
  ignore_meta_window! (event.subwindow);
  mouse = (Some (*event), get_window_geometry (event.subwindow));
  workspaces[active_workspace].focus (event.subwindow);
}


pub unsafe fn motion (event: &XButtonEvent) {
  log::trace! ("Event: motion");
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
  log::trace! ("Event: button_release");
  if let Some (s) = mouse.0 {
    if s.subwindow != X_NONE {
      ignore_meta_window! (s.subwindow);
      if s.button == 1 && s.state == (*config).modifier | MOD_SHIFT {
        action::move_snap (
          win2client (s.subwindow),
          event.x_root as u32,
          event.y_root as u32
        );
      }
      else {
        // Commit the new window geometry into the client
        let c = &mut win2client (s.subwindow);
        c.geometry = get_window_geometry (s.subwindow);
        c.prev_geometry = c.geometry;
      }
    }
  }
  mouse.0 = None;
}


pub unsafe fn key_press (event: &XKeyEvent) {
  log::trace! ("Event: key_press");
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
  log::trace! ("Event: map_request");
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
    let max_x = (window_area.w - g.w) as i32;
    let max_y = (window_area.h - g.h) as i32;
    c.move_and_resize (g.move_ (
      rng.gen_range (window_area.x..=max_x),
      rng.gen_range (window_area.y..=max_y)
    ));
    workspaces[active_workspace].push (c);
    log::info! ("Mapped new client: {} ({})", name, window);
  }
}


pub unsafe fn enter (event: &XCrossingEvent) {
  log::trace! ("Event: enter");
  if event.subwindow == X_NONE {
    return;
  }
  XSetInputFocus (display, event.window, RevertToParent, CurrentTime);
}


pub unsafe fn configure_request (event: &XConfigureRequestEvent) {
  log::trace! ("Event: configure_request");
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

pub unsafe fn client_message (_event: &XClientMessageEvent) {
  log::trace! ("Event: client_message");
}

pub unsafe fn mapping_notify (_event: &XMappingEvent) {
  log::trace! ("Event: mapping_notify");
}

pub unsafe fn destroy_notify (_event: &XDestroyWindowEvent) {
  log::trace! ("Event: destroy_notify");
}

