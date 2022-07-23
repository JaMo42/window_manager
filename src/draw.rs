use super::core::*;
use cairo::ffi::*;
use x11::xlib::*;
use super::color::Color;

pub struct Drawing_Context {
  drawable: Drawable,
  gc: GC,
  cairo_surface: cairo::Surface,
  cairo_context: cairo::Context,
  pango_layout: pango::Layout,
}

impl Drawing_Context {
  pub unsafe fn new () -> Self {
    let width = screen_size.w as u32;
    let height = screen_size.h as u32;
    let drawable = XCreatePixmap (
      display,
      root,
      width,
      height,
      XDefaultDepth (display, XDefaultScreen (display)) as u32,
    );
    let cairo_surface_raw = cairo_xlib_surface_create (
      display,
      drawable,
      XDefaultVisual (display, XDefaultScreen (display)),
      width as i32,
      height as i32,
    );
    cairo_xlib_surface_set_size (cairo_surface_raw, width as i32, height as i32);
    let cairo_surface = cairo::Surface::from_raw_full (cairo_surface_raw)
      .expect ("Failed to create cairo surface");
    let cairo_context =
      cairo::Context::new (&cairo_surface).expect ("Failed to create cairo context");
    let pango_layout = pangocairo::create_layout (&cairo_context).unwrap ();
    Self {
      drawable,
      gc: XCreateGC (display, root, 0, std::ptr::null_mut ()),
      cairo_surface,
      cairo_context,
      pango_layout,
    }
  }

  pub unsafe fn rect (&mut self, x: i32, y: i32, w: u32, h: u32, color: u64, fill: bool) {
    XSetForeground (display, self.gc, color);
    if fill {
      XFillRectangle (display, self.drawable, self.gc, x, y, w, h);
    } else {
      XDrawRectangle (display, self.drawable, self.gc, x, y, w - 1, h - 1);
    }
  }

  pub unsafe fn select_font (&mut self, description: &str) {
    self.pango_layout.set_font_description (Some (&pango::FontDescription::from_string (description)));
  }

  #[allow(clippy::too_many_arguments)]
  pub unsafe fn text_in_rect (
    &mut self,
    mut x: i32,
    mut y: i32,
    w: i32,
    h: i32,
    text: &str,
    color: Color,
    center_vertically: bool,
    center_horizontally: bool,
  ) -> i32 {
    self.pango_layout.set_text (text);
    let (mut tw, mut th) = self.pango_layout.size ();
    tw /= pango::SCALE;
    th /= pango::SCALE;
    if center_vertically && th < h {
      y += (h - th) / 2;
    }
    if center_horizontally && tw < w {
      x += (w - tw) / 2;
    }
    self.cairo_context.set_source_rgba (
      color.color.red as f64 / 65535.0,
      color.color.green as f64 / 65535.0,
      color.color.blue as f64 / 65535.0,
      color.color.alpha as f64 / 65535.0
    );
    self.cairo_context.move_to (x as f64, y as f64);
    pangocairo::show_layout (&self.cairo_context, &self.pango_layout);
    tw
  }

  pub unsafe fn text_right (&mut self, mut x: i32, h: i32, text: &str, color: Color) -> i32 {
    self.pango_layout.set_text (text);
    let (tw, mut th) = self.pango_layout.size ();
    th /= pango::SCALE;
    x -= tw / pango::SCALE;
    let y = if th < h {
      (h - th) / 2
    } else {
      0
    };
    self.cairo_context.set_source_rgba (
      color.color.red as f64 / 65535.0,
      color.color.green as f64 / 65535.0,
      color.color.blue as f64 / 65535.0,
      color.color.alpha as f64 / 65535.0
    );
    self.cairo_context.move_to (x as f64, y as f64);
    pangocairo::show_layout (&self.cairo_context, &self.pango_layout);
    x
  }

  pub unsafe fn render (&mut self, win: Window, xoff: i32, yoff: i32, width: u32, height: u32) {
    self.cairo_surface.flush ();
    XCopyArea (
      display,
      self.drawable,
      win,
      self.gc,
      xoff,
      yoff,
      width,
      height,
      xoff,
      yoff,
    );
    XSync (display, X_FALSE);
  }
}
