use super::buttons::Button;
use super::core::*;
use super::geometry::*;
use super::property::{Class_Hints, Motif_Hints, MWM_HINTS_DECORATIONS, WM};
use super::*;
use crate::action;
use crate::desktop_entry::Desktop_Entry;
use crate::x::*;

pub static mut decorated_frame_offset: Geometry = Geometry::new ();
pub static mut border_frame_offset: Geometry = Geometry::new ();
static mut left_buttons_width: u32 = 0;
static mut right_buttons_width: u32 = 0;
static mut title_x: i32 = 0;
static mut icon_position: i32 = 0;
static mut icon_size: u32 = 0;

unsafe fn create_frame (g: Geometry) -> Window {
  Window::builder (&display)
    .position (g.x, g.y)
    .size (g.w, g.h)
    .attributes (|attributes| {
      attributes
        .background_pixmap (XNone)
        .cursor (cursor::normal)
        .override_redirect (true)
        .event_mask (SubstructureRedirectMask);
    })
    .build ()
}

/// Specifies how to change the frame and client geometry when resizing a client
pub enum Client_Geometry {
  /// Set the size of the frame (outer window)
  Frame (Geometry),
  /// Set the size of the frame for snapping (applies gaps)
  Snap (Geometry),
  /// Set the size of the client (inner window)
  Client (Geometry),
}

#[derive(Copy, Clone)]
pub enum Frame_Kind {
  // Frame with title bar and buttons, used for normal clients
  Decorated,
  // Frame with only a border, used for popups
  Border,
  // No visible frame, used for windows with a custom title bar
  None,
}

impl Frame_Kind {
  fn get_frame (&self, mut client_geometry: Geometry) -> Geometry {
    match self {
      Frame_Kind::Decorated => unsafe { client_geometry.get_frame () },
      Frame_Kind::Border => *client_geometry.expand (unsafe { border_frame_offset.x }),
      Frame_Kind::None => client_geometry,
    }
  }

  fn get_client (&self, mut frame_geometry: Geometry) -> Geometry {
    match self {
      Frame_Kind::Decorated => unsafe { frame_geometry.get_client () },
      Frame_Kind::Border => *frame_geometry.expand (0 - unsafe { border_frame_offset.x }),
      Frame_Kind::None => frame_geometry,
    }
  }

  /// Should decorations be drawn on this kind of frame?
  fn should_draw_decorations (&self) -> bool {
    matches! (self, Frame_Kind::Decorated)
  }

  /// Should this kind of frame be drawn at all?
  fn should_draw_border (&self) -> bool {
    !matches! (self, Frame_Kind::None)
  }

  fn parent_offset (&self) -> (i32, i32) {
    match self {
      Frame_Kind::Decorated => unsafe { (decorated_frame_offset.x, decorated_frame_offset.y) },
      Frame_Kind::Border => unsafe { (border_frame_offset.x, border_frame_offset.y) },
      Frame_Kind::None => (0, 0),
    }
  }

  fn frame_offset (&self) -> &'static Geometry {
    static no_offset: Geometry = Geometry::new ();
    match self {
      Frame_Kind::Decorated => unsafe { &decorated_frame_offset },
      Frame_Kind::Border => unsafe { &border_frame_offset },
      Frame_Kind::None => &no_offset,
    }
  }
}

pub struct Client {
  pub window: Window,
  pub frame: Window,
  pub workspace: usize,
  pub snap_state: u8,
  pub is_urgent: bool,
  pub is_fullscreen: bool,
  pub is_dialog: bool,
  pub is_minimized: bool,
  pub border_color: &'static color::Color,
  text_color: &'static color::Color,
  geometry: Geometry,
  prev_geometry: Geometry,
  title: String,
  left_buttons: Vec<Button>,
  right_buttons: Vec<Button>,
  title_space: i32,
  frame_kind: Frame_Kind,
  icon: Option<Box<draw::Svg_Resource>>,
  application_id: String,
  last_click_time: Time,
}

impl Client {
  pub const TITLE_BAR_GRADIENT_FACTOR: f64 = 1.185;
  pub const ICON_TITLE_GAP: i32 = 2;

