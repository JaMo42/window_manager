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
#[macro_use]
mod config;
mod action;
mod event;
mod config_parser;
mod color;
#[macro_use]
mod workspace;
mod hibernate;
#[macro_use]
mod property;
mod cursor;
mod draw;

use crate::core::*;
use client::*;
use geometry::*;
use config::*;
use workspace::*;
use property::Net;
use draw::Drawing_Context;

mod paths {
  pub static mut config: String = String::new ();
  pub static mut autostartrc: String = String::new ();
  pub static mut hiberfile: String = String::new ();
  pub static mut logfile: String = String::new ();

  pub unsafe fn load () {
    let config_dir = if let Ok (xdg_config_home) = std::env::var ("XDG_CONFIG_HOME") {
      format! ("{}/window_manager", xdg_config_home)
    }
    else {
      format! ("{}/.config/window_manager", std::env::var ("HOME").unwrap ())
    };
    if std::fs::create_dir_all (&config_dir).is_err () {
      panic! ("Could not create configuration directory: {}", config_dir);
    }
    config = format! ("{}/config", config_dir);
    autostartrc = format! ("{}/autostartrc", config_dir);
    hiberfile = format! ("{}/.hiberfile", config_dir);
    logfile = format! ("{}/log.txt", config_dir);
  }
}


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
  0
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


unsafe fn update_numlock_mask () {
  let modmap = XGetModifierMapping (display);
  numlock_mask = 0;
  for i in 0..8 {
    for j in 0..(*modmap).max_keypermod {
      let check = *(*modmap).modifiermap.add ((i * (*modmap).max_keypermod + j) as usize);
      if check == XKeysymToKeycode (display, x11::keysym::XK_Num_Lock as u64) {
        numlock_mask = 1 << i;
      }
    }
  }
  XFreeModifiermap (modmap);
}


