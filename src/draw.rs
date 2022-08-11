use super::core::*;
use cairo::ffi::*;
use librsvg::{SvgHandle, CairoRenderer};
use x11::xlib::*;
use super::color::Color;
use super::geometry::Geometry;
use super::paths;

pub mod resources {
  pub static mut close_button: super::Svg_Resource = super::Svg_Resource {
    file: "close_button.svg",
    handle: None,
    renderer: None,
    pattern: None
  };
}


pub struct Svg_Resource {
  file: &'static str,
  handle: Option<SvgHandle>,
  renderer: Option<CairoRenderer<'static>>,
  // The pattern used to draw a colored SVG, it is assumed that the size the
  // SVG is drawn in is always the same and it's always drawn to (0, 0).
  pattern: Option<cairo::Pattern>
}

impl Svg_Resource {
  pub fn is_some (&self) -> bool {
    self.handle.is_some ()
  }
}


pub struct Drawing_Context {
  drawable: Drawable,
  gc: GC,
  cairo_surface: cairo::Surface,
  cairo_context: cairo::Context,
  pango_layout: pango::Layout
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
    let cairo_context = cairo::Context::new (&cairo_surface)
      .expect ("Failed to create cairo context");
    let pango_layout = pangocairo::create_layout (&cairo_context)
      .expect ("Failed to create pango layout");
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

  pub unsafe fn gradient (&mut self, x_: i32, y_: i32, w_: u32, h_: u32, color_1: Color, color_2: Color) {
    let x = x_ as f64;
    let y = y_ as f64;
    let w = w_ as f64;
    let h = h_ as f64;
    let gradient = cairo::LinearGradient::new (x, y, x, y+h);
    gradient.add_color_stop_rgb (0.0, color_1.red, color_1.green, color_1.blue);
    gradient.add_color_stop_rgb (1.0, color_2.red, color_2.green, color_2.blue);
    self.cairo_context.rectangle (x, y, w, h);
    self.cairo_context.set_source (&gradient).unwrap ();
    self.cairo_context.fill ().unwrap ();
  }

  pub unsafe fn draw_svg (&mut self, svg: &Svg_Resource, x: i32, y: i32, w: u32, h: u32) {
    svg.renderer.as_ref ().unwrap ().render_document (
      &self.cairo_context,
      &cairo::Rectangle {
        x: x as f64,
        y: y as f64,
        width: w as f64,
        height: h as f64
      }
    ).unwrap ();
  }

  pub unsafe fn draw_colored_svg (&mut self, svg: &mut Svg_Resource, color: Color, x: i32, y: i32, w: u32, h: u32) {
    // Create a mask from the alpha of the SVG and use that to fill the given color
    if svg.pattern.is_none () {
      self.cairo_context.save ().unwrap ();
      self.cairo_context.push_group ();
      self.draw_svg (svg, x, y, w, h);
      svg.pattern = Some (self.cairo_context.pop_group ().unwrap ());
      self.cairo_context.restore ().unwrap ();
    }
    self.text_color (color);
    self.cairo_context.mask (svg.pattern.as_ref ().unwrap ()).unwrap ();
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
    self.cairo_context.set_source_rgb (color.red, color.green, color.blue);
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
  // Left/Top is excluded as it is the default
  Centered,
  // These are currently not used
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
    self.context.set_source_rgb (color.red, color.green, color.blue);
    self
  }

  pub fn draw (&mut self) -> Geometry {
    self.context.move_to (self.x as f64, self.y as f64);
    pangocairo::show_layout (self.context, self.layout);
    Geometry::from_parts (self.x, self.y, self.width as u32, self.height as u32)
  }
}

pub unsafe fn load_resources () {
  log::info! ("Loading resources");
  let loader = librsvg::Loader::new ();

  let load_svg = |res: &'static mut Svg_Resource| {
    match loader.read_path (format! ("{}/{}", paths::resource_dir, res.file)) {
      Ok (handle) => {
        res.handle = Some (handle);
        res.renderer = Some (CairoRenderer::new (
          res.handle.as_ref ().unwrap ()
        ));
      }
      Err (error) => {
        log::error! ("Failed to load {}: {}", res.file, error);
      }
    }
  };

  load_svg (&mut resources::close_button);
}
