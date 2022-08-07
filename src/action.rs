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
    XSync (display, X_FALSE);
  }
}


pub unsafe fn move_snap_flags (x: c_uint, y: c_uint) ->u8 {
  let mut snap_flags = 0u8;
  snap_flags |= if x > screen_size.w / 2 { SNAP_RIGHT } else { SNAP_LEFT };
  let v = screen_size.h / 4;
  if y < v {
    snap_flags |= SNAP_TOP;
  }
  else if y > screen_size.h - v {
    snap_flags |= SNAP_BOTTOM;
  }
  snap_flags
}


pub unsafe fn snap_geometry (flags: u8) -> Geometry {
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
  // Maximized
  if (flags & SNAP_MAXIMIZED) != 0 {
    target = window_area;
    // We don't care about the gap for maximized windows so we add it here
    // since it gets removed inside `client.move_and_resize` again.
    target.expand ((*config).gap as i32);
  }
  target
}


pub unsafe fn snap (client: &mut Client, flags: u8) {
  if !client.may_resize () {
    return;
  }
  client.snap_state = flags;
  client.move_and_resize (snap_geometry (flags));
}

pub unsafe fn snap_left (client: &mut Client) {
  if (client.snap_state & SNAP_LEFT) == SNAP_LEFT {
    client.snap_state &= !(SNAP_TOP | SNAP_BOTTOM);
  }
  snap (client, SNAP_LEFT | (client.snap_state & (SNAP_TOP | SNAP_BOTTOM)));
}

pub unsafe fn snap_right (client: &mut Client) {
  if (client.snap_state & SNAP_RIGHT) == SNAP_RIGHT {
    client.snap_state &= !(SNAP_TOP | SNAP_BOTTOM);
  }
  snap (client, SNAP_RIGHT | (client.snap_state & (SNAP_TOP | SNAP_BOTTOM)));
}

pub unsafe fn snap_down (client: &mut Client) {
  if client.is_snapped () && client.snap_state != SNAP_MAXIMIZED {
    snap (client, client.snap_state & !SNAP_TOP | SNAP_BOTTOM);
  }
}

pub unsafe fn snap_up (client: &mut Client) {
  if client.is_snapped () && client.snap_state != SNAP_MAXIMIZED {
    snap (client, client.snap_state & !SNAP_BOTTOM | SNAP_TOP);
  }
}

pub unsafe fn center (client: &mut Client) {
  if !client.may_move () {
    return;
  }
  client.prev_geometry.center_inside (&window_area);
  client.unsnap ();
}


pub unsafe fn select_workspace (idx: usize, _: Option<&mut Client>) {
  if idx == active_workspace {
    return;
  }
  for c in workspaces[active_workspace].iter () {
    c.unmap ();
  }
  for c in workspaces[idx].iter_mut () {
    c.map ();
  }
  active_workspace = idx;
  if let Some (focused) = focused_client! () {
    focused.focus ();
  }
  else {
    property::set (
      root, Net::ActiveWindow, XA_WINDOW, 32, std::ptr::null_mut::<c_uchar> (), 0
    );
    bar.draw ();
  }
  set_cardinal! (root, property::atom (Net::CurrentDesktop), active_workspace);
}


pub unsafe fn move_to_workspace (idx: usize, client_: Option<&mut Client>) {
  let client = client_.unwrap ();
  client.workspace = idx;
  let boxed = workspaces[active_workspace].remove (client);
  boxed.unmap ();
  workspaces[idx].push (boxed);
}


pub unsafe fn switch_window () {
  workspaces[active_workspace].switch_window ();
}
