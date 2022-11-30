use super::color::*;
use super::draw::Alignment;
use super::paths;
use super::*;
use crate::config_parser::*;
use crate::error::fatal_error;
use crate::monitors;
use crate::x::string_to_keysym;
use pango::FontDescription;
use std::collections::{BTreeMap, HashMap};
use x11::keysym::*;

#[macro_export]
macro_rules! clean_mods {
  ($mods:expr) => {
    $mods & !(LockMask | unsafe { numlock_mask }) & (MOD_WIN | MOD_ALT | MOD_SHIFT | MOD_CTRL)
  };
}

#[derive(Eq, PartialEq, Hash, Copy, Clone)]
pub struct Key {
  pub modifiers: c_uint,
  pub code: c_uint,
}

impl Key {
  pub fn from_str (key: &str, modifiers: c_uint) -> Self {
    Key {
      modifiers,
      code: unsafe { &display }.keysym_to_keycode (string_to_keysym (key)) as u32,
    }
  }

  pub fn from_sym (sym: c_uint, modifiers: c_uint) -> Self {
    Key {
      modifiers,
      code: unsafe { &display }.keysym_to_keycode (sym as KeySym) as u32,
    }
  }
}

impl ToString for Key {
  fn to_string (&self) -> String {
    unsafe {
      let sym = XKeycodeToKeysym (display.as_raw (), self.code as u8, 0);
      let c_str = XKeysymToString (sym);
      string_from_ptr! (c_str)
    }
  }
}

pub enum Action {
  WM (unsafe fn (&mut Client)),
  WS (unsafe fn (usize, Option<&mut Client>), usize, bool),
  Launch (Vec<String>),
  Generic (unsafe fn ()),
}

impl Action {
  pub fn from_str (s: &str) -> Self {
    use super::platform::actions::*;
    use Action::*;
    match s {
      "close_window" => WM (action::close_client),
      "quit" => Generic (action::quit),
      "quit_dialog" => Generic (action::quit_dialog),
      "snap_maximized" => WM (|c| unsafe { action::snap (c, SNAP_MAXIMIZED) }),
      "snap_left" => WM (action::snap_left),
      "snap_right" => WM (action::snap_right),
      "unsnap_or_center" => WM (|c| unsafe {
        if c.is_snapped () {
          c.unsnap ();
        } else {
          action::center (c);
        }
      }),
      "snap_up" => WM (action::snap_up),
      "snap_down" => WM (action::snap_down),
      "minimize" => WM (action::minimize),
      "unsnap_or_minimize" => WM (|c| unsafe {
        if c.is_snapped () {
          c.unsnap ();
        } else {
          action::minimize (c);
        }
      }),
      "raise_all" => Generic (action::raise_all),
      "mute_volume" => Generic (mute_volume),
      "increase_volume" => Generic (increase_volume),
      "decrease_volume" => Generic (decrease_volume),
      "move_to_next_monitor" => WM (action::move_to_next_monitor),
      "move_to_prev_monitor" => WM (action::move_to_prev_monitor),
      _ => my_panic! ("action::from_str: unknown action: {}", s),
    }
  }
}

#[derive(Debug)]
pub enum Height {
  FontPlus (u32),
  Absolute (u32),
}

impl Height {
  pub fn get (&self, font: Option<&FontDescription>) -> u32 {
    match *self {
      Height::FontPlus (n) => n + unsafe { &mut *draw }.font_height (font),
      Height::Absolute (n) => n,
    }
  }
}

impl std::str::FromStr for Height {
  type Err = std::num::ParseIntError;

  fn from_str (s: &str) -> Result<Self, Self::Err> {
    let has_plus = s.starts_with ('+');
    let num_s = if has_plus { &s[1..] } else { s };
    let n = num_s.parse ()?;
    if has_plus || n == 0 {
      Ok (Height::FontPlus (n))
    } else {
      Ok (Height::Absolute (n))
    }
  }
}

/// Find the absolute path for the icon theme.
/// Causes a fatal error if the theme is not found.
fn find_icon_theme (maybe_theme_name: Option<String>) -> String {
  let home = std::env::var ("HOME").unwrap ();
  let directories = [
    "/usr/share/icons".to_string (),
    format! ("{}/{}", home, ".local/share/icons"),
    format! ("{}/{}", home, ".icons"),
  ];
  let theme_given = maybe_theme_name.is_some ();
  let theme_name = maybe_theme_name.unwrap_or ("Papirus".to_string ());
  log::trace! ("Looking for icon theme");
  for d in directories {
    let path = format! ("{}/{}", d, theme_name);
    log::trace! (" - {}", path);
    if std::fs::metadata (&path).is_ok () {
      log::trace! (" -> found");
      return path;
    }
  }
  log::trace! (" -> none found");
  unsafe {
    let message = if theme_given {
      format! ("Icon theme not found: {}", theme_name)
    } else {
      format! ("No icon theme specified and the default 'Papirus' was not found")
    };
    fatal_error (&message);
  }
}

pub struct Config {
  pub modifier: c_uint,
  pub key_binds: HashMap<Key, Action>,
  // Internal border between the client and the actual window (only affects
  // snapped windows)
  pub gap: c_uint,
  // Window border width
  pub border_width: c_int,
  pub workspace_count: usize,
  pub meta_window_classes: Vec<String>,
  pub colors: Color_Scheme,
  pub bar_font: FontDescription,
  pub bar_opacity: u32,
  pub bar_time_format: String,
  pub bar_power_supply: String,
  pub bar_height: u32,
  pub bar_update_interval: u64,
  pub title_font: FontDescription,
  pub title_height: Height,
  pub title_alignment: Alignment,
  pub left_buttons: Vec<String>,
  pub right_buttons: Vec<String>,
  pub button_icon_size: u32,
  pub circle_buttons: bool,
  pub default_notification_timeout: i32,
  pub icon_theme: String,
  pub window_icon_size: u32,
  pub dock_pinned: Vec<String>,
  pub dock_focused_client_on_top: bool,
  pub dock_focus_urgent: bool,
  pub dock_item_size: u32,
  pub dock_icon_size: u32,
  pub dock_context_show_workspaces: bool,
  pub double_click_time: Time,
}

