use crate::action;
use crate::client::Client;
use crate::context_menu::{Context_Menu, Indicator};
use crate::desktop_entry::Desktop_Entry;
use crate::draw::{self, Drawing_Context, Svg_Resource};
use crate::error::message_box;
use crate::geometry::Geometry;
use crate::process::{run_or_message_box, split_commandline};
use crate::set_window_kind;
use crate::x::{Window, XWindow};
use crate::{core::*, window_title};
use std::ptr::NonNull;
use x11::xlib::*;

unsafe fn get_icon (maybe_name_or_path: Option<String>) -> Option<Box<Svg_Resource>> {
  if let Some (app_icon) = maybe_name_or_path.and_then (|name| {
    // Same as `draw::get_app_icon` but we already have the desktop entry so
    // we don't want to use that
    let icon_path = if name.starts_with ('/') {
      name
    } else {
      format! (
        "/usr/share/icons/{}/48x48/apps/{}.svg",
        (*config).icon_theme,
        name
      )
    };
    Svg_Resource::open (&icon_path)
  }) {
    Some (app_icon)
  } else {
    draw::get_icon ("applications-system")
  }
}

fn get_title_and_unsaved_changes (client: &Client) -> (String, bool) {
  // As far as I can tell there is no property or other way for windows to
  // signal that they have unsaved changes so we look for common indicators
  // in the window title.
  // If we find such an indicator it is removed from the returned title.
  let unsaved_indicators = &["*", "●", "+"];
  let mut title = unsafe { window_title (client.window) };
  let mut has_unsaved = false;
  for indicator in unsaved_indicators {
    if title.starts_with (indicator) {
      title.remove (0);
      title = title.trim_start ().to_string ();
      has_unsaved = true;
      break;
    } else if title.ends_with (indicator) {
      title.pop ();
      title = title.trim_end ().to_string ();
      has_unsaved = true;
      break;
    }
  }
  (title, has_unsaved)
}

pub struct Item {
  // Name of the .desktop file, used of the entry does not specify a name
  app_name: String,
  desktop_entry: Desktop_Entry,
  action_icons: Vec<Option<Box<Svg_Resource>>>,
  instances: Vec<NonNull<Client>>,
  window: Window,
  icon: Box<Svg_Resource>,
  size: u32,
  command: Vec<String>,
  geometry: Geometry,
  is_pinned: bool,
  hovered: bool,
  focused_instance: usize,
  has_urgent: bool,
}

impl Item {
  pub unsafe fn create (
    dock_window: XWindow,
    app_name: &str,
    is_pinned: bool,
    size: u32,
    x: i32,
    y: i32,
    dc: &mut Drawing_Context,
  ) -> Option<Box<Self>> {
    let de = if let Some (entry) = Desktop_Entry::new (app_name) {
      entry
    } else {
      if is_pinned {
        message_box (
          "Application not found",
          &format! ("'{}' was not found and got removed from the dock", app_name),
        );
      }
      return None;
    };
    let window = Window::builder (&display)
      .size (size, size)
      .position (x, y)
      .parent (dock_window)
      .attributes (|attributes| {
        attributes.event_mask (EnterWindowMask | LeaveWindowMask | ButtonPressMask);
      })
      .build ();
    let icon = if let Some (icon) = get_icon (de.icon.clone ()) {
      icon
    } else {
      message_box (
        &format! ("No suitable icon found for '{}'", app_name),
        "It got removed from the dock",
      );
      return None;
    };
    let mut action_icons = Vec::new ();
    for action in de.actions.iter () {
      if let Some (icon_name_or_path) = action.icon.as_ref () {
        action_icons.push (draw::get_app_icon (icon_name_or_path));
      } else {
        action_icons.push (None);
      }
    }
    set_window_kind (window, Window_Kind::Dock_Item);
    window.map ();
    window.clear ();
    let command = split_commandline (de.exec.as_ref ().unwrap ());
    let mut this = Box::new (Self {
      app_name: app_name.to_owned (),
      desktop_entry: de,
      action_icons,
      instances: Vec::new (),
      window,
      icon,
      size,
      command,
      geometry: Geometry::from_parts (x, y, size, size),
      is_pinned,
      hovered: false,
      focused_instance: 0,
      has_urgent: false,
    });
    window.save_context (super::item_context, this.as_mut () as *mut Item as XPointer);
    this.redraw (dc, false);
    Some (this)
  }

  pub fn destroy (&self) {
    self.window.destroy ();
  }

  pub fn geometry (&self) -> &Geometry {
    &self.geometry
  }

  unsafe fn draw_indicator (&self, dc: &mut Drawing_Context) {
    if !self.instances.is_empty () {
      let h = self.geometry.h / 16;
      let w = self.geometry.w / 4;
      let x = (self.geometry.w - w) as i32 / 2;
      let y = (self.geometry.h - h) as i32;
      dc.rect (x, y, w, h)
        .color ((*config).colors.dock_indicator)
        .corner_radius (0.5)
        .draw ();
      dc.render (self.window, x, y, w, h);
    }
  }

  pub unsafe fn redraw (&mut self, dc: &mut Drawing_Context, hovered: bool) {
    self.hovered = true;
    let icon_size = self.size * (*config).dock_icon_size / 100;
    let icon_position = (self.size - icon_size) as i32 / 2;
    dc.square (0, 0, self.size)
      .color ((*config).colors.bar_background)
      .draw ();
    if hovered || self.has_urgent {
      let color = if self.has_urgent {
        (*config).colors.dock_urgent
      } else {
        (*config).colors.dock_hovered
      };
      dc.square (0, 0, self.size)
        .corner_radius (0.1)
        .color (color)
        .stroke (1, color.scale (4.0 / 3.0))
        .draw ();
    }
    dc.draw_svg (
      self.icon.as_mut (),
      icon_position,
      icon_position,
      icon_size,
      icon_size,
    );
    self.draw_indicator (dc);
    dc.render (self.window, 0, 0, self.size, self.size);
  }

