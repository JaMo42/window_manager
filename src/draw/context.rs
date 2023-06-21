use crate::{
    color::Color,
    error::{fatal_error, OrFatal},
    rectangle::Rectangle,
    x::{Display, Visual, Window},
};
use cairo::{Context, Operator, XCBConnection, XCBDrawable, XCBSurface, XCBVisualType};
use itertools::Itertools;
use pango::{FontDescription, Layout};
use std::{ptr::NonNull, sync::Arc};
use xcb::{
    x::{
        ChangeGc, CopyArea, CreateGc, Drawable, Gc, Gcontext, GetImage, ImageFormat, Pixmap,
        PolyFillRectangle, Visualtype,
    },
    Xid,
};

use super::{
    shape::{ShapeBuilder, ShapeKind},
    Svg, TextBuilder,
};

fn find_visual_type(display: &Display, visual: &Visual) -> *mut Visualtype {
    for screen in display.connection().get_setup().roots() {
        for depth in screen
            .allowed_depths()
            .filter(|d| d.depth() == visual.depth)
        {
            for visual_type in depth.visuals() {
                if visual_type.visual_id() == visual.id {
                    // cairo wants a `NonNull` which requires a mutable pointer
                    return visual_type as *const Visualtype as *mut Visualtype;
                }
            }
        }
    }
    // *should* be impossible as we get the visual from the x server in the first place.
    fatal_error(
        display,
        format!("Can somehow no longer find visual with id {}", visual.id),
    );
}

pub struct DrawingContext {
    display: Arc<Display>,
    pixmap: Pixmap,
    gc: Gcontext,
    surface: XCBSurface,
    context: Context,
    layout: Layout,
    depth: u8,
}

unsafe impl Send for DrawingContext {}

impl DrawingContext {
    /// Creates a new drawing context for the given display.
    pub fn create(display: Arc<Display>, (width, height): (u16, u16)) -> Self {
        log::trace!("draw: creating context");
        let visual = display.truecolor_visual();
        let pixmap = display.create_pixmap(None, visual.depth, width, height);
        let surface = unsafe {
            // According to this issue in the xcb crate you're supposed to just
            // cast these like this:
            // https://github.com/rust-x-bindings/rust-xcb/issues/200
            use cairo_sys::{xcb_connection_t, xcb_visualtype_t};
            let connection = display.connection().get_raw_conn();
            let connection = NonNull::new(connection as *mut xcb_connection_t).unwrap_unchecked();
            let visual_type = find_visual_type(&display, visual);
            let visual_type = NonNull::new(visual_type as *mut xcb_visualtype_t).unwrap_unchecked();
            XCBSurface::create(
                &XCBConnection(connection),
                &XCBDrawable(pixmap.resource_id()),
                &XCBVisualType(visual_type),
                width as i32,
                height as i32,
            )
            .unwrap_or_fatal(&display)
        };
        let context = Context::new(&surface).unwrap_or_fatal(&display);
        context.set_operator(Operator::Source);
        let layout = pangocairo::create_layout(&context);
        let gc = display.connection().generate_id();
        display
            .try_void_request(&CreateGc {
                cid: gc,
                drawable: Drawable::Pixmap(pixmap),
                value_list: &[],
            })
            .or_fatal(&display);
        display.flush();
        let depth = visual.depth;
        Self {
            display,
            pixmap,
            gc,
            surface,
            context,
            layout,
            depth,
        }
    }

    pub fn destroy(&self) {
        log::trace!("draw: destroying context");
        self.display.free_pixmap(self.pixmap);
    }

    /// Resizes the context to the given size.
    pub fn resize(&mut self, width: u16, height: u16) {
        log::trace!("draw: resizing context: width={width} height={height}");
        self.display.free_pixmap(self.pixmap);
        self.pixmap = self.display.create_pixmap(None, self.depth, width, height);
        self.surface
            .set_drawable(
                &XCBDrawable(self.pixmap.resource_id()),
                width as i32,
                height as i32,
            )
            .or_fatal(&self.display);
        self.context.set_source_rgba(0.0, 0.0, 0.0, 1.0);
        self.context.paint().ok();
    }

    /// Get the cairo context.
    pub fn cairo(&self) -> &Context {
        &self.context
    }

    pub fn pixmap(&self) -> Pixmap {
        self.pixmap
    }

    /// Performs a `CopyArea` request using the X graphics context of this drawing context.
    pub fn copy_area(
        &self,
        src: Drawable,
        dst: Drawable,
        rect: impl Into<Rectangle>,
        at: (i16, i16),
    ) {
        let (x, y, width, height) = rect.into().into_parts();
        self.display.void_request(&CopyArea {
            src_drawable: src,
            dst_drawable: dst,
            gc: self.gc,
            src_x: x,
            src_y: y,
            dst_x: at.0,
            dst_y: at.1,
            width,
            height,
        });
        self.display.flush();
    }

    pub fn render_at(&self, to: &Window, rect: impl Into<Rectangle>, at: (i16, i16)) {
        self.copy_area(
            Drawable::Pixmap(self.pixmap),
            Drawable::Window(to.handle()),
            rect,
            at,
        );
    }

    pub fn render(&self, to: &Window, rect: impl Into<Rectangle>) {
        let rect = rect.into();
        self.render_at(to, rect, (rect.x, rect.y));
    }

