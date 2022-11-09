use std::thread;
use libc::{c_long, c_uint, c_uchar};
use std::ffi::CString;
use x11::xlib::*;
use crate::core::*;
use crate::property;
use crate::property::Net;
use crate::cursor;
use super::xembed;
use super::tray_client::Tray_Client;

const ORIENTATION_HORIZONTAL: c_uint = 0;

const OPCODE_REQUEST_DOCK: c_uint = 0;
//const OPCODE_BEGIN_MESSAGE: c_uint = 1;
//const OPCODE_CANCEL_MESSAGE: c_uint = 2;


pub struct Tray_Manager {
  window: Window,
  clients: Vec<Tray_Client>,
  notify_thread: Option<thread::JoinHandle<()>>,
  current_mapped_count: usize,
  height: u32,
  is_mapped: bool
}


impl Tray_Manager {
  pub const fn new () -> Self {
    Self {
      window: X_NONE,
      clients: Vec::new (),
      notify_thread: None,
      current_mapped_count: 0,
      height: 0,
      is_mapped: false
    }
  }

  /// Returns the tray window
  pub fn window (&self) -> Window {
    self.window
  }

  /// Notifies clients about the new tray manager
  unsafe fn notify_clients (&self) {
    let mut event: XEvent = std::mem::zeroed ();
    let message = &mut event.client_message;
    message.window = root;
    message.message_type = property::atom (property::Other::Manager);
    message.format = 32;
    message.data.set_long (0, CurrentTime as c_long);
    message.data.set_long (1, property::atom (Net::SystemTrayS0) as c_long);
    message.data.set_long (2, self.window as c_long);
    message.data.set_long (3, 0);
    message.data.set_long (4, 0);
    XSendEvent (display, root, X_FALSE, NoEventMask, &mut event);
    XSync (display, X_FALSE);
  }

  /// Does a non-blocking delayed call of `notify_clients`
  unsafe fn notify_clients_after (&mut self, milliseconds: u64) {
    log::trace! ("tray: notifying clients about new manager in {milliseconds}ms");
    let this: &'static Self = &*(self as *const Self);
    self.notify_thread = Some (thread::spawn (move || {
      thread::sleep (std::time::Duration::from_millis (milliseconds));
      (*this).notify_clients ();
    }));
  }

  /// Creates the tray window and gets the tray selection
  pub unsafe fn create (height: u32) -> Self {
    log::trace! ("Creating tray window");
    let screen = XDefaultScreen (display);
    let mut attributes: XSetWindowAttributes = uninitialized! ();
    attributes.override_redirect = X_TRUE;
    attributes.background_pixel = (*config).colors.bar_background.pixel;
    attributes.event_mask = SubstructureRedirectMask | StructureNotifyMask
      | ExposureMask | PropertyChangeMask;
    attributes.cursor = cursor::normal;
    let window = XCreateWindow (
      display,
      root,
      (screen_size.w - height) as i32,
      0,
      height,
      height,
      0,
      XDefaultDepth (display, screen),
      CopyFromParent as u32,
      XDefaultVisual(display, screen),
      CWOverrideRedirect|CWBackPixel|CWEventMask|CWCursor,
      &mut attributes
    );
    meta_windows.push (window);
    let window_type_dock = property::atom (property::Net::WMWindowTypeDock);
    property::set (
      window,
      property::Net::WMWindowType,
      XA_ATOM,
      32,
      &window_type_dock,
      1
    );
    if (*config).bar_opacity != 100 {
      let atom = XInternAtom (display, c_str! ("_NET_WM_WINDOW_OPACITY"), X_FALSE);
      let value = 42949672u32 * (*config).bar_opacity as u32;
      set_cardinal! (window, atom, value);
    }
    set_cardinal! (
      window,
      property::atom (Net::SystemTrayOrientation),
      ORIENTATION_HORIZONTAL
    );
    {
      let protocols = &mut [property::atom (property::WM::TakeFocus)];
      XSetWMProtocols (
        display,
        window,
        protocols.as_mut_ptr (),
        protocols.len () as i32
      );
    }

    let atom = property::atom (Net::SystemTrayS0);
    if XGetSelectionOwner (display, atom) != X_NONE {
      my_panic! ("Tray selection already owned");
    }
    log::trace! ("Settings tray selection");
    XSetSelectionOwner (display, atom, window, CurrentTime);
    if XGetSelectionOwner (display, atom) != window {
      my_panic! ("Failed to set tray selection");
    }

    let mut this = Self {
      window,
      clients: Vec::with_capacity (8),
      notify_thread: None,
      current_mapped_count: 0,
      height,
      is_mapped: false
    };
    this.notify_clients_after (1000);
    this
  }

  /// Resizes and positions the window. The main bar window is resized as needed.
  unsafe fn resize_window (&mut self) {
    let width = self.width ();
    bar.resize (screen_size.w - width);
    if width != 0 {
      if !self.is_mapped {
        XMapWindow (display, self.window);
        self.is_mapped = true;
      }
      XMoveResizeWindow (
        display,
        self.window,
        (screen_size.w - width) as i32,
        0,
        width,
        self.height
      );
    } else {
      XUnmapWindow (display, self.window);
      self.is_mapped = false;
    }
  }

  /// Returns the visible width of the tray
  fn width (&self) -> u32 {
    self.current_mapped_count as u32 * self.height
  }

  /// Retrieves the number of mapped icon windows
  unsafe fn mapped_count (&self) -> usize {
    self.clients.iter ()
      .fold (0, |acc, icon| acc + icon.is_mapped () as usize)
  }

