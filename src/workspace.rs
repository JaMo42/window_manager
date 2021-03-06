use std::ops::{Deref, DerefMut};
use x11::xlib::*;
use super::core::*;
use super::client::Client;
use super::property;

#[macro_export]
macro_rules! focused_client {
  () => {
    workspaces[active_workspace].clients.first_mut ()
  }
}

pub struct Workspace {
  pub clients: Vec<Client>
}

impl Workspace {
  pub fn new () -> Workspace {
    Workspace {
      clients: Vec::new ()
    }
  }

  pub unsafe fn push (&mut self, client: Client) {
    if let Some (prev) = self.clients.first () {
      XSetWindowBorder (
        display, prev.window, (*config).colors.normal.pixel
      );
    }
    XSetWindowBorder (display, client.window, (*config).colors.focused.pixel);
    self.clients.insert (0, client);
    self.clients[0].focus ();
  }

  pub fn remove (&mut self, client: &Client) {
    if let Some (idx) = self.clients.iter ().position (|c| c.window == client.window) {
      self.clients.remove (idx);
      // Update focused window
      unsafe {
        if let Some (first) = self.clients.first_mut () {
          first.focus ();
        }
        else {
          property::delete (root, property::Net::ActiveWindow);
          bar.draw ();
        }
      }
    }
  }

  pub unsafe fn focus (&mut self, window: Window) {
    if let Some (prev) = self.clients.first () {
      if window == prev.window {
        return;
      }
      XSetWindowBorder (
        display, prev.window, (*config).colors.normal.pixel
      );
    }
    if window == X_NONE {
      log::warn! ("Tried to focus None");
    }
    else if let Some (idx) = self.clients.iter ().position (|c| c.window == window) {
      if idx != 0 {
        let c = self.clients.remove (idx);
        self.clients.insert (0, c);
      }
      self.clients[0].focus ();
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
            XSetWindowBorder (
              display,
              self.clients[switch_idx].window,
              (*config).colors.normal.pixel
            );
            switch_idx = (switch_idx + 1) % self.clients.len ();
            XSetWindowBorder (
              display,
              self.clients[switch_idx].window,
              (*config).colors.selected.pixel
            );
            XRaiseWindow (display, self.clients[switch_idx].window);
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
    let focused_win = self.clients[switch_idx].window;
    self.focus (focused_win);
    // Re-grab main input
    super::grab_keys ();
  }

  pub fn has_urgent (&self) -> bool {
    self.clients.iter ().any (|&c| c.is_urgent)
  }
}

/*impl IntoIterator for Workspace {
  type Item = Client;
  type IntoIter = std::vec::IntoIter<Self::Item>;

  fn into_iter (self) -> Self::IntoIter {
    self.clients.into_iter ()
  }
}*/

impl Deref for Workspace {
  type Target = [Client];
  fn deref (&self) -> &Self::Target {
    &self.clients[..]
  }
}

impl DerefMut for Workspace {
  fn deref_mut (&mut self) -> &mut Self::Target {
    &mut self.clients[..]
  }
}
