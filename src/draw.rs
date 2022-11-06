use super::core::*;
use cairo::ffi::*;
use librsvg::{SvgHandle, CairoRenderer};
use x11::xlib::*;
use super::color::Color;
use super::geometry::Geometry;
use super::paths;

pub mod resources {
  use super::Svg_Resource;
  pub static mut close_button: Svg_Resource = Svg_Resource::new ("close_button.svg");
  pub static mut maximize_button: Svg_Resource = Svg_Resource::new ("maximize_button.svg");
  pub static mut minimize_button: Svg_Resource = Svg_Resource::new ("minimize_button.svg");
  pub static mut calendar: Svg_Resource = Svg_Resource::new ("calendar.svg");
  pub static mut volume: Svg_Resource = Svg_Resource::new ("volume.svg");
  pub static mut volume_muted: Svg_Resource = Svg_Resource::new ("volume_muted.svg");
  pub static mut battery_full: Svg_Resource = Svg_Resource::new ("battery_full.svg");
  pub static mut battery_critical: Svg_Resource = Svg_Resource::new ("battery_critical.svg");
  pub static mut battery_charging: Svg_Resource = Svg_Resource::new ("battery_charging.svg");
  pub static mut battery_bars: [Svg_Resource; 6] = [
    Svg_Resource::new ("battery_1_bar.svg"),
    Svg_Resource::new ("battery_2_bar.svg"),
    Svg_Resource::new ("battery_3_bar.svg"),
    Svg_Resource::new ("battery_4_bar.svg"),
    Svg_Resource::new ("battery_5_bar.svg"),
    Svg_Resource::new ("battery_6_bar.svg")
  ];
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
  pub const fn new (file: &'static str) -> Self {
    Self {
      file,
      handle: None,
      renderer: None,
      pattern: None
    }
  }

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

  pub unsafe fn fill_rect (&mut self, x: i32, y: i32, w: u32, h: u32, color: Color) {
    XSetForeground (display, self.gc, color.pixel);
    XFillRectangle (display, self.drawable, self.gc, x, y, w, h);
  }

  pub unsafe fn rect (&mut self, x: i32, y: i32, w: u32, h: u32) -> Shape_Builder {
    Shape_Builder::new (
      &mut self.cairo_context,
      Shape::Rectangle,
      Geometry::from_parts (x, y, w, h)
    )
  }

  pub unsafe fn square (&mut self, x: i32, y: i32, side: u32) -> Shape_Builder {
    self.rect (x, y, side, side)
  }

  #[allow(dead_code)] // Turns out we only draw circles using their bounding box so far
  pub unsafe fn circle (&mut self, center_x: i32, center_y: i32, radius: u32) -> Shape_Builder {
    Shape_Builder::new (
      &mut self.cairo_context,
      Shape::Ellipse,
      Geometry::from_parts (
        center_x - radius as i32,
        center_y - radius as i32,
        2*radius,
        2*radius
      )
    )
  }

  pub unsafe fn shape (&mut self, kind: Shape, bounding_box: Geometry) -> Shape_Builder {
    Shape_Builder::new (&mut self.cairo_context, kind, bounding_box)
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


#[allow(dead_code)]
#[derive(Copy, Clone)]
pub enum Alignment {
  Left, Top,
  Centered,
  Right, Bottom
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

  pub fn get_width (&self) -> u32 {
    self.width as u32
  }

  pub fn get_height (&self) -> u32 {
    self.height as u32
  }

  pub fn at (&mut self, x: i32, y: i32) -> &mut Self {
    self.x = x;
    self.y = y;
    self
  }

  #[allow(dead_code)]
  pub fn at_right (&mut self, x: i32, y: i32) -> &mut Self {
    self.x = x - self.width;
    self.y = y;
    self
  }

  pub fn align_horizontally (&mut self, alignment: Alignment, width: i32) -> &mut Self {
    if self.width < width {
      match alignment {
        Alignment::Left => {},
        Alignment::Centered => self.x += (width - self.width) / 2,
        Alignment::Right => self.x += width - self.width,
        _ => my_panic! ("Invalid value for horizontal alignment")
      }
    }
    self
  }

  pub fn align_vertically (&mut self, alignment: Alignment, height: i32) -> &mut Self {
    if self.height < height {
      match alignment {
        Alignment::Top => {},
        Alignment::Centered => self.y += (height - self.height) / 2,
        Alignment::Bottom => self.y += height - self.height,
        _ => my_panic! ("Invalid value for horizontal alignment")
      }
    }
    self
  }

  pub fn color (&mut self, color: Color) -> &mut Self {
    self.context.set_source_rgb (color.red, color.green, color.blue);
    self
  }

  pub fn width (&mut self, width: i32) -> &mut Self {
    self.layout.set_width (width * pango::SCALE);
    self.layout.set_ellipsize (pango::EllipsizeMode::Middle);
    let (width, height) = self.layout.size ();
    self.width = width;
    self.height = height;
    self
  }

  pub fn draw (&mut self) -> Geometry {
    self.context.move_to (self.x as f64, self.y as f64);
    pangocairo::show_layout (self.context, self.layout);
    Geometry::from_parts (self.x, self.y, self.width as u32, self.height as u32)
  }
}


#[derive(Copy, Clone)]
pub enum Shape {
  Rectangle,
  Ellipse,
}

pub struct Shape_Builder<'a> {
  context: &'a mut cairo::Context,
  shape: Shape,
  bounding_box: Geometry,
  stroke: Option<(u32, Color)>,
  gradient: Option<((f64, f64), Color, (f64, f64), Color)>,
  color: Option<Color>,
  // percentage of the bounding boxes smaller side to use as corner radius
  corner_radius_percent: Option<f64>,
}

#[allow(dead_code)]
impl<'a> Shape_Builder<'a> {
  pub fn new (context: &'a mut cairo::Context, shape: Shape, bounding_box: Geometry) -> Self {
    Self {
      context,
      shape,
      bounding_box,
      stroke: None,
      gradient: None,
      color: None,
      corner_radius_percent: None,
    }
  }

