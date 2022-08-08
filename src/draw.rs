use super::core::*;
use cairo::ffi::*;
use librsvg::{SvgHandle, CairoRenderer};
use x11::xlib::*;
use super::color::Color;
use super::geometry::Geometry;
use super::paths;

pub struct Resources {
  pub close_button: Option<SvgHandle>
}

pub struct Drawing_Context {
  drawable: Drawable,
  gc: GC,
  cairo_surface: cairo::Surface,
  cairo_context: cairo::Context,
  pango_layout: pango::Layout,
  pub resources: Resources
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
      resources: Resources {
        close_button: None
      }
    }
  }

  pub unsafe fn load_resources (&mut self) {
    log::info! ("Loading resources");
    let loader = librsvg::Loader::new ();
    self.resources.close_button = loader.read_path (
      format! ("{}/close_button.svg", paths::resource_dir)
    ).ok ();
  }

  pub unsafe fn rect (&mut self, x: i32, y: i32, w: u32, h: u32, color: u64, fill: bool) {
    XSetForeground (display, self.gc, color);
    if fill {
      XFillRectangle (display, self.drawable, self.gc, x, y, w, h);
    } else {
      XDrawRectangle (display, self.drawable, self.gc, x, y, w - 1, h - 1);
    }
  }

  pub unsafe fn draw_svg (&mut self, svg: &SvgHandle, x: i32, y: i32, w: u32, h: u32) {
    CairoRenderer::new (svg).render_document (
      &self.cairo_context,
      &cairo::Rectangle {
        x: x as f64,
        y: y as f64,
        width: w as f64,
        height: h as f64
      }
    ).unwrap ();
  }

  pub unsafe fn draw_colored_svg (&mut self, svg: &SvgHandle, color: Color, x: i32, y: i32, w: u32, h: u32) {
    // Create a mask from the alpha of the SVG and use that to fill the given color
    self.cairo_context.save ().unwrap ();
    self.cairo_context.push_group ();
    self.draw_svg (svg, x, y, w, h);
    let pattern = self.cairo_context.pop_group ().unwrap ();
    self.text_color (color);
    self.cairo_context.mask (&pattern).unwrap ();
    self.cairo_context.restore ().unwrap ();
  }

  pub unsafe fn select_font (&mut self, description: &str) {
    self.pango_layout.set_font_description (Some (&pango::FontDescription::from_string (description)));
  }

  pub unsafe fn font_height (&mut self, description: Option<&str>) -> u32 {
    if let Some (d) = description {
      self.select_font (d);
    }
    self.pango_layout.set_text ("Mgj가|");
    (self.pango_layout.size ().1 / pango::SCALE) as u32
  }

  pub fn text_color (&mut self, color: Color) {
    self.cairo_context.set_source_rgb (
      color.color.red as f64 / 65535.0,
      color.color.green as f64 / 65535.0,
      color.color.blue as f64 / 65535.0
    );
  }

  pub unsafe fn text (&mut self, text: &str) -> Rendered_Text {
    self.pango_layout.set_text (text);
    Rendered_Text::from_context (self)
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


pub enum Alignment {
  // Left is excluded as it is the default
  Centered,
  //Right,
  //Bottom
}


pub struct Rendered_Text<'a> {
  layout: &'a mut pango::Layout,
  context: &'a mut cairo::Context,
  width: i32,
  height: i32,
  x: i32,
  y: i32
}

impl<'a> Rendered_Text<'a> {
  pub unsafe fn from_context (context: &'a mut Drawing_Context) -> Self {
    let (width, height) = context.pango_layout.size ();
    Self {
      layout: &mut context.pango_layout,
      context: &mut context.cairo_context,
      width: width / pango::SCALE,
      height: height / pango::SCALE,
      x: 0,
      y: 0
    }
  }

  pub fn at (&mut self, x: i32, y: i32) -> &mut Self {
    self.x = x;
    self.y = y;
    self
  }

  pub fn at_right (&mut self, x: i32, y: i32) -> &mut Self {
    self.x = x - self.width;
    self.y = y;
    self
  }

  pub fn align_horizontally (&mut self, alignment: Alignment, width: i32) -> &mut Self {
    if self.width < width {
      if matches! (alignment, Alignment::Centered) {
        self.x += (width - self.width) / 2;
      }
      else {
        self.x += width - self.width;
      }
    }
    self
  }

  pub fn align_vertically (&mut self, alignment: Alignment, height: i32) -> &mut Self {
    if self.height < height {
      if matches! (alignment, Alignment::Centered) {
        self.y += (height - self.height) / 2;
      }
      else {
        self.y += height - self.height;
      }
    }
    self
  }

  pub fn color (&mut self, color: Color) -> &mut Self {
    self.context.set_source_rgb (
      color.color.red as f64 / 65535.0,
      color.color.green as f64 / 65535.0,
      color.color.blue as f64 / 65535.0
    );
    self
  }

  pub fn draw (&mut self) -> Geometry {
    self.context.move_to (self.x as f64, self.y as f64);
    pangocairo::show_layout (self.context, self.layout);
    Geometry::from_parts (self.x, self.y, self.width as u32, self.height as u32)
  }
}
