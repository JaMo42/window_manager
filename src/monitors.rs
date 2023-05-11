use crate::{
    client::Client,
    config::{Config, Size},
    rectangle::Rectangle,
    x::{randr, Display, Window},
};
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::sync::Arc;

static MONITORS: RwLock<Monitors> = RwLock::new(Monitors::new());

/// Acquires the monitors list.
pub fn monitors() -> RwLockReadGuard<'static, Monitors> {
    MONITORS.read()
}

/// Acquires the monitors list.
pub fn monitors_mut() -> RwLockWriteGuard<'static, Monitors> {
    MONITORS.write()
}

fn dpi(dpmm: f64) -> u32 {
    (dpmm * 25.4).round() as u32
}

#[derive(Copy, Clone, Debug)]
pub struct WindowAreaPadding {
    pub top: Size,
    pub bottom: Size,
    pub left: Size,
    pub right: Size,
}

impl WindowAreaPadding {
    /// Checks that none of the size values are `PercentOfFont`.
    #[rustfmt::skip]
    pub fn is_valid(&self) -> bool {
           !matches!(self.top, Size::PercentOfFont(_))
        && !matches!(self.bottom, Size::PercentOfFont(_))
        && !matches!(self.left, Size::PercentOfFont(_))
        && !matches!(self.right, Size::PercentOfFont(_))
    }
}

#[derive(Debug, Clone)]
pub struct Monitor {
    geometry: Rectangle,
    window_area: Rectangle,
    dpmm: f64,
    scaling_factor: f64,
    name: String,
    index: usize,
}

impl Monitor {
    fn from_randr((index, monitor): (usize, randr::Monitor)) -> Self {
        let geometry = Rectangle::new(monitor.x, monitor.y, monitor.width, monitor.height);
        Self {
            geometry,
            // Real window geometry is set later as the first time we query
            // the monitors the configuration is not loaded.
            window_area: geometry,
            dpmm: monitor.dpmm,
            scaling_factor: 1.0,
            name: monitor.name.unwrap_or_else(|| "?".to_string()),
            index,
        }
    }

    fn new(name: &str, index: usize, geometry: Rectangle, dpmm: f64) -> Self {
        Self {
            geometry,
            window_area: geometry,
            dpmm,
            scaling_factor: 1.0,
            name: name.to_string(),
            index,
        }
    }

    fn set_window_area(&mut self, padding: &WindowAreaPadding) {
        let top = padding
            .top
            .resolve(Some(self.dpmm), Some(self.geometry.height), None);
        let bottom = padding
            .bottom
            .resolve(Some(self.dpmm), Some(self.geometry.height), None);
        let left = padding
            .left
            .resolve(Some(self.dpmm), Some(self.geometry.width), None);
        let right = padding
            .right
            .resolve(Some(self.dpmm), Some(self.geometry.width), None);
        self.window_area = Rectangle::new(
            self.geometry.x + left as i16,
            self.geometry.y + top as i16,
            self.geometry.width - (left + right),
            self.geometry.height - (top + bottom),
        );
    }

    pub fn geometry(&self) -> &Rectangle {
        &self.geometry
    }

    pub fn window_area(&self) -> &Rectangle {
        assert!(self.geometry != self.window_area);
        &self.window_area
    }

    pub fn dpmm(&self) -> f64 {
        self.dpmm
    }

    /// Get the scaling factor relative to the primary monitor.
    pub fn scaling_factor(&self) -> f64 {
        self.scaling_factor
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn index(&self) -> usize {
        self.index
    }
}

impl PartialEq for Monitor {
    fn eq(&self, other: &Self) -> bool {
        self.geometry == other.geometry
            && self.name == other.name
            && (self.dpmm - other.dpmm).abs() <= 0.01
    }
}

pub struct Monitors {
    monitors: Vec<Monitor>,
    primary: usize,
    bar_height: u16,
}

impl Monitors {
    /// Default to 144 dpi if querying fails.
    const DEFAULT_DPMM: f64 = 144.0 / 25.4;

    pub const fn new() -> Self {
        Self {
            monitors: Vec::new(),
            primary: 0,
            bar_height: 0,
        }
    }

