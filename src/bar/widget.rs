use crate::color::Color;
use crate::core::*;
use crate::draw::{resources, Alignment, Svg_Resource};
use crate::ewmh;
use crate::platform;
use crate::property;
use crate::tooltip::tooltip;
use crate::x::Window;
use crate::{get_window_geometry, set_window_kind, set_window_opacity};
use x11::xlib::*;

unsafe fn create_window () -> Window {
  let window = Window::builder (&display)
    .attributes (|attributes| {
      attributes
        .background_pixel ((*config).colors.bar_background.pixel)
        .event_mask (ButtonPressMask | EnterWindowMask | LeaveWindowMask)
        .backing_store (WhenMapped);
    })
    .build ();
  ewmh::set_window_type (window, property::Net::WMWindowTypeDock);
  set_window_opacity (window, (*config).bar_opacity);
  set_window_kind (window, Window_Kind::Status_Bar);
  window.map ();
  window
}

pub trait Widget {
  unsafe fn resize (&self, x: i32, width: u32, height: u32) {
    self.window ().move_and_resize (x, 0, width, height);
  }

  fn window (&self) -> Window;

  unsafe fn update (&mut self, height: u32, gap: u32) -> u32;

  unsafe fn click (&mut self, _event: &XButtonEvent) {}

  unsafe fn enter (&mut self) {}

  unsafe fn leave (&mut self) {}

  fn invalidate (&mut self) {}
}

unsafe fn draw_icon_and_text (
  string: &str,
  icon: Option<&'static mut Svg_Resource>,
  color: Option<Color>,
  height: u32,
) -> u32 {
  const ICON_TEXT_GAP: u32 = 3;
  let color = color.unwrap_or ((*config).colors.bar_text);
  let mut width = 0;
  (*draw).fill_rect (0, 0, screen_size.w, height, (*config).colors.bar_background);
  if let Some (svg) = icon {
    let size = (height as f64 * 0.9).round () as u32;
    let pos = ((height - size) / 2) as i32;
    (*draw).draw_colored_svg (svg, color, pos, pos, size, size);
    width += height;
  }
  if string.is_empty () {
    return width;
  }
  width += ICON_TEXT_GAP;
  // Font is selected by bar
  width += (*draw)
    .text (string)
    .at (width as i32, 0)
    .align_vertically (Alignment::Centered, height as i32)
    .color (color)
    .draw ()
    .w;
  width
}

unsafe fn resize_and_render (window: Window, width: u32, height: u32, gap: u32) {
  window.resize (width + gap, height);
  (*draw).render (window, 0, 0, width, height);
}

pub struct DateTime {
  window: Window,
  last_label: String,
  width: u32,
}

impl DateTime {
  pub fn new () -> Option<Self> {
    Some (Self {
      window: unsafe { create_window () },
      last_label: String::new (),
      width: 0,
    })
  }
}

impl Widget for DateTime {
  fn window (&self) -> Window {
    self.window
  }

  unsafe fn update (&mut self, height: u32, gap: u32) -> u32 {
    let now = chrono::Local::now ();
    let label = format! ("{}", now.format ((*config).bar_time_format.as_str ()));
    if label == self.last_label {
      return self.width;
    }
    let width = draw_icon_and_text (&label, Some (&mut resources::calendar), None, height);
    resize_and_render (self.window, width, height, gap);
    self.last_label = label;
    self.width = width;
    width
  }

  fn invalidate (&mut self) {
    self.last_label.clear ()
  }
}

pub struct Battery {
  window: Window,
  hover_text: String,
  last_capacity: String,
  last_status: String,
  width: u32,
}

impl Battery {
  pub fn new () -> Option<Self> {
    if std::fs::metadata (format! (
      "/sys/class/power_supply/{}",
      unsafe { &*config }.bar_power_supply
    ))
    .is_ok ()
    {
      Some (Self {
        window: unsafe { create_window () },
        hover_text: String::new (),
        last_capacity: String::new (),
        last_status: String::new (),
        width: 0,
      })
    } else {
      None
    }
  }

