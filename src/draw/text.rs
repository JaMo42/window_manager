use crate::{color::Color, rectangle::Rectangle};
use cairo::Context;
use pango::{EllipsizeMode, Layout};

#[derive(Copy, Clone, Debug)]
#[allow(dead_code)]
pub enum Alignment {
    _Begin,
    _Center,
    _End,
}

impl Alignment {
    pub const TOP: Self = Self::_Begin;
    pub const LEFT: Self = Self::_Begin;
    pub const CENTER: Self = Self::_Center;
    pub const BOTTOM: Self = Self::_End;
    pub const RIGHT: Self = Self::_End;
}

pub struct TextBuilder<'a> {
    context: &'a Context,
    layout: &'a Layout,
    rect: Rectangle,
    text_width: u16,
    text_height: u16,
    color: Option<Color>,
    is_markup: bool,
}

impl<'a> TextBuilder<'a> {
    pub(super) fn new(
        text: &str,
        markup: bool,
        rect: Rectangle,
        context: &'a Context,
        layout: &'a Layout,
    ) -> Self {
        if markup {
            layout.set_markup(text);
        } else {
            layout.set_text(text);
        }
        let (width, height) = layout.size();
        Self {
            context,
            layout,
            rect,
            text_width: (width / pango::SCALE) as u16,
            text_height: (height / pango::SCALE) as u16,
            color: None,
            is_markup: markup,
        }
    }

    pub(super) fn from_layout(rect: Rectangle, context: &'a Context, layout: &'a Layout) -> Self {
        let (width, height) = layout.size();
        Self {
            context,
            layout,
            rect,
            text_width: (width / pango::SCALE) as u16,
            text_height: (height / pango::SCALE) as u16,
            color: None,
            // This is only used to fix `set_text` after a `set_markup` call for
            // main layout but we assume that external layouts only use one of the
            // methods or deal with if themselves. We also don't want to clear
            // the content of the layout.
            is_markup: false,
        }
    }

    pub fn width(&self) -> u16 {
        self.text_width
    }

    pub fn height(&self) -> u16 {
        self.text_height
    }

    /// Changes the position of the rectangle passed to `DrawingContext::text`
    /// or `DrawingContext::markup`. This resets the vertical and horizontal
    /// alignment of the text.
    pub fn at(&mut self, x: i16, y: i16) -> &mut Self {
        self.rect.x = x;
        self.rect.y = y;
        self
    }

    /// Vertically aligns the text. This should only be called once.
    pub fn vertical_alignment(&mut self, alignment: Alignment) -> &mut Self {
        match alignment {
            Alignment::_Begin => {}
            Alignment::_Center => {
                self.rect.y += (self.rect.height as i16 - self.text_height as i16) / 2
            }
            Alignment::_End => self.rect.y += self.rect.height as i16 - self.text_height as i16,
        }
        self
    }

    /// Horizontally aligns the text. This should only be called once.
    pub fn horizontal_alignment(&mut self, alignment: Alignment) -> &mut Self {
        match alignment {
            Alignment::_Begin => {}
            Alignment::_Center => {
                self.rect.x += (self.rect.width as i16 - self.text_width as i16) / 2
            }
            Alignment::_End => self.rect.x += self.rect.width as i16 - self.text_width as i16,
        }
        self
    }

    /// Sets the color to use for drawing. This will change the color of the
    /// drawing context after `TextBuilder::draw` is called.
    pub fn color(&mut self, color: Color) -> &mut Self {
        self.color = Some(color);
        self
    }

    /// Ellipsize the text using the width of the rectangle passed to
    /// `DrawingContext::text` or `DrawingContext::markup`.
    pub fn ellipsize(&mut self, mode: EllipsizeMode) -> &mut Self {
        self.layout.set_width(self.rect.width as i32 * pango::SCALE);
        self.layout.set_ellipsize(mode);
        self
    }

    /// Draws the text to the `DrawingContext` and returns the final bounding
    /// box of it.
    pub fn draw(&mut self) -> Rectangle {
        self.context.move_to(self.rect.x as f64, self.rect.y as f64);
        if let Some(color) = self.color {
            let (r, g, b, a) = color.components();
            self.context.set_source_rgba(r, g, b, a);
        }
        pangocairo::show_layout(self.context, self.layout);
        if self.is_markup {
            // XXX: Without this the first word of a following non-markup text
            // would be incorrectly styled.
            self.layout.set_markup("");
        }
        Rectangle::new(self.rect.x, self.rect.y, self.text_width, self.text_height)
    }
}
