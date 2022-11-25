use crate::client::Client;
use crate::core::*;
use crate::cursor;
use crate::draw::Drawing_Context;
use crate::error::fatal_error;
use crate::ewmh;
use crate::geometry::Geometry;
use crate::property::Net;
use crate::timeout_thread::Repeatable_Timeout_Thread;
use crate::x::Window;
use crate::{set_window_kind, set_window_opacity};
use cairo::{Context, Surface};
use cairo_sys::cairo_xlib_surface_create;
use std::ptr::NonNull;
use x11::xlib::*;

use super::item::Item;

unsafe fn create_window (
  x: i32,
  y: i32,
  width: u32,
  height: u32,
  vi: &XVisualInfo,
  colormap: Colormap,
) -> Window {
  log::trace! ("dock: creating dock window");
  Window::builder (&display)
    .position (x, y)
    .size (width, height)
    .attributes (|attributes| {
      attributes
        .override_redirect (true)
        .cursor (cursor::normal)
        .background_pixel (0)
        .border_pixel (0)
        // See `create_show_window` for why need to handle clicks.
        .event_mask (LeaveWindowMask | ButtonPressMask)
        .colormap (colormap);
    })
    .depth (vi.depth)
    .visual (vi.visual)
    .build ()
}

unsafe fn create_drawing_context (
  height: u32,
  drawable: Drawable,
  vi: &XVisualInfo,
) -> Drawing_Context {
  log::trace! ("dock: creating drawing context");
  let width = screen_size.w;
  let pixmap = XCreatePixmap (display.as_raw (), drawable, width, height, vi.depth as u32);
  let gc = XCreateGC (display.as_raw (), pixmap, 0, std::ptr::null_mut ());
  let surface = {
    let raw = cairo_xlib_surface_create (
      display.as_raw (),
      pixmap,
      vi.visual,
      width as i32 + 1,
      height as i32 + 1,
    );
    Surface::from_raw_full (raw).unwrap_or_else (|_| fatal_error ("Failed to create cairo surface"))
  };
  let context =
    Context::new (&surface).unwrap_or_else (|_| fatal_error ("Failed to create cairo context"));
  let layout = pangocairo::create_layout (&context)
    .unwrap_or_else (|| fatal_error ("Failed to create pango layout"));

  context.set_operator (cairo::Operator::Source);

  Drawing_Context::from_parts (pixmap, gc, surface, context, layout)
}

unsafe fn create_show_window (vi: &XVisualInfo, colormap: Colormap) -> Window {
  log::trace! ("dock: creating show window");
  let height = 10;
  Window::builder (&display)
    .size (1920, height)
    .position (0, (screen_size.h - height) as i32)
    .attributes (|attributes| {
      attributes
        .override_redirect (true)
        .cursor (cursor::normal)
        .background_pixel (0)
        .border_pixel (0)
        .backing_store (Always)
        .colormap (colormap)
        // We need ButtonPressMask on this since otherwise we'd get
        // `leave -> press (on root with this as subwindow) -> enter` and we do
        // not want those extra enter and leave events.
        .event_mask (EnterWindowMask | LeaveWindowMask | ButtonPressMask);
    })
    .depth (vi.depth)
    .visual (vi.visual)
    .build ()
}

unsafe fn create_windows_and_drawing_context (
  x: i32,
  y: i32,
  width: u32,
  height: u32,
) -> (Window, Window, Drawing_Context) {
  let vi = display
    .match_visual_info (32, TrueColor)
    .unwrap_or_else (|| fatal_error ("No 32bit truecolor visual found"));
  let colormap = display.create_colormap (vi.visual, AllocNone);

  let window = create_window (x, y, width, height, &vi, colormap);
  ewmh::set_window_type (window, Net::WMWindowTypeDock);
  set_window_kind (window, Window_Kind::Dock);
  window.set_class_hint ("Window_manager_dock", "window_manager_dock");
  window.map_raised ();

  let show_window = create_show_window (&vi, colormap);
  ewmh::set_window_type (show_window, Net::WMWindowTypeDesktop);
  set_window_opacity (show_window, 100);
  set_window_kind (show_window, Window_Kind::Dock_Show);
  show_window.set_class_hint ("Window_manager_dock", "window_manager_dock");
  show_window.map ();

  let dc = create_drawing_context (height, window.handle (), &vi);

  (window, show_window, dc)
}

