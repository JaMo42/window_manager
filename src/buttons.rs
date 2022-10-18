use super::core::*;
use std::ptr::NonNull;
use libc::c_uint;
use x11::xlib::*;
use super::draw::{resources, Svg_Resource};
use super::client::{Client, frame_offset};
use super::action;
use super::color::Color;

static mut size: u32 = 0;
static mut icon_size: u32 = 0;
static mut icon_position: i32 = 0;
static mut circle_size: u32 = 0;
static mut circle_position: i32 = 0;


pub struct Button {
  owner: NonNull<Client>,
  icon: &'static mut Svg_Resource,
  base_color: Color,
  hovered_color: Color,
  action: unsafe fn (&mut Client),
  pub window: Window,
}

impl Button {
  unsafe fn new (
    owner: &mut Client,
    icon: &'static mut Svg_Resource,
    base_color: Color,
    hovered_color: Color,
    action: unsafe fn (&mut Client)
  ) -> Self {
    let button_size = frame_offset.y as u32;
    let mut attributes: XSetWindowAttributes = uninitialized! ();
    attributes.override_redirect = X_TRUE;
    attributes.event_mask = ButtonPressMask|ButtonReleaseMask|EnterWindowMask|LeaveWindowMask;
    attributes.background_pixmap = X_NONE;
    attributes.save_under = X_FALSE;
    attributes.backing_store = NotUseful;
    let window = XCreateWindow (
      display,
      owner.frame,
      0,
      0,
      button_size,
      button_size,
      0,
      CopyFromParent,
      InputOutput as c_uint,
      CopyFromParent as *mut Visual,
      CWEventMask|CWOverrideRedirect|CWBackPixmap|CWSaveUnder|CWBackingStore,
      &mut attributes
    );
    XSaveContext (display, window, wm_context, owner as *mut Client as XPointer);
    Self {
      owner: NonNull::new_unchecked (owner as *mut Client),
      icon,
      base_color,
      hovered_color,
      action,
      window
    }
  }

  pub unsafe fn draw (&mut self, hovered: bool) {
    let color = if (*config).circle_buttons {
      self.hovered_color.scale (0.3)
    } else {
      if hovered {
        self.hovered_color
      } else {
        self.base_color
      }
    };
    let below = *self.owner.as_ref ().border_color;
    (*draw).gradient (
      0,
      0,
      size,
      size,
      below.scale (Client::TITLE_BAR_GRADIENT_FACTOR),
      below
    );

    if (*config).circle_buttons {
      let border_color = self.owner.as_ref ().border_color.pixel;
      let is_focused = border_color == (*config).colors.focused.pixel
        || border_color == (*config).colors.selected.pixel;
      (*draw).ellipse_outline (
        circle_position, circle_position,
        circle_size, circle_size,
        if hovered || is_focused {
          self.hovered_color
        } else {
          self.base_color
        },
        1.0,
        0.9
      );
    }

    if !(*config).circle_buttons || hovered {
      if resources::close_button.is_some () {
        (*draw).draw_colored_svg (
          self.icon,
          color,
          icon_position, icon_position,
          icon_size, icon_size
        );
      }
      else {
        (*draw).ellipse (
          icon_position, icon_position,
          icon_size, icon_size,
          color
        );
      }
    }

    (*draw).render (self.window, 0, 0, size, size);
  }

  pub unsafe fn move_ (&self, index: i32, left: bool) {
    let x = if left {
      frame_offset.y * index
    } else {
      let width = self.owner.as_ref ().geometry.get_frame (&frame_offset).w;
      width as i32 - frame_offset.y * (index + 1)
    };
    XMoveWindow (display, self.window, x, 0);
  }

  pub unsafe fn click (&mut self) {
    (self.action) (self.owner.as_mut());
  }
}


pub unsafe fn close_button (owner: &mut Client) -> Button {
  Button::new (
    owner,
    &mut resources::close_button,
    (*config).colors.close_button,
    (*config).colors.close_button_hovered,
    action::close_client
  )
}


pub unsafe fn maximize_button (owner: &mut Client) -> Button {
  Button::new (
    owner,
    &mut resources::maximize_button,
    (*config).colors.maximize_button,
    (*config).colors.maximize_button_hovered,
    action::toggle_maximized
  )
}


pub unsafe fn minimize_button (owner: &mut Client) -> Button {
  Button::new (
    owner,
    &mut resources::minimize_button,
    (*config).colors.minimize_button,
    (*config).colors.minimize_button_hovered,
    action::minimize
  )
}


pub unsafe fn from_string (owner: &mut Client, name: &str) -> Button {
  match name {
    "close" => close_button (owner),
    "maximize" => maximize_button (owner),
    "minimize" => minimize_button (owner),
    _ => {
      my_panic! ("Invalid button name");
    }
  }
}


pub unsafe fn set_size (title_bar_height: u32) {
  size = title_bar_height;
  if (*config).circle_buttons {
    let s = size as f64;
    let c =  s * ((*config).button_icon_size as f64 / 100.0);
    let i = 2.0 * f64::sqrt (f64::powi (c / 2.0, 2) / 2.0);
    circle_size = c.round () as u32;
    circle_position = ((s - c) / 2.0).round () as i32;
    icon_size = i.ceil () as u32;
    icon_position = ((s - i) / 2.0).round () as i32;
  }
  else {
    icon_size = size * (*config).button_icon_size as u32 / 100;
    icon_position = (size - icon_size) as i32 / 2;
  }
}
