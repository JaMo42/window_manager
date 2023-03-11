use rand::{thread_rng, Rng};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Rectangle {
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}

impl Default for Rectangle {
    fn default() -> Self {
        Self::zeroed()
    }
}

impl Rectangle {
    pub const fn zeroed() -> Self {
        Self::new(0, 0, 0, 0)
    }

    pub const fn new(x: i16, y: i16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn from_parts((x, y, width, height): (i16, i16, u16, u16)) -> Self {
        Self::new(x, y, width, height)
    }

    pub fn into_parts(self) -> (i16, i16, u16, u16) {
        (self.x, self.y, self.width, self.height)
    }

    pub fn into_xcb(self) -> xcb::x::Rectangle {
        xcb::x::Rectangle {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }

    pub fn into_cairo(self) -> cairo::Rectangle {
        cairo::Rectangle::new(
            self.x as f64,
            self.y as f64,
            self.width as f64,
            self.height as f64,
        )
    }

    pub fn with_x(mut self, x: i16) -> Self {
        self.x = x;
        self
    }

    pub fn with_y(mut self, y: i16) -> Self {
        self.y = y;
        self
    }

    pub fn with_height(mut self, height: u16) -> Self {
        self.height = height;
        self
    }

    /// Moves the given rectangle to the given position, consuming the original.
    pub fn at(mut self, x: i16, y: i16) -> Self {
        self.x = x;
        self.y = y;
        self
    }

    /// Returns x-coordinate of the right edge.
    pub fn right_edge(&self) -> i16 {
        self.x + self.width as i16
    }

    /// Returns y-coordinate of the bottom edge.
    pub fn bottom_edge(&self) -> i16 {
        self.y + self.height as i16
    }

    /// Returns the center point.
    pub fn center(&self) -> (i16, i16) {
        (
            self.x + self.width as i16 / 2,
            self.y + self.height as i16 / 2,
        )
    }

    /// Does this rectangle contain the given point?
    #[rustfmt::skip]
    pub fn contains(&self, (x, y): (i16, i16)) -> bool {
           (x >= self.x)
        && (x < (self.x + self.width as i16))
        && (y >= self.y)
        && (y < (self.y + self.height as i16))
    }

    /// Grows/shrinks the rectangle by the given amount in each direction.
    /// The center point stays at the same position.
    pub fn resize(&mut self, by: i16) -> &mut Self {
        self.x -= by;
        self.y -= by;
        if by < 0 {
            let by = 2 * (-by) as u16;
            self.width -= by;
            self.height -= by;
        } else {
            let by = 2 * by as u16;
            self.width += by;
            self.height += by;
        }
        self
    }

    /// Returns a new rectangle scaled by the given percentage
    /// The given percentage is an integer in the range [0; 100].
    /// The center point stays at the same position.
    pub fn scale(&self, percent: u16) -> Self {
        let width = self.width * percent / 100;
        let height = self.height * percent / 100;
        let x = self.x + (self.width - width) as i16 / 2;
        let y = self.y + (self.height - height) as i16 / 2;
        Self::new(x, y, width, height)
    }

    /// Clamps this rectangles dimensions and position to be entirely inside
    /// `parent`. Modifies this rectangel and returns a reference to itself.
    pub fn clamp_inside(&mut self, parent: &Rectangle) -> &mut Self {
        if self.x < parent.x {
            self.x = parent.x;
        }
        if self.y < parent.y {
            self.y = parent.y;
        }
        if self.width > parent.width {
            self.width = parent.width;
        }
        if self.height > parent.height {
            self.height = parent.height;
        }
        if self.x + self.width as i16 > parent.x + parent.width as i16 {
            self.x = parent.x + (parent.width - self.width) as i16;
        }
        if self.y + self.height as i16 > parent.y + parent.height as i16 {
            self.y = parent.y + (parent.height - self.height) as i16;
        }
        self
    }

    /// Centers this rectangle inside `parent`. This rectangle may be larger
    /// than `parent`.
    pub fn center_inside(&mut self, parent: &Rectangle) -> &mut Self {
        self.x = parent.x + (parent.width as i16 - self.width as i16) / 2;
        self.y = parent.y + (parent.height as i16 - self.height as i16) / 2;
        self
    }

    /// Gives this rectangle a random position inside `parent`. If this rectangle
    /// is larger than `parent` its size is clamped.
    pub fn random_position_inside(&mut self, parent: &Rectangle) {
        let mut rng = thread_rng();
        if self.width >= parent.width {
            self.x = parent.x;
            self.width = parent.width;
        } else {
            let max = (parent.width - self.width) as i16 + parent.x;
            self.x = rng.gen_range(parent.x..=max);
        }
        if self.height >= parent.height {
            self.y = parent.y;
            self.height = parent.height;
        } else {
            let max = (parent.height - self.height) as i16 + parent.y;
            self.y = rng.gen_range(parent.y..=max);
        }
    }
}

impl std::fmt::Display for Rectangle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}x{}+{}+{}", self.width, self.height, self.x, self.y)
    }
}

