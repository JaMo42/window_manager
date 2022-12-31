use crate::action::snap_no_update;
use crate::as_static::AsStaticMut;
use crate::client::Client;
use crate::core::*;
use crate::dock;
use crate::monitors;
use crate::property;
use crate::split_handles::{self, SplitHandles};
use crate::x::{window::ToXWindow, Window, XNone, XWindow};
use std::ops::{Deref, DerefMut};
use x11::xlib::*;

#[macro_export]
macro_rules! focused_client {
  () => {
    workspaces[active_workspace]
      .clients
      .iter_mut()
      .find(|c| !c.is_minimized)
      .map(|c| &mut **c)
  };
}

pub struct Workspace {
  // Clients need to be boxed so they have the same address throughout their
  // lifetime since we store the address of the client as context in its
  // associated windows.
  #[allow(clippy::vec_box)]
  pub clients: Vec<Box<Client>>,
  // Each item corresponds the monitor at that index.
  #[allow(clippy::vec_box)] // Same as above
  pub splits: Vec<Box<SplitHandles>>,
  index: usize,
}

impl Workspace {
  pub fn new(index: usize) -> Workspace {
    let mut splits = Vec::with_capacity(monitors::count());
    for i in 0..monitors::count() {
      splits.push(SplitHandles::new(index, monitors::at_index(i)));
    }
    Workspace {
      clients: Vec::new(),
      splits,
      index,
    }
  }

  pub unsafe fn push(&mut self, client: Box<Client>) {
    if let Some(prev) = self.clients.first_mut() {
      prev.set_border(&(*config).colors.normal);
    }
    let is_snapped = client.is_snapped();
    self.clients.insert(0, client);
    self.clients[0].focus();
    dock::keep_open(false);
    if is_snapped {
      let c = self.clients[0].as_static_mut();
      // Re-snap it since it may come from a different workspace with different
      // split sizes.
      snap_no_update(c, c.snap_state);
      self.new_snapped_client(c);
    }
  }

  pub unsafe fn remove(&mut self, client: &Client) -> Box<Client> {
    if let Some(idx) = self.clients.iter().position(|c| c.window == client.window) {
      let c = self.clients.remove(idx);
      if let Some(first) = focused_client!() {
        first.focus();
      } else {
        property::delete(root, property::Net::ActiveWindow);
        display.set_input_focus(PointerRoot as XWindow);
        dock::keep_open(true);
      }
      if client.is_snapped() {
        self.remove_snapped_client(client);
      }
      return c;
    }
    my_panic!("tried to remove client not on workspace");
  }

  pub unsafe fn focus_client(&mut self, idx: usize) {
    let window = self.clients[idx].window;
    if let Some(prev) = self.clients.first_mut() {
      if prev.window == window {
        prev.focus();
        return;
      }
      prev.set_border(&(*config).colors.normal);
    }
    if idx != 0 {
      let c = self.clients.remove(idx);
      self.clients.insert(0, c);
    }
    self.clients[0].focus();
  }

  pub unsafe fn focus<W: ToXWindow>(&mut self, window: W) {
    let window = window.to_xwindow();
    if window == XNone || root == window {
      log::warn!(
        "Tried to focus {}",
        if window == XNone { "None" } else { "Root" }
      );
    } else if let Some(idx) = self
      .clients
      .iter()
      .position(|c| c.window == window || c.frame == window)
    {
      self.focus_client(idx);
    } else {
      my_panic!("Trying to focus window on a different workspace");
    }
  }

