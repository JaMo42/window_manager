use xcb::x::{Colormap, Cursor, Cw, EventMask};

/// Builder for changing window attributes.
pub struct WindowAttributes {
    value_list: Vec<Cw>,
}

impl WindowAttributes {
    pub const fn new() -> Self {
        Self {
            value_list: Vec::new(),
        }
    }

    pub fn value_list(&mut self) -> &[Cw] {
        self.value_list.sort();
        &self.value_list
    }

    pub fn event_mask(&mut self, mask: EventMask) -> &mut Self {
        self.value_list.push(Cw::EventMask(mask));
        self
    }

    pub fn background_pixel(&mut self, pixel: u32) -> &mut Self {
        self.value_list.push(Cw::BackPixel(pixel));
        self
    }

    pub fn border_pixel(&mut self, pixel: u32) -> &mut Self {
        self.value_list.push(Cw::BorderPixel(pixel));
        self
    }

    pub fn override_redirect(&mut self) -> &mut Self {
        self.value_list.push(Cw::OverrideRedirect(true));
        self
    }

    pub fn colormap(&mut self, cmap: Colormap) -> &mut Self {
        self.value_list.push(Cw::Colormap(cmap));
        self
    }

    pub fn cursor(&mut self, cursor: Cursor) -> &mut Self {
        self.value_list.push(Cw::Cursor(cursor));
        self
    }
}
