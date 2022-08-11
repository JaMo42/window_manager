use std::ffi::CString;
use std::mem::size_of;
use std::collections::BTreeMap;
use x11::xlib::*;
use x11::xft::{XftColor, XftColorAllocName};
use super::core::display;

#[derive(Copy, Clone)]
pub struct Color {
  pub pixel: u64,
  pub red: f64,
  pub green: f64,
  pub blue: f64
}

impl Color {
  pub const fn from_rgb (red: f64, green: f64, blue: f64) -> Self {
    Color {
      pixel: 0,
      red,
      green,
      blue
    }
  }

  pub unsafe fn alloc_from_hex (hex: &str) -> Self {
    let screen = XDefaultScreen (display);
    let visual = XDefaultVisual (display, screen);
    let color_map = XDefaultColormap (display, screen);
    let mut xcolor: XftColor = uninitialized! ();
    XftColorAllocName (display, visual, color_map, c_str! (hex), &mut xcolor);
    Color {
      pixel: xcolor.pixel,
      red: xcolor.color.red as f64 / 0xffff as f64,
      green: xcolor.color.green as f64 / 0xffff as f64,
      blue: xcolor.color.blue as f64 / 0xffff as f64
    }
  }

  pub fn scale (&self, factor: f64) -> Self {
    Self::from_rgb (
      self.red * factor,
      self.green * factor,
      self.blue * factor
    )
  }
}


pub enum Color_Config {
  Default,
  Hex (String),
  Link (String)
}

impl std::default::Default for Color_Config {
  fn default() -> Self {
    Color_Config::Default
  }
}


pub struct Color_Scheme_Config {
  pub cfg: [Color_Config; COLOR_COUNT]
}

impl Color_Scheme_Config {
  pub fn new () -> Self {
    Color_Scheme_Config { cfg: Default::default () }
  }

  pub fn set (&mut self, elem: &str, cfg: Color_Config) {
    self.cfg[unsafe { color_index (elem) }] = cfg;
  }
}


#[repr(C)]
pub struct Color_Scheme {
  pub focused: Color,
  pub focused_text: Color,
  pub normal: Color,
  pub normal_text: Color,
  pub selected: Color,
  pub selected_text: Color,
  pub urgent: Color,
  pub urgent_text: Color,
  pub close_button: Color,
  pub close_button_hovered: Color,
  pub background: Color,
  pub bar_background: Color,
  pub bar_text: Color,
  pub bar_workspace: Color,
  pub bar_workspace_text: Color,
  pub bar_active_workspace: Color,
  pub bar_active_workspace_text: Color,
  pub bar_urgent_workspace: Color,
  pub bar_urgent_workspace_text: Color,
}
const COLOR_COUNT: usize = size_of::<Color_Scheme> () / size_of::<Color> ();
const COLOR_NAMES: [&str; COLOR_COUNT] = [
  "Focused",
  "FocusedText",
  "Normal",
  "NormalText",
  "Selected",
  "SelectedText",
  "Urgent",
  "UrgentText",
  "CloseButton",
  "CloseButtonHovered",
  "Background",
  "Bar::Background",
  "Bar::Text",
  "Bar::Workspace",
  "Bar::WorkspaceText",
  "Bar::ActiveWorkspace",
  "Bar::ActiveWorkspaceText",
  "Bar::UrgentWorkspace",
  "Bar::UrgentWorkspaceText"
];
const DEFAULT_CONFIG: [&str; COLOR_COUNT] = [
  // Window borders
    // Focused
    "#005577",
    "#000000",
    // Normal
    "#444444",
    "#eeeeee",
    // Selected
    "#007755",
    "#000000",
    // Urgent
    "#770000",
    "#000000",
    // Close button
    "#000000",
    "#ff1111",

  // Background
  "#000000",

  // Bar
    // Background
    "#111111",
    // Text
    "#eeeeee",
    // Workspaces
    "Bar::Background",
    "Bar::Text",
    // Active workspace
    "Focused",
    "FocusedText",
    // Workspace with urgent client
    "Urgent",
    "UrgentText",
];

impl std::ops::Index<usize> for Color_Scheme {
  type Output = Color;

  fn index (&self, index: usize) -> &Color {
    let p = self as *const Color_Scheme as *const Color;
    unsafe {
      &*p.add (index)
    }
  }
}

impl std::ops::IndexMut<usize> for Color_Scheme {
  fn index_mut (&mut self, index: usize) -> &mut Color {
    let p = self as *mut Color_Scheme as *mut Color;
    unsafe {
      &mut *p.add (index)
    }
  }
}

impl Color_Scheme {
  pub unsafe fn new (cfg: &Color_Scheme_Config, defs: &BTreeMap<String, Color>) -> Self {
    let mut result: Color_Scheme = uninitialized! ();
    let mut set: [bool; COLOR_COUNT] = [false; COLOR_COUNT];
    let mut links = Vec::<(usize, usize)>::new ();
    for i in 0..COLOR_COUNT {
      match &cfg.cfg[i] {
        Color_Config::Default => {
          if DEFAULT_CONFIG[i].starts_with ('#') {
            result[i] = Color::alloc_from_hex (DEFAULT_CONFIG[i]);
            set[i] = true;
          } else {
            links.push ((i, color_index (DEFAULT_CONFIG[i])));
          }
        },
        Color_Config::Hex (string) => {
          result[i] = Color::alloc_from_hex (string.as_str ());
          set[i] = true;
        },
        Color_Config::Link (target) => {
          if let Some (def) = defs.get (target) {
            result[i] = *def;
            set[i] = true;
          } else {
            links.push ((i, color_index (target.as_str ())));
          }
        }
      }
    }

    let mut did_change = true;
    while did_change && !links.is_empty() {
      did_change = false;
      for i in (0..links.len ()).rev () {
        if set[links[i].1] {
          result[links[i].0] = result[links[i].1];
          set[links[i].0] = true;
          links.remove (i);
          did_change = true;
        }
      }
    }
    if !links.is_empty () {
      log::error! ("Unresolved links: {:?}", links);
      panic! ("unresolved links");
    }

    result
  }

  pub fn new_uninit () -> Self {
    unsafe {
      uninitialized! ()
    }
  }
}

unsafe fn color_index (name: &str) -> usize {
  for (i, color) in COLOR_NAMES.iter ().enumerate () {
    if *color == name {
      return i;
    }
  }
  log::error! ("Invalid color name: {}", name);
  panic! ("invalid color name");
}
