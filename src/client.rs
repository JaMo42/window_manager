use super::buttons::Button;
use super::core::*;
use super::geometry::*;
use super::property::{Motif_Hints, MWM_HINTS_DECORATIONS, WM};
use super::*;

pub static mut decorated_frame_offset: Geometry = Geometry::new ();
pub static mut border_frame_offset: Geometry = Geometry::new ();
static mut left_buttons_width: u32 = 0;
static mut right_buttons_width: u32 = 0;
static mut title_x: i32 = 0;
static mut icon_position: i32 = 0;
static mut icon_size: u32 = 0;

unsafe fn create_frame (g: Geometry) -> Window {
  let mut attributes: XSetWindowAttributes = uninitialized!();
  attributes.background_pixmap = X_NONE;
  attributes.cursor = cursor::normal;
  attributes.override_redirect = X_TRUE;
  attributes.event_mask = SubstructureRedirectMask;
  let screen = XDefaultScreen (display);

  XCreateWindow (
    display,
    root,
    g.x,
    g.y,
    g.w,
    g.h,
    0,
    XDefaultDepth (display, screen),
    InputOutput as u32,
    XDefaultVisual (display, screen),
    CWBackPixmap | CWEventMask | CWCursor,
    &mut attributes,
  )
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
  geometry: Geometry,
  prev_geometry: Geometry,
  title: String,
  left_buttons: Vec<Button>,
  right_buttons: Vec<Button>,
  title_space: i32,
  frame_kind: Frame_Kind,
  icon: Option<Box<draw::Svg_Resource>>,
}

impl Client {
  pub const TITLE_BAR_GRADIENT_FACTOR: f64 = 1.185;
  pub const ICON_TITLE_GAP: i32 = 2;

  pub unsafe fn new (window: Window) -> Box<Self> {
    let geometry = get_window_geometry (window);

    let mut attributes: XSetWindowAttributes = uninitialized!();
    attributes.event_mask = StructureNotifyMask | PropertyChangeMask;
    attributes.do_not_propagate_mask = ButtonPressMask | ButtonReleaseMask;
    XChangeWindowAttributes (
      display,
      window,
      CWEventMask | CWDontPropagate,
      &mut attributes,
    );
    XSetWindowBorderWidth (display, window, 0);

    let mut frame_kind = Frame_Kind::Decorated;
    let mut is_dialog = false;

    if let Some (motif_hints) = Motif_Hints::get (window) {
      // Assume that is has its own title bar if it specifies any decorations
      if motif_hints.flags & MWM_HINTS_DECORATIONS == MWM_HINTS_DECORATIONS {
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
    XReparentWindow (display, window, frame, reparent_x, reparent_y);

    let icon = if (*config).window_icon_size > 0 && frame_kind.should_draw_decorations () {
      property::Class_Hints::new (window).and_then (|h| draw::get_app_icon (&h.name))
    } else {
      None
    };

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
      geometry,
      prev_geometry: geometry,
      title: window_title (window),
      left_buttons: Vec::new (),
      right_buttons: Vec::new (),
      title_space: 0,
      frame_kind,
      icon,
    });
    let this = result.as_mut () as *mut Client as XPointer;
    XSaveContext (display, window, wm_context, this);
    XSaveContext (display, frame, wm_context, this);
    set_window_kind (window, Window_Kind::Client);
    set_window_kind (frame, Window_Kind::Frame);

    ewmh::set_allowed_actions (window, !is_dialog);

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

    XMapSubwindows (display, frame);

    result
  }

