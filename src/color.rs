use std::ops::Mul;

#[derive(Copy, Clone, Debug)]
pub struct Color {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

impl Color {
    pub const fn new(red: f64, green: f64, blue: f64, alpha: f64) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    pub const fn new_rgb(red: f64, green: f64, blue: f64) -> Self {
        Self::new(red, green, blue, 1.0)
    }

    /// Constructs a new color with channels given as bytes.
    pub fn new_bytes(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red: red as f64 / 255.0,
            green: green as f64 / 255.0,
            blue: blue as f64 / 255.0,
            alpha: alpha as f64 / 255.0,
        }
    }

    pub fn components(&self) -> (f64, f64, f64, f64) {
        (self.red, self.green, self.blue, self.alpha)
    }

    pub fn rgb_components(&self) -> (f64, f64, f64) {
        (self.red, self.green, self.blue)
    }

    pub fn with_alpha(mut self, alpha: f64) -> Self {
        self.alpha = alpha;
        self
    }

    /// Multiplies each component with the given value and clamps the result
    /// in the range [0.0; 1.0].
    pub fn scale(&self, factor: f64) -> Self {
        Self::new(
            (self.red * factor).clamp(0.0, 1.0),
            (self.green * factor).clamp(0.0, 1.0),
            (self.blue * factor).clamp(0.0, 1.0),
            (self.alpha * factor).clamp(0.0, 1.0),
        )
    }

    /// Packs the color in the format `0xAARRGGBB`.
    pub fn pack(&self) -> u32 {
        let r = (self.red * 255.0).round() as u32;
        let g = (self.green * 255.0).round() as u32;
        let b = (self.blue * 255.0).round() as u32;
        let a = (self.alpha * 255.0).round() as u32;
        (a << 24) | (r << 16) | (g << 8) | b
    }
}

impl Mul<f64> for Color {
    type Output = Color;

    fn mul(self, rhs: f64) -> Self::Output {
        self.scale(rhs)
    }
}

/// Background and text color pair for window borders.
/// Also handles scaling for the gradient on title bars.
pub struct BorderColor {
    /// Some value that should be different between border kinds,
    /// used for comparison.
    kind: u8,
    is_focused: bool,
    border: Color,
    text: Color,
}

impl BorderColor {
    const TITLE_BAR_GRADIENT_FACTOR: f64 = 1.185;

    pub fn new(kind: u8, is_focused: bool, border: Color, text: Color) -> Self {
        Self {
            kind,
            is_focused,
            border,
            text,
        }
    }

    pub fn kind(&self) -> u8 {
        self.kind
    }

    /// Is a window with this border color considered to be focused?
    pub fn is_focused(&self) -> bool {
        self.is_focused
    }

    /// Get the base border color.
    pub fn border(&self) -> Color {
        self.border
    }

    /// Get the top color for the title bar gradient.
    pub fn top(&self) -> Color {
        self.border.scale(Self::TITLE_BAR_GRADIENT_FACTOR)
    }

    /// Get the text color.
    pub fn text(&self) -> Color {
        self.text
    }
}

impl PartialEq for BorderColor {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}
