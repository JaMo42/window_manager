use libc::{c_uchar, c_uint};
use std::ffi::CString;
use x11::xlib::*;
use crate::core::*;
use crate::{get_window_geometry, set_window_kind};
use crate::property;
use crate::draw::{Alignment, Svg_Resource, resources};
use crate::platform;
use crate::tooltip::tooltip;

unsafe fn create_window () -> Window {
  let mut attributes: XSetWindowAttributes = uninitialized! ();
  attributes.background_pixel = (*config).colors.bar_background.pixel;
  attributes.event_mask = ButtonPressMask | EnterWindowMask | LeaveWindowMask;
  let window = XCreateWindow (
    display, root,
    0, 0, 10, 10,
    0,
    CopyFromParent,
    CopyFromParent as u32,
    CopyFromParent as *mut Visual,
    CWBackPixel|CWEventMask,
    &mut attributes
  );
    let window_type_dock = property::atom (property::Net::WMWindowTypeDock);
    property::set (
      window,
      property::Net::WMWindowType,
      XA_ATOM,
      32,
      &window_type_dock,
      1
    );
    if (*config).bar_opacity != 100 {
      let atom = XInternAtom (display, c_str! ("_NET_WM_WINDOW_OPACITY"), X_FALSE);
      let value = 42949672u32 * (*config).bar_opacity as u32;
      set_cardinal! (window, atom, value);
    }
  set_window_kind (window, Window_Kind::Status_Bar);
  XMapWindow (display, window);
  window
}

pub trait Widget {
  unsafe fn resize (&self, x: i32, width: u32, height: u32) {
    XMoveResizeWindow (
      display, self.window (),
      x, 0,
      width, height
    );
  }

  fn window (&self) -> Window;

  unsafe fn update (&mut self, height: u32, gap: u32) -> u32;

  unsafe fn click (&mut self, _event: &XButtonEvent) {}

  unsafe fn enter (&mut self) {}

  unsafe fn leave (&mut self) {}
}



unsafe fn draw_icon_and_text (string: &str, icon: Option<&'static mut Svg_Resource>, height: u32) -> u32 {
  const ICON_TEXT_GAP: i32 = 3;
  let mut width = 0;
  (*draw).fill_rect (0, 0, screen_size.w, height, (*config).colors.bar_background);
  if icon.is_some () {
    let size = (height as f64 * 0.9).round () as u32;
    let pos = ((height - size) / 2) as i32;
    (*draw).draw_colored_svg (
      icon.unwrap_unchecked (),
      (*config).colors.bar_text,
      pos,pos,
      size, size
    );
    width += height + ICON_TEXT_GAP as u32;
  }
  if string.is_empty () {
    return width;
  }
  // Font is selected by bar
  (*draw).text_color((*config).colors.bar_text);
  width += (*draw).text (&string)
    .at (width as i32, 0)
    .align_vertically (Alignment::Centered, height as i32)
    .draw ()
    .w;
  width
}

unsafe fn resize_and_render (window: Window, width: u32, height: u32, gap: u32) {
  XResizeWindow (display, window, width + gap, height);
  (*draw).render (window, 0, 0, width, height);
}



pub struct DateTime {
  window: Window
}

impl DateTime {
  pub fn new () -> Self {
    Self {
      window: unsafe { create_window () }
    }
  }
}

impl Widget for DateTime {
  fn window (&self) -> Window {
    self.window
  }

  unsafe fn update (&mut self, height: u32, gap: u32) -> u32 {
    let now = chrono::Local::now ();
    let label = format! ("{}", now.format ((*config).bar_time_format.as_str ()));
    let width = draw_icon_and_text (&label, Some (&mut resources::calendar), height);
    resize_and_render (self.window, width, height, gap);
    width
  }
}



pub struct Battery {
  window: Window,
  hover_text: String
}

impl Battery {
  pub fn new () -> Self {
    Self {
      window: unsafe { create_window () },
      hover_text: String::new ()
    }
  }
}

impl Widget for Battery {
  fn window (&self) -> Window {
    self.window
  }

  unsafe fn update (&mut self, height: u32, gap: u32) -> u32 {
    let power_supply = "BAT0";
    let mut capacity = std::fs::read_to_string (
      format! ("/sys/class/power_supply/{}/capacity", power_supply)
    ).expect("Could not read battery status");
    capacity.pop ();
    let mut status = std::fs::read_to_string (
      format! ("/sys/class/power_supply/{}/status", power_supply)
    ).expect("Could not read battery status");
    status.pop ();
    self.hover_text = format! ("{}, {}", power_supply, status);
    let label = format! ("{}%", capacity);
    let width = draw_icon_and_text (&label, Some (&mut resources::battery), height);
    resize_and_render (self.window, width, height, gap);
    width
  }