  pub unsafe fn new (window: Window) -> Box<Self> {
    let geometry = get_window_geometry (window);
    let mut class_hint = Class_Hints::new (window);

    if let Some (h) = &mut class_hint {
      if h.name == "Mail" && h.class == "thunderbird-default" {
        h.class = "thunderbird".to_string ();
      } else if h.name == "Navigator" && h.class == "Firefox-esr" {
        h.class = "firefox-esr".to_string ();
      }
    }

    window.change_attributes (|attributes| {
      attributes
        .event_mask (StructureNotifyMask | PropertyChangeMask)
        .do_not_propagate_mask (ButtonPressMask | ButtonReleaseMask);
    });
    window.set_border_width (0);

    let mut frame_kind = Frame_Kind::Decorated;
    let mut is_dialog = false;

    if let Some (motif_hints) = Motif_Hints::get (window) {
      if motif_hints.flags & MWM_HINTS_DECORATIONS == MWM_HINTS_DECORATIONS
        && motif_hints.decorations == 0
      {
        frame_kind = Frame_Kind::None;
      }
    } else if property::get_atom (window, Net::WMWindowType)
      == property::atom (Net::WMWindowTypeDialog)
    {
      is_dialog = true;
      frame_kind = Frame_Kind::Border;
    }

    let frame = create_frame (frame_kind.get_frame (geometry));
    let (reparent_x, reparent_y) = frame_kind.parent_offset ();
    window.reparent (frame, reparent_x, reparent_y);

    // Get the application id (first one that matches):
    // 1. Check if the window has _GTK_APPLICATION_ID set and a desktop entry
    //    for the ID exists.
    // If the window has class hints:
    //   2. Check if a desktop entry exists for the name
    //   3. Check if a desktop entry exists for the class
    //   4. Use the name
    // 5. Use the window title
    let application_id = property::get_string (window, property::Other::GtkApplicationId)
      .filter (|gtk_id| Desktop_Entry::entry_name (gtk_id).is_some ())
      .or_else (|| {
        class_hint
          .as_ref ()
          .and_then (|h| Desktop_Entry::entry_name (&h.name))
      })
      .or_else (|| {
        class_hint
          .as_ref ()
          .and_then (|h| Desktop_Entry::entry_name (&h.class))
      })
      .or_else (|| class_hint.as_ref ().map (|h| h.name.clone ()))
      .unwrap_or_else (|| window_title (window));

    let icon = draw::get_app_icon (&application_id);

    let mut result = Box::new (Client {
      window,
      frame,
      workspace: active_workspace,
      snap_state: 0,
      is_urgent: false,
      is_fullscreen: false,
      is_dialog,
      is_minimized: false,
      border_color: &(*config).colors.normal,
      text_color: &(*config).colors.normal_text,
      geometry,
      prev_geometry: geometry,
      title: window_title (window),
      left_buttons: Vec::new (),
      right_buttons: Vec::new (),
      title_space: 0,
      frame_kind,
      icon,
      application_id,
      last_click_time: 0,
    });
    let this = result.as_mut () as *mut Client as XPointer;
    window.save_context (wm_context, this);
    frame.save_context (wm_context, this);
    set_window_kind (window, Window_Kind::Client);
    set_window_kind (frame, Window_Kind::Frame);

    ewmh::set_allowed_actions (window, !is_dialog);
    ewmh::set_frame_extents (window, frame_kind.frame_offset ());

    if result.frame_kind.should_draw_decorations () {
      let mut i = 0;
      for name in (*config).left_buttons.iter () {
        let b = buttons::from_string (&mut result, name);
        b.move_ (i, true);
        result.left_buttons.push (b);
        i += 1;
      }
      i = 0;
      // Reverse the iterator so the leftmost button in the config is the
      // leftmost button on the window
      for name in (*config).right_buttons.iter ().rev () {
        let b = buttons::from_string (&mut result, name);
        b.move_ (i, false);
        result.right_buttons.push (b);
        i += 1;
      }
    }

    frame.map_subwindows ();

    result
  }

  pub unsafe fn dummy (window: Window) -> Self {
    Client {
      window,
      frame: Window::uninit (),
      workspace: 0,
      snap_state: 0,
      is_urgent: false,
      is_fullscreen: false,
      is_dialog: false,
      is_minimized: false,
      border_color: &*(1 as *const color::Color),
      text_color: &*(1 as *const color::Color),
      geometry: uninitialized! (),
      prev_geometry: uninitialized! (),
      title: String::new (),
      left_buttons: Vec::new (),
      right_buttons: Vec::new (),
      title_space: 0,
      frame_kind: Frame_Kind::Decorated,
      icon: None,
      application_id: String::new (),
      last_click_time: 0,
    }
  }

