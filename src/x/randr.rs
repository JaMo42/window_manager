use super::{Display, Window};
use std::sync::Arc;
use xcb::randr::{GetMonitors, MonitorInfo};

pub struct Monitor {
    pub name: Option<String>,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub dpmm: f64,
    pub is_primary: bool,
}

impl Monitor {
    fn new(name: Option<String>, info: &MonitorInfo) -> Self {
        let horizontal_dpmm = info.width() as f64 / info.width_in_millimeters() as f64;
        let vertical_dpmm = info.height() as f64 / info.height_in_millimeters() as f64;
        let dpmm = f64::min(horizontal_dpmm, vertical_dpmm);
        Self {
            name,
            x: info.x(),
            y: info.y(),
            width: info.width(),
            height: info.height(),
            dpmm,
            is_primary: info.primary(),
        }
    }
}

pub fn query_screens(display: &Display) -> xcb::Result<Vec<Monitor>> {
    Ok(display
        .request_with_reply(&GetMonitors {
            window: display.root(),
            get_active: true,
        })?
        .monitors()
        .map(|info| {
            let name = display.get_atom_name(info.name());
            Monitor::new(name, info)
        })
        .collect())
}

pub fn main_monitor_geometry(display: &Arc<Display>) -> (i16, i16, u16, u16) {
    if let Ok(mons) = query_screens(display) {
        if let Some(main) = mons.into_iter().find(|m| m.is_primary) {
            return (main.x, main.y, main.width, main.height);
        }
    }
    Window::from_handle(display.clone(), display.root()).get_geometry()
}
