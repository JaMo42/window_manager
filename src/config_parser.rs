use super::color::{Color, Color_Config, Color_Scheme, Color_Scheme_Config};
use super::config::{Action, Config, Key};
use super::core::*;
use super::error::message_box;
use super::paths;
use super::process::split_commandline;
use std::collections::BTreeMap;
use std::fs::read_to_string;
use std::os::raw::c_uint;
use toml::value::Table;

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Parsed_Config {
  pub general: Option<General>,
  pub layout: Option<Layout>,
  pub window: Option<Window>,
  pub theme: Option<Theme>,
  pub keys: Option<Keys>,
  pub bar: Option<Bar>,
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
}

#[derive(Deserialize, Debug, Default)]
pub struct Window {
  pub border: Option<i32>,
  pub title_font: Option<String>,
  pub title_bar_height: Option<String>,
  pub title_alignment: Option<String>,
  pub right_buttons: Option<Vec<String>>,
  pub left_buttons: Option<Vec<String>>,
  pub icon_size: Option<u8>,
  pub circle_buttons: Option<bool>,
  pub button_icon_size: Option<u8>,
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
  pub opacity: Option<u8>,
  pub height: Option<String>,
  pub time_format: Option<String>,
  pub power_supply: Option<String>,
  pub update_interval: Option<u64>,
}

pub fn parse (pathname: &str) -> Result<Parsed_Config, toml::de::Error> {
  if let Ok (content) = read_to_string (pathname) {
    toml::from_str (&content)
  } else {
    let default_config = "[keys.bindings]\n'Mod+Shift+Q' = \"quit\"\n'Mod+Return' = \"$xterm\"";
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
    if action.starts_with ('$') {
      cfg.add (
        Key::from_str (&key, mods),
        Action::Launch (split_commandline (&action[1..])),
      );
    } else {
      cfg.add (Key::from_str (&key, mods), Action::from_str (action));
    }
  }
}

pub fn parse_color_scheme (name: String) -> Result<Color_Scheme, String> {
  use std::fs::File;
  use std::io::{BufRead, BufReader};
  macro_rules! E {
    ($result:expr) => {
      $result.map_err (|e| e.to_string ())?
    };
  }
  let mut color_scheme_config = Color_Scheme_Config::new ();
  let mut color_defs: BTreeMap<String, Color> = BTreeMap::new ();
  let pathname = format! ("{}/{}", unsafe { &paths::colors_dir }, name);
  let file = E! (File::open (pathname));
  for l1 in BufReader::new (file).lines () {
    let l2 = E! (l1);
    if l2.is_empty () || l2.starts_with ('#') {
      continue;
    }
    let mut line = l2.split (' ');
    let op = E! (line.next ().ok_or ("Missing operation".to_string ()));
    let elem = E! (line.next ().ok_or ("Missing element".to_string ()));
    let color = E! (line
      .next ()
      .ok_or ("Missing color or link name".to_string ()));
    match op {
      "def_color" => {
        color_defs.insert (elem.to_string (), unsafe { Color::alloc_from_hex (&color) });
      }
      "color" => {
        color_scheme_config.set (
          &elem,
          if color.starts_with ('#') {
            Color_Config::Hex (color.to_string ())
          } else {
            Color_Config::Link (color.to_string ())
          },
        )?;
      }
      _ => {
        return Err ("Invalid operation".to_string ());
      }
    }
  }
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

pub fn modifiers_from_string (s: &String) -> c_uint {
  let mut mods = 0;
  for mod_str in s.split ('+') {
    mods |= str2mod (mod_str, 0);
  }
  mods
}

fn mods_and_key_from_string (s: &String, user_mod: c_uint) -> (c_uint, String) {
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
