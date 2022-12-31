use crate::platform;

/// Dbus server to interact with the window manager.
/// It uses the name `com.github.JaMo42.window_manager.SessionManager` and path
/// `/com/github/JaMo42/window_manager/SessionManager`.
///
/// Internally messages sent to the server are stored in a queue and an
/// empty X event with the non-standard type `SessionManagerEvent` is put in
/// the X event queue. Events are then processed from the main event loop.
///
/// The server implements the following methods:
///
///   Quit (s) -> ()
///     Valid values for the given string are:
///       "logout"    quits the window manager and logs out the user.
///       "sleep"     suspends the system, desipite being part of the Quit
///                   method, this does not quit the window manager.
///       "restart"   quits the window manager and reboots the system.
///       "shutdown"  quits the window manager and shuts down the system.
///     Passing an invalid argument will still quit the window manager.
use crate::action;
use crate::core::*;
use crate::error;
use futures::executor;
use x11::xlib::*;
use zbus::{dbus_interface, Connection, Result};

const NAME: &str = "com.github.JaMo42.window_manager.SessionManager";
const PATH: &str = "/com/github/JaMo42/window_manager/SessionManager";

static mut _dbus_connection: Option<Connection> = None;
static mut _manager: Manager = Manager::new();

fn dbus_connection() -> &'static mut Connection {
  unsafe { _dbus_connection.as_mut().unwrap_unchecked() }
}

pub fn manager() -> &'static mut Manager {
  unsafe { &mut _manager }
}

unsafe fn notify_mainloop() {
  let mut event: XEvent = zeroed!();
  event.type_ = SessionManagerEvent;
  display.push_event(&mut event);
}

enum Message {
  Quit(String),
}

pub struct Manager {
  messages: Vec<Message>,
}

impl Manager {
  const fn new() -> Self {
    Self {
      messages: Vec::new(),
    }
  }

  fn push_message(&mut self, message: Message) {
    self.messages.push(message);
    unsafe {
      notify_mainloop();
    }
  }

  pub fn process(&mut self) {
    match self.messages.remove(0) {
      Message::Quit(reason) => unsafe {
        if reason == "sleep" {
          platform::suspend()
            .map_err(|error| error::message_box("'systemctl suspend' failed", &error.to_string()))
            .ok();
        } else {
          quit_reason = reason;
          action::quit();
        }
      },
    }
  }
}

struct Server {
  manager: &'static mut Manager,
}

#[dbus_interface(name = "com.github.JaMo42.window_manager.SessionManager")]
impl Server {
  async fn quit(&mut self, reason: &str) {
    log::trace!("SessionManager: Quit called");
    log::trace!("              : reason = {}", reason);
    self.manager.push_message(Message::Quit(reason.to_owned()));
  }
}

async unsafe fn do_init() -> Result<()> {
  let connection = Connection::session().await?;
  let server = Server { manager: manager() };
  connection.object_server().at(PATH, server).await?;
  connection.request_name(NAME).await?;
  _dbus_connection = Some(connection);
  Ok(())
}

async fn do_quit() -> Result<()> {
  dbus_connection()
    .object_server()
    .remove::<Server, _>(PATH)
    .await?;
  dbus_connection().release_name(NAME).await?;
  Ok(())
}

pub unsafe fn init() {
  if let Err(error) = executor::block_on(do_init()) {
    log::error!("Failed to initialize session manager server: {}", error);
  } else {
    log::info!("Initialized session manager server");
  }
}

pub unsafe fn quit() {
  if let Err(error) = executor::block_on(do_quit()) {
    log::error!("Error during session manager server shutdown: {}", error);
  } else {
    log::info!("Terminated session manager server");
  }
}
