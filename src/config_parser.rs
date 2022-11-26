use super::color::{Color, Color_Config, Color_Scheme, Color_Scheme_Config};
use super::config::{Action, Config, Key};
use super::core::*;
use super::error::message_box;
use super::paths;
use super::process::split_commandline;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs::read_to_string;
use std::os::raw::c_uint;
use toml::value::Table;

#[derive(Deserialize, Debug)]
pub struct Parsed_Config {
  pub general: Option<General>,
  pub layout: Option<Layout>,
  pub window: Option<Window>,
  pub theme: Option<Theme>,
  pub keys: Option<Keys>,
  pub bar: Option<Bar>,
  pub dock: Option<Dock>,
}

#[derive(Deserialize, Debug, Default)]
pub struct General {
  pub meta_window_classes: Option<Vec<String>>,
  pub default_notification_timeout: Option<u64>,
}

#[derive(Deserialize, Debug, Default)]
pub struct Layout {
  pub workspaces: Option<usize>,
  pub gaps: Option<u32>,
  pub pad: Option<(i32, i32, i32, i32)>,
  pub secondary_pad: Option<(i32, i32, i32, i32)>,
}

#[derive(Deserialize, Debug, Default)]
pub struct Window {
  pub border: Option<i32>,
  pub title_font: Option<String>,
  pub title_bar_height: Option<String>,
  pub title_alignment: Option<String>,
  pub right_buttons: Option<Vec<String>>,
  pub left_buttons: Option<Vec<String>>,
  pub icon_size: Option<u32>,
  pub circle_buttons: Option<bool>,
  pub button_icon_size: Option<u32>,
}

#[derive(Deserialize, Debug, Default)]
pub struct Theme {
  pub colors: Option<String>,
  pub icons: Option<String>,
}

#[derive(Deserialize, Debug, Default)]
pub struct Keys {
  pub r#mod: Option<String>,
  pub bindings: Option<Table>,
}

#[derive(Deserialize, Debug, Default)]
pub struct Bar {
  pub font: Option<String>,
  pub opacity: Option<u32>,
  pub height: Option<String>,
  pub time_format: Option<String>,
  pub power_supply: Option<String>,
  pub update_interval: Option<u64>,
}

#[derive(Deserialize, Debug, Default)]
pub struct Dock {
  pub pinned: Option<Vec<String>>,
  pub focused_client_on_top: Option<bool>,
  pub focus_urgent: Option<bool>,
  pub item_size: Option<u32>,
  pub icon_size: Option<u32>,
  pub context_show_workspaces: Option<bool>,
}

pub fn parse (pathname: &str) -> Result<Parsed_Config, toml::de::Error> {
  if let Ok (content) = read_to_string (pathname) {
    toml::from_str (&content)
  } else {
    let default_config = "[keys.bindings]\n'Mod+Shift+Q' = \"quit\"\n'Mod+Return' = \"$ xterm\"";
    message_box (
      "Config file not found",
      "Win+Shift+Q has been bound to quit.\nWin+Return launches xterm.",
    );
    toml::from_str (default_config)
  }
}

pub fn parse_key_bindings (table: &Table, cfg: &mut Config, m: u32) {
  for (key, action) in table.iter () {
    let (mods, key) = mods_and_key_from_string (key, m);
    let action = action.as_str ().unwrap ();
    if let Some (commandline) = action.strip_prefix ('$') {
      cfg.add (
        Key::from_str (&key, mods),
        Action::Launch (split_commandline (commandline)),
      );
    } else {
      cfg.add (Key::from_str (&key, mods), Action::from_str (action));
    }
  }
}

fn parse_color_scheme_defs (
  palette: &Table,
  defs: &mut BTreeMap<String, Color>,
) -> Result<(), String> {
  for (name, color) in palette.iter () {
    defs.insert (name.to_owned (), unsafe {
      Color::alloc_from_hex (
        color
          .as_str ()
          .ok_or_else (|| "Color values must be strings".to_string ())?,
      )
    });
  }
  Ok (())
}

fn parse_color_scheme_walk (
  table: &Table,
  path: &mut Vec<String>,
  cfg: &mut Color_Scheme_Config,
) -> Result<(), String> {
  for (key, value) in table.iter () {
    if key == "palette" {
      continue;
    } else if value.is_table () {
      path.push (key.clone ());
      parse_color_scheme_walk (value.as_table ().unwrap (), path, cfg)?;
    } else {
      let elem = format! ("{}.{}", path.join ("."), key);
      let color_or_link = value
        .as_str ()
        .ok_or_else (|| "Color values must be strings".to_string ())?
        .to_owned ();
      cfg.set (
        &elem,
        if color_or_link.starts_with ('#') {
          Color_Config::Hex (color_or_link)
        } else {
          Color_Config::Link (color_or_link)
        },
      )?;
    }
  }
  path.pop ();
  Ok (())
}

pub fn parse_color_scheme (name: String) -> Result<Color_Scheme, String> {
  macro_rules! E {
    ($result:expr) => {
      $result.map_err (|e| e.to_string ())?
    };
  }
  let mut color_scheme_config = Color_Scheme_Config::new ();
  let mut color_defs: BTreeMap<String, Color> = BTreeMap::new ();
  let scheme = {
    let pathname = format! ("{}/{}.toml", unsafe { &paths::colors_dir }, name);
    let content = E! (read_to_string (&pathname));
    E! (toml::from_str::<Table> (&content))
  };
  if let Some (palette) = scheme.get ("palette") {
    parse_color_scheme_defs (
      palette
        .as_table ()
        .ok_or_else (|| "Palette must be a table".to_string ())?,
      &mut color_defs,
    )?;
  }
  let mut path = Vec::new ();
  parse_color_scheme_walk (&scheme, &mut path, &mut color_scheme_config)?;
  unsafe { Color_Scheme::new (&color_scheme_config, &color_defs) }
}

fn str2mod (s: &str, m: c_uint) -> c_uint {
  match s.trim () {
    "Win" => MOD_WIN,
    "Shift" => MOD_SHIFT,
    "Alt" => MOD_ALT,
    "Ctrl" => MOD_CTRL,
    "Mod" => m,
    _ => 0,
  }
}

pub fn modifiers_from_string (s: &str) -> c_uint {
  let mut mods = 0;
  for mod_str in s.split ('+') {
    mods |= str2mod (mod_str, 0);
  }
  mods
}

fn mods_and_key_from_string (s: &str, user_mod: c_uint) -> (c_uint, String) {
  let mut mods = 0;
  let mut key = String::new ();
  let mut it = s.split ('+').peekable ();
  while let Some (i) = it.next () {
    if it.peek ().is_some () {
      mods |= str2mod (i, user_mod);
    } else {
      key = i.to_string ();
    }
  }
  (mods, key)
}
