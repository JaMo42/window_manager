use std::ffi::CString;
use x11::xlib::*;
use x11::xft::{XftColor, XftColorAllocName};
use super::core::display;

macro_rules! xft_color_new {
  () => {
    XftColor {
      pixel: 0,
      color: x11::xrender::XRenderColor {red: 0, green: 0, blue: 0, alpha: 0}
    }
  }
}

pub type Color = XftColor;

pub struct Color_Scheme {
  pub focused: Color,
  pub normal: Color,
  pub background: Color,
  pub selected: Color,
  pub urgent: Color,
  pub bar_background: Color,
  pub bar_text: Color,
  pub bar_workspace: Color,
  pub bar_workspace_text: Color,
  pub bar_active_workspace: Color,
  pub bar_active_workspace_text: Color,
  pub bar_urgent_workspace: Color,
  pub bar_urgent_workspace_text: Color
}

impl Color_Scheme {
  pub fn new () -> Color_Scheme {
    Color_Scheme {
      focused: xft_color_new! (),
      normal: xft_color_new! (),
      background: xft_color_new! (),
      selected: xft_color_new! (),
      urgent: xft_color_new! (),
      bar_background: xft_color_new! (),
      bar_text: xft_color_new! (),
      bar_workspace: xft_color_new! (),
      bar_workspace_text: xft_color_new! (),
      bar_active_workspace: xft_color_new! (),
      bar_active_workspace_text: xft_color_new! (),
      bar_urgent_workspace: xft_color_new! (),
      bar_urgent_workspace_text: xft_color_new! ()
    }
  }

  pub unsafe fn load_defaults (&mut self) {
    let scn = XDefaultScreen (display);
    let vis = XDefaultVisual (display, scn);
    let cm = XDefaultColormap (display, scn);
    macro_rules! set_color {
      ($elem:expr, $hex:expr) => {
        XftColorAllocName (
          display, vis, cm,
          c_str! ($hex),
          &mut $elem
        );
      }
    }
    set_color! (self.focused, "#005577");
    set_color! (self.normal, "#444444");
    set_color! (self.background, "#000000");
    set_color! (self.selected, "#007755");
    set_color! (self.urgent, "#770000");

    set_color! (self.bar_background, "#000000");
    set_color! (self.bar_text, "#eeeeee");
    set_color! (self.bar_workspace, "#000000");
    set_color! (self.bar_workspace_text, "#eeeeee");
    set_color! (self.bar_active_workspace, "#005577");
    set_color! (self.bar_active_workspace_text, "#000000");
    set_color! (self.bar_urgent_workspace, "#770000");
    set_color! (self.bar_urgent_workspace_text, "#000000");
  }

  fn _get_elem (&self, element: &str) -> XftColor {
    match element {
      "Focused" => self.focused,
      "Normal" => self.normal,
      "Background" => self.background,
      "Selected" => self.selected,
      "Urgent" => self.urgent,
      "Bar::Background" => self.bar_background,
      "Bar::Text" => self.bar_text,
      "Bar::Workspace" => self.bar_workspace,
      "Bar::WorkspaceText" => self.bar_workspace_text,
      "Bar::ActiveWorkspace" => self.bar_active_workspace,
      "Bar::ActiveWorkspaceText" => self.bar_active_workspace_text,
      "Bar::UrgentWorkspace" => self.bar_urgent_workspace,
      "Bar::UrgentWorkspaceText" => self.bar_urgent_workspace_text,
      _ => panic! ("Color_Scheme::set: unknown element: {}", element)
    }
  }

  fn _get_elem_mut (&mut self, element: &str) -> &mut XftColor {
    match element {
      "Focused" => &mut self.focused,
      "Normal" => &mut self.normal,
      "Background" => &mut self.background,
      "Selected" => &mut self.selected,
      "Urgent" => &mut self.urgent,
      "Bar::Background" => &mut self.bar_background,
      "Bar::Text" => &mut self.bar_text,
      "Bar::Workspace" => &mut self.bar_workspace,
      "Bar::WorkspaceText" => &mut self.bar_workspace_text,
      "Bar::ActiveWorkspace" => &mut self.bar_active_workspace,
      "Bar::ActiveWorkspaceText" => &mut self.bar_active_workspace_text,
      "Bar::UrgentWorkspace" => &mut self.bar_urgent_workspace,
      "Bar::UrgentWorkspaceText" => &mut self.bar_urgent_workspace_text,
      _ => panic! ("Color_Scheme::set: unknown element: {}", element)
    }
  }

  pub fn set (&mut self, element: &str, color_hex: &str) {
    let dest: *mut XftColor = self._get_elem_mut (element);
    unsafe {
      let scn = XDefaultScreen (display);
      XftColorAllocName (
        display,
        XDefaultVisual (display, scn),
        XDefaultColormap (display, scn),
        c_str! (color_hex),
        dest
      );
    }
  }

  pub fn link (&mut self, dest: &str, source: &str) {
    *self._get_elem_mut (dest) = self._get_elem (source);
  }
}
