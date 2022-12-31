// Need to allow this to match on the XEvent types (KeyPress etc.)
#![allow(non_upper_case_globals)]
// Also disable other superfluous naming style warnings
#![allow(non_camel_case_types)]

use std::os::raw::*;
use x11::xlib::*;

mod x;

#[macro_use]
mod core;
mod client;
mod geometry;
#[macro_use]
mod config;
mod action;
mod color;
mod config_parser;
mod event;
#[macro_use]
mod workspace;
#[macro_use]
mod property;
mod as_static;
mod bar;
mod buttons;
mod context_menu;
mod cursor;
mod desktop_entry;
mod dock;
mod draw;
mod error;
mod ewmh;
mod icon_group;
mod monitors;
mod mouse;
mod notifications;
mod platform;
mod process;
mod session_manager;
mod split_handles;
mod timeout_thread;
mod tooltip;
mod update_thread;
mod xdnd;

use crate::core::*;
use bar::Bar;
use client::*;
use config::*;
use draw::DrawingContext;
use geometry::*;
use property::Net;
use update_thread::UpdateThread;
use workspace::*;
use x::{window::AsXWindow, Display, Window, XDisplay, XNone, XWindow};

mod paths {
  pub static mut config: String = String::new();
  pub static mut autostartrc: String = String::new();
  pub static mut logfile: String = String::new();
  pub static mut resource_dir: String = String::new();
  pub static mut colors_dir: String = String::new();

  pub unsafe fn load() {
    let config_dir = if let Ok(xdg_config_home) = std::env::var("XDG_CONFIG_HOME") {
      format!("{}/window_manager", xdg_config_home)
    } else {
      format!("{}/.config/window_manager", std::env::var("HOME").unwrap())
    };
    if std::fs::create_dir_all(&config_dir).is_err() {
      my_panic!("Could not create configuration directory: {}", config_dir);
    }
    config = format!("{}/config.toml", config_dir);
    autostartrc = format!("{}/autostartrc", config_dir);
    logfile = format!("{}/log.txt", config_dir);
    resource_dir = format!("{}/res", config_dir);
    colors_dir = format!("{}/colors", config_dir);
  }
}

unsafe extern "C" fn x_error(my_display: XDisplay, event: *mut XErrorEvent) -> c_int {
  const ERROR_TEXT_SIZE: usize = 1024;
  let mut error_text_buf: [c_char; ERROR_TEXT_SIZE] = [0; ERROR_TEXT_SIZE];
  let error_text = &mut error_text_buf as *mut c_char;
  XGetErrorText(
    my_display,
    (*event).error_code as i32,
    error_text,
    ERROR_TEXT_SIZE as i32,
  );
  let error_msg = string_from_ptr!(error_text);
  eprintln!("window_manager|x-error: {}", error_msg);
  log::error!("\x1b[31mX Error: {}\x1b[0m", error_msg);
  0
}

unsafe fn connect() {
  x::init_threads();
  display = Display::connect(None);
  root = Window::from_handle(&display, display.root());
  screen_size = Geometry::from_parts(0, 0, display.width(), display.height());
}

unsafe fn update_numlock_mask() {
  let modmap = display.get_modifier_mapping();
  numlock_mask = 0;
  'outer: for i in 0..8 {
    for j in 0..(*modmap).max_keypermod {
      let check = *(*modmap)
        .modifiermap
        .add((i * (*modmap).max_keypermod + j) as usize);
      if check == display.keysym_to_keycode(x11::keysym::XK_Num_Lock as KeySym) {
        numlock_mask = 1 << i;
        break 'outer;
      }
    }
  }
  XFreeModifiermap(modmap);
}

unsafe fn grab_key_with_toggle_mods(code: u32, modifiers: u32) {
  for extra in [0, LockMask, numlock_mask, LockMask | numlock_mask] {
    display.grab_key(code, modifiers | extra);
  }
}

unsafe fn ungrab_key_with_toggle_mods(code: u32, modifiers: u32) {
  for extra in [0, LockMask, numlock_mask, LockMask | numlock_mask] {
    display.ungrab_key(code, modifiers | extra);
  }
}

unsafe fn grab_keys() {
  update_numlock_mask();
  display.ungrab_key(AnyKey as u32, AnyModifier);
  for key in (*config).key_binds.keys() {
    grab_key_with_toggle_mods(key.code, key.modifiers);
  }
}

unsafe fn grab_buttons() {
  display.ungrab_button(AnyButton as u32, AnyModifier);
  display.grab_button(1, (*config).modifier);
  display.grab_button(1, (*config).modifier | MOD_SHIFT);
  display.grab_button(3, (*config).modifier);
}