  unsafe fn get_icon (status: &str, capacity: u32) -> (&'static mut Svg_Resource, Color) {
    let c = (*config).colors.bar_text;
    if status == "Charging" || status == "Not charging" {
      (&mut resources::battery_charging, c)
    } else if capacity >= 90 {
      (&mut resources::battery_full, c)
    } else if capacity < 10 {
      (&mut resources::battery_critical, (*config).colors.urgent)
    } else {
      let percent = (capacity - 10) as f64 / 80.0;
      let idx = (percent * (resources::battery_bars.len ()) as f64) as usize;
      (resources::battery_bars.get (idx), c)
    }
  }
}

impl Widget for Battery {
  fn window (&self) -> Window {
    self.window
  }

  unsafe fn update (&mut self, height: u32, gap: u32) -> u32 {
    let mut capacity = std::fs::read_to_string (format! (
      "/sys/class/power_supply/{}/capacity",
      (*config).bar_power_supply
    ))
    .expect ("Could not read battery status");
    capacity.pop ();
    let mut status = std::fs::read_to_string (format! (
      "/sys/class/power_supply/{}/status",
      (*config).bar_power_supply
    ))
    .expect ("Could not read battery status");
    status.pop ();
    if capacity == self.last_capacity && status == self.last_status {
      return self.width;
    }
    self.hover_text = format! ("{}, {}", (*config).bar_power_supply, status);
    let label = format! ("{}%", capacity);
    let (icon, color) = Self::get_icon (&status, capacity.parse ().unwrap ());
    let width = draw_icon_and_text (&label, Some (icon), Some (color), height);
    resize_and_render (self.window, width, height, gap);
    self.last_capacity = capacity;
    self.last_status = status;
    self.width = width;
    width
  }

  unsafe fn enter (&mut self) {
    // not optimal since this contains the gap on one side
    let g = get_window_geometry (self.window);
    tooltip.show (&self.hover_text, g.x + g.w as i32 / 2, g.h as i32);
  }

  unsafe fn leave (&mut self) {
    tooltip.close ();
  }

  fn invalidate (&mut self) {
    self.last_capacity.clear ()
  }
}

pub struct Volume {
  window: Window,
  last_level: u32,
  // This is a bool but we need a 3rd state for the initial/invalidated value
  last_mute_state: u8,
  width: u32,
}

impl Volume {
  pub fn new () -> Option<Self> {
    if crate::platform::get_volume_info ().is_some () {
      Some (Self {
        window: unsafe { create_window () },
        last_level: 101,
        last_mute_state: 2,
        width: 0,
      })
    } else {
      None
    }
  }
}

impl Widget for Volume {
  fn window (&self) -> Window {
    self.window
  }

