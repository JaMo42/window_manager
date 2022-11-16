use super::tray_client::Tray_Client;
use super::xembed;
use crate::core::*;
use crate::cursor;
use crate::property;
use crate::property::Net;
use crate::x::{Window, XFalse, XNone, XWindow};
use libc::{c_long, c_uchar, c_uint};
use std::thread;
use x11::xlib::*;

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
  is_mapped: bool,
}

impl Tray_Manager {
  pub const fn new () -> Self {
    Self {
      window: Window::uninit (),
      clients: Vec::new (),
      notify_thread: None,
      current_mapped_count: 0,
      height: 0,
      is_mapped: false,
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
    message.window = root.handle ();
    message.message_type = property::atom (property::Other::Manager);
    message.format = 32;
    message.data.set_long (0, CurrentTime as c_long);
    message
      .data
      .set_long (1, property::atom (Net::SystemTrayS0) as c_long);
    message.data.set_long (2, self.window.handle () as c_long);
    message.data.set_long (3, 0);
    message.data.set_long (4, 0);
    XSendEvent (
      display.as_raw (),
      root.handle (),
      XFalse,
      NoEventMask,
      &mut event,
    );
    display.sync (false);
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
    let window = Window::builder (&display)
      .position ((screen_size.w - height) as i32, 0)
      .size (height, height)
      .attributes (|attributes| {
        attributes
          .override_redirect (true)
          .background_pixel ((*config).colors.bar_background.pixel)
          .event_mask (
            SubstructureRedirectMask | StructureNotifyMask | ExposureMask | PropertyChangeMask,
          )
          .cursor (cursor::normal);
      })
      .build ();
    meta_windows.push (window);
    crate::ewmh::set_window_type (window, property::Net::WMWindowTypeDock);
    crate::set_window_kind (window, Window_Kind::Meta_Or_Unmanaged);
    if (*config).bar_opacity != 100 {
      let atom = display.intern_atom ("_NET_WM_WINDOW_OPACITY");
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
        display.as_raw (),
        window.handle (),
        protocols.as_mut_ptr (),
        protocols.len () as i32,
      );
    }

    let selection = property::atom (Net::SystemTrayS0);
    if display.get_selection_owner (selection) != XNone {
      my_panic! ("Tray selection already owned");
    }
    log::trace! ("Settings tray selection");
    display.set_selection_ownder (selection, window);
    if window != display.get_selection_owner (selection) {
      my_panic! ("Failed to set tray selection");
    }

    let mut this = Self {
      window,
      clients: Vec::with_capacity (8),
      notify_thread: None,
      current_mapped_count: 0,
      height,
      is_mapped: false,
    };
    this.notify_clients_after (1000);
    this
  }

  pub unsafe fn destroy (&mut self) {
    for c in self.clients.iter () {
      c.window ().reparent (root, 0, 0);
    }
    self.window.destroy ();
    if let Some (notify_thread) = self.notify_thread.take () {
      notify_thread.join ().ok ();
    }
    self.notify_thread = None;
  }

  /// Resizes and positions the window. The main bar window is resized as needed.
  unsafe fn resize_window (&mut self) {
    let width = self.width ();
    bar.resize (screen_size.w - width);
    if width != 0 {
      if !self.is_mapped {
        self.window.map ();
        self.is_mapped = true;
      }
      self
        .window
        .move_and_resize ((screen_size.w - width) as i32, 0, width, self.height);
    } else {
      self.window.unmap ();
      self.is_mapped = false;
    }
  }

  /// Returns the visible width of the tray
  fn width (&self) -> u32 {
    self.current_mapped_count as u32 * self.height
  }

  /// Retrieves the number of mapped icon windows
  unsafe fn mapped_count (&self) -> usize {
    self
      .clients
      .iter ()
      .fold (0, |acc, icon| acc + icon.is_mapped () as usize)
  }