unsafe fn select_input(mut mask: c_long) {
  if mask == 0 {
    mask = SubstructureRedirectMask
      | SubstructureNotifyMask
      | ButtonPressMask
      | ButtonReleaseMask
      | PointerMotionMask
      | StructureNotifyMask
      | PropertyChangeMask;
  }
  root.change_event_mask(mask);
}

fn run_autostartrc() {
  use std::process::{Command, Stdio};
  let path = unsafe { &paths::autostartrc };
  if std::path::Path::new(path).exists() {
    Command::new("bash")
      .arg(path.as_str())
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .spawn()
      .expect("failed to run autostartrc");
  } else {
    log::info!("No autostartrc found");
  }
  log::info!("Ran autostartrc");
}

unsafe fn init() {
  wm_context = x::unique_context();
  wm_winkind_context = x::unique_context();
  property::load_atoms();
  property::init_set_root_properties();
  workspaces.reserve((*config).workspace_count);
  for _ in 0..(*config).workspace_count {
    let index = workspaces.len();
    workspaces.push(Workspace::new(index));
  }
  workspaces[0].split_handles_visible(true);
  x::set_error_handler(x_error);
  root.set_background(&(*config).colors.background);
  root.clear();
  cursor::load_cursors();
  grab_keys();
  grab_buttons();
  select_input(0);
  // Ignore SIGCHLD so we don't leave defunct processes behind
  process::ignore_sigchld(true);
  run_autostartrc();
  if cfg!(feature = "bar") {
    bar = Bar::create();
    bar.build();
    bar::tray = bar::tray_manager::TrayManager::create(bar.height);
    if (*config).bar_update_interval > 0 {
      bar::update_thread = Some(UpdateThread::new(
        (*config).bar_update_interval,
        bar::update,
      ));
    }
  }
  dock::create();
  client::set_border_info();
  notifications::init();
  session_manager::init();
  xdnd::listen();
}

const fn event_name(type_: c_int) -> &'static str {
  const EVENT_NAMES: [&str; 36] = [
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
    "GenericEvent",
  ];
  EVENT_NAMES[type_ as usize]
}

unsafe fn handle_unknown_event(event: &XEvent) -> bool {
  use std::sync::Once;
  use x11::xfixes::XFixesSelectionNotifyEvent;
  static mut selection_notify: i32 = -1;
  static INIT: Once = Once::new();
  INIT.call_once(|| {
    selection_notify = xdnd::get_selection_notify_event_type();
  });
  if event.type_ == selection_notify {
    if cfg!(feature = "xdnd-hack") {
      xdnd::selection_notify(&*(event as *const XEvent as *const XFixesSelectionNotifyEvent));
    }
  } else {
    return false;
  }
  true
}

unsafe fn run() {
  let mut event: XEvent = zeroed!();
  running = true;
  display.sync(false);
  while running {
    display.next_event(&mut event);
    if std::option_env!("WM_LOG_ALL_EVENTS").is_some() {
      if event.type_ as usize > 35 {
        log::warn!(
          "\x1b[2mEvent: \x1b[33m{:>2} Greater than LastEvent: {}\x1b[0m",
          event.type_,
          LASTEvent
        );
      } else {
        log::trace!(
          "\x1b[2mEvent: \x1b[36m{:>2} \x1b[32m{} \x1b[39mby \x1b[36m{}\x1b[0m",
          event.type_,
          event_name(event.type_),
          event.any.window
        );
      }
    }
    match event.type_ {
      ButtonPress => event::button_press(&event.button),
      ButtonRelease => event::button_relase(),
      ClientMessage => event::client_message(&event.client_message),
      ConfigureNotify => event::configure_notify(&event.configure),
      ConfigureRequest => event::configure_request(&event.configure_request),
      DestroyNotify => event::destroy_notify(&event.destroy_window),
      EnterNotify => event::crossing(&event.crossing),
      Expose => event::expose(&event.expose),
      KeyPress => event::key_press(&event.key),
      LeaveNotify => event::crossing(&event.crossing),
      MapNotify => event::map_notify(&event.map),
      MappingNotify => event::mapping_notify(&event.mapping),
      MapRequest => event::map_request(&event.map_request),
      MotionNotify => event::motion(&event.motion),
      PropertyNotify => event::property_notify(&event.property),
      UnmapNotify => event::unmap_notify(&event.unmap),
      SessionManagerEvent => session_manager::manager().process(),
      _ => {
        if handle_unknown_event(&event) {
          continue;
        }
        if std::option_env!("WM_LOG_ALL_EVENTS").is_some() {
          log::trace!("\x1b[2m     : Unhandeled\x1b[0m");
        }
      }
    }
  }
}

