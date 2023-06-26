use crate::{
    client::FrameKind,
    config::Config,
    draw::DrawingContext,
    monitors::{monitors, Monitor},
    rectangle::Rectangle,
};
use pango::FontDescription;

pub fn lerp(a: f64, b: f64, w: f64) -> f64 {
    a + w * (b - a)
}

#[derive(Default, Copy, Clone, Debug, Eq, PartialEq)]
struct MonInfo {
    width: u16,
    height: u16,
    dpmm: u32,
}

impl MonInfo {
    fn new(mon: &Monitor) -> Self {
        Self {
            width: mon.geometry().width,
            height: mon.geometry().height,
            dpmm: (mon.dpmm() * 100.0).round() as u32,
        }
    }
}

pub trait Layout {
    fn new(config: &Config, dc: &DrawingContext, monitor: &Monitor) -> Self;
}

/// Checks if `instances` already contains a suitable instance for `monitor`.
fn contains_suitable<L>(instances: &[(MonInfo, L)], info: MonInfo) -> bool {
    instances.iter().any(|(m, _)| *m == info)
}

#[derive(Debug, Default)]
pub struct LayoutClass<L> {
    current_mon: MonInfo,
    instances: Vec<(MonInfo, L)>,
}

impl<L: Layout> LayoutClass<L> {
    pub fn new(config: &Config, dc: &DrawingContext) -> Self {
        let mut instances = Vec::new();
        let monitors = monitors();
        for mon in monitors.iter() {
            let info = MonInfo::new(mon);
            if !contains_suitable(&instances, info) {
                instances.push((info, L::new(config, dc, mon)));
            }
        }
        Self {
            current_mon: MonInfo {
                width: 0,
                height: 0,
                dpmm: 0,
            },
            instances,
        }
    }

    fn lookup(&self, info: MonInfo) -> &L {
        &self
            .instances
            .iter()
            .find(|(m, _)| *m == info)
            .expect("missing instance in layout class")
            .1
    }

    /// Gets the appropriate instance for the given monitor.
    /// Does not update the checked monitor for `get_if_different`.
    pub fn get(&self, monitor: &Monitor) -> &L {
        self.lookup(MonInfo::new(monitor))
    }