pub struct Dock {
  window: Window,
  // Similar to clients, items store a pointer to themselves as context on the
  // window so we need to box them so they keep the same pointer through their
  // lifetime.
  #[allow(clippy::vec_box)]
  items: Vec<Box<Item>>,
  drawing_context: Drawing_Context,
  // Invisible window at the bottom of the screen which is used to show the dock
  // when it is hovered
  show_window: Window,
  hide_thread: Repeatable_Timeout_Thread,
  visible: bool,
  geometry: Geometry,
  keep_open: bool,
  item_size: u32,
}

impl Dock {
  const PADDING: u32 = 15;

  fn item_position (index: usize, size: u32) -> i32 {
    (Self::PADDING + index as u32 * (size + Self::PADDING)) as i32
  }

  fn size (items: usize, size: u32) -> (u32, u32) {
    (
      items as u32 * size + items as u32 * Self::PADDING + Self::PADDING,
      size + 2 * Self::PADDING,
    )
  }

  fn position (width: u32, height: u32) -> (i32, i32) {
    unsafe {
      (
        screen_size.x + (screen_size.w - width) as i32 / 2,
        screen_size.y + (screen_size.h - height) as i32,
      )
    }
  }

  /// `size` specifies the for the icons, the dock dimensions are derived from it.
  pub unsafe fn create (size: u32) -> Self {
    let pinned = &(*config).dock_pinned;
    let (width, height) = Self::size (usize::max (pinned.len (), 1), size);
    let (x, y) = Self::position (width, height);

    display.sync (false);

    let (window, show_window, mut my_draw) =
      create_windows_and_drawing_context (x, y, width, height);

    log::trace! ("dock: adding pinned items");
    let mut items = Vec::new ();
    for (i, p) in pinned.iter ().enumerate () {
      if let Some (item) = Item::create (
        window.handle (),
        p,
        true,
        size,
        Self::item_position (i, size),
        Self::PADDING as i32,
        &mut my_draw,
      ) {
        items.push (item);
      }
    }

    display.sync (false);

    Self {
      window,
      items,
      drawing_context: my_draw,
      show_window,
      hide_thread: Repeatable_Timeout_Thread::new (|| {
        super::the ().hide ();
      }),
      visible: false,
      geometry: Geometry::from_parts (x, y, width, height),
      keep_open: false,
      item_size: size,
    }
  }

  pub fn destroy (&mut self) {
    self.window.destroy ();
    self.show_window.destroy ();
    self.hide_thread.destroy ();
  }

  pub fn redraw (&mut self) {
    let context = self.drawing_context.cairo_context ();
    let r = self.geometry.h as f64 * 0.2;
    let w = self.geometry.w as f64;
    let h = self.geometry.h as f64;

    // Clear area below rounded corners
    context.set_source_rgba (0.0, 0.0, 0.0, 0.0);
    context.rectangle (0.0, 0.0, r, r);
    context.fill ().unwrap ();
    context.rectangle (w - r, 0.0, r, r);
    context.fill ().unwrap ();
    // Draw rectangle with top corners rounded
    context.move_to (0.0, h);
    context.arc (r, r, r, 180.0f64.to_radians (), 270.0f64.to_radians ());
    context.arc (w - r, r, r, -90.0f64.to_radians (), 0.0f64.to_radians ());
    context.line_to (w, h);
    context.close_path ();
    let c = unsafe { &(*config).colors.dock_background };
    context.set_source_rgba (c.red, c.green, c.blue, 1.0);
    context.fill ().unwrap ();

    unsafe {
      self
        .drawing_context
        .render (self.window, 0, 0, self.geometry.w, self.geometry.h);
    }

    for item in self.items.iter_mut () {
      unsafe {
        item.redraw (&mut self.drawing_context, false);
      }
    }
  }