unsafe fn cleanup() {
  // Close all open clients
  log::trace!("Closing clients");
  for ws in workspaces.iter_mut() {
    for c in ws.iter_mut() {
      c.window.kill_client();
      c.destroy();
      c.window.destroy();
    }
  }
  // Close meta windows
  log::trace!("Killing meta windows");
  for w in meta_windows.iter() {
    w.kill_client();
    w.destroy();
  }
  // Un-grab keys and buttons
  log::trace!("Un-grabbing keys and buttons");
  for key in (*config).key_binds.keys() {
    ungrab_key_with_toggle_mods(key.code, key.modifiers);
  }
  display.ungrab_button(1, (*config).modifier);
  display.ungrab_button(1, (*config).modifier | MOD_SHIFT);
  display.ungrab_button(3, (*config).modifier);
  // Properties
  log::trace!("Removing EWMH root properties");
  property::wm_check_window.destroy();
  property::delete(root, Net::ActiveWindow);
  // Components
  log::trace!("Freeing cursors");
  cursor::free_cursors();
  log::trace!("Terminating dbus services");
  notifications::quit();
  session_manager::quit();
  log::trace!("Destroying tooltip window");
  tooltip::tooltip.destroy();
  log::trace!("Destroying bar");
  if let Some(t) = bar::update_thread.take() {
    t.stop();
  }
  bar.destroy();
  log::trace!("Destroying drawing context");
  (*draw).destroy();
  log::trace!("Destroying dock");
  dock::destroy();
}

fn get_window_geometry(window: Window) -> Geometry {
  let mut x: c_int = 0;
  let mut y: c_int = 0;
  let mut w: c_uint = 0;
  let mut h: c_uint = 0;
  let mut _border_width: c_uint = 0;
  let mut _depth: c_uint = 0;
  let mut _root: XWindow = 0;
  unsafe {
    XGetGeometry(
      display.as_raw(),
      window.handle(),
      &mut _root,
      &mut x,
      &mut y,
      &mut w,
      &mut h,
      &mut _border_width,
      &mut _depth,
    );
  }
  Geometry { x, y, w, h }
}

unsafe fn window_title(window: Window) -> String {
  // _NET_WM_NAME
  if let Some(net_wm_name) = property::get_string(window, Net::WMName) {
    net_wm_name
  }
  // XA_WM_NAME
  else if let Some(xa_wm_name) = property::get_string(window, XA_WM_NAME) {
    xa_wm_name
  }
  // XFetchName / Default
  else {
    let mut title_c_str: *mut c_char = std::ptr::null_mut();
    XFetchName(display.as_raw(), window.handle(), &mut title_c_str);
    if title_c_str.is_null() {
      "?".to_string()
    } else {
      let title = string_from_ptr!(title_c_str);
      XFree(title_c_str as *mut c_void);
      title
    }
  }
}

unsafe fn update_client_list() {
  // We can't delete a window from the client list property so we have to
  // rebuild it when deleting a window
  property::delete(root, Net::ClientList);
  for ws in workspaces.iter() {
    for c in ws.iter() {
      property::append(root, Net::ClientList, XA_WINDOW, 32, &c.window.handle(), 1);
    }
  }
}

unsafe fn get_window_kind<W: AsXWindow>(window: W) -> Option<WindowKind> {
  let window = window.as_xwindow();
  let mut data: XPointer = std::ptr::null_mut();
  if window == root.handle() {
    Some(WindowKind::Root)
  } else if window == XNone
    || XFindContext(display.as_raw(), window, wm_winkind_context, &mut data) != 0
  {
    None
  } else if !data.is_null() {
    // Can't do conversions in the match
    const kind_root: usize = WindowKind::Root as usize;
    const kind_client: usize = WindowKind::Client as usize;
    const kind_frame: usize = WindowKind::Frame as usize;
    const kind_frame_button: usize = WindowKind::Frame_Button as usize;
    const kind_status_bar: usize = WindowKind::Status_Bar as usize;
    const kind_notification: usize = WindowKind::Notification as usize;
    const kind_meta_or_unmanaged: usize = WindowKind::Meta_Or_Unmanaged as usize;
    const kind_tray_client: usize = WindowKind::Tray_Client as usize;
    const kind_dock: usize = WindowKind::Dock as usize;
    const kind_dock_item: usize = WindowKind::Dock_Item as usize;
    const kind_dock_show: usize = WindowKind::Dock_Show as usize;
    const kind_context_menu: usize = WindowKind::Context_Menu as usize;
    const kind_split_handle: usize = WindowKind::Split_Handle as usize;
    Some(match data as usize {
      kind_root => WindowKind::Root,
      kind_client => WindowKind::Client,
      kind_frame => WindowKind::Frame,
      kind_frame_button => WindowKind::Frame_Button,
      kind_status_bar => WindowKind::Status_Bar,
      kind_notification => WindowKind::Notification,
      kind_meta_or_unmanaged => WindowKind::Meta_Or_Unmanaged,
      kind_tray_client => WindowKind::Tray_Client,
      kind_dock => WindowKind::Dock,
      kind_dock_item => WindowKind::Dock_Item,
      kind_dock_show => WindowKind::Dock_Show,
      kind_context_menu => WindowKind::Context_Menu,
      kind_split_handle => WindowKind::Split_Handle,
      _ => {
        my_panic!("Invalid Window_Kind value on {}: {}", window, data as usize);
      }
    })
  } else {
    None
  }
}