  // If `window` is the window of a tray client, remove that client
  pub unsafe fn maybe_remove_client (&mut self, window: XWindow) -> bool {
    if let Some (idx) = self.find_client_index (window) {
      self.clients.remove (idx);
      self.arrange_icons ();
      log::debug! ("\x1b[92mRemoved tray icon {}\x1b[0m", window);
      self.refresh ();
      true
    } else {
      false
    }
  }

  /// Finds the index of a client by its window
  pub unsafe fn find_client_index (&self, window: XWindow) -> Option<usize> {
    self.clients.iter ().position (|c| c.window () == window)
  }

  /// Finds a client by its window
  pub unsafe fn find_client (&mut self, window: XWindow) -> Option<&mut Tray_Client> {
    self
      .find_client_index (window)
      .map (|idx| &mut self.clients[idx])
  }

  /// Updates the position of all clients
  unsafe fn arrange_icons (&mut self) {
    for (idx, icon) in self
      .clients
      .iter ()
      .filter (|c| c.is_mapped ())
      .enumerate ()
    {
      icon.set_position (idx as i32 * self.height as i32, 0);
    }
  }

  /// Refreshes the tray by clearing it and reconfiguring its clients
  pub unsafe fn refresh (&self) {
    if !self.is_mapped {
      return;
    }

    self.window.clear ();

    let mut x = 0;
    for client in self.clients.iter () {
      if client.is_mapped () {
        // Some clients need this or they have the wrong size (?)
        client.configure (x, 0);
        x += self.height as i32;
      }
    }

    display.flush ();
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
  unsafe fn dock (&mut self, window: XWindow) {
    let client_idx = self.clients.len () as i32;

    self.clients.push (Tray_Client::new (window, self.height));
    let client = self.clients.last_mut ().unwrap ();
    let window = client.window ();

    client.query_xembed_info ();
    //let should_map = client.xembed_info ().is_mapped ();

    log::trace! ("tray: update client attributes");
    window.change_attributes (|attributes| {
      attributes
        .event_mask (StructureNotifyMask | PropertyChangeMask)
        .background_pixel ((*config).colors.bar_background.pixel);
    });

    log::trace! ("tray: reparent client");
    {
      let x = client_idx * self.height as i32;
      window.reparent (self.window, x, 0);
    }

    log::trace! ("tray: send xembed notification");
    xembed::embed (window, self.window, client.xembed_info ().version ());

    // if should_map
    client.set_mapped (true);

    self.sort_clients ();

    // TODO: some clients are never mapped for some reason (observed with `nm-tray`)
    //if should_map {
    if true {
      log::trace! ("tray: map client");
      window.map ();
      self.update_mapped_count ();
    } else {
      log::trace! ("tray: client should not be mapped");
    }

    log::debug! ("\x1b[92mtray: added tray icon {}\x1b[0m", window);
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
      self.dock (event.data.get_long (2) as XWindow);
    }
  }

  /// Handles a PropertyNodify event
  pub unsafe fn property_notifty (&mut self, event: &XPropertyEvent) {
    if let Some (client) = self.find_client (event.window) {
      client.query_xembed_info ();
      client.update_mapped_state ();
      log::trace! (
        "\x1b[92mtray: {} is now {}\x1b[0m",
        event.window,
        if client.xembed_info ().is_mapped () {
          "mapped"
        } else {
          "unmapped"
        }
      );
      self.update_mapped_count ();
    }
  }

  /// Handles a MapNotify event
  pub unsafe fn map_notify (&mut self, event: &XMapEvent) {
    if let Some (client) = self.find_client (event.window) {
      log::debug! ("\x1b[92mtray: mapped tray icon {}\x1b[0m", event.window);
      client.set_mapped (true);
      self.update_mapped_count ();
    }
  }

  /// Handles a UnmapNotify event
  pub unsafe fn unmap_notify (&mut self, event: &XUnmapEvent) {
    if let Some (client) = self.find_client (event.window) {
      log::debug! ("\x1b[92mtray: unmapped tray icon {}\x1b[0m", event.window);
      client.set_mapped (false);
      self.update_mapped_count ();
    }
  }
}
