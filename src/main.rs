// Need to allow this to match on the XEvent types (KeyPress etc.)
#![allow(non_upper_case_globals)]
// Also disable other superfluous naming style warnings
#![allow(non_camel_case_types)]

use std::os::raw::*;
use x11::xlib::*;

#[macro_use]
mod core;
mod client;
mod geometry;
mod config;
mod action;
mod event;
mod config_parser;
mod color;
#[macro_use]
mod workspace;
mod hibernate;

use crate::core::*;
use client::*;
use geometry::*;
use config::*;
use workspace::*;

unsafe extern "C" fn x_error (my_display: *mut Display, event: *mut XErrorEvent) -> c_int {
  const ERROR_TEXT_SIZE: usize = 1024;
  let mut error_text_buf: [c_char; ERROR_TEXT_SIZE] = [0; ERROR_TEXT_SIZE];
  let error_text = &mut error_text_buf as *mut c_char;
  XGetErrorText (
    my_display, (*event).error_code as i32, error_text, ERROR_TEXT_SIZE as i32
  );
  let error_msg = std::ffi::CStr::from_ptr (error_text).to_str ().unwrap ().to_string ();
  eprintln! ("window_manager|x-error: {}", error_msg);
  log::error! ("X Error: {}", error_msg);
  return 0;
}


unsafe fn connect () {
  display = XOpenDisplay (std::ptr::null ());
  if display.is_null () {
    eprintln! ("can't open display");
    std::process::exit (1);
  }
  root = XDefaultRootWindow (display);
  let scn = XDefaultScreen (display);
  screen_size = Geometry::from_parts (
    0, 0,
    XDisplayWidth (display, scn) as u32, XDisplayHeight (display, scn) as u32
  );
}


unsafe fn grab_keys () {
  // MAYBE_TODO: dwm seems to grab each key with the numlock and 'LockMask'
  // modifiers as well and then ignores those when handling key presses, maybe
  // do that as well?
  XUngrabKey (display, AnyKey as i32, AnyModifier, root);
  for (key, _) in &(*config).key_binds {
    XGrabKey (
      display,
      key.code as c_int,
      key.modifiers,
      root,
      X_TRUE,
      GrabModeAsync,
      GrabModeAsync
    );
  }
}


unsafe fn grab_buttons () {
  XUngrabButton (display, AnyButton as u32, AnyModifier, root);
  XGrabButton (
    display,
    1,
    (*config).modifier,
    root,
    X_TRUE,
    (ButtonPressMask|ButtonReleaseMask|PointerMotionMask) as u32,
    GrabModeAsync,
    GrabModeAsync,
    X_NONE,
    X_NONE
  );
  XGrabButton (
    display,
    1,
    (*config).modifier | MOD_SHIFT,
    root,
    X_TRUE,
    (ButtonPressMask|ButtonReleaseMask|PointerMotionMask) as u32,
    GrabModeAsync,
    GrabModeAsync,
    X_NONE,
    X_NONE
  );
  XGrabButton (
    display,
    3,
    (*config).modifier,
    root,
    X_TRUE,
    (ButtonPressMask|ButtonReleaseMask|PointerMotionMask) as u32,
    GrabModeAsync,
    GrabModeAsync,
    X_NONE,
    X_NONE
  );
}


unsafe fn select_input (mut mask: c_long) {
  if mask == 0 {
    mask = SubstructureRedirectMask | SubstructureNotifyMask
      | ButtonPressMask | PointerMotionMask | EnterWindowMask
      | LeaveWindowMask | StructureNotifyMask | PropertyChangeMask;
  }
  let mut wa: XSetWindowAttributes = uninitialized! ();
  wa.event_mask = mask;
  XChangeWindowAttributes (display, root, CWEventMask, &mut wa);
  XSelectInput (display, root, wa.event_mask);
}


unsafe fn init () {
  // Create workspaces
  for _ in 0..(*config).workspace_count {
    workspaces.push (Workspace::new ());
  }
  // Set error handler
  XSetErrorHandler (Some (x_error));
  // Colors
  XSetWindowBackground (display, root, (*config).colors.background.pixel);
  XClearWindow (display, root);
  // Hibernation
  if (*config).hibernate {
    select_input (SubstructureRedirectMask);
    if hibernate::load ().is_err () {
      log::error! ("Could not read hiberfile");
    }
  }
  // Grab input
  grab_keys ();
  grab_buttons ();
  // Select input
  select_input (0);
  // Run autostart script
  // TODO: don't rely on relative path
  if std::path::Path::new ("./autostartrc").exists () {
    std::process::Command::new ("bash")
      .arg ("./autostartrc")
      .spawn ()
      .expect ("failed to run autostartrc");
  }
  else {
    log::info! ("No autostartrc found");
  }
}