  pub unsafe fn show (&mut self) {
    if !self.visible {
      self.window.map_raised ();
      self.visible = true;
      self.redraw ();
    }
  }

  pub unsafe fn hide (&mut self) {
    if self.visible {
      self.window.unmap ();
      self.visible = false;
    }
    self.keep_open = false;
  }

  pub fn geometry (&self) -> &Geometry {
    &self.geometry
  }

  pub fn contains (&self, x: i32, y: i32) -> bool {
    self.geometry.contains (x, y)
  }

  pub fn drawing_context (&mut self) -> &mut Drawing_Context {
    &mut self.drawing_context
  }

  pub fn window (&self) -> &Window {
    &self.window
  }

  /// Hides the bar after the given time.
  pub fn hide_after (&mut self, after_ms: u64) {
    self.cancel_hide ();
    if self.keep_open {
      return;
    }
    self.hide_thread.start (after_ms);
  }

  /// Cancels a `hide_after` request.
  pub fn cancel_hide (&mut self) {
    self.hide_thread.cancel ();
  }

  pub fn keep_open (&mut self, yay_or_nay: bool) {
    self.keep_open = yay_or_nay;
    unsafe {
      if yay_or_nay {
        self.show ();
      } else if display
        .query_pointer_position ()
        .map (|(x, y)| !self.geometry.contains (x, y))
        .unwrap_or (true)
      {
        self.hide ();
      }
    }
  }

  fn find_item (&self, name: &str) -> Option<usize> {
    self.items.iter ().position (|item| item.name () == name)
  }

  fn resize_window (&mut self, item_count: usize) {
    let (width, height) = Self::size (item_count, self.item_size);
    let (x, y) = Self::position (width, height);
    self.window.move_and_resize (x, y, width, height);
    self.geometry = Geometry::from_parts (x, y, width, height);
  }

  pub unsafe fn add_client (&mut self, client: NonNull<Client>) {
    let name = client.as_ref ().application_id ();
    if name.starts_with ("window_manager_") {
      // These don't have desktop entries and in case of
      // `window_manager_message_box` would cause an infinite cycle of spawning
      // error message boxes
      return;
    }
    if let Some (index) = self.find_item (name) {
      self.items[index].add_instance (client);
    } else if let Some (mut item) = Item::create (
      self.window.handle (),
      name,
      false,
      self.item_size,
      Self::item_position (self.items.len (), self.item_size),
      Self::PADDING as i32,
      self.drawing_context (),
    ) {
      item.add_instance (client);
      self.resize_window (self.items.len () + 1);
      self.items.push (item);
      self.redraw ();
    }
  }

  fn find_client_item (&mut self, client: &Client) -> Option<usize> {
    // TODO: just searching through all instances of all items for the window
    //       of the client may be faster the getting its name.
    self.find_item (client.application_id ())
  }

  pub unsafe fn remove_client (&mut self, client: &Client) {
    if let Some (index) = self.find_client_item (client) {
      if self.items[index].remove_instance (client) {
        self.items.remove (index).destroy ();
        self.resize_window (usize::max (self.items.len (), 1));
        self.redraw ();
      }
    }
  }

  pub unsafe fn update_focus (&mut self, client: &Client) {
    if let Some (index) = self.find_client_item (client) {
      self.items[index].focus (client);
    }
  }

  pub unsafe fn update_urgency (&mut self, client: &Client) {
    if let Some (index) = self.find_client_item (client) {
      self.items[index].urgent (client.is_urgent);
      self.items[index].redraw (&mut self.drawing_context, false);
    }
  }
}
