use std::fs::File;
use std::io::Write;
use std::iter::Peekable;
use std::os::raw::*;
use x11::xlib::*;
use super::core::*;
use super::client::Client;
use super::geometry::Geometry;
use super::paths;

// XRes bindings
// (source file locations based on the version I have on my machine :^))

// X11/extensions/XRes.h:35
#[repr(C)]
struct XResClientIdSpec {
  client: XID,
  mask: c_uint
}

// X11/extensions/XRes.h:40
#[repr(C)]
struct XResClientIdValue {
  spec: XResClientIdSpec,
  length: c_long,
  value: *mut c_void
}

extern "C" {
  // X11/extensions/XRes.h:102
  fn XResQueryClientIds (
    dpy: *mut Display,
    num_specs: c_long,
    clients_specs: *mut XResClientIdSpec,
    num_ids: *mut c_long,
    clients_ids: *mut *mut XResClientIdValue
  ) -> Status;

  // X11/extensions/XRes.h:113
  fn XResGetClientPid (
    value: *mut XResClientIdValue
  ) -> libc::pid_t;

  // X11/extensions/XRes.h:115
  fn XResClientIdsDestroy (
    num_idx: c_long,
    client_ids: *mut XResClientIdValue
  );
}

// X11/extensions/XRes.h:26
const XRES_CLIENT_ID_PID: c_uint = 1;
// X11/extensions/XRes.h: 32
const XRES_CLIENT_ID_PID_MASK: c_uint = 1 << XRES_CLIENT_ID_PID;

// Hibernation

unsafe fn pid_of_window (window: Window) -> libc::pid_t {
  let mut spec: XResClientIdSpec = uninitialized! ();
  let mut client_ids: *mut XResClientIdValue = std::ptr::null_mut ();
  let mut num_ids: c_long = 0;
  let mut pid: libc::pid_t = -1;

  spec.client = window;
  spec.mask = XRES_CLIENT_ID_PID_MASK;

  XResQueryClientIds (display, 1, &mut spec, &mut num_ids, &mut client_ids);

  for i in 0..num_ids {
    if (*client_ids.add (i as usize)).spec.mask == XRES_CLIENT_ID_PID_MASK {
      pid = XResGetClientPid (client_ids.add (i as usize));
      break;
    }
  }

  XResClientIdsDestroy (num_ids, client_ids);

  pid
}

fn commandline_of_pid (pid: libc::pid_t) -> String {
  std::fs::read_to_string (format! ("/proc/{}/cmdline", pid)).unwrap ()
}

pub unsafe fn store () -> Result<(), std::io::Error> {
  log::info! ("Writing hibernation info");
  let mut file = File::create (&paths::hiberfile).unwrap ();
  // Active workspace
  file.write (&active_workspace.to_le_bytes ())?;
  // Clients
  for ws_idx in 0..workspaces.len () {
    if workspaces[ws_idx].clients.is_empty () {
      continue;
    }
    // Workspace identifier
    file.write (b"#")?;
    file.write (&ws_idx.to_le_bytes ())?;
    for client in workspaces[ws_idx].iter ().rev () {
      let pid = pid_of_window (client.window);
      let commandline = commandline_of_pid (pid);
      // Command
      file.write (commandline.as_bytes ())?;
      file.write (b"\0")?;
      // Geometry
      if client.is_snapped {
        let g = client.geometry;
        let pg = client.prev_geometry;
        file.write (b"S")?;
        file.write (&g.x.to_le_bytes ())?;
        file.write (&g.y.to_le_bytes ())?;
        file.write (&g.w.to_le_bytes ())?;
        file.write (&g.h.to_le_bytes ())?;
        file.write (&pg.x.to_le_bytes ())?;
        file.write (&pg.y.to_le_bytes ())?;
        file.write (&pg.w.to_le_bytes ())?;
        file.write (&pg.h.to_le_bytes ())?;
      }
      else {
        let g = client.geometry;
        file.write (b"F")?;
        file.write (&g.x.to_le_bytes ())?;
        file.write (&g.y.to_le_bytes ())?;
        file.write (&g.w.to_le_bytes ())?;
        file.write (&g.h.to_le_bytes ())?;
      }
    }
  }
  Ok (())
}

fn read_until (s: &mut Peekable<impl Iterator<Item=u8>>, delim: u8) -> String {
  let mut ss = String::new ();
  while let Some (c) = s.next_if (|x| *x != delim) {
    ss.push (c as char);
  }
  ss
}

