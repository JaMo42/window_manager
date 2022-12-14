use std::cell::RefCell;
use std::rc::Rc;

use super::as_static::AsStaticMut;
use super::core::*;
use super::cursor;
use super::draw::Drawing_Context;
use super::error::fatal_error;
use super::ewmh;
use super::geometry::Geometry;
use super::monitors::{self, Monitor};
use super::mouse::{Finish_Reason, Tracked_Motion};
use super::property::Net;
use crate::color::Color;
use crate::set_window_kind;
use crate::x::{self, Window, XNone, XWindow};
use cairo::{Context, Surface};
use cairo_sys::cairo_xlib_surface_create;
use x11::xlib::*;

/// Split handles:
///    +----------+--------------------+
///    |          |                    |
///    |          |                    |
///    |          |                    |
///    |          +--------------------+ < right
/// .> +----------+                    |
/// |  |          |                    |
/// |  |          |                    |
/// |  +----------+--------------------+
/// |             ^
/// |             vertical
/// left
///
/// Splits are per-workspace and per-monitor, a split handle is activated if
/// a snapped window on either of its sides exists.
/// If a handle gets deactivated it gets reset to a 50/50 split.

const WIDTH: u32 = 15;
const WIDTH_OFFSET: i32 = WIDTH as i32 / 2;

static mut g_common: Option<Common> = None;
static mut g_context: XContext = XNone as i32;

/// Common data used by all `Split_Handle` instances.
struct Common {
  vi: XVisualInfo,
  colormap: Colormap,
  draw: Drawing_Context,
}

impl Common {
  unsafe fn new () -> Self {
    let vi = display
      .match_visual_info (32, TrueColor)
      .unwrap_or_else (|| fatal_error ("No 32bit truecolor visual found"));
    let colormap = display.create_colormap (vi.visual, AllocNone);
    let (width, height) = monitors::max_size ();
    let pixmap = XCreatePixmap (
      display.as_raw (),
      root.handle (),
      width,
      height,
      vi.depth as u32,
    );
    let gc = XCreateGC (display.as_raw (), pixmap, 0, std::ptr::null_mut ());
    let surface = {
      let raw = cairo_xlib_surface_create (
        display.as_raw (),
        pixmap,
        vi.visual,
        width as i32 + 1,
        height as i32 + 1,
      );
      Surface::from_raw_full (raw)
        .unwrap_or_else (|_| fatal_error ("Failed to create cairo surface"))
    };
    let context =
      Context::new (&surface).unwrap_or_else (|_| fatal_error ("Failed to create cairo context"));
    let layout = pangocairo::create_layout (&context)
      .unwrap_or_else (|| fatal_error ("Failed to create pango layout"));

    context.set_operator (cairo::Operator::Source);

    let dc = Drawing_Context::from_parts (pixmap, gc, surface, context, layout);

    Self {
      vi,
      colormap,
      draw: dc,
    }
  }
}

fn common () -> &'static mut Common {
  unsafe {
    if g_common.is_none () {
      g_common = Some (Common::new ());
    }
    g_common.as_mut ().unwrap_unchecked ()
  }
}

#[derive(Copy, Clone)]
pub enum Role {
  Left,
  Vertical,
  Right,
}

struct Split_Handle {
  window: Window,
  geometry: Geometry,
  role: Role,
  workspace: usize,
  monitor: usize,
}

impl Split_Handle {
  fn new (geometry: Geometry, role: Role, workspace: usize, monitor: usize) -> Box<Self> {
    let common = common ();
    let window = Window::builder (unsafe { &display })
      .position (geometry.x, geometry.y)
      .size (geometry.w, geometry.h)
      .attributes (|attributes| {
        attributes
          .override_redirect (true)
          .background_pixel (0)
          .border_pixel (0)
          .colormap (common.colormap)
          .save_under (true)
          .event_mask (ButtonPressMask | ButtonReleaseMask | EnterWindowMask | LeaveWindowMask);
      })
      .depth (common.vi.depth)
      .visual (common.vi.visual)
      .build ();
    unsafe {
      ewmh::set_window_type (window, Net::WMWindowTypeDesktop);
      set_window_kind (window, Window_Kind::Split_Handle);
      if g_context == XNone as i32 {
        g_context = x::unique_context ();
      }
    }
    window.set_class_hint ("Window_manager_split_handle", "window_manager_split_handle");
    window.lower ();
    let mut this = Box::new (Self {
      window,
      geometry,
      role,
      workspace,
      monitor,
    });
    window.save_context (
      unsafe { g_context },
      this.as_mut () as *mut Self as XPointer,
    );
    this
  }

  fn is_horizontal (&self) -> bool {
    !matches! (self.role, Role::Vertical)
  }