impl From<(i16, i16, u16, u16)> for Rectangle {
    fn from(val: (i16, i16, u16, u16)) -> Self {
        Rectangle::from_parts(val)
    }
}

impl From<Rectangle> for (i16, i16, u16, u16) {
    fn from(val: Rectangle) -> Self {
        val.into_parts()
    }
}

/// Offset of a point in side a rectangle on a single axis.
/// `Static` is a fixed amount of pixels from the top/left edge, `Percent` is
/// a percentage of the height/width offset of the top/left edge.
#[derive(Debug)]
pub enum Offset {
    Static(i16),
    Percent(f32),
}

/// Offset of a pint insde a rectangle.
#[derive(Debug)]
pub struct PointOffset {
    x: Offset,
    y: Offset,
}

impl PointOffset {
    /// Get the point offset of the given point inside `rect`.
    /// The given thresholds specify the maximum distance from the top/left edge
    /// either axis can have to be `Static`, any value grater than it results in
    /// a `Percent` offset.
    pub fn offset_inside(
        (x, y): (i16, i16),
        rect: &Rectangle,
        x_static_threshold: i16,
        y_static_threshold: i16,
    ) -> Self {
        let x_inside = x - rect.x;
        let y_inside = y - rect.y;
        let x_offset = if x_inside > x_static_threshold {
            Offset::Percent(x_inside as f32 / rect.width as f32)
        } else {
            Offset::Static(x_inside)
        };
        let y_offset = if y_inside > y_static_threshold {
            Offset::Percent(y_inside as f32 / rect.height as f32)
        } else {
            Offset::Static(y_inside)
        };
        PointOffset {
            x: x_offset,
            y: y_offset,
        }
    }

    /// Get the point inside the given rectangle at the offset.
    pub fn point_inside(&self, rect: &Rectangle) -> (i16, i16) {
        let x = match self.x {
            Offset::Static(offset) => offset,
            Offset::Percent(percent) => (rect.width as f32 * percent) as i16,
        };
        let y = match self.y {
            Offset::Static(offset) => offset,
            Offset::Percent(percent) => (rect.height as f32 * percent) as i16,
        };
        (x, y)
    }
}

#[derive(Copy, Clone, Debug)]
pub enum ShowAt {
    #[allow(dead_code)]
    TopLeft((i16, i16)),
    TopCenter((i16, i16)),
    BottomCenter((i16, i16)),
}

impl ShowAt {
    /// Translates the given rectangle to the specified origin.
    pub fn translate(self, rect: impl Into<Rectangle>) -> Rectangle {
        match self {
            Self::TopLeft((x, y)) => rect.into().at(x, y),
            Self::TopCenter((x, y)) => {
                let rect = rect.into();
                rect.at(x - rect.width as i16 / 2, y)
            }
            Self::BottomCenter((x, y)) => {
                let rect = rect.into();
                rect.at(x - rect.width as i16 / 2, y - rect.height as i16)
            }
        }
    }

    pub fn anchor(&self) -> (i16, i16) {
        match *self {
            Self::TopLeft((x, y)) => (x, y),
            Self::TopCenter((x, y)) => (x, y),
            Self::BottomCenter((x, y)) => (x, y),
        }
    }
}