impl Config {
  fn maybe_load () -> Result<Self, String> {
    use std::str::FromStr;
    macro_rules! E {
      ($result:expr) => {
        $result.map_err (|e| e.to_string ())?
      };
    }
    let c = E! (parse (unsafe { &paths::config }));
    let general = c.general.unwrap_or_default ();
    let layout = c.layout.unwrap_or_default ();
    let window = c.window.unwrap_or_default ();
    let theme = c.theme.unwrap_or_default ();
    let keys = c.keys.unwrap_or_default ();
    let bar_ = c.bar.unwrap_or_default ();
    let dock = c.dock.unwrap_or_default ();

    let mut this = Config {
      modifier: keys
        .r#mod
        .map (|m| modifiers_from_string (&m))
        .unwrap_or (MOD_WIN),
      key_binds: HashMap::new (),
      gap: layout.gaps.unwrap_or (0),
      border_width: window.border.unwrap_or (0),
      workspace_count: layout.workspaces.unwrap_or (1),
      meta_window_classes: general.meta_window_classes.unwrap_or_default (),
      colors: if let Some (name) = theme.colors {
        E! (parse_color_scheme (name))
      } else {
        unsafe { Color_Scheme::new (&Color_Scheme_Config::new (), &BTreeMap::new ())? }
      },
      bar_font: FontDescription::from_string (
        &bar_.font.unwrap_or_else (|| "sans 14".to_string ()),
      ),
      bar_opacity: bar_.opacity.unwrap_or (100).clamp (0, 100),
      bar_time_format: bar_
        .time_format
        .unwrap_or_else (|| "%a %b %e %H:%M %Y".to_string ()),
      bar_power_supply: bar_.power_supply.unwrap_or_else (|| "BAT0".to_string ()),
      bar_height: 0,
      bar_update_interval: bar_.update_interval.unwrap_or (10000),
      title_font: FontDescription::from_string (
        &window.title_font.unwrap_or_else (|| "sans 14".to_string ()),
      ),
      title_height: E! (Height::from_str (
        &window
          .title_bar_height
          .unwrap_or_else (|| "+2".to_string ())
      )),
      title_alignment: E! (Alignment::from_str (
        &window
          .title_alignment
          .unwrap_or_else (|| "Left".to_string ())
      )),
      left_buttons: window.left_buttons.unwrap_or_default (),
      right_buttons: window.right_buttons.unwrap_or_default (),
      button_icon_size: window.button_icon_size.unwrap_or (75).clamp (0, 100),
      circle_buttons: window.circle_buttons.unwrap_or (false),
      default_notification_timeout: general.default_notification_timeout.unwrap_or (6000) as i32,
      icon_theme: find_icon_theme (theme.icons),
      window_icon_size: window.icon_size.unwrap_or (0).clamp (0, 100),
      dock_pinned: dock.pinned.unwrap_or_default (),
      dock_focused_client_on_top: dock.focused_client_on_top.unwrap_or (false),
      dock_focus_urgent: dock.focus_urgent.unwrap_or (false),
      dock_item_size: dock.item_size.unwrap_or (80),
      dock_icon_size: dock.icon_size.unwrap_or (85),
      dock_context_show_workspaces: dock.context_show_workspaces.unwrap_or (true),
      double_click_time: general.double_click_time.unwrap_or (500),
    };
    if cfg! (feature = "bar") {
      this.bar_height = E! (Height::from_str (
        &bar_.height.unwrap_or_else (|| "+5".to_string ())
      ))
      .get (Some (&this.bar_font));
    }
    if let Some (table) = keys.bindings {
      let m = this.modifier;
      parse_key_bindings (&table, &mut this, m);
    }
    // Pre-defined key bindings
    for ws_idx in 0..this.workspace_count {
      let sym = XK_1 + ws_idx as u32;
      this.add (
        Key::from_sym (sym, this.modifier),
        Action::WS (action::select_workspace, ws_idx, false),
      );
      this.add (
        Key::from_sym (sym, this.modifier | MOD_SHIFT),
        Action::WS (action::move_to_workspace, ws_idx, true),
      );
    }
    this.add (
      Key::from_sym (XK_Tab, MOD_ALT),
      Action::Generic (action::switch_window),
    );
    // Set window area
    monitors::set_window_areas (
      layout.pad.unwrap_or ((this.bar_height as i32, 0, 0, 0)),
      layout.secondary_pad.unwrap_or ((0, 0, 0, 0)),
    );
    Ok (this)
  }

  pub fn load () -> Self {
    match Self::maybe_load () {
      Ok (this) => this,
      Err (error) => unsafe {
        fatal_error (&format! ("Could not load configuration:\n\t{}", error));
      },
    }
  }

  pub fn add (&mut self, key: Key, action: Action) {
    self.key_binds.insert (key, action);
  }

  pub fn get (&self, key_code: c_uint, modifiers: c_uint) -> Option<&Action> {
    self.key_binds.get (&Key {
      modifiers: clean_mods! (modifiers),
      code: key_code,
    })
  }
}
