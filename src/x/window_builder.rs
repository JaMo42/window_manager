use super::display::Display;
use super::window::{To_XWindow, Window};
use super::*;

#[derive(Copy, Clone)]
pub struct Window_Attributes {
  attributes: XSetWindowAttributes,
  valuemask: c_ulong,
}

impl Window_Attributes {
  pub fn new () -> Self {
    Self {
      attributes: unsafe { std::mem::MaybeUninit::zeroed ().assume_init () },
      valuemask: 0,
    }
  }

  pub fn build (self) -> (XSetWindowAttributes, c_ulong) {
    (self.attributes, self.valuemask)
  }

  pub fn background_pixmap (&mut self, pixmap: Pixmap) -> &mut Self {
    self.attributes.background_pixmap = pixmap;
    self.valuemask |= CWBackPixmap;
    self
  }

  pub fn background_pixel (&mut self, pixel: c_ulong) -> &mut Self {
    self.attributes.background_pixel = pixel;
    self.valuemask |= CWBackPixel;
    self
  }

  pub fn border_pixmap (&mut self, pixmap: Pixmap) -> &mut Self {
    self.attributes.border_pixmap = pixmap;
    self.valuemask |= CWBorderPixmap;
    self
  }

  pub fn border_pixel (&mut self, pixel: c_ulong) -> &mut Self {
    self.attributes.border_pixel = pixel;
    self.valuemask |= CWBorderPixel;
    self
  }

  pub fn bit_gravity (&mut self, gravity: c_int) -> &mut Self {
    self.attributes.bit_gravity = gravity;
    self.valuemask |= CWBitGravity;
    self
  }

  pub fn win_gravity (&mut self, gravity: c_int) -> &mut Self {
    self.attributes.win_gravity = gravity;
    self.valuemask |= CWWinGravity;
    self
  }

  pub fn backing_store (&mut self, cfg: c_int) -> &mut Self {
    self.attributes.backing_store = cfg;
    self.valuemask |= CWBackingStore;
    self
  }

  pub fn backing_planes (&mut self, planes: c_ulong) -> &mut Self {
    self.attributes.backing_planes = planes;
    self.valuemask |= CWBackingPlanes;
    self
  }

  pub fn backing_pixel (&mut self, pixel: c_ulong) -> &mut Self {
    self.attributes.backing_pixel = pixel;
    self.valuemask |= CWBackingPixel;
    self
  }

  pub fn save_under (&mut self, yay_or_nay: bool) -> &mut Self {
    self.attributes.save_under = yay_or_nay as Bool;
    self.valuemask |= CWSaveUnder;
    self
  }

  pub fn event_mask (&mut self, mask: c_long) -> &mut Self {
    self.attributes.event_mask = mask;
    self.valuemask |= CWEventMask;
    self
  }

  pub fn do_not_propagate_mask (&mut self, mask: c_long) -> &mut Self {
    self.attributes.do_not_propagate_mask = mask;
    self.valuemask |= CWDontPropagate;
    self
  }

  pub fn override_redirect (&mut self, yay_or_nay: bool) -> &mut Self {
    self.attributes.override_redirect = yay_or_nay as Bool;
    self.valuemask |= CWOverrideRedirect;
    self
  }

  pub fn colormap (&mut self, colormap: Colormap) -> &mut Self {
    self.attributes.colormap = colormap;
    self.valuemask |= CWColormap;
    self
  }

  pub fn cursor (&mut self, cursor: Cursor) -> &mut Self {
    self.attributes.cursor = cursor;
    self.valuemask |= CWCursor;
    self
  }
}

pub struct Window_Builder {
  display: XDisplay,
  parent: XWindow,
  x: i32,
  y: i32,
  w: u32,
  h: u32,
  border_width: u32,
  depth: i32,
  class: u32,
  visual: *mut Visual,
  attributes: Window_Attributes,
}

impl Window_Builder {
  pub fn new (display: &Display) -> Self {
    Self {
      display: display.as_raw (),
      parent: display.root (), // or XNone?
      x: 0,
      y: 0,
      w: 3 * 160,
      h: 3 * 90,
      border_width: 0,
      depth: CopyFromParent,
      class: InputOutput as u32,
      visual: CopyFromParent as *mut Visual,
      attributes: Window_Attributes::new (),
    }
  }

  pub fn parent<W: To_XWindow> (&mut self, handle: W) -> &mut Self {
    self.parent = handle.to_xwindow ();
    self
  }

  pub fn position (&mut self, x: i32, y: i32) -> &mut Self {
    self.x = x;
    self.y = y;
    self
  }

  pub fn size (&mut self, w: u32, h: u32) -> &mut Self {
    self.w = w;
    self.h = h;
    self
  }

  pub fn border_width (&mut self, width: u32) -> &mut Self {
    self.border_width = width;
    self
  }

  pub fn depth (&mut self, depth: i32) -> &mut Self {
    self.depth = depth;
    self
  }

  pub fn class (&mut self, class: u32) -> &mut Self {
    self.class = class;
    self
  }

  pub fn visual (&mut self, visual: *mut Visual) -> &mut Self {
    self.visual = visual;
    self
  }

  pub fn attributes (&mut self, f: fn (&mut Window_Attributes)) -> &mut Self {
    f (&mut self.attributes);
    self
  }

  pub fn colormap (&mut self, colormap: Colormap) -> &mut Self {
    // A colormap is likely a local variable which could not be captured by the
    // function passed to `attributes` so this function is used to set it.
    self.attributes.colormap (colormap);
    self
  }

  pub fn build (&mut self) -> Window {
    let (mut attributes, valuemask) = self.attributes.build ();
    let window = unsafe {
      XCreateWindow (
        self.display,
        self.parent,
        self.x,
        self.y,
        self.w,
        self.h,
        self.border_width,
        self.depth,
        self.class,
        self.visual,
        valuemask,
        &mut attributes,
      )
    };
    Window::from_handle (&self.display, window)
  }
}
