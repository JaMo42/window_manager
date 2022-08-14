use x11::xlib::*;
use super::core::*;
use super::geometry::*;
use super::*;
use super::property::WM;
use super::buttons::Button;

pub static mut frame_offset: Geometry = Geometry::new ();
static mut left_buttons_width: u32 = 0;
static mut right_buttons_width: u32 = 0;
static mut title_x: i32 = 0;


unsafe fn create_frame (base_geometry: &Geometry) -> Window {
  let g = base_geometry.get_frame (&frame_offset);
  let mut attributes: XSetWindowAttributes = uninitialized! ();
  attributes.background_pixmap = X_NONE;
  attributes.cursor = cursor::normal;
  attributes.override_redirect = X_TRUE;
  attributes.event_mask = SubstructureRedirectMask;
  attributes.save_under = X_FALSE;
  let screen = XDefaultScreen (display);

  XCreateWindow (
    display, root,
    g.x, g.y,
    g.w, g.h,
    0,
    XDefaultDepth (display, screen),
    InputOutput as u32,
    XDefaultVisual (display, screen),
    CWBackPixmap|CWEventMask|CWCursor|CWSaveUnder,
    &mut attributes
  )
}


pub struct Client {
  pub window: Window,
  pub frame: Window,
  pub geometry: Geometry,
  pub prev_geometry: Geometry,
  pub workspace: usize,
  pub snap_state: u8,
  pub is_urgent: bool,
  pub is_fullscreen: bool,
  pub is_dialog: bool,
  pub is_minimized: bool,
  pub border_color: &'static color::Color,
  title: String,
  left_buttons: Vec<Button>,
  right_buttons: Vec<Button>,
  title_space: i32
}

impl Client {
  pub const TITLE_BAR_GRADIENT_FACTOR: f64 = 1.185;

  pub unsafe fn new (window: Window) -> Box<Self> {
    let geometry = get_window_geometry (window);

    let mut attributes: XSetWindowAttributes = uninitialized! ();
    attributes.event_mask = StructureNotifyMask | PropertyChangeMask;
    attributes.do_not_propagate_mask = ButtonPressMask | ButtonReleaseMask;
    XChangeWindowAttributes (display, window, CWEventMask|CWDontPropagate, &mut attributes);
    XSetWindowBorderWidth (display, window, 0);

    let frame = create_frame (&geometry);
    XReparentWindow (display, window, frame, frame_offset.x, frame_offset.y);

    let mut c = Box::new (Client {
      window,
      frame,
      geometry,
      prev_geometry: geometry,
      workspace: active_workspace,
      snap_state: 0,
      is_urgent: false,
      is_fullscreen: false,
      is_dialog: false,
      is_minimized: false,
      border_color: &(*config).colors.normal,
      title: window_title (window),
      left_buttons: Vec::new (),
      right_buttons: Vec::new (),
      title_space: 0
    });
    let this = &mut *c as *mut Client as XPointer;
    XSaveContext (display, window, wm_context, this);
    XSaveContext (display, frame, wm_context, this);

    let mut i = 0;
    for name in (*config).left_buttons.iter () {
      let b = buttons::from_string (&mut c, name);
      b.move_ (i, true);
      c.left_buttons.push (b);
      i += 1;
    }
    i = 0;
    // Reverse the iterator so the leftmost button in the config is the
    // leftmost button on the window
    for name in (*config).right_buttons.iter ().rev () {
      let b = buttons::from_string (&mut c, name);
      b.move_ (i, false);
      c.right_buttons.push (b);
      i += 1;
    }

    XMapSubwindows (display, frame);

    c
  }

