use super::core::*;
use super::ewmh;
use super::geometry::Geometry;
use super::property::Net;
use super::set_window_kind;
use crate::draw::Alignment;
use crate::draw::Svg_Resource;
use crate::x::{lookup_keysym, Window};
use x11::xlib::*;

static mut shown: Option<Context_Menu> = None;
static mut mouse_on_shown: bool = false;

pub enum Indicator {
  Check,
  Diamond,
  Circle,
  Exclamation,
}

impl Indicator {
  fn symbol (&self) -> &str {
    match self {
      &Self::Check => "✔",
      &Self::Diamond => "♦",
      &Self::Circle => "⚫",
      &Self::Exclamation => "❗",
    }
  }

  fn width () -> u32 {
    use std::sync::Once;
    static once_flag: Once = Once::new ();
    static mut result: u32 = 0;
    once_flag.call_once (|| unsafe {
      (*draw).select_font (&(*config).bar_font);
      // Should be widest a normal character can be
      result = (*draw).text ("가").get_width ();
    });
    unsafe { result }
  }
}

pub struct Action {
  name: String,
  index: usize,
  icon: Option<&'static mut Svg_Resource>,
  indicator: Option<Indicator>,
}

impl Action {
  fn new (name: String, index: usize) -> Self {
    Self {
      name,
      index,
      icon: None,
      indicator: None,
    }
  }

  pub fn icon (&mut self, icon: Option<&'static mut Svg_Resource>) -> &mut Self {
    self.icon = icon;
    self
  }

  pub fn indicator (&mut self, indicator: Option<Indicator>) -> &mut Self {
    self.indicator = indicator;
    self
  }
}

enum Item {
  Action (Action),
  Divider,
}

impl Item {
  fn unwrap_action (&mut self) -> &mut Action {
    match self {
      Self::Action (ref mut action) => action,
      _ => panic! ("Item::unwrap_action on non-action item"),
    }
  }
}

pub struct Context_Menu {
  items: Vec<Item>,
  // Y position of the lower edge of items
  item_positions: Vec<i32>,
  select: Box<dyn FnMut(Option<usize>)>,
  window: Window,
  // Size of the window, with `x` and `y` being `0`.
  // The position of the window is stored in the `x` and `y` members.
  window_geometry: Geometry,
  min_width: u32,
  selected: Option<usize>,
  content_geometry: Geometry,
  action_count: usize,
  has_at_least_one_indicator: bool,
  always_show_indicator_column: bool,
  x: i32,
  y: i32,
}

impl Context_Menu {
  const PADDING: u32 = 12;
  const BUTTON_PADDING: u32 = 4;
  const DIVIDER_HEIGHT: u32 = 2;
  const DIVIDER_SPACE: u32 = 16;

  pub fn new (select: Box<dyn FnMut(Option<usize>)>) -> Self {
    Self {
      items: Vec::new (),
      item_positions: Vec::new (),
      select,
      window: Window::uninit (),
      window_geometry: Geometry::new (),
      min_width: 0,
      selected: None,
      content_geometry: Geometry::new (),
      action_count: 0,
      has_at_least_one_indicator: false,
      always_show_indicator_column: false,
      x: 0,
      y: 0,
    }
  }

  pub fn action (&mut self, name: String) -> &mut Action {
    self
      .items
      .push (Item::Action (Action::new (name, self.action_count)));
    self.action_count += 1;
    self.items.last_mut ().unwrap ().unwrap_action ()
  }

  pub fn divider (&mut self) -> &mut Self {
    self.items.push (Item::Divider);
    self
  }

  pub fn min_width (&mut self, width: u32) -> &mut Self {
    self.min_width = width;
    self
  }

  pub fn always_show_indicator_column (&mut self) -> &mut Self {
    self.always_show_indicator_column = true;
    self
  }

