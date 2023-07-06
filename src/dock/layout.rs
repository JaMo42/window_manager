use crate::{config::Config, monitors::monitors, rectangle::Rectangle};

#[derive(Default, Debug)]
pub struct DockLayout {
    height: u16,
    y: i16,
    screen_width: u16,
    padding: u16,
    item: Rectangle,
    icon: Rectangle,
    show_window: Rectangle,
    offset: i16,
}

impl DockLayout {
    pub fn compute(&mut self, config: &Config) {
        let dpmm = monitors().primary().dpmm();
        let screen_rect = *monitors().primary().geometry();
        self.height = config
            .dock
            .height
            .resolve(Some(dpmm), Some(screen_rect.height), None);
        self.offset = config
            .dock
            .offset
            .resolve(Some(dpmm), Some(screen_rect.height), None) as i16;
        self.y = (screen_rect.height - self.height) as i16 - self.offset;
        self.screen_width = screen_rect.width;
        // TODO: maybe use percentage of height of something
        self.padding = 15;
        let item_size = config
            .dock
            .item_size
            .resolve(Some(dpmm), Some(self.height), None);
        let item_y = (self.height - item_size) as i16 / 2;
        self.item = Rectangle::new(0, item_y, item_size, item_size);
        let icon_size = config
            .dock
            .icon_size
            .resolve(Some(dpmm), Some(item_size), None);
        let icon_pos = (item_size - icon_size) as i16 / 2;
        self.icon = Rectangle::new(icon_pos, icon_pos, icon_size, icon_size);
        let show_window_height = 100;
        self.show_window = Rectangle::new(
            0,
            (screen_rect.height - show_window_height) as i16,
            screen_rect.width,
            show_window_height,
        );
    }

    pub fn dock(&self, items: usize) -> Rectangle {
        let width = self.padding + (self.item.width + self.padding) * items as u16;
        let x = (self.screen_width - width) as i16 / 2;
        Rectangle::new(x, self.y, width, self.height)
    }

    pub fn item(&self, idx: usize) -> Rectangle {
        self.item
            .with_x(self.padding as i16 + (self.item.width + self.padding) as i16 * idx as i16)
    }

    pub fn icon(&self) -> Rectangle {
        self.icon
    }

    pub fn show_window(&self) -> Rectangle {
        self.show_window
    }

    pub fn small_show_window(&self) -> Rectangle {
        const HEIGHT: u16 = 1;
        let mut base = self.show_window();
        base.y += (base.height - HEIGHT) as i16;
        base.height = HEIGHT;
        base
    }

    pub fn is_offset(&self) -> bool {
        self.offset > 0
    }
}
