use crate::core::display;
use std::collections::BTreeMap;
use std::mem::size_of;
use x11::xft::{XftColor, XftColorAllocName};

#[derive(Copy, Clone)]
pub struct Color {
  pub pixel: u64,
  pub red: f64,
  pub green: f64,
  pub blue: f64,
}

impl Color {
  pub const fn from_rgb(red: f64, green: f64, blue: f64) -> Self {
    Color {
      pixel: 0,
      red,
      green,
      blue,
    }
  }

  pub unsafe fn alloc_from_hex(hex: &str) -> Self {
    let mut xcolor: XftColor = zeroed!();
    XftColorAllocName(
      display.as_raw(),
      display.default_visual(),
      display.default_colormap(),
      c_str!(hex),
      &mut xcolor,
    );
    Color {
      pixel: xcolor.pixel,
      red: xcolor.color.red as f64 / 0xffff as f64,
      green: xcolor.color.green as f64 / 0xffff as f64,
      blue: xcolor.color.blue as f64 / 0xffff as f64,
    }
  }

  pub fn scale(&self, factor: f64) -> Self {
    Self::from_rgb(
      (self.red * factor).clamp(0.0, 1.0),
      (self.green * factor).clamp(0.0, 1.0),
      (self.blue * factor).clamp(0.0, 1.0),
    )
  }
}

#[derive(Clone)]
pub enum Color_Config {
  Default,
  Hex(String),
  Link(String),
}

pub struct Color_Scheme_Config {
  pub cfg: Vec<Color_Config>,
}

impl Color_Scheme_Config {
  pub fn new() -> Self {
    Color_Scheme_Config {
      cfg: vec![Color_Config::Default; COLOR_COUNT],
    }
  }

  pub fn set(&mut self, elem: &str, cfg: Color_Config) -> Result<(), String> {
    self.cfg[unsafe { color_index(elem)? }] = cfg;
    Ok(())
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
  pub maximize_button: Color,
  pub maximize_button_hovered: Color,
  pub minimize_button: Color,
  pub minimize_button_hovered: Color,
  pub background: Color,
  pub bar_background: Color,
  pub bar_text: Color,
  pub bar_workspace: Color,
  pub bar_workspace_text: Color,
  pub bar_active_workspace: Color,
  pub bar_active_workspace_text: Color,
  pub bar_urgent_workspace: Color,
  pub bar_urgent_workspace_text: Color,
  pub notification_background: Color,
  pub notification_text: Color,
  pub tooltip_background: Color,
  pub tooltip_text: Color,
  pub dock_background: Color,
  pub dock_hovered: Color,
  pub dock_urgent: Color,
  pub dock_indicator: Color,
  pub context_menu_background: Color,
  pub context_menu_text: Color,
  pub context_menu_divider: Color,
}
const COLOR_COUNT: usize = size_of::<Color_Scheme>() / size_of::<Color>();
const COLOR_NAMES: [&str; COLOR_COUNT] = [
  "window.focused",
  "window.focused_text",
  "window.normal",
  "window.normal_text",
  "window.selected",
  "window.selected_text",
  "window.urgent",
  "window.urgent_text",
  "window.buttons.close",
  "window.buttons.close_hovered",
  "window.buttons.maximize",
  "window.buttons.maximize_hovered",
  "window.buttons.minimize",
  "window.buttons.minimize_hovered",
  "misc.background",
  "bar.background",
  "bar.text",
  "bar.workspace",
  "bar.workspace_text",
  "bar.active_workspace",
  "bar.active_workspace_text",
  "bar.urgent_workspace",
  "bar.urgent_workspace_text",
  "notifications.background",
  "notifications.text",
  "tooltip.background",
  "tooltip.text",
  "dock.background",
  "dock.hovered",
  "dock.urgent",
  "dock.indicator",
  "context_menu.background",
  "context_menu.text",
  "context_menu.divider",
];
const DEFAULT_CONFIG: [&str; COLOR_COUNT] = [
  // Window borders
  // Focused
  "#EEEEEE",
  "#111111",
  // Normal
  "#111111",
  "#EEEEEE",
  // Selected
  "#777777",
  "#111111",
  // Urgent
  "#CC1111",
  "#111111",
  // Buttons
  // Close
  "#444444",
  "#CC0000",
  // Maximize
  "window.buttons.close",
  "#00CC00",
  // Minimize
  "window.buttons.close",
  "#CCCC00",
  // Background
  "#000000",
  // Bar
  // Background
  "#111111",
  // Text
  "#EEEEEE",
  // Workspaces
  "bar.background",
  "bar.text",
  // Active workspace
  "window.focused",
  "window.focused_text",
  // Workspace with urgent client
  "window.urgent",
  "window.urgent_text",
  // Notifications
  "bar.background",
  "bar.text",
  // Tooltip
  "bar.background",
  "bar.text",
  // Dock
  "bar.background",
  "window.focused",
  "window.urgent",
  "bar.text",
  // Context menu
  "bar.background",
  "bar.text",
  "bar.text",
];

impl std::ops::Index<usize> for Color_Scheme {
  type Output = Color;

  fn index(&self, index: usize) -> &Color {
    let p = self as *const Color_Scheme as *const Color;
    unsafe { &*p.add(index) }
  }
}

impl std::ops::IndexMut<usize> for Color_Scheme {
  fn index_mut(&mut self, index: usize) -> &mut Color {
    let p = self as *mut Color_Scheme as *mut Color;
    unsafe { &mut *p.add(index) }
  }
}

impl Color_Scheme {
  pub unsafe fn new(
    cfg: &Color_Scheme_Config,
    defs: &BTreeMap<String, Color>,
  ) -> Result<Self, String> {
    let mut result: Color_Scheme = zeroed!();
    let mut set: [bool; COLOR_COUNT] = [false; COLOR_COUNT];
    let mut links = Vec::<(usize, usize)>::new();
    for i in 0..COLOR_COUNT {
      match &cfg.cfg[i] {
        Color_Config::Default => {
          if DEFAULT_CONFIG[i].starts_with('#') {
            result[i] = Color::alloc_from_hex(DEFAULT_CONFIG[i]);
            set[i] = true;
          } else {
            links.push((i, color_index(DEFAULT_CONFIG[i])?));
          }
        }
        Color_Config::Hex(string) => {
          result[i] = Color::alloc_from_hex(string.as_str());
          set[i] = true;
        }
        Color_Config::Link(target) => {
          if let Some(def) = defs.get(target) {
            result[i] = *def;
            set[i] = true;
          } else {
            links.push((i, color_index(target.as_str())?));
          }
        }
      }
    }

    let mut did_change = true;
    while did_change && !links.is_empty() {
      did_change = false;
      for i in (0..links.len()).rev() {
        if set[links[i].1] {
          result[links[i].0] = result[links[i].1];
          set[links[i].0] = true;
          links.remove(i);
          did_change = true;
        }
      }
    }

    if !links.is_empty() {
      Err("Unresolved links in color scheme: {}".to_string())
    } else {
      Ok(result)
    }
  }
}

unsafe fn color_index(name: &str) -> Result<usize, String> {
  for (i, color) in COLOR_NAMES.iter().enumerate() {
    if *color == name {
      return Ok(i);
    }
  }
  Err(format!("Invalid color name: {}", name))
}
