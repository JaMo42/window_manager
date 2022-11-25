use super::core::*;
use super::ewmh;
use super::geometry::Geometry;
use super::property::Net;
use super::set_window_kind;
use crate::x::Window;

pub struct Tooltip {
  window: Window,
  geometry: Geometry,
  active: bool,
}

// As this is intended to be shown under the mouse cursor it only really makes
// sense to only have one, so it is stored globally so whoever uses it doesn't
// need to care about storing it.
pub static mut tooltip: Tooltip = Tooltip::new ();

impl Tooltip {
  const BORDER: u32 = 5;

  pub const fn new () -> Self {
    Self {
      window: Window::uninit (),
      geometry: Geometry::new (),
      active: false,
    }
  }

  unsafe fn create (&mut self) {
    self.window = Window::builder (&display)
      .attributes (|attributes| {
        attributes
          .background_pixel ((*config).colors.bar_background.pixel)
          .override_redirect (true);
      })
      .build ();
    ewmh::set_window_type (self.window, Net::WMWindowTypeTooltip);
    set_window_kind (self.window, Window_Kind::Meta_Or_Unmanaged);
  }

  unsafe fn move_and_resize (&mut self, x: i32, y: i32, w: u32, h: u32) {
    self.geometry = Geometry::from_parts (x - w as i32 / 2, y, w, h);
    if self.geometry.x as u32 + self.geometry.w > screen_size.w {
      self.geometry.x = (screen_size.w - self.geometry.w) as i32;
    }
    self.window.move_and_resize (
      self.geometry.x,
      self.geometry.y,
      self.geometry.w,
      self.geometry.h,
    );
  }

  pub unsafe fn show (&mut self, string: &str, x: i32, y: i32) {
    if self.window.is_none () {
      self.create ();
    }
    if self.active {
      self.close ();
    }
    (*draw).select_font (&(*config).bar_font);
    let mut text = (*draw).text (string);
    let width = text.get_width () + 2 * Self::BORDER;
    // Add one to the height as the text width is based on the baseline and
    // I find this makes it look at bit better without looking uncentered.
    let height = text.get_height () + 2 * Self::BORDER + 1;
    self.move_and_resize (x, y, width, height);
    (*draw).fill_rect (0, 0, width, height, (*config).colors.bar_background);
    text
      .at (Self::BORDER as i32, Self::BORDER as i32)
      .color ((*config).colors.bar_text)
      .draw ();
    self.window.map_raised ();
    (*draw).render (self.window, 0, 0, width, height);
    self.active = true;
  }

  pub unsafe fn close (&mut self) {
    if self.active {
      self.window.unmap ();
      self.active = false;
      display.sync (false);
    }
  }

  pub unsafe fn destroy (&mut self) {
    if self.window.is_some () {
      self.window.destroy ();
    }
  }

  // Calculates the height for a single line of text.
  pub unsafe fn height () -> u32 {
    (*draw).font_height (Some (&(*config).bar_font)) + 2 * Self::BORDER
  }
}

impl Drop for Tooltip {
  fn drop (&mut self) {
    unsafe {
      self.destroy ();
    }
  }
}
