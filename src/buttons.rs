use crate::draw::Shape;
use crate::geometry::Geometry;

use crate::action;
use crate::client::{decorated_frame_offset, Client};
use crate::color::Color;
use crate::core::*;
use crate::draw::{resources, SvgResource};
use crate::set_window_kind;
use crate::x::{Window, XNone};
use std::ptr::NonNull;
use x11::xlib::*;

static mut size: u32 = 0;
static mut icon_size: u32 = 0;
static mut icon_position: i32 = 0;
static mut circle_size: u32 = 0;
static mut circle_position: i32 = 0;

pub struct Button {
  owner: NonNull<Client>,
  icon: &'static mut SvgResource,
  base_color: Color,
  hovered_color: Color,
  action: unsafe fn(&mut Client),
  pub window: Window,
}

impl Button {
  unsafe fn new(
    owner: &mut Client,
    icon: &'static mut SvgResource,
    base_color: Color,
    hovered_color: Color,
    action: unsafe fn(&mut Client),
  ) -> Self {
    let button_size = decorated_frame_offset.y as u32;
    let window = Window::builder(&display)
      .parent(owner.frame)
      .size(button_size, button_size)
      .attributes(|attributes| {
        attributes
          .override_redirect(true)
          .event_mask(ButtonPressMask | ButtonReleaseMask | EnterWindowMask | LeaveWindowMask)
          .background_pixmap(XNone)
          .save_under(false)
          .backing_store(NotUseful);
      })
      .build();
    window.save_context(wm_context, owner as *mut Client as XPointer);
    set_window_kind(window, WindowKind::FrameButton);
    Self {
      owner: NonNull::new_unchecked(owner as *mut Client),
      icon,
      base_color,
      hovered_color,
      action,
      window,
    }
  }

  pub unsafe fn draw(&mut self, hovered: bool) {
    let color = if (*config).circle_buttons {
      self.hovered_color.scale(0.3)
    } else if hovered {
      self.hovered_color
    } else {
      self.base_color
    };

    // Redraw window border below button
    let below = *self.owner.as_ref().border_color;
    (*draw)
      .square(0, 0, size)
      .vertical_gradient(below.scale(Client::TITLE_BAR_GRADIENT_FACTOR), below)
      .draw();

    // Draw circle
    if (*config).circle_buttons {
      let border_color = self.owner.as_ref().border_color.pixel;
      let is_focused = border_color == (*config).colors.focused.pixel
        || border_color == (*config).colors.selected.pixel;
      let color = if hovered || is_focused {
        self.hovered_color
      } else {
        self.base_color
      };
      let outline_color = color.scale(0.9);
      (*draw)
        .shape(
          Shape::Ellipse,
          Geometry::from_parts(circle_position, circle_position, circle_size, circle_size),
        )
        .color(color)
        .stroke(1, outline_color)
        .draw();
    }

    // Draw icon or fallback
    if !(*config).circle_buttons || hovered {
      if resources::close_button.is_some() {
        (*draw).draw_colored_svg(
          self.icon,
          color,
          icon_position,
          icon_position,
          icon_size,
          icon_size,
        );
      } else {
        (*draw)
          .shape(
            Shape::Ellipse,
            Geometry::from_parts(icon_position, icon_position, icon_size, icon_size),
          )
          .color(color)
          .draw();
      }
    }

    (*draw).render(self.window, 0, 0, size, size);
  }

  pub unsafe fn move_(&self, index: i32, left: bool) {
    let x = if left {
      decorated_frame_offset.y * index
    } else {
      let width = self.owner.as_ref().frame_geometry().w;
      width as i32 - decorated_frame_offset.y * (index + 1)
    };
    self.window.r#move(x, 0);
  }

  pub unsafe fn click(&mut self) {
    (self.action)(self.owner.as_mut());
  }
}

pub unsafe fn close_button(owner: &mut Client) -> Button {
  Button::new(
    owner,
    &mut resources::close_button,
    (*config).colors.close_button,
    (*config).colors.close_button_hovered,
    action::close_client,
  )
}

pub unsafe fn maximize_button(owner: &mut Client) -> Button {
  Button::new(
    owner,
    &mut resources::maximize_button,
    (*config).colors.maximize_button,
    (*config).colors.maximize_button_hovered,
    action::toggle_maximized,
  )
}

pub unsafe fn minimize_button(owner: &mut Client) -> Button {
  Button::new(
    owner,
    &mut resources::minimize_button,
    (*config).colors.minimize_button,
    (*config).colors.minimize_button_hovered,
    action::minimize,
  )
}

pub unsafe fn from_string(owner: &mut Client, name: &str) -> Button {
  match name {
    "close" => close_button(owner),
    "maximize" => maximize_button(owner),
    "minimize" => minimize_button(owner),
    _ => {
      my_panic!("Invalid button name");
    }
  }
}

pub unsafe fn set_size(title_bar_height: u32) {
  size = title_bar_height;
  if (*config).circle_buttons {
    let size_ = size as f64;
    let circle_diameter = size_ * ((*config).button_icon_size as f64 / 100.0);
    // Square insize the circle where all 4 corners youch the circle.
    let icon_size_ = 2.0 * f64::sqrt(f64::powi(circle_diameter / 2.0, 2) / 2.0);
    circle_size = circle_diameter.round() as u32;
    circle_position = ((size_ - circle_diameter) / 2.0).round() as i32;
    icon_size = icon_size_.ceil() as u32;
    icon_position = ((size_ - icon_size_) / 2.0).round() as i32;
  } else {
    icon_size = size * (*config).button_icon_size as u32 / 100;
    icon_position = (size - icon_size) as i32 / 2;
  }
}