  // If `window` is the window of a tray client, remove that client
  pub unsafe fn maybe_remove_client (&mut self, window: Window) -> bool {
    if let Some (idx) = self.find_client_index (window) {
      self.clients.remove (idx);
      self.arrange_icons ();
      log::debug !("\x1b[92mRemoved tray icon {}\x1b[0m", window);
      self.refresh ();
      true
    } else {
      false
    }
  }

  /// Finds the index of a client by its window
  pub unsafe fn find_client_index (&self, window: Window) -> Option<usize> {
    self.clients.iter ()
                .position (|c| c.window () == window)
  }

  /// Finds a client by its window
  pub unsafe fn find_client (&mut self, window: Window) -> Option<&mut Tray_Client> {
    self.find_client_index (window)
        .map (|idx| &mut self.clients[idx])
  }

  /// Updates the position of all clients
  unsafe fn arrange_icons (&mut self) {
    for (idx, icon) in self.clients.iter ()
                                       .filter (|c| c.is_mapped ())
                                       .enumerate () {
      icon.set_position (idx as i32 * self.height as i32, 0);
    }
  }

  /// Refreshes the tray by clearing it and reconfiguring its clients
  pub unsafe fn refresh (&self) {
    if !self.is_mapped {
      return;
    }

    XClearWindow (display, self.window);

    let mut x = 0;
    for client in self.clients.iter () {
      if client.is_mapped () {
        // Some clients need this or they have the wrong size (?)
        client.configure (x, 0);
        x += self.height as i32;
      }
    }

    XFlush (display);
  }

  /// Reconfigures the tray
  pub unsafe fn reconfigure (&mut self) {
    self.current_mapped_count = self.mapped_count ();
    self.resize_window ();
    self.arrange_icons ();
    self.refresh ();
  }

  /// Sorts clients alphabetically by their class
  unsafe fn sort_clients (&mut self) {
    self.clients.sort_by_key (|c| c.class ());
  }

  /// Handles a docking request
  unsafe fn dock (&mut self, window: Window) {
    let client_idx = self.clients.len () as i32;

    self.clients.push (Tray_Client::new (window, self.height));
    let client = self.clients.last_mut ().unwrap ();

    client.query_xembed_info ();
    //let should_map = client.xembed_info ().is_mapped ();

    log::trace! ("tray: update client attributes");
    let mut attributes: XSetWindowAttributes = uninitialized! ();
    attributes.event_mask = StructureNotifyMask | PropertyChangeMask;
    attributes.background_pixel = (*config).colors.bar_background.pixel;
    XChangeWindowAttributes (display, window, CWEventMask|CWBackPixel, &mut attributes);

    log::trace! ("tray: reparent client");
    {
      let x = client_idx * self.height as i32;
      XReparentWindow (display, window, self.window, x, 0);
    }

    log::trace! ("tray: send xembed notification");
    xembed::embed (window, self.window, client.xembed_info ().version ());

    self.sort_clients ();

    // TODO: some clients are never mapped for some reason (observed with `nm-tray`)
    //if should_map {
    if true {
      log::trace! ("tray: map client");
      XMapWindow (display, window);
      self.update_mapped_count ();
    } else {
      log::trace! ("tray: client should not be mapped");
    }

    log::debug !("\x1b[92mtray: added tray icon {}\x1b[0m", window);
  }

  /// Checks the mapped count and reconfigures the tray if neccessary
  unsafe fn update_mapped_count (&mut self) {
    let mapped_count = self.mapped_count ();
    if mapped_count != self.current_mapped_count {
      self.current_mapped_count = mapped_count;
      self.reconfigure ();
    }
  }

  /// Handles a client message
  pub unsafe fn client_message (&mut self, event: &XClientMessageEvent) {
    let opcode = event.data.get_long (1) as u32;
    const OPCODE_NAMES: [&str; 3] = ["REQUEST_DOCK", "BEGIN_MESSAGE", "CANCEL_MESSAGE"];
    log::debug! ("Tray client message");
    log::debug! ("  Opcode: {} ({})", opcode, OPCODE_NAMES[opcode as usize]);
    if opcode == OPCODE_REQUEST_DOCK {
      self.dock (event.data.get_long (2) as Window);
    }
  }

  /// Handles a PropertyNodify event
  pub unsafe fn property_notifty (&mut self, event: &XPropertyEvent) {
    if let Some (client) = self.find_client (event.window) {
      client.query_xembed_info ();
      client.update_mapped_state ();
      log::trace! ("\x1b[92mtray: {} is now {}\x1b[0m", event.window,
                   if client.xembed_info ().is_mapped () {"mapped"} else {"unmapped"});
      self.update_mapped_count ();
    }
  }

  /// Handles a MapNotify event
  pub unsafe fn map_notify (&mut self, event: &XMapEvent) {
    if let Some (client) = self.find_client (event.window) {
      log::debug !("\x1b[92mtray: mapped tray icon {}\x1b[0m", event.window);
      client.set_mapped (true);
      self.update_mapped_count ();
    }
  }

  /// Handles a UnmapNotify event
  pub unsafe fn unmap_notify (&mut self, event: &XUnmapEvent) {
    if let Some (client) = self.find_client (event.window) {
      log::debug !("\x1b[92mtray: unmapped tray icon {}\x1b[0m", event.window);
      client.set_mapped (false);
      self.update_mapped_count ();
    }
  }
}

impl Drop for Tray_Manager {
  fn drop (&mut self) {
    if let Some (notify_thread) = self.notify_thread.take () {
      notify_thread.join ().ok ();
    }
  }
}
