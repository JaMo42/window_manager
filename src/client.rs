use x11::xlib::*;
use super::core::*;
use super::geometry::*;
use super::*;
use super::property::WM;
use super::draw::resources;

pub static mut frame_offset: Geometry = Geometry::new ();


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


unsafe fn create_auxilarry_windows (frame: Window, frame_size: &Geometry) -> Window {
  let button_size = frame_offset.y as u32;
  let button_pos = frame_size.w - button_size;
  let mut attributes: XSetWindowAttributes = uninitialized! ();
  attributes.override_redirect = X_TRUE;
  attributes.event_mask = ButtonPressMask|ButtonReleaseMask|EnterWindowMask|LeaveWindowMask;
  attributes.background_pixmap = X_NONE;
  attributes.save_under = X_FALSE;
  attributes.backing_store = NotUseful;

  XCreateWindow (
    display,
    frame,
    button_pos as i32,
    0,
    button_size,
    button_size,
    0,
    CopyFromParent,
    InputOutput as c_uint,
    CopyFromParent as *mut Visual,
    CWEventMask|CWOverrideRedirect|CWBackPixmap|CWSaveUnder|CWBackingStore,
    &mut attributes
  )
}


pub struct Client {
  pub window: Window,
  pub frame: Window,
  pub close_button: Window,
  pub geometry: Geometry,
  pub prev_geometry: Geometry,
  pub workspace: usize,
  pub snap_state: u8,
  pub is_urgent: bool,
  pub is_fullscreen: bool,
  pub is_dialog: bool,
  border_color: &'static color::Color,
  title: String,
  close_button_state: bool
}

impl Client {
  pub unsafe fn new (window: Window) -> Box<Self> {
    let geometry = get_window_geometry (window);

    let mut attributes: XSetWindowAttributes = uninitialized! ();
    attributes.event_mask = StructureNotifyMask | PropertyChangeMask;
    attributes.do_not_propagate_mask = ButtonPressMask | ButtonReleaseMask;
    XChangeWindowAttributes (display, window, CWEventMask|CWDontPropagate, &mut attributes);
    XSetWindowBorderWidth (display, window, 0);

    let frame = create_frame (&geometry);
    XReparentWindow (display, window, frame, frame_offset.x, frame_offset.y);

    let close_button= create_auxilarry_windows (
      frame, &geometry.get_frame (&frame_offset)
    );

    XMapSubwindows (display, frame);
    let mut c = Box::new (Client {
      window,
      frame,
      close_button,
      geometry,
      prev_geometry: geometry,
      workspace: active_workspace,
      snap_state: 0,
      is_urgent: false,
      is_fullscreen: false,
      is_dialog: false,
      border_color: &(*config).colors.normal,
      title: window_title (window),
      close_button_state: false
    });
    let this = &mut *c as *mut Client as XPointer;
    XSaveContext (display, window, wm_context, this);
    XSaveContext (display, frame, wm_context, this);
    XSaveContext (display, close_button, wm_context, this);
    c
  }

  pub unsafe fn dummy (window: Window) -> Self {
    Client {
      window,
      frame: X_NONE,
      close_button: X_NONE,
      geometry: uninitialized! (),
      prev_geometry: uninitialized! (),
      workspace: 0,
      snap_state: 0,
      is_urgent: false,
      is_fullscreen: false,
      is_dialog: false,
      border_color: &*(1 as *const color::Color),
      title: String::new (),
      close_button_state: false
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
    XMapWindow (display, self.close_button);
    self.set_border (&(*config).colors.focused);
  }

  pub unsafe fn unmap (&self) {
    XUnmapWindow (display, self.frame);
  }

  pub unsafe fn draw_border (&mut self) {
    (*draw).rect (
      0,
      0,
      self.geometry.w + frame_offset.w,
      self.geometry.h + frame_offset.h,
      self.border_color.pixel,
      true
    );
    (*draw).select_font (&(*config).title_font);
    (*draw).text (&self.title)
      .at (frame_offset.x, 0)
      .align_vertically (draw::Alignment::Centered, frame_offset.y)
      .color ((*config).colors.bar_active_workspace_text)
      .draw ();
    (*draw).render (
      self.frame,
      0,
      0,
      self.geometry.w + frame_offset.w,
      self.geometry.h + frame_offset.h
    );
    self.draw_close_button (self.close_button_state);
  }

  pub unsafe fn set_border (&mut self, color: &'static color::Color) {
    self.border_color = color;
    self.draw_border ();
    self.draw_close_button (self.close_button_state);
  }

  pub unsafe fn draw_close_button (&mut self, hovered: bool) {
    const ICON_SIZE_PERCENT: u32 = 75;
    let size = frame_offset.y as u32;
    let icon_size = size * ICON_SIZE_PERCENT / 100;
    let icon_position = (size - icon_size) as i32 / 2;
    let color = if hovered {
      (*config).colors.close_button_hovered
    } else {
      (*config).colors.close_button
    };
    (*draw).rect (0, 0, size, size, self.border_color.pixel, true);

    if resources::close_button.is_some () {
      (*draw).draw_colored_svg (
        &mut resources::close_button,
        color,
        icon_position, icon_position,
        icon_size, icon_size
      );
    }
    else {
      (*draw).rect (
        icon_position, icon_position,
        icon_size, icon_size,
        color.pixel, true
      );
    }

    (*draw).render (self.close_button, 0, 0, size, size);
    self.close_button_state = hovered;
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
    let fg = target.get_frame (&frame_offset);
    XMoveResizeWindow (
      display, self.frame,
      fg.x,
      fg.y,
      fg.w,
      fg.h
    );
    XMoveWindow (display, self.close_button, fg.w as i32 - frame_offset.y, 0);
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
      bar.draw ();
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
}
