use std::ops::{Deref, DerefMut};
use x11::xlib::*;
use super::core::*;
use super::client::Client;
use super::property;

#[macro_export]
macro_rules! focused_client {
  () => {
    workspaces[active_workspace].clients.first_mut ().map (|box_| &mut **box_)
  }
}

pub struct Workspace {
  // Clients need to be boxed so they have the same address throughout their
  // lifetime since we store the address of the client as context in its
  // associated windows.
  #[allow(clippy::vec_box)]
  pub clients: Vec<Box<Client>>
}

impl Workspace {
  pub fn new () -> Workspace {
    Workspace {
      clients: Vec::new ()
    }
  }

  pub unsafe fn push (&mut self, client: Box<Client>) {
    if let Some (prev) = self.clients.first_mut () {
      prev.set_border ((*config).colors.normal);
    }
    self.clients.insert (0, client);
    self.clients[0].focus ();
  }

  pub fn remove (&mut self, client: &Client) -> Box<Client> {
    if let Some (idx) = self.clients.iter ().position (|c| c.window == client.window) {
      let c = self.clients.remove (idx);
      // Update focused window
      unsafe {
        if let Some (first) = self.clients.first_mut () {
          first.focus ();
        }
        else {
          property::delete (root, property::Net::ActiveWindow);
          XSetInputFocus (display, PointerRoot as u64, RevertToPointerRoot, CurrentTime);
          bar.draw ();
        }
      }
      return c;
    }
    panic! ("tried to remove client not on workspace");
  }

  unsafe fn focus_client (&mut self, idx: usize) {
    let window = self.clients[idx].window;
    if let Some (prev) = self.clients.first_mut () {
      if prev.window == window {
        prev.focus ();
        return;
      }
      prev.set_border ((*config).colors.normal);
    }
    if idx != 0 {
      let c = self.clients.remove (idx);
      self.clients.insert (0, c);
    }
    self.clients[0].focus ();
  }

  pub unsafe fn focus (&mut self, window: Window) {
    if window == X_NONE || window == root {
      log::warn! ("Tried to focus {}", if window == X_NONE { "None" } else { "Root" });
    }
    else if let Some (idx) = self.clients.iter ().position (
      |c| c.window == window || c.frame == window)
    {
      self.focus_client (idx);
    }
    else {
      panic! ("Trying to focus window on a different workspace");
    }
  }

  pub unsafe fn switch_window (&mut self) {
    if self.clients.len () <= 1 {
      return;
    }
    // Create dummy window to handle window switch loop input
    let s = XDefaultScreen (display);
    let w = XCreateSimpleWindow (
      display, root,
      0, 0,
      1, 1,
      0,
      XBlackPixel (display, s), XBlackPixel (display, s)
    );
    XMapWindow (display, w);
    XSelectInput (display, w, KeyPressMask | KeyReleaseMask);
    XSetInputFocus (display, w, RevertToParent, CurrentTime);
    XGrabKeyboard (display, w, X_FALSE, GrabModeAsync, GrabModeAsync, CurrentTime);
    XSync (display, X_TRUE);
    // Add the first Tab back to the event queue
    {
      let mut ev: XEvent = uninitialized! ();
      ev.type_ = KeyPress;
      ev.key.keycode = 0x17;
      XPutBackEvent (display, &mut ev);
    }
    // Run window switcher loop
    let mut switch_idx = 0;
    let mut event: XEvent = uninitialized! ();
    loop {
      XMaskEvent (display, KeyPressMask|KeyReleaseMask, &mut event);
      match event.type_ {
        KeyPress => {
          if event.key.keycode == 0x17 {
            // TODO: fix fullscreen windows
            self.clients[switch_idx].set_border ((*config).colors.normal);
            switch_idx = (switch_idx + 1) % self.clients.len ();
            self.clients[switch_idx].set_border ((*config).colors.selected);
            self.clients[switch_idx].raise ();
          }
        }
        KeyRelease => {
          if event.key.keycode == 0x40 {
            break;
          }
        }
        _ => unreachable! ()
      }
    }
    // Clean up
    XUngrabKeyboard (display, CurrentTime);
    XSetInputFocus (display, X_NONE, RevertToParent, CurrentTime);
    XDestroyWindow (display, w);
    // Focus the resulting window
    self.focus_client (switch_idx);
    // Re-grab main input
    super::grab_keys ();
    XSync (display, X_FALSE);
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
