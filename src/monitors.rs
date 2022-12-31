use crate::client::Client;
use crate::core::*;
use crate::geometry::Geometry;

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Monitor {
  number: i32,
  geometry: Geometry,
  window_area: Geometry,
  index: usize,
}

impl Monitor {
  pub fn new(number: i32, geometry: Geometry) -> Self {
    Self {
      number,
      geometry,
      window_area: geometry,
      index: usize::MAX,
    }
  }

  fn set_window_area(&mut self, margin: (i32, i32, i32, i32)) {
    self.window_area = Geometry::from_parts(
      self.geometry.x + margin.2,
      self.geometry.y + margin.0,
      self.geometry.w - (margin.2 + margin.3) as u32,
      self.geometry.h - (margin.0 + margin.1) as u32,
    );
  }

  pub fn number(&self) -> i32 {
    self.number
  }

  pub fn geometry(&self) -> &Geometry {
    &self.geometry
  }

  pub fn window_area(&self) -> &Geometry {
    &self.window_area
  }

  pub fn index(&self) -> usize {
    self.index
  }
}

static mut monitors: Vec<Monitor> = Vec::new();
static mut max_width: u32 = 0;
static mut max_height: u32 = 0;

pub unsafe fn query() {
  if let Some(screens) = display.query_screens() {
    log::info!("Monitors:");
    for screen in screens.iter() {
      log::info!(
        "  {}: {}x{}+{}+{}",
        screen.screen_number,
        screen.width,
        screen.height,
        screen.x_org,
        screen.y_org
      );
    }
    max_width = 0;
    max_height = 0;
    let mut index = 0;
    monitors = screens
      .iter()
      .map(|info| {
        max_width = u32::max(max_width, info.width as u32);
        max_height = u32::max(max_height, info.height as u32);
        let mut mon = Monitor::new(
          info.screen_number,
          Geometry::from_parts(
            info.x_org as i32,
            info.y_org as i32,
            info.width as u32,
            info.height as u32,
          ),
        );
        mon.index = index;
        index += 1;
        mon
      })
      .collect();
  } else {
    log::info!("Xinerama inacitve");
    log::info!("Display size: {}x{}", screen_size.w, screen_size.h);
    monitors.push(Monitor::new(0, screen_size));
    max_width = screen_size.w;
    max_height = screen_size.h;
  }
}

pub fn main() -> &'static Monitor {
  // isn't it always the first?
  unsafe { &monitors }
    .iter()
    .find(|m| m.number() == 0)
    .unwrap_or_else(|| unsafe { &monitors }.first().unwrap())
}

fn find_closest(x: i32, y: i32) -> &'static Monitor {
  fn distance(x: i32, y: i32, g: &Geometry) -> f32 {
    let dx = i32::max(0, i32::max(g.x - x, x - g.x + g.w as i32)) as f32;
    let dy = i32::max(0, i32::max(g.y - y, y - g.y + g.h as i32)) as f32;
    (dx * dx + dy * dy).sqrt()
  }
  if unsafe { monitors.len() } == 1 {
    return main();
  }
  let mut idx = 0;
  unsafe {
    let mut min_d = distance(x, y, &monitors[0].geometry);
    for (i, mon) in monitors.iter().enumerate().skip(1) {
      let d = distance(x, y, &mon.geometry);
      if d < min_d {
        min_d = d;
        idx = i;
      }
    }
    &monitors[idx]
  }
}

/// Returns the monitor containing the given point. If no monitor contains it
/// the main monitor is returned.
pub fn at(x: i32, y: i32) -> &'static Monitor {
  unsafe { &monitors }
    .iter()
    .find(|m| m.geometry.contains(x, y))
    .unwrap_or_else(|| find_closest(x, y))
}

/// Gets the monitor containing the client.
pub fn containing(client: &Client) -> &'static Monitor {
  // We use the saved geometry so when moving a snapped client we can just
  // move it's saved geometry and re-snap it.
  // For unsnapped windows this is the same as `frame_geometry`.
  let (x, y) = client.saved_geometry().center_point();
  at(x, y)
}

pub fn get(number: i32) -> Option<&'static Monitor> {
  unsafe { monitors.iter() }.find(|m| m.number == number)
}

pub fn at_index(idx: usize) -> &'static Monitor {
  unsafe { &monitors[idx] }
}

pub fn set_window_areas(main_margin: (i32, i32, i32, i32), secondary_margin: (i32, i32, i32, i32)) {
  for monitor in unsafe { monitors.iter_mut() } {
    monitor.set_window_area(if monitor.number == 0 {
      main_margin
    } else {
      secondary_margin
    });
  }
}

pub unsafe fn update() -> bool {
  let old = monitors.clone();
  monitors.clear();
  query();
  !(old.len() == monitors.len() && monitors.iter().zip(&old).all(|(new, old)| new == old))
}

/// Maximum width and height of any monitor (both values don't need to come from
/// the same monitor).
pub fn max_size() -> (u32, u32) {
  unsafe { (max_width, max_height) }
}

pub fn count() -> usize {
  unsafe { monitors.len() }
}
