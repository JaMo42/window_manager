use crate::client::Client;
use crate::core::*;
use crate::geometry::Geometry;

pub struct Monitor {
  number: i32,
  geometry: Geometry,
  window_area: Geometry,
}

impl Monitor {
  pub fn new (number: i32, geometry: Geometry) -> Self {
    Self {
      number,
      geometry,
      window_area: geometry,
    }
  }

  fn set_window_area (&mut self, margin: (i32, i32, i32, i32)) {
    self.window_area = Geometry::from_parts (
      self.geometry.x + margin.2,
      self.geometry.y + margin.0,
      self.geometry.w - (margin.2 + margin.3) as u32,
      self.geometry.h - (margin.0 + margin.1) as u32,
    );
  }

  pub fn number (&self) -> i32 {
    self.number
  }

  pub fn geometry (&self) -> &Geometry {
    &self.geometry
  }

  pub fn window_area (&self) -> &Geometry {
    &self.window_area
  }
}

static mut monitors: Vec<Monitor> = Vec::new ();

pub unsafe fn query () {
  if let Some (screens) = display.query_screens () {
    log::info! ("Monitors:");
    for screen in screens.iter () {
      log::info! (
        "  {}: {}x{}+{}+{}",
        screen.screen_number,
        screen.width,
        screen.height,
        screen.x_org,
        screen.y_org
      );
    }
    monitors = screens
      .iter ()
      .map (|info| {
        Monitor::new (
          info.screen_number,
          Geometry::from_parts (
            info.x_org as i32,
            info.y_org as i32,
            info.width as u32,
            info.height as u32,
          ),
        )
      })
      .collect ();
  } else {
    log::info! ("Xinerama inacitve");
    log::info! ("Display size: {}x{}", screen_size.w, screen_size.h);
    monitors.push (Monitor::new (0, screen_size.clone ()));
  }
}

pub fn main () -> &'static Monitor {
  // isn't it always the first?
  unsafe { &monitors }
    .iter ()
    .find (|m| m.number () == 0)
    .unwrap_or (unsafe { &monitors }.first ().unwrap ())
}

/// Returns the monitor containing the given point. If no monitor contains it
/// the main monitor is returned.
pub fn at (x: i32, y: i32) -> &'static Monitor {
  unsafe { &monitors }
    .iter ()
    .find (|m| m.geometry.contains (x, y))
    .unwrap_or_else (|| main ())
}

/// Gets the monitor containing the client.
pub fn containing (client: &mut Client) -> &'static Monitor {
  // We use the saved geometry so when moving a snapped client we can just
  // move it's saved geometry and re-snap it.
  // For unsnapped windows this is the same as `frame_geometry`.
  let (x, y) = client.saved_geometry ().center_point ();
  at (x, y)
}

pub fn get (number: i32) -> Option<&'static Monitor> {
  unsafe { monitors.iter () }.find (|m| m.number == number)
}

pub fn set_window_areas (
  main_margin: (i32, i32, i32, i32),
  secondary_margin: (i32, i32, i32, i32),
) {
  for monitor in unsafe { monitors.iter_mut () } {
    monitor.set_window_area (if monitor.number == 0 {
      main_margin
    } else {
      secondary_margin
    });
  }
}
