use std::os::raw::*;
use std::collections::HashMap;
use std::ffi::CString;
use x11::xlib::*;
use x11::keysym::*;
use super::*;
use super::config_parser;
use super::color::Color_Scheme;
use super::paths;

#[macro_export]
macro_rules! clean_mods {
  ($mods:expr) => {
    $mods
      & unsafe { !(LockMask | numlock_mask) }
      & (MOD_WIN | MOD_ALT | MOD_SHIFT | MOD_CTRL)
  }
}

#[derive(Eq, Hash, Copy, Clone)]
pub struct Key {
  pub modifiers: c_uint,
  pub code: c_uint,
}

impl Key {
  pub fn from_str (key: *const c_char, modifiers: c_uint) -> Self {
    Key {
      modifiers: modifiers,
      code: unsafe {
        XKeysymToKeycode (display, XStringToKeysym (key)) as c_uint
      }
    }
  }

  pub fn from_sym (sym: c_uint, modifiers: c_uint) -> Self {
    Key {
      modifiers: modifiers,
      code: unsafe {
        XKeysymToKeycode (display, sym as c_ulong) as c_uint
      }
    }
  }
}

impl PartialEq for Key {
  fn eq (&self, other: &Self) -> bool {
    self.modifiers == other.modifiers && self.code == other.code
  }
}



pub enum Action {
  WM (unsafe fn (&mut Client)),
  WS (unsafe fn (usize, Option<&mut Client>), usize, bool),
  Launch (String),
  Generic (unsafe fn ())
}


pub struct Config {
  pub modifier: c_uint,
  pub key_binds: HashMap<Key, Action>,
  // padding: (top, bottom, left, right)
  // Spacing from the respective screen border for snapped windows
  pub padding: (c_int, c_int, c_int, c_int),
  // Internal border between the client and the actual window (only affects
  // snapped windows)
  pub gap: c_uint,
  // Window border width
  pub border_width: c_int,
  pub workspace_count: usize,
  pub meta_window_names: Vec<String>,
  pub colors: Color_Scheme,
  pub hibernate: bool
}

impl Config {
  pub fn new () -> Self {
    Config {
      modifier: MOD_WIN,
      key_binds: HashMap::new (),
      padding: (0, 0, 0, 0),
      gap: 0,
      border_width: 0,
      workspace_count: 1,
      meta_window_names: Vec::new (),
      colors: Color_Scheme::new (),
      hibernate: false
    }
  }

  pub fn add (&mut self, key: Key, action: Action) {
    self.key_binds.insert (key, action);
  }

  pub fn get (&self, key_code: c_uint, modifiers: c_uint) -> Option<&Action> {
    self.key_binds.get (&Key { modifiers: clean_mods! (modifiers), code: key_code })
  }

  pub fn load (&mut self) {
    unsafe {
      self.colors.load_defaults ()
    };
    // Parse file
    let source = std::fs::read_to_string (unsafe { &paths::config }).unwrap ();
    let parser = config_parser::Parser::new (source.chars ());
    for def in parser {
      use config_parser::Definition_Type::*;
      match def {
        Workspaces (count) => {
          log::info! ("config: workspace count: {}", count);
          self.workspace_count = count;
        }
        Gaps (size) => {
          log::info! ("config: gaps: {}", size);
          self.gap = size;
        }
        Padding (t, b, l, r) => {
          log::info! ("config: padding: {} {} {} {}", t, b, l ,r);
          self.padding = (t, b, l, r);
        }
        Border (width) => {
          log::info! ("config: border width: {}", width);
          // Needs to be i32 for `XWindowChanges.border_width` but we want to
          // parse it as unsigned.
          self.border_width = width as i32;
        }
        Meta (title) => {
          log::info! ("config: meta window: {}", title);
          self.meta_window_names.push (title);
        }
        Mod (modifier) => {
          log::info! ("config: user modifier: {}", modifier);
          self.modifier = modifier;
        }
        Bind_Key (modifier, key_str, action_str) => {
          log::info! ("config: bind: {}+{} -> {}", modifier, key_str, action_str);
          self.add (
            Key::from_str (c_str! (key_str.as_str ()), modifier),
            action::from_str (&action_str)
          );
        }
        Bind_Command (modifier, key_str, command) => {
          log::info! ("config: bind: {}+{} -> launch: '{}'", modifier, key_str, command);
          self.add (
            Key::from_str (c_str! (key_str.as_str ()), modifier),
            Action::Launch (command)
          );
        }
        Color (element, color_hex) => {
          log::info! ("config: color: {} {}", element, color_hex);
          self.colors.set (&element, &color_hex);
        }
        Hibernate => {
          log::info! ("config: enable hibernation");
          self.hibernate = true;
        }
      }
    }
    // Pre-defined key bindings
    for ws_idx in 0..self.workspace_count {
      let sym = XK_1 + ws_idx as u32;
      self.add (
        Key::from_sym (sym, self.modifier),
        Action::WS (action::select_workspace, ws_idx, false)
      );
      self.add (
        Key::from_sym (sym, self.modifier | MOD_SHIFT),
        Action::WS (action::move_to_workspace, ws_idx, true)
      );
    }
    self.add (
      Key::from_sym (XK_Tab, MOD_ALT),
      Action::Generic (action::switch_window)
    );
    // Set window area
    unsafe {
      window_area = Geometry::from_parts (
        screen_size.x + self.padding.2,
        screen_size.y + self.padding.0,
        screen_size.w - (self.padding.2 + self.padding.3) as u32,
        screen_size.h - (self.padding.0 + self.padding.1) as u32
      );
    }
  }
}

