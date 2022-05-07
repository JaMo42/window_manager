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

type Color = XftColor;

pub struct Color_Scheme {
  pub focused: Color,
  pub normal: Color,
  pub background: Color,
  pub selected: Color,
  pub urgent: Color
}

impl Color_Scheme {
  pub fn new () -> Color_Scheme {
    Color_Scheme {
      focused: xft_color_new! (),
      normal: xft_color_new! (),
      background: xft_color_new! (),
      selected: xft_color_new! (),
      urgent: xft_color_new! ()
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
  }

  pub fn set (&mut self, element: &String, color_hex: &String) {
    let dest: *mut XftColor = match element.as_str () {
      "Focused" => &mut self.focused,
      "Normal" => &mut self.normal,
      "Background" => &mut self.background,
      "Selected" => &mut self.selected,
      "Urgent" => &mut self.urgent,
      _ => panic! ("Color_Scheme::set: unknown element: {}", element)
    };
    unsafe {
      let scn = XDefaultScreen (display);
      XftColorAllocName (
        display,
        XDefaultVisual (display, scn),
        XDefaultColormap (display, scn),
        c_str! (color_hex.as_str ()),
        dest
      );
    }
  }
}