  pub fn color (&mut self, color: Color) -> &mut Self {
    self.color = Some (color);
    self
  }

  pub fn gradient (&mut self, p1: (f64, f64), c1: Color, p2: (f64, f64), c2: Color) -> &mut Self {
    self.gradient = Some ((p1, c1, p2, c2));
    self
  }

  // top -> bottom
  pub fn vertical_gradient (&mut self, top: Color, bottom: Color) -> &mut Self {
    self.gradient ((0.0, 0.0), top, (0.0, 1.0), bottom)
  }

  // left -> right
  pub fn horizontal_gradient (&mut self, left: Color, right: Color) -> &mut Self {
    self.gradient ((0.0, 0.0), left, (1.0, 0.0), right)
  }

  pub fn stroke (&mut self, width: u32, color: Color) -> &mut Self {
    self.stroke = Some ((width, color));
    // Shrink the shape since half of the stoke lies outside the path
    self.bounding_box.expand (-(width as i32));
    self
  }

  pub fn corner_radius (&mut self, percent: f64) -> &mut Self {
    self.corner_radius_percent = Some (percent);
    self
  }

  pub fn draw (&self) {
    self.set_path ();
    self.set_color ();
    self.do_draw ();
  }

  fn set_path (&self) {
    let x = self.bounding_box.x as f64;
    let y = self.bounding_box.y as f64;
    let w = self.bounding_box.w as f64;
    let h = self.bounding_box.h as f64;
    match self.shape {
      Shape::Rectangle => {
        if let Some (crp) = self.corner_radius_percent {
          let r = f64::min (w, h) * crp;
          self.context.new_sub_path ();
          self.context.arc (x + w - r, y + r, r, -90.0f64.to_radians (), 0.0f64.to_radians ());
          self.context.arc (x + w - r, y + h - r, r, 0.0f64.to_radians (), 90.0f64.to_radians ());
          self.context.arc (x + r, y + h - r, r, 90.0f64.to_radians (), 180.0f64.to_radians ());
          self.context.arc (x + r, y + r, r, 180.0f64.to_radians (), 270.0f64.to_radians ());
          self.context.close_path ();
        } else {
          self.context.rectangle (x, y, w, h);
        }
      }
      Shape::Ellipse => {
        if cfg! (debug_assertions) && self.corner_radius_percent.is_some () {
          log::warn! ("ignoring corner radius for ShapeBuilder of type Circle");
        }
        self.context.save ().unwrap ();
        self.context.translate (x, y);
        self.context.scale (w / 2.0, h / 2.0);
        self.context.arc (1.0, 1.0, 1.0, 0.0, 360.0f64.to_radians ());
        self.context.restore ().unwrap ();
      }
    }
  }

  fn set_color (&self) {
    if let Some (g) = self.gradient {
      let (p1, c1, p2, c2) = g;
      let gradient = cairo::LinearGradient::new (
        p1.0,
        p1.1,
        p2.0 * self.bounding_box.w as f64,
        p2.1 * self.bounding_box.h as f64
      );
      gradient.add_color_stop_rgb (0.0, c1.red, c1.green, c1.blue);
      gradient.add_color_stop_rgb (1.0, c2.red, c2.green, c2.blue);
      self.context.set_source (&gradient).unwrap ();
    } else if let Some (c) = self.color {
      self.context.set_source_rgb (c.red, c.green, c.blue);
    }
  }

  fn do_draw (&self) {
    if let Some ((w, c)) = self.stroke {
      self.context.fill_preserve ().unwrap ();
      self.context.set_source_rgb (c.red, c.green, c.blue);
      self.context.set_line_width (w as f64);
      self.context.stroke ().unwrap ();
    } else {
      self.context.fill ().unwrap ();
    }
  }
}


pub unsafe fn load_resources () {
  unsafe fn load_svg (res: &'static mut Svg_Resource) {
    let loader = librsvg::Loader::new ();
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
  }

  log::info! ("Loading resources");
  load_svg (&mut resources::close_button);
  load_svg (&mut resources::maximize_button);
  load_svg (&mut resources::minimize_button);
  load_svg (&mut resources::calendar);
  load_svg (&mut resources::volume);
  load_svg (&mut resources::volume_muted);
  load_svg (&mut resources::battery_full);
  load_svg (&mut resources::battery_critical);
  load_svg (&mut resources::battery_charging);
  for i in resources::battery_bars.iter_mut () {
    load_svg (i);
  }
}


#[allow(dead_code)]
pub unsafe fn get_app_icon (app_name: &str) {
  let _desktop_file_path = format! ("/usr/share/applications/{}.desktop", app_name);
  let icon_name = "applications-system.svg"; // TODO: read from desktop file
  let icon_theme = "Papirus"; // TODO: get from config
  let _icon_path = format! ("/usr/share/icons/{}/48x48/apps/{}", icon_theme, icon_name);
}