  pub unsafe fn dummy (window: Window) -> Self {
    Client {
      window,
      frame: X_NONE,
      workspace: 0,
      snap_state: 0,
      is_urgent: false,
      is_fullscreen: false,
      is_dialog: false,
      is_minimized: false,
      border_color: &*(1 as *const color::Color),
      geometry: uninitialized!(),
      prev_geometry: uninitialized!(),
      title: String::new (),
      left_buttons: Vec::new (),
      right_buttons: Vec::new (),
      title_space: 0,
      frame_kind: Frame_Kind::Decorated,
      icon: None,
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

  pub unsafe fn map (&mut self) {
    XMapWindow (display, self.frame);
  }

  pub unsafe fn unmap (&self) {
    XUnmapWindow (display, self.frame);
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
        .color ((*config).colors.bar_active_workspace_text)
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
  pub fn modify_saved_geometry (&mut self, f: fn(&mut Geometry)) {
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
    XMoveResizeWindow (display, self.frame, fx, fy, fw, fh);
    for i in 0..self.left_buttons.len () {
      self.left_buttons[i].move_ (i as i32, true);
    }
    for i in 0..self.right_buttons.len () {
      self.right_buttons[i].move_ (i as i32, false);
    }
    XResizeWindow (display, self.window, cw, ch);
    self.configure ();
    self.draw_border ();
    XSync (display, X_FALSE);
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
      XRaiseWindow (display, self.window);
    } else {
      self.set_border (&(*config).colors.focused);
      XRaiseWindow (display, self.frame);
    }
    XSetInputFocus (display, self.window, RevertToParent, CurrentTime);
    self.send_event (property::atom (WM::TakeFocus));
    property::set (root, Net::ActiveWindow, XA_WINDOW, 32, &self.window, 1);
    XSync (display, X_FALSE);
  }

  pub unsafe fn raise (&self) {
    if self.is_fullscreen {
      XRaiseWindow (display, self.window);
    } else {
      XRaiseWindow (display, self.frame);
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
    let hints = XGetWMHints (display, self.window);
    if !hints.is_null () {
      (*hints).flags = if urgency {
        (*hints).flags | XUrgencyHint
      } else {
        (*hints).flags & !XUrgencyHint
      };
      XSetWMHints (display, self.window, hints);
      XFree (hints as *mut c_void);
    }
    bar.invalidate_widgets ();
    bar.draw ();
  }

  pub unsafe fn update_hints (&mut self) {
    let hints = XGetWMHints (display, self.window);
    if !hints.is_null () {
      if let Some (focused) = focused_client!() {
        if *focused == *self && ((*hints).flags & XUrgencyHint) != 0 {
          // It's being made urgent but it's already the active window
          (*hints).flags &= !XUrgencyHint;
          XSetWMHints (display, self.window, hints);
        }
      } else {
        self.is_urgent = ((*hints).flags & XUrgencyHint) != 0;
      }
      XFree (hints as *mut c_void);
    }
  }

  pub unsafe fn send_event (&self, protocol: Atom) -> bool {
    let mut protocols: *mut Atom = std::ptr::null_mut ();
    let mut is_supported = false;
    let mut count: c_int = 0;
    if XGetWMProtocols (display, self.window, &mut protocols, &mut count) != 0 {
      for i in 0..count {
        is_supported = *protocols.add (i as usize) == protocol;
        if is_supported {
          break;
        }
      }
      XFree (protocols as *mut c_void);
    }
    if is_supported {
      let mut event: XEvent = uninitialized!();
      event.type_ = ClientMessage;
      event.client_message.window = self.window;
      event.client_message.message_type = property::atom (WM::Protocols);
      event.client_message.format = 32;
      event.client_message.data.set_long (0, protocol as i64);
      event.client_message.data.set_long (1, CurrentTime as i64);
      XSendEvent (display, self.window, X_FALSE, NoEventMask, &mut event) != 0
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
      self.snap_state = SNAP_NONE;
      XReparentWindow (display, self.window, root, 0, 0);
      XResizeWindow (display, self.window, screen_size.w, screen_size.h);
      XRaiseWindow (display, self.window);
      XSetInputFocus (display, self.window, RevertToNone, CurrentTime);
      ewmh::set_net_wm_state (self, &[property::atom (Net::WMStateFullscreen)]);
    } else {
      let (reparent_x, reparent_y) = self.frame_kind.parent_offset ();
      XReparentWindow (display, self.window, self.frame, reparent_x, reparent_y);
      self.move_and_resize (Client_Geometry::Frame (self.prev_geometry));
      self.focus ();
      ewmh::set_net_wm_state (self, &[]);
    }
  }

  pub unsafe fn configure (&self) {
    let g = self.client_geometry ();
    let mut ev: XConfigureEvent = uninitialized!();
    ev.type_ = ConfigureNotify;
    ev.display = display;
    ev.event = self.window;
    ev.window = self.window;
    ev.x = g.x;
    ev.x = g.x;
    ev.width = g.w as i32;
    ev.height = g.h as i32;
    ev.border_width = 0;
    ev.above = X_NONE;
    ev.override_redirect = X_FALSE;
    XSendEvent (
      display,
      self.window,
      X_FALSE,
      StructureNotifyMask,
      &mut ev as *mut XConfigureEvent as *mut XEvent,
    );
  }

  pub unsafe fn click (&mut self, window: Window) {
    for b in self.buttons_mut () {
      if b.window == window {
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
    XDeleteContext (display, self.window, wm_context);
    for b in self.buttons () {
      XDeleteContext (display, b.window, wm_context);
    }
    XDeleteContext (display, self.frame, wm_context);
    XSelectInput (display, self.frame, X_NONE as i64);
    XDestroyWindow (display, self.frame);
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
  icon_size = decorated_frame_offset.y as u32 * (*config).window_icon_size as u32 / 100;
  icon_position = (decorated_frame_offset.y - icon_size as i32) / 2;
}