  pub unsafe fn switch_window(&mut self) {
    const RATE: u64 = 1000 / 10;
    if self.clients.len() <= 1 {
      if self.clients.len() == 1 && self.clients[0].is_minimized {
        self.clients[0].focus();
      }
      return;
    }
    // Create dummy window to handle window switch loop input
    let w = display.create_simple_window();
    w.map();
    XSelectInput(display.as_raw(), w.handle(), KeyPressMask | KeyReleaseMask);
    display.set_input_focus(w);
    display.grab_keyboard(w);
    display.sync(true);
    // Add the first Tab back to the event queue
    {
      let mut ev: XEvent = zeroed!();
      ev.type_ = KeyPress;
      ev.key.keycode = 0x17;
      ev.key.time = RATE + 1;
      display.push_event(&mut ev);
    }
    // Run window switcher loop
    let mut switch_idx = 0;
    let mut event: XEvent = zeroed!();
    let mut last_time = 0;
    loop {
      display.mask_event(KeyPressMask | KeyReleaseMask, &mut event);
      match event.type_ {
        KeyPress => {
          if event.key.time - last_time < RATE {
            continue;
          }
          last_time = event.key.time;
          if event.key.keycode == 0x17 {
            if self.clients[switch_idx].is_minimized {
              self.clients[switch_idx].unmap();
            } else {
              self.clients[switch_idx].set_border(&(*config).colors.normal);
            }
            if event.key.state & MOD_SHIFT != 0 {
              if switch_idx == 0 {
                switch_idx = self.clients.len() - 1;
              } else {
                switch_idx -= 1;
              }
            } else {
              switch_idx = (switch_idx + 1) % self.clients.len();
            }
            if self.clients[switch_idx].is_minimized {
              self.clients[switch_idx].map();
            }
            self.clients[switch_idx].set_border(&(*config).colors.selected);
            self.clients[switch_idx].raise();
          }
        }
        KeyRelease => {
          if event.key.keycode == 0x40 {
            break;
          }
        }
        _ => unreachable!(),
      }
    }
    // Clean up
    display.ungrab_keyboard();
    // Focus the resulting window
    self.focus_client(switch_idx);
    // Re-grab main input
    super::grab_keys();
    display.sync(false);
  }

  pub fn has_urgent(&self) -> bool {
    self.clients.iter().any(|c| c.is_urgent)
  }

  pub fn contains(&self, window: Window) -> bool {
    self.clients.iter().any(|c| c.window == window)
  }

  pub fn split_handles_visible(&self, yay_or_nay: bool) {
    for split_handle in self.splits.iter() {
      split_handle.visible(yay_or_nay);
    }
  }

  pub fn update_snapped_clients(&mut self) {
    for handles in self.splits.iter_mut() {
      handles.vertical_clients = 0;
      handles.left_clients = 0;
      handles.right_clients = 0;
    }
    for client in self.clients.iter() {
      if client.snap_state != SNAP_NONE {
        let (x, y) = client.saved_geometry().center_point();
        let mon_idx = monitors::at(x, y).index();
        if client.snap_state != SNAP_MAXIMIZED {
          self.splits[mon_idx].vertical_clients += 1;
        }
        if client.snap_state & (SNAP_TOP | SNAP_BOTTOM) != 0 {
          self.splits[mon_idx].left_clients += ((client.snap_state & SNAP_LEFT) != 0) as u32;
          self.splits[mon_idx].right_clients += ((client.snap_state & SNAP_RIGHT) != 0) as u32;
        }
      }
    }
    for handles in self.splits.iter_mut() {
      handles.update_activated();
      if self.index == unsafe { active_workspace } {
        handles.visible(true);
      }
    }
  }

  pub fn new_snapped_client(&mut self, client: &Client) {
    let mon_idx = monitors::containing(client).index();
    if client.snap_state != SNAP_MAXIMIZED {
      self.splits[mon_idx].vertical_clients += 1;
    }
    if client.snap_state & (SNAP_TOP | SNAP_BOTTOM) != 0 {
      self.splits[mon_idx].left_clients += ((client.snap_state & SNAP_LEFT) != 0) as u32;
      self.splits[mon_idx].right_clients += ((client.snap_state & SNAP_RIGHT) != 0) as u32;
    }
    if self.index == unsafe { active_workspace } {
      self.splits[mon_idx].visible(true);
    }
  }

  pub fn remove_snapped_client(&mut self, client: &Client) {
    let mon_idx = monitors::containing(client).index();
    if client.snap_state != SNAP_MAXIMIZED {
      self.splits[mon_idx].vertical_clients -= 1;
    }
    if client.snap_state & (SNAP_TOP | SNAP_BOTTOM) != 0 {
      self.splits[mon_idx].left_clients -= ((client.snap_state & SNAP_LEFT) != 0) as u32;
      self.splits[mon_idx].right_clients -= ((client.snap_state & SNAP_RIGHT) != 0) as u32;
    }
    self.splits[mon_idx].update_activated();
    if self.index == unsafe { active_workspace } {
      self.splits[mon_idx].visible(true);
    }
  }

  pub fn update_split_sizes(&mut self, monitor: usize, role: split_handles::Role, position: i32) {
    self.splits[monitor].update(role, position);
    for client in self.clients.iter_mut() {
      if client.snap_state != SNAP_NONE {
        unsafe {
          snap_no_update(client, client.snap_state);
        }
      }
    }
  }
}

impl Deref for Workspace {
  type Target = [Box<Client>];
  fn deref(&self) -> &Self::Target {
    &self.clients[..]
  }
}

impl DerefMut for Workspace {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.clients[..]
  }
}
