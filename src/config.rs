use std::os::raw::*;
use std::collections::{HashMap, BTreeMap};
use std::ffi::CString;
use x11::xlib::*;
use x11::keysym::*;
use super::*;
use super::config_parser;
use super::color::*;
use super::paths;
use super::color;

#[macro_export]
macro_rules! clean_mods {
  ($mods:expr) => {
    $mods
      & !(LockMask | unsafe { numlock_mask })
      & (MOD_WIN | MOD_ALT | MOD_SHIFT | MOD_CTRL)
  }
}

#[derive(Eq, PartialEq, Hash, Copy, Clone)]
pub struct Key {
  pub modifiers: c_uint,
  pub code: c_uint,
}

impl Key {
  pub fn from_str (key: *const c_char, modifiers: c_uint) -> Self {
    Key {
      modifiers,
      code: unsafe {
        XKeysymToKeycode (display, XStringToKeysym (key)) as c_uint
      }
    }
  }

  pub fn from_sym (sym: c_uint, modifiers: c_uint) -> Self {
    Key {
      modifiers,
      code: unsafe {
        XKeysymToKeycode (display, sym as c_ulong) as c_uint
      }
    }
  }
}


pub enum Action {
  WM (unsafe fn (&mut Client)),
  WS (unsafe fn (usize, Option<&mut Client>), usize, bool),
  Launch (String),
  Generic (unsafe fn ())
}

impl Action {
  pub fn from_str (s: &str) -> Self {
    use Action::*;
    match s {
      "close_window" => WM (action::close_client),
      "quit" => Generic (action::quit),
      "snap_maximized" => WM (
        |c| unsafe {
          action::snap (c, SNAP_MAXIMIZED)
        }
      ),
      "snap_left" => WM (action::snap_left),
      "snap_right" => WM (action::snap_right),
      "unsnap_or_center" => WM (
        |c| unsafe {
          if c.is_snapped () {
            c.unsnap ();
          }
          else {
            action::center (c);
          }
        }
      ),
      "snap_up" => WM (action::snap_up),
      "snap_down" => WM (action::snap_down),
      "minimize" => WM (action::minimize),
      _ => panic! ("action::from_str: unknown action: {}", s)
    }
  }
}


pub enum Height {
  FontPlus (u32),
  Absolute (u32)
}

impl Height {
  pub unsafe fn get (&self, font: Option<&str>) -> u32 {
    match *self {
      Height::FontPlus (n) => n + (*draw).font_height (font),
      Height::Absolute (n) => n
    }
  }
}

impl std::fmt::Display for Height {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
    match *self {
      Height::FontPlus(n) => if n == 0 {
        write! (f, "same as font height")
      }
      else {
        write! (f, "font height + {}", n)
      },
      Height::Absolute(n) => write! (f, "{}", n)
    }
  }
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
  pub meta_window_classes: Vec<String>,
  pub colors: Color_Scheme,
  pub bar_font: String,
  pub bar_opacity: u8,
  pub bar_time_format: String,
  pub bar_height: Height,
  pub title_font: String,
  pub title_height: Height
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
      meta_window_classes: Vec::new (),
      colors: Color_Scheme::new_uninit (),
      bar_font: "sans".to_string (),
      bar_opacity: 100,
      bar_time_format: "%a %b %e %T %Y".to_string (),
      bar_height: Height::FontPlus (5),
      title_font: "sans".to_string (),
      title_height: Height::FontPlus (1)
    }
  }

  pub fn add (&mut self, key: Key, action: Action) {
    self.key_binds.insert (key, action);
  }

  pub fn get (&self, key_code: c_uint, modifiers: c_uint) -> Option<&Action> {
    self.key_binds.get (&Key { modifiers: clean_mods! (modifiers), code: key_code })
  }

  pub fn load (&mut self) {
    // Parse file
    let source = std::fs::read_to_string (unsafe { &paths::config }).unwrap ();
    let parser = config_parser::Parser::new (source.chars ());
    let mut color_scheme_config = Color_Scheme_Config::new ();
    let mut color_defs: BTreeMap<String, Color> = BTreeMap::new ();
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
          self.meta_window_classes.push (title);
        }
        Mod (modifier) => {
          log::info! ("config: user modifier: {}", modifier);
          self.modifier = modifier;
        }
        Bind_Key (modifier, key_str, action_str) => {
          log::info! ("config: bind: {}+{} -> {}", modifier, key_str, action_str);
          self.add (
            Key::from_str (c_str! (key_str.as_str ()), modifier),
            Action::from_str (&action_str)
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
          log::info! ("config: color: {} -> {}", element, color_hex);
          color_scheme_config.set (
            &element,
            if color_hex.starts_with ('#') {
              Color_Config::Hex (color_hex)
            } else {
              Color_Config::Link (color_hex)
            }
          );
        }
        Def_Color (name, color_hex) => {
          log::info! ("config: color definition: {} -> {}", name, color_hex);
          color_defs.insert (
            name,
            unsafe { color::Color::alloc_from_hex (&color_hex) }
          );
        }
        Bar_Font (description) => {
          log::info! ("config: bar font: {}", description);
          self.bar_font = description
        }
        Bar_Opacity (percent) => {
          assert! (percent <= 100);
          log::info! ("config: bar opacity: {}%", percent);
          self.bar_opacity = percent;
        }
        Bar_Time_Format (format) => {
          log::info! ("config: bar time format: '{}'", format);
          self.bar_time_format = format;
        }
        Bar_Height (height) => {
          log::info! ("config: bar height: {}", height);
          self.bar_height = height;
        }
        Title_Font (description) => {
          log::info! ("config: title bar font: {}", description);
          self.title_font = description;
        }
        Title_Height (height) => {
          log::info! ("config: title bar height: {}", height);
          self.title_height = height;
        }
      }
    }
    // Set color scheme
    self.colors = unsafe { Color_Scheme::new (&color_scheme_config, &color_defs) };
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