fn read_bytes (s: &mut Peekable<impl Iterator<Item=u8>>, n: usize, out: &mut [u8])  {
  for i in 0..n {
    out[i] = s.next ().unwrap ();
  }
}

fn read_usize (s: &mut Peekable<impl Iterator<Item=u8>>) -> usize {
  let mut bytes = [0u8; 8];
  read_bytes (s, 8, &mut bytes);
  usize::from_le_bytes (bytes)
}

fn read_c_int (s: &mut Peekable<impl Iterator<Item=u8>>) -> c_int {
  let mut bytes = [0u8; 4];
  read_bytes (s, 4, &mut bytes);
  c_int::from_le_bytes (bytes)
}

fn read_c_uint (s: &mut Peekable<impl Iterator<Item=u8>>) -> c_uint {
  let mut bytes = [0u8; 4];
  read_bytes (s, 4, &mut bytes);
  c_uint::from_le_bytes (bytes)
}

fn skip_until (s: &mut Peekable<impl Iterator<Item=u8>>, delim: u8) {
  while s.next_if (|x| *x != delim).is_some () {}
}

pub unsafe fn load () -> Result<(), std::io::Error> {
  log::info! ("Rebuilding hibernated state");
  let hiberfile: Vec<u8> = std::fs::read (&paths::hiberfile)?;
  let mut it = hiberfile.into_iter ().peekable ();
  let mut ws_idx: usize = 0;
  // Active workspace
  active_workspace = read_usize (&mut it);
  if active_workspace >= (*config).workspace_count {
    log::warn! ("Hibernated active workspace is greater than current workspace count");
    active_workspace = 0;
  }
  // Clients
  loop {
    // Check if empty
    if it.peek ().is_none () {
      break;
    }
    // Check for workspace specifier
    if *it.peek ().unwrap () == b'#' {
      it.next ().unwrap ();
      ws_idx = read_usize (&mut it);
      if ws_idx >= (*config).workspace_count {
        log::warn! ("Cannot load workspace {} from hibernation (not enough workspaces)", ws_idx + 1);
        // Skip until next workspace identifier
        // We could just break here since we serealize workspaces in-order but
        // we'll keep going, mainly to print the warning message for each
        // workspace we cannot reconstruct.
        skip_until (&mut it, b'#');
        continue;
      }
    }
    // Program
    let program = read_until (&mut it, b'\0');
    it.next ().unwrap ();
    // Args
    let mut args = Vec::<String>::new ();
    loop {
      let arg = read_until (&mut it, b'\0');
      if arg.is_empty () {
        it.next ().unwrap ();
        break;
      }
      args.push (arg);
      it.next ().unwrap ();
    }
    // Geometry
    let g: Geometry;
    let pg: Geometry;
    let snapped: bool;
    match it.next ().unwrap () {
      b'S' => {
        // Snapped
        snapped = true;
        g = Geometry::from_parts (
          read_c_int (&mut it),
          read_c_int (&mut it),
          read_c_uint (&mut it),
          read_c_uint (&mut it)
        );
        pg = Geometry::from_parts (
          read_c_int (&mut it),
          read_c_int (&mut it),
          read_c_uint (&mut it),
          read_c_uint (&mut it)
        );
      },
      b'F' => {
        // Floating
        snapped = false;
        g = Geometry::from_parts (
          read_c_int (&mut it),
          read_c_int (&mut it),
          read_c_uint (&mut it),
          read_c_uint (&mut it)
        );
        pg = g;
      },
      _ => unreachable! ()
    }
    // Run the program
    std::process::Command::new (program)
      .args (args)
      .spawn ()
      .expect ("Failed to run program");
    // Get the window
    let w: Window;
    let mut event: XEvent = uninitialized! ();
    loop {
      XNextEvent (display, &mut event);
      match event.type_ {
        MapRequest => {
          w = event.map_request.window;
          break;
        }
        _ => {}
      }
    }
    // Create client
    let mut c = Client::new (w);
    c.is_snapped = snapped;
    c.move_and_resize (g);
    c.prev_geometry = pg;
    // Add client to workspace
    workspaces[ws_idx].push (c);
    if ws_idx != active_workspace {
      XUnmapWindow (display, c.window);
    }
  }
  // Set input focus
  if let Some (focused) = focused_client! () {
    XSetInputFocus (display, focused.window, RevertToParent, CurrentTime);
  }
  Ok (())
}

