use super::client::*;
use super::core::*;
use super::geometry::*;
use super::monitors::Monitor;
use super::process;
use super::property::WM;
use super::*;

pub unsafe fn quit () {
  running = false;
}

pub unsafe fn quit_dialog () {
  process::run_or_message_box (&["window_manager_quit"]);
}

pub unsafe fn close_client (client: &mut Client) {
  if !client.send_event (property::atom (WM::DeleteWindow)) {
    client.window.kill_client ();
    display.sync (false);
  }
}

pub unsafe fn move_snap_flags (x: c_uint, y: c_uint) -> u8 {
  let mut snap_flags = 0u8;
  snap_flags |= if x > screen_size.w / 2 {
    SNAP_RIGHT
  } else {
    SNAP_LEFT
  };
  let v = screen_size.h / 4;
  if y < v {
    snap_flags |= SNAP_TOP;
  } else if y > screen_size.h - v {
    snap_flags |= SNAP_BOTTOM;
  }
  snap_flags
}

pub unsafe fn snap_geometry (flags: u8, window_area: &Geometry) -> Geometry {
  let mut target = Geometry::new ();
  // Top / Bottom / Full height
  if (flags & SNAP_TOP) != 0 {
    target.y = window_area.y;
    target.h = window_area.h / 2;
  } else if (flags & SNAP_BOTTOM) != 0 {
    target.y = window_area.y + (window_area.h / 2) as c_int;
    target.h = window_area.h / 2;
  } else {
    target.y = window_area.y;
    target.h = window_area.h;
  }
  // Left / Right
  if (flags & SNAP_LEFT) != 0 {
    target.x = window_area.x;
    target.w = window_area.w / 2;
  } else if (flags & SNAP_RIGHT) != 0 {
    target.x = window_area.x + (window_area.w / 2) as c_int;
    target.w = window_area.w / 2;
  }
  // Maximized
  if (flags & SNAP_MAXIMIZED) != 0 {
    target = *window_area;
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
  if flags == SNAP_MAXIMIZED {
    ewmh::set_net_wm_state (
      client,
      &[
        property::atom (Net::WMStateMaximizedHorz),
        property::atom (Net::WMStateMaximizedVert),
      ],
    );
  }
  let window_area = monitors::containing (client).window_area ();
  client.move_and_resize (Client_Geometry::Snap (snap_geometry (flags, window_area)));
}

pub unsafe fn snap_left (client: &mut Client) {
  if (client.snap_state & SNAP_LEFT) == SNAP_LEFT {
    client.snap_state &= !(SNAP_TOP | SNAP_BOTTOM);
  }
  snap (
    client,
    SNAP_LEFT | (client.snap_state & (SNAP_TOP | SNAP_BOTTOM)),
  );
}

pub unsafe fn snap_right (client: &mut Client) {
  if (client.snap_state & SNAP_RIGHT) == SNAP_RIGHT {
    client.snap_state &= !(SNAP_TOP | SNAP_BOTTOM);
  }
  snap (
    client,
    SNAP_RIGHT | (client.snap_state & (SNAP_TOP | SNAP_BOTTOM)),
  );
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
  let window_area = monitors::containing (client).window_area ();
  client.modify_saved_geometry (move |g| {
    g.center_inside (window_area);
  });
  // Need any non-none snap state for the unsnap function
  client.snap_state = SNAP_MAXIMIZED;
  client.unsnap ();
}

pub unsafe fn minimize (client: &mut Client) {
  if client.is_fullscreen {
    return;
  }
  client.is_minimized = true;
  client.unmap ();
  ewmh::set_net_wm_state (client, &[property::atom (Net::WMStateHidden)]);
  if let Some (f) = focused_client! () {
    workspaces[active_workspace].focus (f.window);
  } else {
    property::delete (root, property::Net::ActiveWindow);
    display.set_input_focus (root);
  }
}

pub unsafe fn raise_all () {
  for c in workspaces[active_workspace].iter_mut () {
    if c.is_minimized {
      c.unminimize (true);
    }
  }
  workspaces[active_workspace].focus_client (0);
}

pub unsafe fn toggle_maximized (client: &mut Client) {
  if client.snap_state & SNAP_MAXIMIZED == SNAP_MAXIMIZED {
    client.unsnap ();
  } else {
    snap (client, SNAP_MAXIMIZED);
  }
}

pub unsafe fn select_workspace (idx: usize, _: Option<&mut Client>) {
  if idx == active_workspace {
    return;
  }
  for c in workspaces[active_workspace].iter () {
    c.unmap ();
  }
  for c in workspaces[idx].iter_mut () {
    if !c.is_minimized {
      c.map ();
      c.draw_border ();
    }
  }
  active_workspace = idx;
  if let Some (focused) = focused_client! () {
    focused.focus ();
    dock::keep_open (false);
  } else {
    property::set (
      root,
      Net::ActiveWindow,
      XA_WINDOW,
      32,
      std::ptr::null_mut::<c_uchar> (),
      0,
    );
    dock::keep_open (true);
  }
  set_cardinal! (root, property::atom (Net::CurrentDesktop), active_workspace);
  bar.draw ();
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

pub fn move_to_monitor (client: &mut Client, cur: &Monitor, mon: &Monitor) {
  let cg = cur.geometry ();
  let ng = mon.geometry ();
  let mut g = client.saved_geometry ();
  g.x = g.x - cg.x + ng.x;
  g.y = g.y - cg.y + ng.y;
  g.clamp (mon.window_area ());
  unsafe {
    if client.is_snapped () {
      client.modify_saved_geometry (|sg| {
        *sg = g;
      });
      snap (client, client.snap_state);
    } else {
      client.move_and_resize (Client_Geometry::Frame (g));
      client.save_geometry ();
    }
  }
}

pub fn move_to_next_monitor (client: &mut Client) {
  if !client.may_move () {
    return;
  }
  let current = monitors::containing (client);
  if let Some (next) = monitors::get (current.number () + 1) {
    move_to_monitor (client, current, next);
  }
}

pub fn move_to_prev_monitor (client: &mut Client) {
  if !client.may_move () {
    return;
  }
  let current = monitors::containing (client);
  if let Some (prev) = monitors::get (current.number () - 1) {
    move_to_monitor (client, current, prev);
  }
}

pub fn grid_resize (client: &mut Client) {
  let area = monitors::containing (client).window_area ();
  let dimensions = format! ("{},{},{},{}", area.x, area.y, area.w, area.h);
  let (vertical_cells, horizontal_cells) = unsafe {&*config}.grid_resize_grid_size;
  let cells = format! ("{},{}", vertical_cells, horizontal_cells);
  let color = unsafe {&*config}.colors.selected;
  let color = format! ("--color={},{},{}", color.red, color.green, color.blue);
  log_error! (process::run (&[
    "grid-resize",
    &format! ("{}", client.window),
    &dimensions,
    &cells,
    &color,
    "--live",
    "--method=message"
  ]));
}