  pub unsafe fn dummy (window: Window) -> Self {
    Client {
      window,
      frame: X_NONE,
      geometry: uninitialized! (),
      prev_geometry: uninitialized! (),
      workspace: 0,
      snap_state: 0,
      is_urgent: false,
      is_fullscreen: false,
      is_dialog: false,
      is_minimized: false,
      border_color: &*(1 as *const color::Color),
      title: String::new (),
      left_buttons: Vec::new (),
      right_buttons: Vec::new (),
      title_space: 0
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
    let frame_size = self.geometry.get_frame (&frame_offset);
    (*draw).rect (
      0,
      frame_offset.y,
      frame_size.w,
      frame_size.h - frame_offset.y as u32,
      *self.border_color,
      true
    );
    (*draw).gradient (
      0,
      0,
      frame_size.w,
      frame_offset.y as u32,
      self.border_color.scale (Self::TITLE_BAR_GRADIENT_FACTOR),
      *self.border_color
    );

    (*draw).select_font (&(*config).title_font);
    (*draw).text (&self.title)
      .at (title_x, 0)
      .align_vertically (draw::Alignment::Centered, frame_offset.y)
      .align_horizontally((*config).title_alignment, self.title_space)
      .color ((*config).colors.bar_active_workspace_text)
      .width (self.title_space)
      .draw ();
    (*draw).render (
      self.frame,
      0,
      0,
      self.geometry.w + frame_offset.w,
      self.geometry.h + frame_offset.h
    );
    for b in self.buttons_mut () {
      b.draw (false);
    }
  }

  pub unsafe fn set_border (&mut self, color: &'static color::Color) {
    self.border_color = color;
    self.draw_border ();
  }

  pub unsafe fn set_title (&mut self, title: &str) {
    self.title.clear ();
    self.title.push_str (title);
    self.draw_border ();
  }

  pub unsafe fn move_and_resize (&mut self, target: Geometry) {
    let mut client_geometry = target;
    if self.is_snapped () {
      client_geometry.expand (-((*config).gap as i32));
    }
    self.set_position_and_size (client_geometry.get_client (&frame_offset));
  }

  pub unsafe fn set_position_and_size (&mut self, target: Geometry) {
    self.geometry = target;
    self.title_space = (self.geometry.w - left_buttons_width - right_buttons_width) as i32;
    let fg = target.get_frame (&frame_offset);
    XMoveResizeWindow (
      display, self.frame,
      fg.x,
      fg.y,
      fg.w,
      fg.h
    );
    for i in 0..self.left_buttons.len (){
      self.left_buttons[i].move_ (i as i32, true);
    }
    for i in 0..self.right_buttons.len (){
      self.right_buttons[i].move_ (i as i32, false);
    }
    XResizeWindow (display, self.window, target.w, target.h);
    self.configure ();
    self.draw_border ();
    XSync (display, X_FALSE);
  }

  pub unsafe fn unsnap (&mut self) {
    self.snap_state = SNAP_NONE;
    self.move_and_resize (self.prev_geometry);
  }

  pub unsafe fn focus (&mut self) {
    if self.is_urgent {
      self.set_urgency (false);
    }
    if self.is_fullscreen {
      XRaiseWindow (display, self.window);
    }
    else {
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
    }
    else {
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
    bar.draw ();
  }

  pub unsafe fn update_hints (&mut self) {
    let hints = XGetWMHints (display, self.window);
    if !hints.is_null () {
      if let Some (focused) = focused_client! () {
        if *focused == *self && ((*hints).flags & XUrgencyHint) != 0 {
          // It's being made urgent but it's already the active window
          (*hints).flags &= !XUrgencyHint;
          XSetWMHints (display, self.window, hints);
        }
      }
      else {
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
      let mut event: XEvent = uninitialized! ();
      event.type_ = ClientMessage;
      event.client_message.window = self.window;
      event.client_message.message_type = property::atom (WM::Protocols);
      event.client_message.format = 32;
      event.client_message.data.set_long (0, protocol as i64);
      event.client_message.data.set_long (1, CurrentTime as i64);
      XSendEvent (display, self.window, X_FALSE, NoEventMask, &mut event) != 0
    }
    else {
      false
    }
  }

  pub unsafe fn set_fullscreen (&mut self, state: bool) {
    if state == self.is_fullscreen {
      return;
    }
    self.is_fullscreen = state;
    if state {
      property::set (self.window, Net::WMState, XA_ATOM, 32,
        &property::atom (Net::WMStateFullscreen), 1);
      self.snap_state = SNAP_NONE;
      XReparentWindow (display, self.window, root, 0, 0);
      XResizeWindow (display, self.window, screen_size.w, screen_size.h);
      XRaiseWindow (display, self.window);
      XSetInputFocus (display, self.window, RevertToNone, CurrentTime);
    }
    else {
      property::set (self.window, Net::WMState, XA_ATOM, 32,
        std::ptr::null::<c_uchar> (), 0);
      XReparentWindow (display, self.window, self.frame, frame_offset.x, frame_offset.y);
      self.move_and_resize (self.prev_geometry);
      self.focus ();
    }
  }

  pub unsafe fn configure (&self) {
    let mut ev: XConfigureEvent = uninitialized! ();
    ev.type_ = ConfigureNotify;
    ev.display = display;
    ev.event = self.window;
    ev.window = self.window;
    ev.x = self.geometry.x;
    ev.x = self.geometry.x;
    ev.width = self.geometry.w as i32;
    ev.height = self.geometry.h as i32;
    ev.border_width = 0;
    ev.above = X_NONE;
    ev.override_redirect = X_FALSE;
    XSendEvent (
      display,
      self.window,
      X_FALSE,
      StructureNotifyMask,
      &mut ev as *mut XConfigureEvent as *mut XEvent
    );
  }

  pub unsafe fn click (&mut self, window: Window) -> bool {
    for b in self.buttons_mut () {
      if b.window == window {
        b.click ();
        return true;
      }
    }
    false
  }

  pub fn buttons (&self) -> std::iter::Chain<std::slice::Iter<'_, Button>, std::slice::Iter<'_, Button>> {
    self.left_buttons.iter ().chain (self.right_buttons.iter ())
  }

  pub fn buttons_mut (&mut self) -> std::iter::Chain<std::slice::IterMut<'_, Button>, std::slice::IterMut<'_, Button>> {
    self.left_buttons.iter_mut ().chain (self.right_buttons.iter_mut ())
  }
}

impl PartialEq for Client {
  fn eq (&self, other: &Self) -> bool {
    self.window == other.window
  }
}

impl std::fmt::Display for Client {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
    let title = unsafe { window_title (self.window) };
    write! (f, "'{}' ({})", title, self.window)
  }
}


pub unsafe fn set_border_info () {
  let title_height = (*config).title_height.get (Some (&(*config).title_font));
  let b = (*config).border_width;
  frame_offset = Geometry::from_parts  (
    b,
    title_height as i32,
    2 * b as u32,
    title_height + b as u32
  );
  left_buttons_width = (*config).left_buttons.len () as u32 * title_height;
  right_buttons_width = (*config).right_buttons.len () as u32 * title_height;
  title_x = left_buttons_width as i32 + b;
  buttons::set_size (title_height);
}