unsafe fn set_window_kind(window: Window, kind: WindowKind) {
  window.save_context(wm_winkind_context, kind as usize as XPointer);
}

unsafe fn is_kind<W: AsXWindow>(window: W, kind: WindowKind) -> bool {
  if let Some(window_kind) = get_window_kind(window) {
    kind == window_kind
  } else {
    false
  }
}

/// Sets the `_NET_WM_WINDOW_OPACITY` property. This has no effect on the
/// window manager but a compositor may use this to set the opacity of the
/// entire window.
unsafe fn set_window_opacity(window: Window, percent: u32) {
  if percent != 100 {
    let value = 42949672u32 * percent;
    property::set(window, Net::WMWindowOpacity, XA_CARDINAL, 32, &value, 1);
  }
}

#[allow(dead_code)]
unsafe fn list_properties(window: Window) {
  log::info!("Properties for {} ({})", window_title(window), window);
  let atoms = {
    let mut n = 0;
    let p = XListProperties(display.as_raw(), window.handle(), &mut n);
    std::slice::from_raw_parts(p, n as usize)
  };
  for atom in atoms {
    log::info!("  {}", display.get_atom_name(*atom));
  }
}

/// Remove the hitbox of the given window.
fn mouse_passthrough(window: Window) {
  use x11::xfixes::*;
  unsafe {
    let region = XFixesCreateRegion(display.as_raw(), std::ptr::null_mut(), 0);
    // 2 = ShapeInput
    XFixesSetWindowShapeRegion(display.as_raw(), window.handle(), 2, 0, 0, region);
    XFixesDestroyRegion(display.as_raw(), region);
  }
}

unsafe fn configure_logging() {
  use log::LevelFilter;
  use log4rs::{
    append::file::FileAppender,
    config::{Appender, Config, Logger, Root},
    encode::pattern::PatternEncoder,
  };
  let log_file = FileAppender::builder()
    .append(false)
    .encoder(Box::new(PatternEncoder::new("{l:<5}| {m}\n")))
    .build(paths::logfile.as_str())
    .unwrap();
  let log_config = Config::builder()
    .appender(Appender::builder().build("log_file", Box::new(log_file)))
    // Enable logging for this crate
    .logger(Logger::builder().appender("log_file").build(
      "window_manager",
      if cfg!(debug_assertions) {
        LevelFilter::Trace
      } else {
        LevelFilter::Info
      },
    ))
    // librsvg and zbus use the root logger so turn that off
    .build(Root::builder().build(LevelFilter::Off))
    .unwrap();
  log4rs::init_config(log_config).unwrap();
}

fn main() {
  unsafe {
    paths::load();
    configure_logging();
    // Run window manager
    std::env::set_var("WM", "window_manager");
    log::trace!("Connecting to X server");
    connect();
    monitors::query();
    log::trace!("Loading configuration");
    let config_instance = Config::load();
    config = &config_instance;
    let mut drawing_context_instance = DrawingContext::new();
    draw = &mut drawing_context_instance;
    draw::load_resources();
    log::trace!("Initializing");
    init();
    log::trace!("Running");
    run();
    log::trace!("Cleaning up");
    cleanup();
    log_error!(
      match quit_reason.as_str() {
        "logout" => {
          log::info!("Logging out");
          // not implemented
          //system_shutdown::logout ()
          platform::logout()
        }
        "restart" => {
          log::info!("Rebooting system");
          system_shutdown::reboot()
        }
        "shutdown" => {
          log::info!("Shutting down system");
          system_shutdown::shutdown()
        }
        _ => Ok(()),
      },
      "  Failed:"
    );
    // For some reason this quites the program so we need to handle the quit
    // reasons before
    log::trace!("Closing X server connection");
    display.close();
  }
}