  unsafe fn enter (&mut self) {
    // not optimal since this contains the gap on one side
    let g = get_window_geometry (self.window);
    tooltip.show (&self.hover_text, g.x - g.w as i32 / 2, g.h as i32);
  }

  unsafe fn leave (&mut self) {
    tooltip.close ();
  }
}



pub struct Volume {
  window: Window
}

impl Volume {
  pub fn new () -> Self {
    Self {
      window: unsafe { create_window () }
    }
  }
}

impl Widget for Volume {
  fn window (&self) -> Window {
    self.window
  }

  unsafe fn update (&mut self, height: u32, gap: u32) -> u32 {
    if let Some ((is_muted, level)) = platform::get_volume_info () {
      let width = if is_muted {
        draw_icon_and_text ("muted", Some (&mut resources::volume_muted), height)
      } else {
        let label = format! ("{}%", level);
        draw_icon_and_text (&label, Some (&mut resources::volume), height)
      };
      resize_and_render (self.window, width, height, gap);
      width
    } else {
      1
    }
  }

  unsafe fn click (&mut self, event: &XButtonEvent) {
    use crate::platform::actions::*;
    if event.button == Button1 || event.button == Button2 || event.button == Button3 {
      mute_volume ();
    } else if event.button == Button5 {
      // Scroll up
      increase_volume ();
    } else if event.button == Button4 {
      // Scroll down
      decrease_volume ();
    }
  }
}



pub struct Workspace_Widget {
  window: Window
}

impl Workspace_Widget {
  pub fn new () -> Self {
    let window = unsafe { create_window () };
    unsafe { XResizeWindow (
      display,
      window,
      // Note assumes this is never the rightmost widget, this way we don't
      // need to resize on every update
      bar.height * workspaces.len () as u32 + super::Bar::WIDGET_GAP as u32,
      bar.height
    )};
    Self {
      window
    }
  }
}

impl Widget for Workspace_Widget {
  fn window (&self) -> Window {
    self.window
  }

  unsafe fn update (&mut self, height: u32, _gap: u32) -> u32 {
    let width = height * workspaces.len () as u32;
    (*draw).fill_rect (0, 0, screen_size.w, height, (*config).colors.bar_background);
    for (idx, workspace) in workspaces.iter ().enumerate () {
      let color = if workspace.has_urgent () {
        (*config).colors.bar_urgent_workspace
      } else if idx == active_workspace {
        (*config).colors.bar_active_workspace
      } else {
        (*config).colors.bar_workspace
      };
      (*draw).square ((idx as u32 * height) as i32, 0, height)
        .color (color)
        .stroke (2, color.scale (0.8))
        .draw ();
      (*draw).text (format! ("{}", idx+1).as_str ())
        .at ((idx as u32 * height) as i32, 0)
        .align_horizontally (Alignment::Centered, height as i32)
        .align_vertically (Alignment::Centered, height as i32)
        .color (
          if workspace.has_urgent () {
            (*config).colors.bar_urgent_workspace_text
          }
          else if idx == active_workspace {
            (*config).colors.bar_active_workspace_text
          } else {
            (*config).colors.bar_text
          }
        )
        .draw ();
    }
    (*draw).render (self.window, 0, 0, width, height);
    width
  }

  unsafe fn click (&mut self, event: &XButtonEvent) {
    use crate::action::select_workspace;
    if event.button == Button1 || event.button == Button2 || event.button == Button3 {
      // Left/Middle/Right click selects workspace under cursor
      select_workspace (event.x as usize / bar.height as usize, None);
    }
    else if event.button == Button5 {
      // Scrolling up selects the next workspace
      select_workspace ((active_workspace + 1) % workspaces.len (), None)
    }
    else if event.button == Button4 {
      // Scrolling down selects the previous workspace
      if active_workspace == 0 {
        select_workspace (workspaces.len () - 1, None);
      }
      else {
        select_workspace (active_workspace - 1, None);
      }
    }
  }
}

pub const fn null_ptr () -> *mut dyn Widget {
  // The actual type doesn't matter here, just need to have one
  std::ptr::null_mut::<Workspace_Widget> ()
}
