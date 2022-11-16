use super::client::Client;
use super::core::*;
use super::property;
use crate::x::{window::Into_Window, Window, XNone, XWindow};
use std::ops::{Deref, DerefMut};
use x11::xlib::*;

#[macro_export]
macro_rules! focused_client {
  () => {
    workspaces[active_workspace]
      .clients
      .iter_mut ()
      .find (|c| !c.is_minimized)
      .map (|c| &mut **c)
  };
}

pub struct Workspace {
  // Clients need to be boxed so they have the same address throughout their
  // lifetime since we store the address of the client as context in its
  // associated windows.
  #[allow(clippy::vec_box)]
  pub clients: Vec<Box<Client>>,
}

impl Workspace {
  pub fn new () -> Workspace {
    Workspace {
      clients: Vec::new (),
    }
  }

  pub unsafe fn push (&mut self, client: Box<Client>) {
    if let Some (prev) = self.clients.first_mut () {
      prev.set_border (&(*config).colors.normal);
    }
    self.clients.insert (0, client);
    self.clients[0].focus ();
  }

  pub unsafe fn remove (&mut self, client: &Client) -> Box<Client> {
    if let Some (idx) = self
      .clients
      .iter ()
      .position (|c| c.window == client.window)
    {
      let c = self.clients.remove (idx);
      if let Some (first) = focused_client! () {
        first.focus ();
      } else {
        property::delete (root, property::Net::ActiveWindow);
        display.set_input_focus (PointerRoot as XWindow);
      }
      return c;
    }
    my_panic! ("tried to remove client not on workspace");
  }

  pub unsafe fn focus_client (&mut self, idx: usize) {
    let window = self.clients[idx].window;
    if let Some (prev) = self.clients.first_mut () {
      if prev.window == window {
        prev.focus ();
        return;
      }
      prev.set_border (&(*config).colors.normal);
    }
    if idx != 0 {
      let c = self.clients.remove (idx);
      self.clients.insert (0, c);
    }
    self.clients[0].focus ();
  }

  pub unsafe fn focus<W: Into_Window> (&mut self, window: W) {
    let window = window.into_window ();
    if window == XNone || root == window {
      log::warn! (
        "Tried to focus {}",
        if window == XNone { "None" } else { "Root" }
      );
    } else if let Some (idx) = self
      .clients
      .iter ()
      .position (|c| c.window == window || c.frame == window)
    {
      self.focus_client (idx);
    } else {
      my_panic! ("Trying to focus window on a different workspace");
    }
  }

  pub unsafe fn switch_window (&mut self) {
    if self.clients.len () <= 1 {
      if self.clients.len () == 1 && self.clients[0].is_minimized {
        self.clients[0].focus ();
      }
      return;
    }
    // Create dummy window to handle window switch loop input
    let w = display.create_simple_window ();
    w.map ();
    XSelectInput (
      display.as_raw (),
      w.handle (),
      KeyPressMask | KeyReleaseMask,
    );
    display.set_input_focus (w);
    display.grab_keyboard (w);
    display.sync (true);
    // Add the first Tab back to the event queue
    {
      let mut ev: XEvent = uninitialized! ();
      ev.type_ = KeyPress;
      ev.key.keycode = 0x17;
      display.push_event (&mut ev);
    }
    // Run window switcher loop
    let mut switch_idx = 0;
    let mut event: XEvent = uninitialized! ();
    loop {
      display.mask_event (KeyPressMask | KeyReleaseMask, &mut event);
      match event.type_ {
        KeyPress => {
          if event.key.keycode == 0x17 {
            if self.clients[switch_idx].is_minimized {
              self.clients[switch_idx].unmap ();
            } else {
              self.clients[switch_idx].set_border (&(*config).colors.normal);
            }
            switch_idx = (switch_idx + 1) % self.clients.len ();
            if self.clients[switch_idx].is_minimized {
              self.clients[switch_idx].map ();
            }
            self.clients[switch_idx].set_border (&(*config).colors.selected);
            self.clients[switch_idx].raise ();
          }
        }
        KeyRelease => {
          if event.key.keycode == 0x40 {
            break;
          }
        }
        _ => unreachable! (),
      }
    }
    // Clean up
    display.ungrab_keyboard ();
    // Focus the resulting window
    self.focus_client (switch_idx);
    // Re-grab main input
    super::grab_keys ();
    display.sync (false);
  }

  pub fn has_urgent (&self) -> bool {
    self.clients.iter ().any (|c| c.is_urgent)
  }

  pub fn contains (&self, window: Window) -> bool {
    self.clients.iter ().any (|c| c.window == window)
  }
}

impl Deref for Workspace {
  type Target = [Box<Client>];
  fn deref (&self) -> &Self::Target {
    &self.clients[..]
  }
}

impl DerefMut for Workspace {
  fn deref_mut (&mut self) -> &mut Self::Target {
    &mut self.clients[..]
  }
}