  pub unsafe fn build (&mut self) -> &mut Self {
    let window = Window::builder (&display)
      // Initiallay give it some large size then shrink if after drawing
      .position (0, 0)
      .size (screen_size.w, screen_size.h)
      .attributes (|attributes| {
        attributes.event_mask (
          ButtonPressMask | PointerMotionMask | EnterWindowMask | LeaveWindowMask | KeyPressMask,
        );
      })
      .build ();
    ewmh::set_window_type (window, Net::WMWindowTypePopupMenu);
    set_window_kind (window, Window_Kind::Context_Menu);
    self.window = window;
    let (width, height) = self.redraw ();
    self.window.resize (width, height);
    self.window_geometry.w = width;
    self.window_geometry.h = height;
    for i in self.items.iter () {
      if let Item::Action (action) = i {
        if action.indicator.is_some () {
          self.has_at_least_one_indicator = true;
          break;
        }
      }
    }
    self
  }

  unsafe fn text_width (&self, height: u32) -> u32 {
    let mut widest = 0;
    (*draw).select_font (&(*config).bar_font);
    for item in self.items.iter () {
      if let Item::Action (action) = item {
        let width = (*draw).text (&action.name).get_width ()
          + if action.icon.is_some () { height } else { 0 };
        widest = u32::max (widest, width);
      }
    }
    widest
  }

  unsafe fn redraw (&mut self) -> (u32, u32) {
    let show_indicator_column =
      self.has_at_least_one_indicator || self.always_show_indicator_column;
    let indicator_width = Indicator::width ();
    let action_height = (*draw).font_height (None);
    let content_width = u32::max (
      self.text_width (action_height) + (2 * indicator_width * show_indicator_column as u32),
      self.min_width,
    );
    let mut y = Self::PADDING as i32;
    let x = Self::PADDING as i32;
    let icon_size = action_height * 90 / 100;
    let icon_position = (action_height - icon_size) as i32 / 2;
    let text_x = if show_indicator_column {
      x + indicator_width as i32
    } else {
      x
    };
    (*draw)
      .rect (0, 0, screen_size.w, screen_size.h)
      .color ((*config).colors.bar_background)
      .draw ();
    let mut action_index = 0;
    let selected = self.selected.unwrap_or (usize::MAX);
    self.item_positions.clear ();
    for item in self.items.iter_mut () {
      let h;
      match item {
        Item::Action (action) => {
          let text_color = if action_index == selected {
            (*draw)
              .rect (x - 2, y - 2, content_width + 4, action_height + 4)
              .corner_radius (0.1)
              .color ((*config).colors.bar_text)
              .draw ();
            (*config).colors.bar_background
          } else {
            (*config).colors.bar_text
          };
          if let Some (indicator) = &action.indicator {
            (*draw)
              .text (indicator.symbol ())
              .at (x, y)
              .color (text_color)
              .align_vertically (Alignment::Centered, action_height as i32)
              .align_horizontally (Alignment::Centered, indicator_width as i32)
              .draw ();
          }
          let mut text_x = text_x;
          if let Some (icon) = &action.icon {
            (*draw).draw_svg (
              icon,
              text_x - icon_position,
              y + icon_position,
              icon_size,
              icon_size,
            );
            text_x += action_height as i32;
          }
          (*draw)
            .text (&action.name)
            .at (text_x, y)
            .color (text_color)
            .align_vertically (crate::draw::Alignment::Centered, action_height as i32)
            .draw ();
          h = action_height + 2 * Self::BUTTON_PADDING;
          action_index += 1;
        }
        Item::Divider => {
          let y = y + (Self::DIVIDER_SPACE - Self::DIVIDER_HEIGHT) as i32 / 2;
          (*draw)
            .rect (x, y, content_width, Self::DIVIDER_HEIGHT)
            .color ((*config).colors.context_menu_divider)
            .draw ();
          h = Self::DIVIDER_SPACE;
        }
      }
      y += h as i32;
      self.item_positions.push (y);
    }
    self.content_geometry = Geometry::from_parts (
      Self::PADDING as i32,
      Self::PADDING as i32,
      content_width,
      y as u32 - Self::PADDING,
    );
    let width = content_width + 2 * Self::PADDING;
    let height = y + Self::PADDING as i32;
    (width, height as u32)
  }

  /// `x` and `y` are currently the coordinates of the center of the bottom
  /// edge as that's what's convenient for the dock.
  pub unsafe fn show_at (mut self, x: i32, y: i32) {
    close_shown ();
    self.x = x - self.window_geometry.w as i32 / 2;
    self.y = y - self.window_geometry.h as i32;
    self.window.r#move (self.x, self.y);
    self.window.map_raised ();
    let (width, height) = self.redraw ();
    (*draw).render (self.window, 0, 0, width, height);
    display.set_input_focus (self.window);
    display.grab_button (1, 0);
    shown = Some (self);
  }

