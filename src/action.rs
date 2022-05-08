use std::os::raw::*;
use x11::xlib::*;
use super::core::*;
use super::geometry::*;
use super::client::*;
use super::*;
use super::property::WM;

pub unsafe fn quit () {
  running = false;
}


pub unsafe fn close_client (client: &mut Client) {
  if !client.send_event (property::atom (WM::DeleteWindow)) {
    XKillClient (display, client.window);
  }
  workspaces[active_workspace].remove (client);
  update_client_list ();
}


pub unsafe fn move_snap (client: &mut Client, x: c_uint, y: c_uint) {
  let mut snap_flags = 0u8;
  snap_flags |= if x > screen_size.w / 2 { SNAP_RIGHT } else { SNAP_LEFT };
  let v = screen_size.h / 4;
  if y < v {
    snap_flags |= SNAP_TOP;
  }
  else if y > screen_size.h - v {
    snap_flags |= SNAP_BOTTOM;
  }
  snap (client, snap_flags);
}


pub unsafe fn snap (client: &mut Client, flags: u8) {
  let mut target = Geometry::new ();
  // Top / Bottom / Full height
  if (flags & SNAP_TOP) != 0 {
    target.y = window_area.y;
    target.h = window_area.h / 2;
  }
  else if (flags & SNAP_BOTTOM) != 0 {
    target.y = window_area.y + (window_area.h / 2) as c_int;
    target.h = window_area.h / 2;
  }
  else {
    target.y = window_area.y;
    target.h = window_area.h;
  }
  // Left / Right
  if (flags & SNAP_LEFT) != 0 {
    target.x = window_area.x;
    target.w = window_area.w / 2;
  }
  else if (flags & SNAP_RIGHT) != 0 {
    target.x = window_area.x + (window_area.w / 2) as c_int;
    target.w = window_area.w / 2;
  }
  // Fullscreen
  if (flags & SNAP_MAXIMIZED) != 0 {
    target = window_area;
    // We don't care about the gap for maximized windows so we add it here
    // since it gets removed inside `client.move_and_resize` again.
    target.expand ((*config).gap as i32);
  }
  client.is_snapped = true;
  client.move_and_resize (target);
}


pub unsafe fn center (client: &mut Client) {
  let x = window_area.x + (window_area.w - client.geometry.w) as i32 / 2;
  let y = window_area.y + (window_area.h - client.geometry.h) as i32 / 2;
  client.unsnap ();
  client.move_and_resize (Geometry::from_parts (
      x, y, client.geometry.w, client.geometry.h
  ));
  client.prev_geometry = client.geometry;
}


pub unsafe fn select_workspace (idx: usize, _: Option<&mut Client>) {
  if idx == active_workspace {
    return;
  }
  for c in workspaces[active_workspace].iter () {
    XUnmapWindow (display, c.window);
  }
  for c in workspaces[idx].iter () {
    XMapWindow (display, c.window);
  }
  active_workspace = idx;
  if let Some (focused) = focused_client! () {
    XSetInputFocus (display, focused.window, RevertToParent, CurrentTime);
  }
}


pub unsafe fn move_to_workspace (idx: usize, client_: Option<&mut Client>) {
  let client = client_.unwrap ();
  workspaces[idx].push (*client);
  workspaces[active_workspace].remove (client);
  XUnmapWindow (display, client.window);
}


pub unsafe fn switch_window () {
  workspaces[active_workspace].switch_window ();
}


pub fn from_str (s: &String) -> super::config::Action {
  match s.as_str  () {
    "close_window" => Action::WM (close_client),
    "quit" => Action::Generic (quit),
    "snap_maximized" => Action::WM (|c| unsafe { snap (c, SNAP_MAXIMIZED) }),
    "snap_left" => Action::WM (|c| unsafe { snap (c, SNAP_LEFT) }),
    "snap_right" => Action::WM (|c| unsafe { snap (c, SNAP_RIGHT) }),
    "unsnap_or_center" =>
      Action::WM (
        |c| unsafe {
          if c.is_snapped {
            c.unsnap ();
          }
          else {
            center (c);
          }
        }
      ),
    _ => panic! ("action::from_str: unknown action: {}", s)
  }
}