    pub fn get_average_svg_color(&self, svg: &Svg, rect: impl Into<Rectangle>) -> Color {
        let rect = rect.into();
        self.fill_rect(rect, Color::new(0.0, 0.0, 0.0, 0.0));
        self.draw_svg(svg, rect);
        let (x, y, width, height) = rect.into_parts();
        // data is BGRA
        let reply = self
            .display
            .request_with_reply(&GetImage {
                format: ImageFormat::ZPixmap,
                drawable: Drawable::Pixmap(self.pixmap),
                x,
                y,
                width,
                height,
                plane_mask: 0xffffffff,
            })
            .unwrap();
        let mut n = 0.0;
        let (red, green, blue) = reply
            .data()
            .iter()
            .cloned()
            .tuples()
            .filter(|&(_, _, _, a)| a != 0)
            .fold((0.0, 0.0, 0.0), |(cma_r, cma_g, cma_b), (b, g, r, _)| {
                n += 1.0;
                (
                    cma_r + (r as f64 - cma_r) / n,
                    cma_g + (g as f64 - cma_g) / n,
                    cma_b + (b as f64 - cma_b) / n,
                )
            });
        Color::new_rgb(
            red as f64 / 255.0,
            green as f64 / 255.0,
            blue as f64 / 255.0,
        )
    }

    pub fn set_color(&self, color: Color) {
        let (r, g, b, a) = color.components();
        self.context.set_source_rgba(r, g, b, a);
    }

    /// Fills the given rectangle with the given color without using cairo.
    pub fn fill_rect(&self, rect: impl Into<Rectangle>, color: Color) {
        self.display.void_request(&ChangeGc {
            gc: self.gc,
            value_list: &[Gc::Foreground(color.pack())],
        });
        self.display.void_request(&PolyFillRectangle {
            drawable: Drawable::Pixmap(self.pixmap),
            gc: self.gc,
            rectangles: &[rect.into().into_xcb()],
        });
    }

    /// Creates a `ShapeBuilder` for the given rectangle.
    pub fn rect(&self, rect: impl Into<Rectangle>) -> ShapeBuilder {
        ShapeBuilder::new(
            self.display.clone(),
            &self.context,
            ShapeKind::Rectangle,
            rect.into().into_cairo(),
        )
    }

    /// Creates a `ShapeBuilder` for an ellipse inside the given rectangle.
    pub fn ellipse(&self, rect: impl Into<Rectangle>) -> ShapeBuilder {
        ShapeBuilder::new(
            self.display.clone(),
            &self.context,
            ShapeKind::Ellipse,
            rect.into().into_cairo(),
        )
    }

    /// Sets the font used for `DrawingContext::text` and `DrawingContext::markup`.
    pub fn set_font(&self, font: &FontDescription) {
        self.layout.set_font_description(Some(font));
    }

    /// Returns the height of the given font, if no font is given the currently
    /// selected font is used.
    pub fn font_height(&self, font: Option<&FontDescription>) -> u16 {
        if let Some(font) = font {
            self.set_font(font);
        }
        //(self.layout.context().metrics(font, None).height() / pango::SCALE) as u16
        // Turns out this hacky variant gave better results
        self.layout.set_text("Mgj가|");
        (self.layout.size().1 / pango::SCALE) as u16
    }

    /// Returns the width a fullwidth unicode character in the given font.
    pub fn fullwidth_character_width(&self, font: Option<&FontDescription>) -> u16 {
        self.text_width("가", font)
    }

    /// Returns the width of the given text in the given font. If not font is
    /// specified the current font is used.
    pub fn text_width(&self, text: &str, font: Option<&FontDescription>) -> u16 {
        if let Some(font) = font {
            self.set_font(font);
        }
        self.layout.set_text(text);
        (self.layout.size().0 / pango::SCALE) as u16
    }

    /// Creates a `TextBuilder` for the given plain string.
    /// The given rectangle is used for positioning, alignment, and ellipsizing
    /// but does not limit the region the text can use otherwise (i.e. it's safe
    /// to just leave the width/height as `0` if it's not used for any of those
    /// operations).
    pub fn text(&self, text: &str, rect: impl Into<Rectangle>) -> TextBuilder {
        TextBuilder::new(text, false, rect.into(), &self.context, &self.layout)
    }

    /// Creates a `TextBuilder` for the given markup string.
    /// See `DrawingContext::text` for what the given rectangle specifies.
    pub fn markup(&self, text: &str, rect: impl Into<Rectangle>) -> TextBuilder {
        TextBuilder::new(text, true, rect.into(), &self.context, &self.layout)
    }

    /// Creates a new text layout.
    pub fn create_layout(&self) -> Layout {
        pangocairo::create_layout(&self.context)
    }

    // TODO: better name
    /// Creates a `TextBuilder` from an existing layout.
    /// See `DrawingContext::text` for what the given rectangle specifies.
    pub fn text_layout<'a, 'b: 'a>(
        &'a self,
        layout: &'b Layout,
        rect: impl Into<Rectangle>,
    ) -> TextBuilder<'a> {
        TextBuilder::from_layout(rect.into(), &self.context, layout)
    }

    /// Draws the svg to the context.
    pub fn draw_svg(&self, svg: &Svg, rect: impl Into<Rectangle>) {
        svg.renderer()
            .render_document(&self.context, &rect.into().into_cairo())
            .or_fatal(&self.display);
    }

    /// Draws to the context using the svg's alpha channel as mask.
    pub fn draw_colored_svg(&self, svg: &Svg, color: Color, rect: impl Into<Rectangle>) {
        let pattern = svg
            .get_pattern(rect.into(), self)
            .unwrap_or_fatal(&self.display);
        self.set_color(color);
        self.context.mask(pattern).or_fatal(&self.display);
    }
}