  pub fn draw_clicked (&self) {
    const LINE_LENGTH: u32 = 32;
    let dc = &mut common ().draw;
    dc.cairo_context ().set_source_rgba (0.05, 0.05, 0.05, 0.9);
    unsafe {
      dc.rect (0, 0, self.geometry.w, self.geometry.h)
        .corner_radius (0.49)
        .stroke (1, Color::from_rgb (0.1, 0.1, 0.1))
        .draw ();
      {
        let (width, height) = if self.is_horizontal () {
          (LINE_LENGTH, WIDTH / 3)
        } else {
          (WIDTH / 3, LINE_LENGTH)
        };
        dc.cairo_context ().set_source_rgba (0.95, 0.95, 0.95, 1.0);
        dc.rect (
          (self.geometry.w - width) as i32 / 2,
          (self.geometry.h - height) as i32 / 2,
          width,
          height,
        )
        .draw ();
      }
      {
        let (width, height) = if self.is_horizontal () {
          (LINE_LENGTH, WIDTH / 5)
        } else {
          (WIDTH / 5, LINE_LENGTH)
        };
        dc.cairo_context ().set_source_rgba (0.05, 0.05, 0.05, 0.9);
        dc.rect (
          (self.geometry.w - width) as i32 / 2,
          (self.geometry.h - height) as i32 / 2,
          width,
          height,
        )
        .draw ();
      }
      dc.render (self.window, 0, 0, self.geometry.w, self.geometry.h);
    }
  }

  pub fn raise_and_draw_hovered (&self) {
    self.window.raise ();
    let dc = &mut common ().draw;
    dc.cairo_context ().set_source_rgba (0.5, 0.5, 0.5, 0.7);
    unsafe {
      dc.rect (0, 0, self.geometry.w, self.geometry.h)
        .corner_radius (0.49)
        .stroke (1, Color::from_rgb (0.5, 0.5, 0.5))
        .draw ();
      dc.render (self.window, 0, 0, self.geometry.w, self.geometry.h);
    }
  }

  pub fn lower_and_clear (&self) {
    self.window.lower ();
    let dc = &mut common ().draw;
    dc.cairo_context ().set_source_rgba (0.0, 0.0, 0.0, 0.0);
    unsafe {
      dc.rect (0, 0, self.geometry.w, self.geometry.h).draw ();
      dc.render (self.window, 0, 0, self.geometry.w, self.geometry.h);
    }
  }

  fn update_window_geometry (&self) {
    self.window.move_and_resize (
      self.geometry.x,
      self.geometry.y,
      self.geometry.w,
      self.geometry.h,
    );
  }

  pub fn set_position (&mut self, position: i32) {
    if self.is_horizontal () {
      self.geometry.y = position;
    } else {
      self.geometry.x = position;
    }
    self.update_window_geometry ();
  }

  pub fn resize (&mut self, to: u32) {
    assert! (self.is_horizontal ());
    if matches! (self.role, Role::Right) {
      self.geometry.x += self.geometry.w as i32 - to as i32;
    }
    self.geometry.w = to;
    self.update_window_geometry ();
  }
}

/// All 3 split handles on a monitor
pub struct Split_Handles {
  vertical_handle: Box<Split_Handle>,
  left_handle: Box<Split_Handle>,
  right_handle: Box<Split_Handle>,
  geometry: Geometry,
  vertical: i32,
  left: i32,
  right: i32,
  screen_number: i32,
  pub vertical_clients: u32,
  pub left_clients: u32,
  pub right_clients: u32,
}

impl Split_Handles {
  pub fn with_percentages (
    workspace: usize,
    mon: &Monitor,
    percentages: &(f64, f64, f64),
  ) -> Box<Self> {
    let g = mon.window_area ();
    let vertical = (g.w as f64 * percentages.0) as i32;
    let left = (g.h as f64 * percentages.1) as i32;
    let right = (g.h as f64 * percentages.2) as i32;
    let vertical_handle = Split_Handle::new (
      Geometry::from_parts (g.x + vertical - WIDTH_OFFSET, g.y, WIDTH, g.h),
      Role::Vertical,
      workspace,
      mon.index (),
    );
    let left_handle = Split_Handle::new (
      Geometry::from_parts (g.x, g.y + left - WIDTH_OFFSET, vertical as u32, WIDTH),
      Role::Left,
      workspace,
      mon.index (),
    );
    let right_handle = Split_Handle::new (
      Geometry::from_parts (
        g.x + vertical,
        g.y + right - WIDTH_OFFSET,
        g.w - vertical as u32,
        WIDTH,
      ),
      Role::Right,
      workspace,
      mon.index (),
    );
    Box::new (Self {
      vertical_handle,
      left_handle,
      right_handle,
      geometry: *g,
      vertical,
      left,
      right,
      screen_number: mon.number (),
      vertical_clients: 0,
      left_clients: 0,
      right_clients: 0
    })
  }