    /// Gets the appropriate instance for the given monitor unless it's equal
    /// to the last instance returned by this function.
    pub fn get_if_different(&mut self, monitor: &Monitor) -> Option<&L> {
        let info = MonInfo::new(monitor);
        if info == self.current_mon {
            None
        } else {
            self.current_mon = info;
            Some(self.lookup(info))
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct ClientLayout {
    /// Frame offset for windows of frame kind [`Decorated`](FrameKind::Decorated).
    decorated_frame_offset: Rectangle,
    /// Frame offset for windows of frame kind [`Border`](FrameKind::Border).
    border_frame_offset: Rectangle,
    /// Rectangle for the application icon.
    icon: Rectangle,
    /// Value to add to `title_width_offset` if the window has no icon.
    icon_space: u16,
    /// Area for the window title, the width is always set to 0 as it depends
    /// on the window size.
    title: Rectangle,
    /// Value to remove from frame width to get the width available for the
    /// window title.
    title_width_offset: u16,
    /// Layout the buttons use withing their rectangle. The rectangles where
    /// buttons are positioned are computed in their respective functions.
    button: ButtonLayout,
    /// Gap around snapped clients.
    gap: i16,
    title_font: FontDescription,
    frame_extents: i16,
}

impl Layout for ClientLayout {
    fn new(config: &Config, dc: &DrawingContext, monitor: &Monitor) -> Self {
        let mut title_font = config.window.title_font.clone();
        if config.window.title_font_size > 0 {
            let percent = config.window.title_font_scaling_percent as f64 / 100.0;
            let scaling_factor = lerp(1.0, monitor.scaling_factor(), percent);
            title_font.set_size(
                (config.window.title_font_size as f64 * scaling_factor).round() as i32
                    * pango::SCALE,
            );
        }
        let title_font_height = dc.font_height(Some(&title_font));
        let title_bar_height = config.window.title_bar_height.resolve(
            Some(monitor.dpmm()),
            None,
            Some(title_font_height),
        );
        let left_buttons_width = title_bar_height * config.window.left_buttons.len() as u16;
        let right_buttons_width = title_bar_height * config.window.right_buttons.len() as u16;
        let icon_size = config.window.icon_size.resolve(
            Some(monitor.dpmm()),
            Some(title_bar_height),
            Some(title_font_height),
        );
        let icon_x = left_buttons_width as i16 + 2;
        let icon_y = (title_bar_height - icon_size) as i16 / 2;
        let title_x = icon_x + icon_size as i16 + 2;
        let border =
            config
                .window
                .border
                .resolve(Some(monitor.dpmm()), None, Some(title_font_height)) as i16;
        let border2 = border as u16 * 2;
        let button = ButtonLayout::new(config, monitor.dpmm(), title_bar_height, title_font_height);
        let extended_frame = config
            .window
            .extend_frame
            .resolve(Some(monitor.dpmm()), None, None) as i16;
        Self {
            decorated_frame_offset: Rectangle::new(
                border,
                title_bar_height as i16,
                border2,
                border as u16 + title_bar_height,
            ),
            border_frame_offset: Rectangle::new(border, border, border2, border2),
            icon: Rectangle::new(icon_x, icon_y, icon_size, icon_size),
            icon_space: icon_size + 2,
            title: Rectangle::new(title_x, 0, 0, title_bar_height),
            title_width_offset: title_x as u16 + right_buttons_width + 2,
            button,
            gap: config
                .layout
                .gaps
                .resolve(Some(monitor.dpmm()), None, Some(title_font_height))
                as i16,
            title_font,
            frame_extents: extended_frame,
        }
    }
}

impl ClientLayout {
    pub fn frame_offset(&self, kind: FrameKind) -> &Rectangle {
        static NO_OFFSET: Rectangle = Rectangle::zeroed();
        match kind {
            FrameKind::Decorated => &self.decorated_frame_offset,
            FrameKind::Border => &self.border_frame_offset,
            FrameKind::None => &NO_OFFSET,
        }
    }

    pub fn get_frame(&self, kind: FrameKind, client: &Rectangle) -> Rectangle {
        let offset = self.frame_offset(kind);
        Rectangle::new(
            client.x - offset.x,
            client.y - offset.y,
            client.width + offset.width,
            client.height + offset.height,
        )
    }

    pub fn get_client(&self, kind: FrameKind, frame: &Rectangle) -> Rectangle {
        let offset = self.frame_offset(kind);
        Rectangle::new(
            frame.x + offset.x,
            frame.y + offset.y,
            frame.width - offset.width,
            frame.height - offset.height,
        )
    }

    pub fn reparent_position(&self, kind: FrameKind) -> (i16, i16) {
        let offset = self.frame_offset(kind);
        (offset.x, offset.y)
    }

    pub fn icon_rect(&self) -> &Rectangle {
        &self.icon
    }

    pub fn title_rect(&self, frame_width: u16, has_icon: bool) -> Rectangle {
        let extra = if has_icon { 0 } else { self.icon_space };
        Rectangle::new(
            self.title.x - extra as i16,
            self.title.y,
            frame_width - self.title_width_offset + extra,
            self.title.height,
        )
    }

    pub fn left_button_rect(&self, idx: usize) -> Rectangle {
        let size = self.decorated_frame_offset.y as u16;
        let x = self.decorated_frame_offset.y * idx as i16;
        Rectangle::new(x, 0, size, size)
    }

    pub fn right_button_rect(&self, idx: usize, width: u16) -> Rectangle {
        let size = self.decorated_frame_offset.y as u16;
        let x = width as i16 - self.decorated_frame_offset.y * (idx as i16 + 1);
        Rectangle::new(x, 0, size, size)
    }

    pub fn title_bar_height(&self) -> u16 {
        self.decorated_frame_offset.y as u16
    }

    pub fn gap(&self) -> i16 {
        self.gap
    }

    pub fn button_layout(&self) -> &ButtonLayout {
        &self.button
    }

    pub fn title_font(&self) -> &FontDescription {
        &self.title_font
    }

    pub fn min_size(&self) -> (u16, u16) {
        const INNER_WIDTH: u16 = 160 * 3;
        const INNER_HEIGHT: u16 = 90 * 3;
        (
            self.title_width_offset + INNER_WIDTH,
            self.decorated_frame_offset.height + INNER_HEIGHT,
        )
    }

    pub fn frame_extents(&self) -> i16 {
        self.frame_extents
    }
}

// Doesn't implement `Layout` as we don't need to create a layout class for it
// and want different arguments for the constructor.
#[derive(Copy, Clone, Debug, Default)]
pub struct ButtonLayout {
    size: u16,
    icon: Rectangle,
    circle: Rectangle,
}

impl ButtonLayout {
    pub fn new(config: &Config, dpmm: f64, title_bar_height: u16, title_font_height: u16) -> Self {
        let icon_size = config.window.button_icon_size.resolve(
            Some(dpmm),
            Some(title_bar_height),
            Some(title_font_height),
        );
        if config.window.circle_buttons {
            let f_size = title_bar_height as f64;
            let circle_diameter = icon_size as f64;
            let f_icon_size = 2.0 * f64::sqrt(f64::powi(circle_diameter / 2.0, 2) / 2.0);
            let circle_size = circle_diameter.round() as u16;
            let circle_position = ((f_size - circle_diameter) / 2.0).round() as i16;
            let icon_size = f_icon_size.ceil() as u16;
            let icon_position = ((f_size - f_icon_size) / 2.0).round() as i16;
            Self {
                size: title_bar_height,
                icon: Rectangle::new(icon_position, icon_position, icon_size, icon_size),
                circle: Rectangle::new(circle_position, circle_position, circle_size, circle_size),
            }
        } else {
            let icon_position = (title_bar_height - icon_size) as i16 / 2;
            Self {
                size: title_bar_height,
                icon: Rectangle::new(icon_position, icon_position, icon_size, icon_size),
                circle: Rectangle::zeroed(),
            }
        }
    }

    pub fn size(&self) -> u16 {
        self.size
    }

    pub fn icon_rect(&self) -> &Rectangle {
        &self.icon
    }

    pub fn circle_rect(&self) -> &Rectangle {
        &self.circle
    }
}