    /// Query monitors using randr.
    pub fn query(&mut self, display: &Arc<Display>) -> &mut Self {
        self.monitors.clear();
        match randr::query_screens(display) {
            Ok(mons) => {
                self.primary = mons.iter().position(|m| m.is_primary).unwrap_or(0);
                self.monitors = mons
                    .into_iter()
                    .enumerate()
                    .map(Monitor::from_randr)
                    .collect();
            }
            Err(error) => {
                log::error!("Failed to query monitors: {error}");
                log::error!(
                    "Using root window dimensions as screen size and defaulting DPI to {}.",
                    dpi(Self::DEFAULT_DPMM)
                );
                let root = Window::from_handle(display.clone(), display.root());
                let geometry = root.get_geometry().into();
                self.monitors = vec![Monitor::new(
                    "<root window>",
                    0,
                    geometry,
                    Self::DEFAULT_DPMM,
                )];
                self.primary = 0;
            }
        }
        let primary_dpmm = self.primary().dpmm();
        for mon in self.monitors.iter_mut() {
            mon.scaling_factor = mon.dpmm / primary_dpmm;
        }
        self
    }

    /// Queries monitors and returns `true` if something changed.
    /// This also sets the window areas.
    pub fn update(&mut self, display: &Arc<Display>, config: &Config) -> bool {
        let old = self.monitors.clone();
        self.query(display);
        self.set_window_areas(&config.layout.padding, &config.layout.secondary_padding);
        self.monitors != old
    }

    pub fn set_window_areas(
        &mut self,
        primary_padding: &WindowAreaPadding,
        secondary_padding: &WindowAreaPadding,
    ) {
        for m in self.monitors.iter_mut() {
            m.set_window_area(if m.index == self.primary {
                primary_padding
            } else {
                secondary_padding
            });
        }
    }

    pub fn set_bar_height(&mut self, height: u16) {
        let p = &mut self.monitors[self.primary];
        p.window_area.y -= self.bar_height as i16;
        p.window_area.y += height as i16;
        self.bar_height = height;
    }

    /// Print monitors info to log.
    pub fn log(&self) {
        log::info!("Monitors:");
        for (i, mon) in self.monitors.iter().enumerate() {
            log::info!(
                "  {}: {}, {} dpi{}",
                mon.name(),
                mon.geometry(),
                dpi(mon.dpmm()),
                if i == self.primary { ", Primary" } else { "" }
            );
        }
    }

    /// Returns the number of monitors.
    pub fn len(&self) -> usize {
        self.monitors.len()
    }

    /// Returns the primary monitor.
    pub fn primary(&self) -> &Monitor {
        &self.monitors[self.primary]
    }

    /// Finds the closest monitor to the given point.
    fn find_closest(&self, point: (i16, i16)) -> &Monitor {
        fn distance((x, y): (i16, i16), g: &Rectangle) -> f32 {
            let dx = i16::max(g.x - x, x - g.x + g.width as i16) as f32;
            let dy = i16::max(g.y - y, y - g.y + g.height as i16) as f32;
            (dx * dx + dy * dy).sqrt()
        }
        if self.monitors.len() == 1 {
            return self.primary();
        }
        let mut idx = 0;
        let mut min_d = distance(point, self.monitors[0].geometry());
        for (i, mon) in self.monitors.iter().enumerate().skip(1) {
            let d = distance(point, mon.geometry());
            if d < min_d {
                idx = i;
                min_d = d;
            }
        }
        &self.monitors[idx]
    }

    /// Finds the monitor containing the given point. If it lies outside of all
    /// monitors the closest monitor is returned.
    pub fn at(&self, point: (i16, i16)) -> &Monitor {
        self.monitors
            .iter()
            .find(|m| m.geometry.contains(point))
            .unwrap_or_else(|| self.find_closest(point))
    }

    /// Returns the monitor containing the given client.
    pub fn containing(&self, client: &Client) -> &Monitor {
        let center = client.saved_geometry().center();
        self.at(center)
    }

    /// Returns the monitor at the given index. If the provided value is out of
    /// bounds the monitor at the nearest bound is returned.
    pub fn get(&self, idx: isize) -> &Monitor {
        if idx < 0 {
            &self.monitors[0]
        } else if (idx as usize) >= self.monitors.len() {
            self.monitors.last().unwrap()
        } else {
            &self.monitors[idx as usize]
        }
    }

    /// Returns an iterator over all monitors.
    pub fn iter(&self) -> std::slice::Iter<Monitor> {
        self.monitors.iter()
    }
}