  pub fn new (workspace: usize, mon: &Monitor) -> Box<Self> {
    Self::with_percentages (workspace, mon, &(0.5, 0.5, 0.5))
  }

  pub fn visible (&self, yay_or_nay: bool) {
    if yay_or_nay {
      if self.vertical_clients != 0 {
        self.vertical_handle.window.map ();
      } else {
        self.vertical_handle.window.unmap ();
      }
      if self.left_clients != 0 {
        self.left_handle.window.map ();
      } else {
        self.left_handle.window.unmap ();
      }
      if self.right_clients != 0 {
        self.right_handle.window.map ();
      } else {
        self.right_handle.window.unmap ();
      }
    } else {
      self.vertical_handle.window.unmap ();
      self.left_handle.window.unmap ();
      self.right_handle.window.unmap ();
    }
  }

  pub fn vertical (&self) -> i32 {
    self.vertical
  }

  pub fn left (&self) -> i32 {
    self.left
  }

  pub fn right (&self) -> i32 {
    self.right
  }

  pub fn geometry (&self) -> &Geometry {
    &self.geometry
  }

  pub fn screen_number (&self) -> i32 {
    self.screen_number
  }

  pub fn update (&mut self, role: Role, position: i32) {
    match role {
      Role::Left => {
        self.left = position + WIDTH as i32 / 2 - self.geometry.y;
        self.left_handle.set_position (position);
      }
      Role::Vertical => {
        self.vertical = position + WIDTH as i32 / 2 - self.geometry.x;
        self.vertical_handle.set_position (position);
        self.left_handle.resize (self.vertical as u32);
        self
          .right_handle
          .resize (self.geometry.w - self.vertical as u32);
      }
      Role::Right => {
        self.right = position + WIDTH as i32 / 2 - self.geometry.y;
        self.right_handle.set_position (position);
      }
    }
  }

  pub fn update_activated (&mut self) {
    let y = self.geometry.h as i32 / 2;
    if self.left_clients == 0 {
      self
        .left_handle
        .set_position (self.geometry.y + y - WIDTH as i32 / 2);
      self.left = y;
    }
    if self.right_clients == 0 {
      self
        .right_handle
        .set_position (self.geometry.y + y - WIDTH as i32 / 2);
      self.right = y;
    }
    if self.vertical_clients == 0 {
      self.vertical = self.geometry.w as i32 / 2;
      self
        .vertical_handle
        .set_position (self.geometry.x + self.vertical - WIDTH as i32 / 2);
      self.left_handle.resize (self.vertical as u32);
      self
        .right_handle
        .resize (self.geometry.w - self.vertical as u32);
    }
  }
}

fn xwindow_to_split_handle (window: XWindow) -> &'static mut Split_Handle {
  unsafe {
    (Window::from_handle (&display, window)
      .find_context (g_context)
      .unwrap () as *mut Split_Handle)
      .as_static_mut ()
  }
}

pub fn crossing (event: &XCrossingEvent) {
  let handle = xwindow_to_split_handle (event.window);
  if event.type_ == EnterNotify {
    handle.raise_and_draw_hovered ();
  } else {
    handle.lower_and_clear ();
  }
  unsafe { display.sync (false) };
}

pub unsafe fn click (event: &XButtonEvent) {
  let handle = xwindow_to_split_handle (event.window);
  handle.draw_clicked ();
  let reset_geometry = handle.geometry;
  let handle = Rc::new (RefCell::new (handle));
  Tracked_Motion::new ()
    .on_motion (&mut |motion: &XMotionEvent, _last_x, _last_y| {
      let mut handle = handle.borrow_mut ();
      if handle.is_horizontal () {
        handle.geometry.y = motion.y - event.y;
      } else {
        handle.geometry.x = motion.x - event.x;
      }
      handle.update_window_geometry ();
    })
    .on_finish (&mut |finish_reason| {
      let mut handle = handle.borrow_mut ();
      match finish_reason {
        Finish_Reason::Finish (x, y) => {
          let pos = if handle.is_horizontal () {
            y - event.y
          } else {
            x - event.x
          };
          workspaces[handle.workspace].update_split_sizes (handle.monitor, handle.role, pos);
          handle.raise_and_draw_hovered ();
          handle.window.lower ();
          display.sync (false);
        }
        _ => {
          handle.geometry = reset_geometry;
          handle.update_window_geometry ();
        }
      }
    })
    .cancel_on_escape ()
    .run (if handle.borrow_mut ().is_horizontal () {
      cursor::resizing_vertical
    } else {
      cursor::resizing_horizontal
    });
}