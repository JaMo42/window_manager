use std::thread;
use zbus::{Connection, Result, SignalContext, dbus_interface};
use zbus::zvariant;
use futures::executor;
use x11::xlib::*;
use super::core::*;
use super::set_window_kind;
use super::property::Net;
use super::ewmh;

/// The notification expired
const CLOSE_REASON_EXPIRED: u32 = 1;
/// The notification was dismissed by the user
const CLOSE_REASON_DISMISSED: u32 = 2;
/// The notification was closed by a call to `CloseNotification`
const CLOSE_REASON_CLOSED: u32 = 3;

static mut _dbus_connection: Option<Connection> = None;
static mut _manager: Manager = Manager::new ();

fn dbus_connection () -> &'static mut Connection {
  unsafe { _dbus_connection.as_mut ().unwrap_unchecked () }
}

fn manager () -> &'static mut Manager {
  unsafe { &mut _manager }
}



struct Notification {
  id: u32,
  window: Window,
  width: u32,
  height: u32,
  summary: String,
  body: String
}

impl Notification {
  pub fn new (id: u32, summary: &str, body: &str) -> Self {
    let window = unsafe {
      let mut attributes: XSetWindowAttributes = uninitialized! ();
      attributes.background_pixel = (*config).colors.bar_background.pixel;
      attributes.event_mask = ButtonPressMask;
      XCreateWindow (
        display, root,
        0, 0, 10, 10,
        0,
        CopyFromParent,
        CopyFromParent as u32,
        CopyFromParent as *mut Visual,
        CWBackPixel|CWEventMask,
        &mut attributes
      )
    };
    unsafe {
      ewmh::set_window_type (window, Net::WMWindowTypeNotification);
      set_window_kind (window, Window_Kind::Notification);
    }
    let mut this = Self {
      id,
      window,
      width: 0,
      height: 0,
      summary: String::new (),
      body: String::new ()
    };
    this.replace (summary, body);
    unsafe { XMapWindow (display, window); }
    this
  }

  pub fn destroy (&self) {
    unsafe {
      XDestroyWindow (display, self.window);
    }
  }

  unsafe fn draw (&self) -> (u32, u32) {
    const BORDER: u32 = 5;
    let width;
    let body_y;
    let height;
    let background = (*config).colors.notification_background;
    let foreground = (*config).colors.notification_text;
    (*draw).select_font(&(*config).bar_font);
    // Determine width needed for the text
    {
      let longer_text = if self.summary.len () > self.body.len () {
        self.summary.as_str ()
      } else {
        self.body.as_str ()
      };
      let text = (*draw).text (longer_text);
      width = text.get_width () + 2*BORDER;
    }
    // Summary
    {
      let mut summary_text = (*draw).text (&self.summary);
      body_y = summary_text.get_height () + 2*BORDER;
      // fill_rect can't use scaled colors
      (*draw).rect (0, 0, width, body_y)
        .color (background.scale (0.9))
        .draw ();
      summary_text.at (BORDER as i32, BORDER as i32)
        .color (foreground)
        .draw ();
    }
    if !self.body.is_empty () {
      // Body
      {
        let mut body_text = (*draw).text (&self.body);
        height = body_y + body_text.get_height () + 2*BORDER;
        (*draw).fill_rect (
          0, body_y as i32,
          width + 2*BORDER, height - body_y,
          background
        );
        body_text.at (BORDER as i32, (body_y + BORDER) as i32)
          .color (foreground)
          .draw ();
      }
      // Separator
      (*draw).rect (0, body_y as i32 - 1, width, 2)
        .color (background.scale (1.1))
        .draw ();
    } else {
      height = body_y;
    }
    // Render
    XResizeWindow (display, self.window, width, height);
    (*draw).render (self.window, 0, 0, width, height);
    (width, height)
  }

  pub fn replace (&mut self, summary: &str, body: &str) {
    self.summary = summary.to_owned ();
    self.body = body.to_owned ();
    unsafe {
      (self.width, self.height) = self.draw ();
    };
  }
}



struct Manager {
  notifications: Vec<Notification>,
  next_id: u32,
  timeout_threads: Vec<thread::JoinHandle<()>>
}

impl Manager {
  const fn new () -> Self {
    Self {
      notifications: Vec::new (),
      next_id: 1,
      timeout_threads: Vec::new ()
    }
  }

  fn get_id (&mut self, replaces: u32) -> u32 {
    if replaces == 0 {
      let id = self.next_id;
      // Could technically still get the same id twice
      self.next_id = self.next_id.overflowing_add (1).0;
      id
    } else {
      replaces
    }
  }

  fn find (&self, id: u32) -> Option<usize> {
    self.notifications.iter ().position (|n| n.id == id)
  }

  fn new_notification (&mut self, id: u32, summary: &str, body: &str) {
    if let Some (idx) = self.find (id) {
      self.notifications[idx].replace (summary, body);
    } else {
      self.notifications.push (Notification::new (id, summary, body));
    }
    self.update ();
    self.arrange ();
  }

  fn close_notification (&mut self, id: u32) {
    if let Some (idx) = self.find (id) {
      self.notifications.remove (idx).destroy ();
      self.arrange ();
    }
  }

  /// Repositions all notifications
  fn arrange (&mut self) {
    let mut y = unsafe { window_area.y };
    let x_right = unsafe { window_area.x + window_area.w as i32};
    for n in self.notifications.iter () {
      unsafe {
        XMoveWindow (display, n.window, x_right - n.width as i32, y);
      }
      y += n.height as i32 + 10;
    }
    unsafe { XSync (display, X_FALSE); }
  }

