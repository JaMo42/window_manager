mod tray_client;
pub mod tray_manager;
mod widget;
mod xembed;

use crate::core::*;
use crate::cursor;
use crate::ewmh;
use crate::monitors;
use crate::property;
use crate::update_thread::Update_Thread;
use crate::x::{Window, XWindow};
use crate::{set_window_kind, set_window_opacity};
use tray_manager::Tray_Manager;
use widget::Widget;
use x11::xlib::*;

pub static mut tray: Tray_Manager = Tray_Manager::new();

pub static mut update_thread: Option<Update_Thread> = None;

pub struct Bar {
  pub width: u32,
  pub height: u32,
  pub window: Window,
  last_scroll_time: Time,
  left_widgets: Vec<Box<dyn Widget>>,
  right_widgets: Vec<Box<dyn Widget>>,
  // The widget under the mouse, gets set on enter/leave events so we don't need
  // to resolve it for clicks
  mouse_widget: *mut dyn Widget,
}

impl Bar {
  /// Space between widgets
  pub const WIDGET_GAP: i32 = 10;
  /// Space between rightmost widget and system tray
  pub const RIGHT_GAP: u32 = 5;

  pub const fn new() -> Self {
    Self {
      width: 0,
      height: 0,
      window: Window::uninit(),
      last_scroll_time: 0,
      left_widgets: Vec::new(),
      right_widgets: Vec::new(),
      mouse_widget: widget::null_ptr(),
    }
  }

  pub unsafe fn create() -> Self {
    let main_mon = monitors::main();
    let width = main_mon.geometry().w;
    let height = (*config).bar_height;
    let x = main_mon.geometry().x;
    let y = main_mon.geometry().y;
    let window = Window::builder(&display)
      .size(width, height)
      .position(x, y)
      .attributes(|attributes| {
        attributes
          .override_redirect(true)
          .background_pixel((*config).colors.bar_background.pixel)
          .event_mask(ExposureMask)
          .cursor(cursor::normal);
      })
      .build();
    ewmh::set_window_type(window, property::Net::WMWindowTypeDock);
    set_window_opacity(window, (*config).bar_opacity);
    // We don't want to interact with the blank part, instead the widgets
    // use `Window_Kind::Status_Bar`.
    set_window_kind(window, Window_Kind::Meta_Or_Unmanaged);
    window.set_class_hint("Window_manager_bar", "window_manager_bar");
    window.map_raised();
    Self {
      width,
      height,
      window,
      last_scroll_time: 0,
      left_widgets: Vec::new(),
      right_widgets: Vec::new(),
      mouse_widget: widget::null_ptr(),
    }
  }

  /// Adds widgets
  pub unsafe fn build(&mut self) {
    macro_rules! push {
      ($target:expr, $widget:ident) => {
        if let Some(w) = widget::$widget::new() {
          $target.push(Box::new(w));
        } else {
          log::warn!("Could not create widget: {}", stringify!($widget));
        }
      };
    }
    push!(self.left_widgets, Workspaces);
    push!(self.right_widgets, Quit);
    push!(self.right_widgets, DateTime);
    push!(self.right_widgets, Volume);
    push!(self.right_widgets, Battery);
  }

  pub unsafe fn draw(&mut self) {
    if !cfg!(feature = "bar") {
      return;
    }
    (*draw).select_font(&(*config).bar_font);

    // Left
    let mut x = 0;
    for w in self.left_widgets.iter_mut() {
      let width = w.update(self.height, Self::WIDGET_GAP as u32);
      w.window().r#move(x, 0);
      x += width as i32;
      x += Self::WIDGET_GAP;
    }
    let mid_x = x;

    // Right
    x = (self.width - Self::RIGHT_GAP) as i32;
    let mut first = true;
    for w in self.right_widgets.iter_mut() {
      if first {
        let width = w.update(self.height, Self::RIGHT_GAP);
        x -= width as i32;
        first = false;
      } else {
        let width = w.update(self.height, Self::WIDGET_GAP as u32);
        x -= width as i32;
        x -= Self::WIDGET_GAP;
      }
      w.window().r#move(x, 0);
    }
    let mid_width = (x - mid_x) as u32;

    self
      .window
      .move_and_resize(mid_x, 0, mid_width, self.height);
    self.window.clear();
    display.sync(false);
  }

  pub unsafe fn redraw_all(&mut self) {
    for w in self
      .left_widgets
      .iter_mut()
      .chain(self.right_widgets.iter_mut())
    {
      w.invalidate();
    }
    self.draw();
  }

  pub fn invalidate_widgets(&mut self) {
    for w in self
      .left_widgets
      .iter_mut()
      .chain(self.right_widgets.iter_mut())
    {
      w.invalidate();
    }
  }

  pub unsafe fn resize(&mut self, width: u32) {
    self.width = width;
    self.draw();
  }

  pub unsafe fn click(&mut self, event: &XButtonEvent) {
    // Limit scroll speed
    // We just do this for all button types as it doesn't matter for normal
    // button presses.
    if (event.time - self.last_scroll_time) <= (1000 / 10) {
      return;
    }
    self.last_scroll_time = event.time;
    (*self.mouse_widget).click(event);
  }

  pub unsafe fn enter(&mut self, window: XWindow) {
    for w in self
      .left_widgets
      .iter_mut()
      .chain(self.right_widgets.iter_mut())
    {
      if w.window() == window {
        self.mouse_widget = w.as_mut();
        w.enter();
        return;
      }
    }
  }

  pub unsafe fn leave(&mut self, _window: XWindow) {
    (*self.mouse_widget).leave();
    self.mouse_widget = widget::null_ptr();
  }

  pub unsafe fn destroy(&mut self) {
    if self.window.is_none() {
      return;
    }
    for w in self.left_widgets.iter() {
      w.window().destroy();
    }
    for w in self.right_widgets.iter() {
      w.window().destroy();
    }
    self.window.destroy();
    self.window = Window::uninit();
    tray.destroy();
  }
}

/// Redraws the bar if the "bar" feature is enabled
pub fn update() {
  // Note: bar.draw already does the feature check, this is mostly just a safe wrapper
  unsafe {
    bar.draw();
  }
}

pub unsafe fn resize() {
  // The tray calls `bar.resize`.
  tray.resize_window();
  bar.redraw_all();
}