  pub fn destroy (&self) {
    self.window.destroy ();
  }

  pub unsafe fn cancel (&mut self) {
    self.select.as_mut () (None);
  }

  fn item_index_at (&self, x: i32, y: i32) -> Option<usize> {
    if !self.content_geometry.contains (x, y) {
      None
    } else {
      let i = self
        .item_positions
        .partition_point (|button_y| *button_y < y);
      match &self.items[i] {
        Item::Action (action) => Some (action.index),
        Item::Divider => None,
      }
    }
  }

  pub unsafe fn click (&mut self, event: &XButtonEvent) {
    let x = event.x_root - self.x;
    let y = event.y_root - self.y;
    // Ignore clicks on dividers
    if let Some (index) = self.item_index_at (x, y) {
      self.select.as_mut () (Some (index));
      close_shown ();
    }
  }

  pub unsafe fn motion (&mut self, event: &XMotionEvent) {
    let before = self.selected;
    self.selected = self.item_index_at (event.x, event.y);
    if self
      .selected
      .zip (before)
      .map (|(s, b)| s != b)
      .unwrap_or (true)
    {
      let (width, height) = self.redraw ();
      (*draw).render (self.window, 0, 0, width, height);
    }
  }

  pub fn key_up (&mut self) {
    self.selected = Some (if let Some (selected) = self.selected {
      if selected == 0 {
        self.action_count - 1
      } else {
        selected - 1
      }
    } else {
      self.action_count - 1
    });
  }

  pub fn key_down (&mut self) {
    self.selected = Some (if let Some (selected) = self.selected {
      if selected == self.action_count - 1 {
        0
      } else {
        selected + 1
      }
    } else {
      0
    });
  }

  pub fn key_select (&mut self) {
    self.select.as_mut () (self.selected);
    unsafe {
      close_shown ();
    }
  }
}

pub unsafe fn close_shown () {
  shown.take ().map (|menu| {
    menu.destroy ();
    display.ungrab_button (1, 0);
    focused_client! ().map (|client| client.focus ());
  });
}

pub unsafe fn click (event: &XButtonEvent) -> bool {
  if let Some (menu) = shown.as_mut () {
    if mouse_on_shown {
      menu.click (event);
    } else {
      shown.as_mut ().map (|menu| menu.cancel ());
      close_shown ();
    }
    true
  } else {
    false
  }
}

pub unsafe fn motion (event: &XMotionEvent) {
  if let Some (menu) = shown.as_mut () {
    menu.motion (event);
  }
}

pub unsafe fn cross (event: &XCrossingEvent) {
  // Since we handle button presses on the root window we get leave and enter
  // events before and after a button press so we need to check if leave events
  // actually left the window. For enter events this does not matter.
  if event.type_ == EnterNotify {
    mouse_on_shown = true;
    raise ();
  } else if shown
    .as_ref ()
    .map (|menu| !menu.window_geometry.contains (event.x, event.y))
    .unwrap_or (false)
  {
    mouse_on_shown = false;
    shown.as_mut ().map (|menu| menu.selected = None);
  }
}

pub unsafe fn key_press (event: &XKeyEvent) {
  use x11::keysym::*;
  match lookup_keysym (event) as u32 {
    XK_Up | XK_k => {
      shown.as_mut ().unwrap ().key_up ();
    }
    XK_Down | XK_j => {
      shown.as_mut ().unwrap ().key_down ();
    }
    XK_Return | XK_space => {
      shown.as_mut ().unwrap ().key_select ();
    }
    _ => {
      // TODO: should key be put back into event queue?
      shown.as_mut ().map (|menu| menu.cancel ());
      close_shown ();
      return;
    }
  }
  shown.as_mut ().map (|menu| {
    let (width, height) = menu.redraw ();
    (*draw).render (menu.window, 0, 0, width, height);
  });
}

pub fn raise () {
  unsafe {
    shown.as_ref ().map (|menu| menu.window.raise ());
  }
}