  pub fn is_snapped (&self) -> bool {
    self.snap_state != SNAP_NONE
  }

  pub fn may_move (&self) -> bool {
    !self.is_fullscreen
  }

  pub fn may_resize (&self) -> bool {
    !(self.is_fullscreen || self.is_dialog)
  }

  pub fn icon (&mut self) -> Option<&'static mut draw::Svg_Resource> {
    self.icon.as_mut ().map (|icon| unsafe {
      let p: *mut draw::Svg_Resource = icon.as_mut () as *mut draw::Svg_Resource;
      &mut *p
    })
  }

  pub fn application_id (&self) -> &str {
    &self.application_id
  }

  pub unsafe fn map (&mut self) {
    self.frame.map ();
  }

  pub unsafe fn unmap (&self) {
    self.frame.unmap ();
  }

  pub unsafe fn draw_border (&mut self) {
    if self.frame_kind.should_draw_decorations () {
      let frame_size = self.frame_geometry ();
      let frame_offset = self.frame_kind.frame_offset ();
      let mut actual_title_x = title_x;

      (*draw).fill_rect (
        0,
        frame_offset.y,
        frame_size.w,
        frame_size.h - frame_offset.y as u32,
        *self.border_color,
      );
      (*draw)
        .rect (0, 0, frame_size.w, frame_offset.y as u32)
        .vertical_gradient (
          self.border_color.scale (Self::TITLE_BAR_GRADIENT_FACTOR),
          *self.border_color,
        )
        .draw ();

      if let Some (icon) = self.icon.as_mut () {
        (*draw).draw_svg (
          icon,
          title_x - Self::ICON_TITLE_GAP + icon_position,
          icon_position,
          icon_size,
          icon_size,
        );
        actual_title_x += frame_offset.y;
      }

      (*draw).select_font (&(*config).title_font);
      (*draw)
        .text (&self.title)
        .at (actual_title_x, 0)
        .align_vertically (draw::Alignment::Centered, frame_offset.y)
        .align_horizontally ((*config).title_alignment, self.title_space)
        .color (*self.text_color)
        .width (self.title_space)
        .draw ();

      let g = self.frame_geometry ();
      (*draw).render (self.frame, 0, 0, g.w, g.h);

      for b in self.buttons_mut () {
        b.draw (false);
      }
    } else if self.frame_kind.should_draw_border () {
      let g = self.frame_geometry ();
      (*draw).fill_rect (0, 0, g.w, g.h, *self.border_color);
      (*draw).render (self.frame, 0, 0, g.w, g.h);
    }
  }

  pub unsafe fn set_border (&mut self, color: &'static color::Color) {
    if self.frame_kind.should_draw_border () {
      self.border_color = color;
      if color.pixel == (*config).colors.normal.pixel {
        self.text_color = &(*config).colors.normal_text;
      } else if color.pixel == (*config).colors.focused.pixel {
        self.text_color = &(*config).colors.focused_text;
      } else if color.pixel == (*config).colors.urgent.pixel {
        self.text_color = &(*config).colors.urgent_text;
      } else if color.pixel == (*config).colors.selected.pixel {
        self.text_color = &(*config).colors.selected_text;
      }
      self.draw_border ();
    }
  }

  pub unsafe fn set_title (&mut self, title: &str) {
    if self.frame_kind.should_draw_decorations () {
      self.title.clear ();
      self.title.push_str (title);
      self.draw_border ();
    }
  }

  /// Rerturns the geometry of the frame window (outer window)
  pub fn frame_geometry (&self) -> Geometry {
    self.frame_kind.get_frame (self.geometry)
  }

  /// Returns the geometry of the client window (inner window)
  pub fn client_geometry (&self) -> Geometry {
    self.geometry
  }

  /// Stores the unsnapped geometry
  pub fn save_geometry (&mut self) {
    if self.is_snapped () {
      log::error! ("Client::save_geometry called while client is snapped");
    }
    self.prev_geometry = self.frame_geometry ();
  }

  /// Returns the frame geometry the client would have if it's not snapped
  pub fn saved_geometry (&self) -> Geometry {
    self.prev_geometry
  }

  /// Modify the saved frame geometry using a callback
  pub fn modify_saved_geometry<F> (&mut self, f: F)
  where
    F: FnOnce(&mut Geometry),
  {
    f (&mut self.prev_geometry);
  }

  pub unsafe fn move_and_resize (&mut self, geom: Client_Geometry) {
    let frame_offset = self.frame_kind.frame_offset ();
    let fx;
    let fy;
    let fw;
    let fh;
    let cw;
    let ch;
    match geom {
      Client_Geometry::Client (g) => {
        fx = g.x - frame_offset.x;
        fy = g.y - frame_offset.y;
        fw = g.w + frame_offset.w;
        fh = g.h + frame_offset.h;
        cw = g.w;
        ch = g.h;
      }
      Client_Geometry::Frame (g) => {
        fx = g.x;
        fy = g.y;
        fw = g.w;
        fh = g.h;
        cw = g.w - frame_offset.w;
        ch = g.h - frame_offset.h;
      }
      Client_Geometry::Snap (g) => {
        fx = g.x + (*config).gap as i32;
        fy = g.y + (*config).gap as i32;
        fw = g.w - 2 * (*config).gap;
        fh = g.h - 2 * (*config).gap;
        cw = fw - frame_offset.w;
        ch = fh - frame_offset.h;
      }
    }
    self.geometry = self
      .frame_kind
      .get_client (Geometry::from_parts (fx, fy, fw, fh));
    self.title_space =
      (self.client_geometry ().w - left_buttons_width - right_buttons_width) as i32;
    if self.icon.is_some () {
      self.title_space -= self.frame_kind.frame_offset ().y + Self::ICON_TITLE_GAP;
    }
    self.frame.move_and_resize (fx, fy, fw, fh);
    for i in 0..self.left_buttons.len () {
      self.left_buttons[i].move_ (i as i32, true);
    }
    for i in 0..self.right_buttons.len () {
      self.right_buttons[i].move_ (i as i32, false);
    }
    self.window.resize (cw, ch);
    self.configure ();
    self.draw_border ();
    display.sync (false);
  }

  pub unsafe fn unsnap (&mut self) {
    if self.is_snapped () {
      self.snap_state = SNAP_NONE;
      self.move_and_resize (Client_Geometry::Frame (self.prev_geometry));
      ewmh::set_net_wm_state (self, &[]);
    }
  }

  pub unsafe fn unminimize (&mut self, redraw: bool) {
    self.map ();
    self.is_minimized = false;
    ewmh::set_net_wm_state (self, &[]);
    if redraw {
      self.draw_border ();
    }
  }

  pub unsafe fn focus (&mut self) {
    if self.is_urgent {
      self.set_urgency (false);
    }
    if self.is_minimized {
      self.unminimize (false);
    }
    if self.is_fullscreen {
      self.window.raise ();
    } else {
      self.set_border (&(*config).colors.focused);
      self.frame.raise ();
    }
    display.set_input_focus (self.window);
    self.send_event (property::atom (WM::TakeFocus));
    property::set (root, Net::ActiveWindow, XA_WINDOW, 32, &self.window, 1);
    display.sync (false);
    dock::focus (self);
  }

  pub unsafe fn raise (&self) {
    if self.is_fullscreen {
      self.window.raise ();
    } else {
      self.frame.raise ();
    }
  }

  pub unsafe fn set_urgency (&mut self, urgency: bool) {
    if urgency == self.is_urgent {
      return;
    }
    self.is_urgent = urgency;
    if urgency {
      self.set_border (&(*config).colors.urgent);
    }
    let hints = self.window.get_wm_hints ();
    if !hints.is_null () {
      (*hints).flags = if urgency {
        (*hints).flags | XUrgencyHint
      } else {
        (*hints).flags & !XUrgencyHint
      };
      self.window.set_wm_hints (hints);
      XFree (hints as *mut c_void);
    }
    bar.invalidate_widgets ();
    bar.draw ();
    dock::update_urgency (self);
  }

  pub unsafe fn update_hints (&mut self) {
    let hints = self.window.get_wm_hints ();
    if !hints.is_null () {
      if let Some (focused) = focused_client! () {
        if *focused == *self && ((*hints).flags & XUrgencyHint) != 0 {
          // It's being made urgent but it's already the active window
          (*hints).flags &= !XUrgencyHint;
          self.window.set_wm_hints (hints);
        }
      } else {
        self.is_urgent = ((*hints).flags & XUrgencyHint) != 0;
      }
      XFree (hints as *mut c_void);
    }
  }

  pub unsafe fn send_event (&self, protocol: Atom) -> bool {
    let mut is_supported = false;
    for p in self.window.get_wm_protocols () {
      is_supported = p == protocol;
      if is_supported {
        break;
      }
    }
    if is_supported {
      self.window.send_client_message (|message| {
        message.message_type = property::atom (WM::Protocols);
        message.format = 32;
        message.data.set_long (0, protocol as i64);
        message.data.set_long (1, CurrentTime as i64);
      })
    } else {
      false
    }
  }

  pub unsafe fn set_fullscreen (&mut self, state: bool) {
    if state == self.is_fullscreen {
      return;
    }
    self.is_fullscreen = state;
    if state {
      self.window.reparent (root, 0, 0);
      self.frame.unmap ();
      let size = monitors::containing (self).geometry ();
      self.window.move_and_resize (size.x, size.y, size.w, size.h);
      self.window.raise ();
      display.set_input_focus (self.window);
      ewmh::set_net_wm_state (self, &[property::atom (Net::WMStateFullscreen)]);
    } else {
      let (reparent_x, reparent_y) = self.frame_kind.parent_offset ();
      self.frame.map ();
      self.window.reparent (self.frame, reparent_x, reparent_y);
      if self.snap_state != SNAP_NONE {
        action::snap (self, self.snap_state);
      } else {
        ewmh::set_net_wm_state (self, &[]);
        self.move_and_resize (Client_Geometry::Frame (self.prev_geometry));
      }
      self.focus ();
    }
    display.flush ();
  }

  pub unsafe fn configure (&self) {
    let g = self.client_geometry ();
    self.window.send_configure_event (|configure| {
      configure.x = g.x;
      configure.x = g.x;
      configure.width = g.w as i32;
      configure.height = g.h as i32;
      configure.border_width = 0;
      configure.above = XNone;
      configure.override_redirect = XFalse;
    });
  }

  pub unsafe fn click (&mut self, window: XWindow) {
    for b in self.buttons_mut () {
      if b.window.handle () == window {
        b.click ();
        return;
      }
    }
  }

  pub fn buttons (
    &self,
  ) -> std::iter::Chain<std::slice::Iter<'_, Button>, std::slice::Iter<'_, Button>> {
    self.left_buttons.iter ().chain (self.right_buttons.iter ())
  }

  pub fn buttons_mut (
    &mut self,
  ) -> std::iter::Chain<std::slice::IterMut<'_, Button>, std::slice::IterMut<'_, Button>> {
    self
      .left_buttons
      .iter_mut ()
      .chain (self.right_buttons.iter_mut ())
  }

  pub unsafe fn destroy (&self) {
    self.window.delete_context (wm_context);
    for b in self.buttons () {
      b.window.delete_context (wm_context);
    }
    self.frame.delete_context (wm_context);
    XSelectInput (display.as_raw (), self.frame.handle (), XNone as i64);
    self.frame.destroy ();
  }

  pub unsafe fn click_frame (&mut self, time: Time) -> bool {
    let d = time - self.last_click_time;
    self.last_click_time = time;
    if d < (*config).double_click_time && self.may_resize () {
      if self.is_snapped () {
        self.unsnap ();
      } else {
        action::snap (self, SNAP_MAXIMIZED);
      }
      true
    } else {
      false
    }
  }
}

impl PartialEq for Client {
  fn eq (&self, other: &Self) -> bool {
    self.window == other.window
  }
}

impl std::fmt::Display for Client {
  fn fmt (&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
    let title = unsafe { window_title (self.window) };
    write! (f, "'{}' ({})", title, self.window)
  }
}

pub unsafe fn set_border_info () {
  let title_height = (*config).title_height.get (Some (&(*config).title_font));
  let b = (*config).border_width;
  decorated_frame_offset = Geometry::from_parts (
    b,
    title_height as i32,
    2 * b as u32,
    title_height + b as u32,
  );
  border_frame_offset = Geometry::from_parts (b, b, 2 * b as u32, 2 * b as u32);
  left_buttons_width = (*config).left_buttons.len () as u32 * title_height;
  right_buttons_width = (*config).right_buttons.len () as u32 * title_height;
  title_x = left_buttons_width as i32 + b;
  buttons::set_size (title_height);
  icon_size = decorated_frame_offset.y as u32 * (*config).window_icon_size / 100;
  icon_position = (decorated_frame_offset.y - icon_size as i32) / 2;
}