  pub unsafe fn focus_instance_client (&self, index: usize) {
    let client = &mut *self.instances[index].as_ptr ();
    if client.workspace != active_workspace {
      action::select_workspace (client.workspace, None);
    }
    workspaces[active_workspace].focus (client.window);
  }

  pub unsafe fn click (&self) {
    if !self.instances.is_empty () {
      if self.has_urgent && (*config).dock_focus_urgent {
        for (index, instance) in self.instances.iter ().enumerate () {
          if instance.as_ref ().is_urgent {
            self.focus_instance_client (index);
            return;
          }
        }
      } else {
        self.focus_instance_client (self.focused_instance);
      }
    } else {
      self.new_instance ();
    }
  }

  pub fn name (&self) -> &str {
    &self.app_name
  }

  pub fn display_name (&self) -> &str {
    &self.desktop_entry.name
  }

  pub fn new_instance (&self) {
    run_or_message_box (&self.command);
  }

  unsafe fn context_action (this: &mut Self, mut choice: usize) {
    // Instances
    if choice < this.instances.len () {
      this.focus_instance_client (choice);
      return;
    }
    choice -= this.instances.len ();
    // Actions
    if choice < this.desktop_entry.actions.len () {
      let action = &this.desktop_entry.actions[choice];
      if let Some (command) = action.exec.clone () {
        let command: Vec<String> = split_commandline (&command);
        run_or_message_box (&command);
      }
      return;
    }
    choice -= this.desktop_entry.actions.len ();
    // Default operations
    match choice {
      0 => {
        this.new_instance ();
      }
      1 => {
        let mut client = this.instances[this.focused_instance];
        if client.as_ref ().is_minimized {
          client.as_mut ().unminimize (true);
        } else {
          action::minimize (client.as_mut ());
        }
      }
      2 => {
        action::close_client (this.instances[this.focused_instance].as_mut ());
      }
      _ => unreachable! (),
    }
  }

  pub unsafe fn context_menu (&mut self) {
    let this = self as *mut Self;
    let mut menu = Context_Menu::new (Box::new (move |selection| {
      if let Some (choice) = selection {
        Self::context_action (&mut *this, choice);
      }
      if !workspaces[active_workspace].clients.is_empty () {
        super::the ().keep_open (false);
      }
    }));
    if !self.instances.is_empty () {
      let mut all_on_current_workspace = true;
      for i in self.instances.iter () {
        if i.as_ref ().workspace != active_workspace {
          all_on_current_workspace = false;
          break;
        }
      }
      self
        .instances
        .iter_mut ()
        .enumerate ()
        .map (|(index, client)| {
          let (title, unsaved) = get_title_and_unsaved_changes (client.as_mut ());
          menu
            .action (title)
            .icon (client.as_mut ().icon ())
            .indicator (if index == self.focused_instance {
              Some (Indicator::Check)
            } else if client.as_ref ().is_urgent {
              Some (Indicator::Exclamation)
            } else if unsaved {
              Some (Indicator::Circle)
            } else if client.as_ref ().is_minimized {
              Some (Indicator::Diamond)
            } else {
              None
            })
            .info (
              if (*config).dock_context_show_workspaces && !all_on_current_workspace {
                format! (" ({})", client.as_ref ().workspace + 1)
              } else {
                String::new ()
              },
            );
        })
        .for_each (drop);
      menu.divider ();
    }
    if !self.desktop_entry.actions.is_empty () {
      self
        .desktop_entry
        .actions
        .iter ()
        .enumerate ()
        .map (|(index, action)| {
          menu.action (action.name.clone ()).icon (
            self.action_icons[index]
              .as_mut ()
              .map (|icon| &mut *(icon.as_mut () as *mut Svg_Resource)),
          );
        })
        .for_each (drop);
      menu.divider ();
    }

    menu.action ("Launch".to_string ());

    if let Some (active) = self.instances.first () {
      menu.action (
        if active.as_ref ().is_minimized {
          "Show"
        } else {
          "Hide"
        }
        .to_string (),
      );
      menu.action ("Quit".to_string ());
    }

    menu
      .min_width (240)
      .always_show_indicator_column ()
      .build ();

    let dock = super::the ();
    let x = dock.geometry ().x + self.geometry.x + self.geometry.w as i32 / 2;
    let y = dock.geometry ().y + self.geometry.y - 5;
    menu.show_at (x, y);
  }

  pub fn add_instance (&mut self, client: NonNull<Client>) {
    self.instances.push (client);
    unsafe {
      self.redraw (super::the ().drawing_context (), self.hovered);
    }
  }

  pub fn remove_instance (&mut self, client: &Client) -> bool {
    if let Some (index) = self
      .instances
      .iter ()
      .position (|c| unsafe { c.as_ref () } == client)
    {
      self.instances.remove (index);
    }
    if self.instances.is_empty () {
      unsafe {
        self.redraw (super::the ().drawing_context (), self.hovered);
      }
    }
    self.instances.is_empty () && !self.is_pinned
  }

  pub fn focus (&mut self, client: &Client) {
    if let Some (index) = self
      .instances
      .iter ()
      .position (|c| unsafe { c.as_ref () } == client)
    {
      if unsafe { &*config }.dock_focused_client_on_top {
        let instance = self.instances.remove (index);
        self.instances.insert (0, instance);
        // Active instance is always 0
      } else {
        self.focused_instance = index;
      }
    }
  }

  pub fn urgent (&mut self, is_urgent: bool) {
    self.has_urgent = is_urgent;
  }
}
