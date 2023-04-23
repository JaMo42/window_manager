use crate::{color::Color, error::OrFatal, x::Display};
use cairo::{Context, LinearGradient};
use std::sync::Arc;

#[derive(Debug)]
pub struct GradientSpec {
    start: Color,
    end: Color,
    start_point: (f64, f64),
    end_point: (f64, f64),
}

impl GradientSpec {
    pub fn new_vertical(top: Color, bottom: Color) -> Self {
        Self {
            start: top,
            end: bottom,
            start_point: (0.0, 0.0),
            end_point: (0.0, 1.0),
        }
    }

    #[allow(dead_code)]
    pub fn new_horizontal(left: Color, right: Color) -> Self {
        Self {
            start: left,
            end: right,
            start_point: (0.0, 0.0),
            end_point: (1.0, 0.0),
        }
    }
}

#[derive(Debug)]
pub enum ColorKind {
    None,
    Solid(Color),
    Gradient(GradientSpec),
}

#[derive(Debug)]
enum CornerRadius {
    None,
    Percent(f64),
    Static(f64),
}

impl CornerRadius {
    fn is_some(&self) -> bool {
        !matches!(self, Self::None)
    }

    fn compute(&self, smaller_side: f64) -> f64 {
        match self {
            Self::None => 0.0,
            Self::Percent(p) => smaller_side * p,
            Self::Static(r) => *r,
        }
    }
}

#[derive(Debug)]
pub enum ShapeKind {
    Rectangle,
    Ellipse,
}

#[derive(Debug)]
pub struct ShapeBuilder<'a> {
    // Needs a display for fatal errors, previously results were just unwrapped.
    display: Arc<Display>,
    kind: ShapeKind,
    context: &'a Context,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    color: ColorKind,
    stroke: Option<(u16, ColorKind)>,
    corner_radius: CornerRadius,
    no_fill: bool,
    clear_area: Option<(f64, f64, f64, f64)>,
}

impl<'a> ShapeBuilder<'a> {
    pub(crate) fn new(
        display: Arc<Display>,
        context: &'a Context,
        kind: ShapeKind,
        rect: cairo::Rectangle,
    ) -> Self {
        Self {
            display,
            kind,
            context,
            x: rect.x(),
            y: rect.y(),
            width: rect.width(),
            height: rect.height(),
            color: ColorKind::None,
            stroke: None,
            corner_radius: CornerRadius::None,
            no_fill: false,
            clear_area: None,
        }
    }

    pub fn color(&mut self, color: Color) -> &mut Self {
        self.color = ColorKind::Solid(color);
        self
    }

    pub fn gradient(&mut self, gradient: GradientSpec) -> &mut Self {
        self.color = ColorKind::Gradient(gradient);
        self
    }

    pub fn stroke(&mut self, width: u16, color: ColorKind) -> &mut Self {
        self.stroke = Some((width, color));
        let outside = width as f64 * 0.5;
        // Half the stroke lies outside the shape but we want to preserve the
        // bounding box so we shrink it.
        self.x += outside;
        self.y += outside;
        self.width -= width as f64;
        self.height -= width as f64;
        self
    }

    /// Do not fill the shape, only useful together with `stroke`.
    pub fn no_fill(&mut self) -> &mut Self {
        self.no_fill = true;
        self
    }

    /// Clear the area below rounded corners for rectangles.
    pub fn clear_below_corners(&mut self) -> &mut Self {
        // If we have a stroke set the stored values not longer cover the entire
        // area below the rounded corners to we need to expand it again.
        let stroke_width = if let Some((width, _)) = &self.stroke {
            *width
        } else {
            0
        } as f64;
        let half = stroke_width * 0.5;
        self.clear_area = Some((
            self.x - half,
            self.y - half,
            self.width + stroke_width,
            self.height + stroke_width,
        ));
        self
    }

    /// Sets the corner radius percentage. The used corner radius will be this
    /// percentage of the shorter side of the bounding box.
    pub fn corner_percent(&mut self, percent: f64) -> &mut Self {
        self.corner_radius = CornerRadius::Percent(percent);
        self
    }