  unsafe fn update (&mut self, height: u32, gap: u32) -> u32 {
    if let Some ((is_muted, level)) = platform::get_volume_info () {
      if level == self.last_level && is_muted as u8 == self.last_mute_state {
        return self.width;
      }
      let width = if is_muted {
        draw_icon_and_text ("muted", Some (&mut resources::volume_muted), None, height)
      } else {
        let label = format! ("{}%", level);
        draw_icon_and_text (&label, Some (&mut resources::volume), None, height)
      };
      resize_and_render (self.window, width, height, gap);
      self.last_level = level;
      self.last_mute_state = is_muted as u8;
      self.width = width;
      width
    } else {
      self.width
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

  fn invalidate (&mut self) {
    self.last_level = 101;
    self.last_mute_state = 2;
  }
}

pub struct Workspaces {
  window: Window,
  last_workspace: usize,
}

impl Workspaces {
  pub fn new () -> Option<Self> {
    let window = unsafe { create_window () };
    unsafe {
      window.resize (
        bar.height * workspaces.len () as u32 + super::Bar::WIDGET_GAP as u32,
        bar.height,
      );
    }
    Some (Self {
      window,
      last_workspace: unsafe { workspaces.len () },
    })
  }
}

impl Widget for Workspaces {
  fn window (&self) -> Window {
    self.window
  }

  unsafe fn update (&mut self, height: u32, _gap: u32) -> u32 {
    let width = height * workspaces.len () as u32;
    if active_workspace == self.last_workspace {
      return width;
    }
    self.last_workspace = active_workspace;
    (*draw).fill_rect (0, 0, screen_size.w, height, (*config).colors.bar_background);
    for (idx, workspace) in workspaces.iter ().enumerate () {
      let color = if workspace.has_urgent () {
        (*config).colors.bar_urgent_workspace
      } else if idx == active_workspace {
        (*config).colors.bar_active_workspace
      } else {
        (*config).colors.bar_workspace
      };
      if color.pixel == (*config).colors.bar_workspace.pixel {
        // Don't draw the outline for the background color
        (*draw).fill_rect ((idx as u32 * height) as i32, 0, height, height, color);
      } else {
        (*draw)
          .square ((idx as u32 * height) as i32, 0, height)
          .color (color)
          .stroke (2, color.scale (0.8))
          .draw ();
      }
      (*draw)
        .text (format! ("{}", idx + 1).as_str ())
        .at ((idx as u32 * height) as i32, 0)
        .align_horizontally (Alignment::Centered, height as i32)
        .align_vertically (Alignment::Centered, height as i32)
        .color (if workspace.has_urgent () {
          (*config).colors.bar_urgent_workspace_text
        } else if idx == active_workspace {
          (*config).colors.bar_active_workspace_text
        } else {
          (*config).colors.bar_text
        })
        .draw ();
    }
    (*draw).render (self.window, 0, 0, width, height);
    width
  }

  unsafe fn click (&mut self, event: &XButtonEvent) {
    use crate::action::select_workspace;
    if event.x >= (bar.height as i32 * workspaces.len () as i32) {
      // Click on padding
      return;
    }
    if event.button == Button1 || event.button == Button2 || event.button == Button3 {
      // Left/Middle/Right click selects workspace under cursor
      select_workspace (event.x as usize / bar.height as usize, None);
    } else if event.button == Button5 {
      // Scrolling up selects the next workspace
      select_workspace ((active_workspace + 1) % workspaces.len (), None)
    } else if event.button == Button4 {
      // Scrolling down selects the previous workspace
      if active_workspace == 0 {
        select_workspace (workspaces.len () - 1, None);
      } else {
        select_workspace (active_workspace - 1, None);
      }
    }
  }

  fn invalidate (&mut self) {
    self.last_workspace = unsafe { workspaces.len () };
  }
}

pub struct Quit {
  window: Window,
  width: u32,
  redraw: bool
}

impl Quit {
  pub fn new () -> Option<Self> {
    let window;
    let width;
    unsafe {
      window = create_window ();
      // Note: assumes this is always the rightmost widget, this way we don't
      // need to resize on every update
      width = bar.height;
      draw_icon_and_text (
        "",
        Some (&mut resources::power),
        Some ((*config).colors.bar_text),
        bar.height,
      );
      resize_and_render (window, bar.height + super::Bar::RIGHT_GAP, bar.height, 0);
    }
    Some (Self { window, width, redraw: false })
  }
}

impl Widget for Quit {
  fn window (&self) -> Window {
    self.window
  }

  unsafe fn update (&mut self, _height: u32, _gap: u32) -> u32 {
    if self.redraw {
      self.width = bar.height;
      draw_icon_and_text (
        "",
        Some (&mut resources::power),
        Some ((*config).colors.bar_text),
        bar.height,
      );
      (*draw).render (self.window, 0, 0, self.width, bar.height);
      self.redraw = false;
    }
    self.width
  }

  unsafe fn click (&mut self, _event: &XButtonEvent) {
    crate::action::quit_dialog ();
  }

  unsafe fn enter (&mut self) {
    // not optimal since this contains the gap on one side
    let g = get_window_geometry (self.window);
    tooltip.show ("Open the quit dialog", g.x + g.w as i32 / 2, g.h as i32);
  }

  unsafe fn leave (&mut self) {
    tooltip.close ();
  }

  fn invalidate (&mut self) {
    self.redraw = true;
  }
}

pub const fn null_ptr () -> *mut dyn Widget {
  // The actual type doesn't matter here, just need to have one
  std::ptr::null_mut::<Workspaces> ()
}