  /// Redraws all notifications
  fn update (&self) {
    for n in self.notifications.iter () {
      unsafe { n.draw () };
    }
  }

  fn maybe_close (&mut self, window: Window) -> bool {
    if let Some (idx) = self.notifications.iter ().position (|n| n.window == window) {
      let id = self.notifications[idx].id;
      self.close_notification (id);
      executor::block_on (signal_close (id, CLOSE_REASON_DISMISSED));
      true
    } else {
      false
    }
  }

  fn join_finished_timeout_threads (&mut self) {
    for i in (0..self.timeout_threads.len ()).rev () {
      if self.timeout_threads[i].is_finished () {
        self.timeout_threads.remove (i).join ().ok ();
      }
    }
  }

  fn close_after (&mut self, id: u32, timeout: i32) {
    self.join_finished_timeout_threads ();
    self.timeout_threads.push (thread::spawn (move || {
      thread::sleep (std::time::Duration::from_millis (timeout as u64));
      manager ().close_notification (id);
      executor::block_on (signal_close (id, CLOSE_REASON_EXPIRED));
    }));
  }
}



struct Server {
  manager: &'static mut Manager
}

#[dbus_interface (name="org.freedesktop.Notifications")]
impl Server {
  /// `org.freedesktop.Notifications.GetServerInformation`
  async fn get_server_information (&self) -> (&str, &str, &str, &str) {
    (
      "window_manager_notification_server",  // name
      "window_manager",  // vendor
      "1.0",  // server version
      "1.2"  // spec version
    )
  }

  /// `org.freedesktop.Notifications.GetCapabilities`
  async fn get_capabilities (&self) -> Vec<&str> {
    vec![
      "body",
      "persistence"
    ]
  }

  /// `org.freedesktop.Notifications.Notify`
  #[allow(clippy::too_many_arguments)]
  async fn notify (
    &mut self,
    _app_name: &str,
    replaces_id: u32,
    _app_icon: &str,
    summary: &str,
    body: &str,
    _actions: Vec<&str>,
    _hints: std::collections::HashMap<&str, zvariant::Value<'_>>,
    expire_timeout: i32
  ) -> u32 {
    let id = self.manager.get_id (replaces_id);
    self.manager.new_notification (id, summary, body);
    if expire_timeout < 0 && unsafe {&*config}.default_notification_timeout != 0{
      manager ().close_after (id, unsafe {&*config}.default_notification_timeout);
    } else if expire_timeout > 0 {
      manager ().close_after (id, expire_timeout);
    }
    id
  }

  /// `org.freedesktop.Notifications.CloseNotification`
  async fn close_notification (&mut self, id: u32) {
    self.manager.close_notification (id);
    signal_close (id, CLOSE_REASON_CLOSED).await;
  }

  /// `org.freedesktop.Notifications.NotificationClosed`
  #[dbus_interface (signal)]
  async fn notification_closed (&self, ctxt: &SignalContext<'_>, id: u32, reason: u32) -> Result<()>;
}



/// Sends a `org.freedesktop.Notifications.NotificationClosed` signal
async fn signal_close (id: u32, reason: u32) {
  let iface_ref = dbus_connection ()
    .object_server ()
    .interface::<_, Server> ("/org/freedesktop/Notifications").await.unwrap ();
  let iface = iface_ref.get_mut ().await;
  iface.notification_closed (iface_ref.signal_context (), id, reason).await.unwrap ();
}

/// async implementation for `init`
async unsafe fn do_init () -> Result<()> {
  let connection = Connection::session ().await?;
  let server = Server {
    manager: manager ()
  };
  connection
    .object_server ()
    .at ("/org/freedesktop/Notifications", server).await?;
  connection
    .request_name ("org.freedesktop.Notifications").await?;
  _dbus_connection = Some (connection);
  Ok (())
}

/// async implementation for `quit`
async fn do_quit () -> Result<()> {
  manager ().join_finished_timeout_threads ();
  dbus_connection ()
    .object_server ()
    .remove::<Server, _> ("/org/freedesktop/Notifications").await?;
  dbus_connection ()
    .release_name ("org.freedesktop.Notifications").await?;
  Ok (())
}

/// Initializes the service
pub unsafe fn init () {
  if let Err (error) = executor::block_on (do_init ()) {
    log::error! ("Failed to initialize notification server: {}", error);
  } else {
    log::info! ("Initialized notification server");
  }
}

/// Terminates the service
pub unsafe fn quit () {
  if let Err (error) = executor::block_on (do_quit ()) {
    log::error! ("Error during notification server shutdown: {}", error);
  } else {
    log::info! ("Terminated notification server");
  }
}

/// If `window` is the window of a notification, closes that notification and
/// returns `true`. Returns `false` otherwise.
pub fn maybe_close (window: Window) -> bool {
  manager ().maybe_close (window)
}

/// Spawns a notification
pub fn notify (summary: &str, body: &str, timeout: i32) {
  let id = manager ().get_id (0);
  manager ().new_notification (id, summary, body);
  if timeout < 0 && unsafe {&*config}.default_notification_timeout != 0{
    manager ().close_after (id, unsafe {&*config}.default_notification_timeout);
  } else if timeout > 0 {
    manager ().close_after (id, timeout);
  }
}