    /// Sets the corner radius to the given value.
    pub fn corner_radius(&mut self, radius: u16) -> &mut Self {
        self.corner_radius = CornerRadius::Static(radius as f64);
        self
    }

    /// Drawins the shape onto its context.
    pub fn draw(&self) {
        if self.clear_area.is_some() {
            self.clear_corners().or_fatal(&self.display);
        }
        self.set_path();
        if !self.no_fill {
            self.set_color(&self.color);
        }
        self.fill();
    }

    fn clear_corners(&self) -> Result<(), cairo::Error> {
        if matches!(self.corner_radius, CornerRadius::None) {
            return Ok(());
        }
        let (x, y, width, height) = self.clear_area.unwrap();
        let size = self.corner_radius.compute(f64::min(width, height));
        let right_x = width - size;
        let bottom_y = height - size;
        self.context.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        self.context.rectangle(x, y, size, size);
        self.context.fill()?;
        self.context.rectangle(right_x, y, size, size);
        self.context.fill()?;
        self.context.rectangle(x, bottom_y, size, size);
        self.context.fill()?;
        self.context.rectangle(right_x, bottom_y, size, size);
        self.context.fill()?;
        Ok(())
    }

    fn set_rectangle_path(&self) {
        if let CornerRadius::None = self.corner_radius {
            self.context
                .rectangle(self.x, self.y, self.width, self.height);
        } else {
            let r = self
                .corner_radius
                .compute(f64::min(self.width, self.height));
            self.context.new_sub_path();
            self.context.arc(
                self.x + self.width - r,
                self.y + r,
                r,
                -90.0f64.to_radians(),
                0.0f64.to_radians(),
            );
            self.context.arc(
                self.x + self.width - r,
                self.y + self.height - r,
                r,
                0.0f64.to_radians(),
                90.0f64.to_radians(),
            );
            self.context.arc(
                self.x + r,
                self.y + self.height - r,
                r,
                90.0f64.to_radians(),
                180.0f64.to_radians(),
            );
            self.context.arc(
                self.x + r,
                self.y + r,
                r,
                180.0f64.to_radians(),
                270.0f64.to_radians(),
            );
            self.context.close_path();
        }
    }

    fn set_ellipse_path(&self) {
        if cfg!(debug_assertions) && self.corner_radius.is_some() {
            log::warn!("Ignoring corner radius for ShapeBuilder of type Circle");
        }
        self.context.save().or_fatal(&self.display);
        self.context.translate(self.x, self.y);
        self.context.scale(self.width / 2.0, self.height / 2.0);
        self.context.arc(1.0, 1.0, 1.0, 0.0, 360.0f64.to_radians());
        self.context.restore().or_fatal(&self.display);
    }

    fn set_path(&self) {
        match self.kind {
            ShapeKind::Rectangle => self.set_rectangle_path(),
            ShapeKind::Ellipse => self.set_ellipse_path(),
        }
    }

    fn set_color(&self, color: &ColorKind) {
        match color {
            ColorKind::None => {}
            ColorKind::Solid(ref color) => {
                let (r, g, b, a) = color.components();
                self.context.set_source_rgba(r, g, b, a);
            }
            ColorKind::Gradient(ref spec) => {
                let gradient = LinearGradient::new(
                    spec.start_point.0,
                    spec.start_point.1,
                    spec.end_point.0 * self.width,
                    spec.end_point.1 * self.height,
                );
                let (r, g, b, a) = spec.start.components();
                gradient.add_color_stop_rgba(0.0, r, g, b, a);
                let (r, g, b, a) = spec.end.components();
                gradient.add_color_stop_rgba(1.0, r, g, b, a);
                self.context.set_source(gradient).unwrap();
            }
        }
    }

    fn fill(&self) {
        if let Some((stroke_width, ref stroke_color)) = self.stroke {
            if !self.no_fill {
                self.context.fill_preserve().or_fatal(&self.display);
            }
            self.set_color(stroke_color);
            self.context.set_line_width(stroke_width as f64);
            self.context.stroke().or_fatal(&self.display);
        } else if !self.no_fill {
            self.context.fill().or_fatal(&self.display);
        }
    }
}