unsafe fn grab_keys () {
  update_numlock_mask ();
  XUngrabKey (display, AnyKey as i32, AnyModifier, root);
  for key in (*config).key_binds.keys () {
    for extra in [0, LockMask, numlock_mask, LockMask|numlock_mask] {
      XGrabKey (
        display,
        key.code as c_int,
        key.modifiers | extra,
        root,
        X_TRUE,
        GrabModeAsync,
        GrabModeAsync
      );
    }
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
  workspaces.reserve ((*config).workspace_count);
  for _ in 0..(*config).workspace_count {
    workspaces.push (Workspace::new ());
  }
  // Set error handler
  XSetErrorHandler (Some (x_error));
  // Colors
  XSetWindowBackground (display, root, (*config).colors.background.pixel);
  XClearWindow (display, root);
  // Properties
  property::load_atoms ();
  property::init_set_root_properties ();
  // Hibernation
  if (*config).hibernate {
    select_input (SubstructureRedirectMask);
    if hibernate::load ().is_err () {
      log::error! ("Could not read hiberfile");
    }
  }
  // Cursors
  cursor::load_cursors ();
  // Grab input
  grab_keys ();
  grab_buttons ();
  // Select input
  select_input (0);
  // Ignore SIGCHLD so we don't leave defunct processes behind
  libc::signal (libc::SIGCHLD, libc::SIG_IGN);
  // Run autostart script
  {
    use std::process::{Command, Stdio};
    if std::path::Path::new (&paths::autostartrc).exists () {
      Command::new ("bash")
        .arg (paths::autostartrc.as_str ())
        .stdout (Stdio::null ())
        .stderr (Stdio::null ())
        .spawn ()
        .expect ("failed to run autostartrc");
    }
    else {
      log::info! ("No autostartrc found");
    }
  }
}

unsafe fn run () {
  const EVENT_NAME: [&str; 36] = [
    "",
    "",
    "KeyPress",
    "KeyRelease",
    "ButtonPress",
    "ButtonRelease",
    "MotionNotify",
    "EnterNotify",
    "LeaveNotify",
    "FocusIn",
    "FocusOut",
    "KeymapNotify",
    "Expose",
    "GraphicsExpose",
    "NoExpose",
    "VisibilityNotify",
    "CreateNotify",
    "DestroyNotify",
    "UnmapNotify",
    "MapNotify",
    "MapRequest",
    "ReparentNotify",
    "ConfigureNotify",
    "ConfigureRequest",
    "GravityNotify",
    "ResizeRequest",
    "CirculateNotify",
    "CirculateRequest",
    "PropertyNotify",
    "SelectionClear",
    "SelectionRequest",
    "SelectionNotify",
    "ColormapNotify",
    "ClientMessage",
    "MappingNotify",
    "GenericEvent"
  ];
  let mut event: XEvent = uninitialized! ();
  running = true;
  XSync (display, X_FALSE);
  while running {
    XNextEvent (display, &mut event);
    if cfg!(debug_assertions) {
      if event.type_ as usize > 35 {
        log::trace! ("Event: {:>2} ???", event.type_);
      }
      else {
        log::trace! ("\x1b[2mEvent: \x1b[36m{:>2} \x1b[32m{}\x1b[0m", event.type_, EVENT_NAME[event.type_ as usize]);
      }
    }
    match event.type_ {
      ButtonPress => event::button_press (&event.button),
      ClientMessage => event::client_message (&event.client_message),
      ConfigureRequest => event::configure_request (&event.configure_request),
      DestroyNotify => event::destroy_notify (&event.destroy_window),
      KeyPress => event::key_press (&event.key),
      MappingNotify => event::mapping_notify (&event.mapping),
      MapRequest => event::map_request (&event.map_request),
      MotionNotify => event::motion (&event.button),
      PropertyNotify => event::property_notify (&event.property),
      _ => {
        if cfg!(debug_assertions) {
          log::trace! ("\x1b[2m     : Unhandeled\x1b[0m");
        }
      }
    }
  }
}


unsafe fn cleanup () {
  // Hibernation
  #[allow(clippy::collapsible_if)]
  if (*config).hibernate {
    if hibernate::store ().is_err () {
      log::error! ("Could not write hiberfile");
      std::fs::remove_file (&paths::hiberfile).ok ();
    }
  }
  // Close all open clients
  for ws in workspaces.iter () {
    for c in ws.iter () {
      XDestroyWindow (display, c.window);
    }
  }
  // Close meta windows
  for w in meta_windows.iter () {
    XKillClient (display, *w);
  }
  // Cursors
  cursor::free_cursors ();
  // Un-grab keys and buttons
  for key in (*config).key_binds.keys () {
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
  // Properties
  XDestroyWindow (display, property::wm_check_window);
  property::delete (root, Net::ActiveWindow);
  XSync (display, X_FALSE);
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

  Geometry { x, y, w, h }
}


unsafe fn window_title (window: Window) -> String {
  // _NET_WM_NAME
  if let Some (net_wm_name) = property::get_string (window, Net::WMName) {
    net_wm_name
  }
  // XA_WM_NAME
  else if let Some (xa_wm_name) = property::get_string (window, XA_WM_NAME) {
    xa_wm_name
  }
  // XFetchName / Default
  else {
    let mut title_c_str: *mut c_char = std::ptr::null_mut ();
    XFetchName (display, window, &mut title_c_str);
    if title_c_str.is_null () {
      "?".to_string ()
    }
    else {
      let title = string_from_ptr! (title_c_str);
      XFree (title_c_str as *mut c_void);
      title
    }
  }
}


unsafe fn update_client_list () {
  // We can't delete a window from the client list property so we have to
  // rebuild it when deleting a window
  property::delete (root, Net::ClientList);
  for ws in workspaces.iter () {
    for c in ws.iter () {
      property::append (root, Net::ClientList, XA_WINDOW, 32, &c.window, 1);
    }
  }
}


fn run_process (command_line: &str) {
  use std::process::{Command, Stdio};
  let mut parts = command_line.split (' ');
  let program = parts.next ().unwrap ();
  let args = parts.collect::<Vec<&str>> ();
  if Command::new (program)
    .args (args)
    .stdout (Stdio::null ())
    .stderr (Stdio::null ())
    .spawn ()
    .is_ok () {
    log::trace! ("Launched process: {}", command_line);
  }
  else {
    log::error! ("Failed to run process: {}", command_line);
  }
}


fn main () {
  unsafe {
    paths::load ();
    // Configure logging
    let log_file = log4rs::append::file::FileAppender::builder ()
      .encoder (Box::new (log4rs::encode::pattern::PatternEncoder::new ("{l:<5}| {m}\n")))
      .build (paths::logfile.as_str ())
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
    std::env::set_var ("WM", "window_manager");
    log::trace! ("Connecting to X server");
    connect ();
    log::info! ("Display size: {}x{}", screen_size.w, screen_size.h);
    log::trace! ("Loading configuration");
    let mut config_instance = Config::new ();
    config_instance.load ();
    config = &config_instance;
    let mut drawing_context_instance = Drawing_Context::new ();
    draw = &mut drawing_context_instance;
    log::trace! ("Initializing");
    init ();
    log::trace! ("Running");
    run ();
    log::trace! ("Cleaning up");
    cleanup ();
    XCloseDisplay (display);
  }
}