unsafe fn run () {
  let mut event: XEvent = uninitialized! ();
  running = true;
  while running {
    XNextEvent (display, &mut event);
    match event.type_ {
      ButtonPress => event::button_press (&event.button),
      ButtonRelease => event::button_release (&event.button),
      ClientMessage => event::client_message (&event.client_message),
      ConfigureRequest => event::configure_request (&event.configure_request),
      ConfigureNotify => event::configure_notify (&event.configure),
      DestroyNotify => event::destroy_notify (&event.destroy_window),
      EnterNotify => event::enter (&event.crossing),
      Expose => todo! (),
      FocusIn => todo! (),
      KeyPress => event::key_press (&event.key),
      MappingNotify => event::mapping_notify (&event.mapping),
      MapRequest => event::map_request (&event.map_request),
      MotionNotify => event::motion (&event.button),
      PropertyNotify => event::property_notify (&event.property),
      UnmapNotify => event::unmap_notify (&event.unmap),
      _ => {}
    }
  }
}


unsafe fn cleanup () {
  // Hibernation
  if (*config).hibernate {
    if hibernate::store ().is_err () {
      log::error! ("Could not write hiberfile");
      std::fs::remove_file ("./.window_manager_hiberfile").ok ();
    }
  }
  // Close all open clients
  for ws in workspaces.iter_mut () {
    for mut c in ws.iter_mut () {
      action::close_client (&mut c);
    }
  }
  // Close meta windows
  for w in meta_windows.iter () {
    XKillClient (display, *w);
  }
  // Un-grab keys and buttons
  for (key, _) in &(*config).key_binds {
    XUngrabKey (
      display,
      key.code as c_int,
      key.modifiers,
      root
    );
  }
  XUngrabButton (
    display,
    1,
    (*config).modifier,
    root
  );
  XUngrabButton (
    display,
    1,
    (*config).modifier | MOD_SHIFT,
    root
  );
  XUngrabButton (
    display,
    3,
    (*config).modifier,
    root
  );
  // Close display
  XCloseDisplay (display);
}


unsafe fn win2client (window: Window) -> &'static mut Client {
  workspaces[active_workspace]
    .iter_mut ()
    .find (|c| c.window == window)
    .unwrap ()
}


fn get_window_geometry (window: Window) -> Geometry {
  let mut x: c_int = 0;
  let mut y: c_int = 0;
  let mut w: c_uint = 0;
  let mut h: c_uint = 0;
  let mut _border_width: c_uint = 0;
  let mut _depth: c_uint = 0;
  let mut _root: Window = 0;

  unsafe {
    XGetGeometry (
      display, window, &mut _root,
      &mut x, &mut y,
      &mut w, &mut h,
      &mut _border_width, &mut _depth
    );
  }

  return Geometry { x, y, w, h };
}


unsafe fn window_title (window: Window) -> String {
  let mut title_c_str: *mut c_char = std::ptr::null_mut ();
  XFetchName (display, window, &mut title_c_str);
  std::ffi::CStr::from_ptr (title_c_str).to_str ().unwrap ().to_owned ()
}


unsafe fn focus_window (window: Window) {
  XSetWindowBorder (display, window, (*config).colors.focused.pixel);
  XSetInputFocus (display, window, RevertToParent, CurrentTime);
  XRaiseWindow (display, window);
}


fn run_process (command_line: &String) {
  let mut parts = command_line.split (' ');
  let program = parts.next ().unwrap ();
  let args = parts.collect::<Vec<&str>> ();
  let r = std::process::Command::new (program)
    .args (args)
    .spawn ();
  if r.is_err () {
    log::error! ("Failed to run program: {}", command_line);
  }
}


fn main () {
  // Configure logging
  let log_file = log4rs::append::file::FileAppender::builder ()
    .encoder (Box::new (log4rs::encode::pattern::PatternEncoder::new ("{l} - {m}\n")))
    .build ("log.txt")
    .unwrap ();
  let log_config = log4rs::config::Config::builder ()
    .appender (log4rs::config::Appender::builder ()
      .build ("log_file", Box::new (log_file)))
    .build (log4rs::config::Root::builder ()
      .appender ("log_file")
      .build (log::LevelFilter::Trace))
    .unwrap ();
  log4rs::init_config (log_config).unwrap ();
  // Run window manager
  unsafe {
    log::trace! ("Connecting to X server");
    connect ();
    log::info! ("Display size: {}x{}", screen_size.w, screen_size.h);
    log::trace! ("Loading configuration");
    let mut config_instance = Config::new ();
    config_instance.load ();
    config = &config_instance;
    log::trace! ("Initializing");
    init ();
    log::trace! ("Running");
    run ();
    log::trace! ("Cleaning up");
    cleanup ();
  }
}

