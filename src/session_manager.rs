use crate::{log_error, platform, window_manager::WindowManager, x::Display};
use parking_lot::Mutex;
use std::sync::{Arc, Weak};
use zbus::{dbus_interface, Result};

pub const SESSION_MANAGER_EVENT: u8 = 254;
const NAME: &str = "com.github.JaMo42.window_manager.SessionManager";
const PATH: &str = "/com/github/JaMo42/window_manager/SessionManager";

enum Message {
    Quit(String),
}

#[derive(Default)]
pub struct SessionManager {
    wm: Weak<WindowManager>,
    messages: Vec<Message>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            wm: Weak::new(),
            messages: Vec::new(),
        }
    }

    pub fn set_window_manager(&mut self, wm: &Arc<WindowManager>) {
        self.wm = Arc::downgrade(wm);
    }

    pub fn register(manager: Arc<Mutex<Self>>) -> Result<()> {
        let dbus = manager.lock().wm.upgrade().unwrap().dbus.clone();
        let server = Server { manager };
        dbus.register_server(NAME, PATH, server)?;
        Ok(())
    }

    pub fn unregister(&self) {
        let wm = self.wm.upgrade().unwrap();
        log_error!(wm.dbus.remove_server::<Server>(NAME, PATH));
    }

    fn notify_mainloop(&self) {
        let event = Display::create_unknown_event(SESSION_MANAGER_EVENT);
        if let Some(wm) = self.wm.upgrade() {
            wm.display.put_back_event(Ok(xcb::Event::Unknown(event)));
        }
    }

    /// The d-bus server calls this when a message is received and it's then
    /// processed through the main loop.
    fn push_message(&mut self, message: Message) {
        self.messages.push(message);
        self.notify_mainloop();
    }

    pub fn process(&mut self) {
        match self.messages.remove(0) {
            Message::Quit(reason) => {
                if reason == "sleep" {
                    log_error!(platform::suspend());
                } else if let Some(wm) = self.wm.upgrade() {
                    wm.quit(Some(reason));
                }
            }
        }
    }
}

struct Server {
    manager: Arc<Mutex<SessionManager>>,
}

#[dbus_interface(name = "com.github.JaMo42.window_manager.SessionManager")]
impl Server {
    async fn quit(&mut self, reason: &str) {
        log::trace!("SessionManager: quit called: {reason}");
        self.manager
            .lock()
            .push_message(Message::Quit(reason.to_owned()));
    }
}
