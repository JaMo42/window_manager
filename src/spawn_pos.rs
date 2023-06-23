use crate::{
    client::Client, config::Config, monitors::monitors, rectangle::Rectangle, workspace::Workspace,
};
use std::{collections::BTreeSet, sync::Arc};

struct BasicRect {
    x: i16,
    y: i16,
    is_window: bool,
}

/// This is the core structure that allows this algorithm to run efficent enough
/// because instead of having to check e.g. every pixel of the screen we add_divide
/// it into larger "basic" rectangles such that each of those basic rectangles is
/// either entirely a window or entirely empty.
struct RectangleMap {
    grid: Vec<BasicRect>,
    real_width: usize,
    width: usize,
    height: usize,
}

impl RectangleMap {
    fn new(xs: &BTreeSet<i16>, ys: &BTreeSet<i16>, windows: &[&Arc<Client>]) -> Option<Self> {
        let mut grid = Vec::with_capacity(xs.len() * ys.len());
        // these are one less than the number of edges because the right and
        // bottom edge elements are just used to get the original rectangle sizes.
        let width = xs.len() - 1;
        let height = ys.len() - 1;
        let mut window_count = 0;
        for y in ys.iter().cloned() {
            for x in xs.iter().cloned() {
                let is_window = windows
                    .iter()
                    .any(|client| client.frame_geometry().contains((x, y)));
                if is_window {
                    window_count += 1;
                }
                grid.push(BasicRect { x, y, is_window });
            }
        }
        if window_count == width * height {
            None
        } else {
            Some(Self {
                grid,
                real_width: xs.len(),
                width,
                height,
            })
        }
    }

    /// Get a basic rectangle at a position.
    fn at(&self, x: usize, y: usize) -> &BasicRect {
        &self.grid[y * self.real_width + x]
    }

    /// Returns the real rectangle for a rectangle of basic rectangles.
    fn get_rect(&self, x: usize, y: usize, width: usize, height: usize) -> Rectangle {
        let topleft = self.at(x, y);
        let bottomright = self.at(x + width, y + height);
        Rectangle::new(
            topleft.x,
            topleft.y,
            (bottomright.x - topleft.x) as u16,
            (bottomright.y - topleft.y) as u16,
        )
    }

    /// Checks if a rectangle is within bounds and does not overlap any windows,
    /// returning `true` if that is the case.
    fn check(&self, x: usize, y: usize, width: usize, height: usize) -> bool {
        if x + width > self.width || y + height > self.height {
            return false;
        }
        for check_y in y..y + height {
            let row_offset = check_y * self.real_width;
            for x in x..x + width {
                if self.grid[row_offset + x].is_window {
                    return false;
                }
            }
        }
        true
    }

    /// Returns an iterator over all positions of basic rectangles that are not
    /// within a window.
    fn iter_origins<'a>(&'a self) -> OriginIterator<'a> {
        OriginIterator::new(self)
    }
}

struct OriginIterator<'a> {
    map: &'a RectangleMap,
    x: usize,
    y: usize,
}

impl<'a> OriginIterator<'a> {
    fn new(map: &'a RectangleMap) -> Self {
        Self { map, x: 0, y: 0 }
    }

    fn inc(&mut self) -> Option<()> {
        self.x += 1;
        if self.x == self.map.width {
            self.x = 0;
            self.y += 1;
        }
        if self.y == self.map.height {
            return None;
        }
        Some(())
    }
}

impl<'a> Iterator for OriginIterator<'a> {
    type Item = (usize, usize);

    fn next(&mut self) -> Option<Self::Item> {
        self.inc()?;
        while self.map.at(self.x, self.y).is_window {
            self.inc()?;
        }
        Some((self.x, self.y))
    }
}

struct RectangleBuilder<'a> {
    map: &'a RectangleMap,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    down: bool,
    target_aspect_ratio: f32,
    target_height: u16,
    target_width: u16,
}

impl<'a> RectangleBuilder<'a> {
    fn new(
        map: &'a RectangleMap,
        origin: (usize, usize),
        aspect_ratio: f32,
        target_width: u16,
        target_height: u16,
    ) -> Self {
        let mut this = Self {
            map,
            x: origin.0,
            y: origin.1,
            width: 1,
            height: 1,
            down: true,
            target_aspect_ratio: aspect_ratio,
            target_width,
            target_height,
        };
        this.swap_direction();
        this
    }

    /// Sets the next growth direction so that it stays as close to the target
    /// aspect ratio as possible.
    fn swap_direction(&mut self) {
        let down_r = self.width as f32 / (self.height + 1) as f32;
        let right_r = (self.height + 1) as f32 / self.width as f32;
        self.down =
            (down_r - self.target_aspect_ratio).abs() < (right_r - self.target_aspect_ratio).abs()
    }

    /// Flips the next growth direction.
    fn swap_direction_unchecked(&mut self) {
        self.down = !self.down;
    }

    /// Try increasing the size of the rectangle, return `false` if the new
    /// rectangle would hit a window of leave the screen. If `false` is returned
    /// the rectangle remains unchanged.
    fn try_grow(&mut self) -> bool {
        if self.down {
            self.height += 1;
        } else {
            self.width += 1;
        }
        if !self.map.check(self.x, self.y, self.width, self.height) {
            if self.down {
                self.height -= 1;
            } else {
                self.width -= 1;
            }
            false
        } else {
            let r = self.map.get_rect(self.x, self.y, self.width, self.height);
            if r.width >= self.target_width || r.height >= self.target_height {
                false
            } else {
                true
            }
        }
    }

    /// Converts it into a "real" rectangle with the actual coordinates instead
    /// of RectangleMap coordinates.
    fn into(self) -> Rectangle {
        self.map.get_rect(self.x, self.y, self.width, self.height)
    }
}

/// Finds open rectangles to try to place the new window into.
fn find_open_spaces(
    screen: Rectangle,
    windows: &[&Arc<Client>],
    aspect_ratio: f32,
    target_width: u16,
    target_height: u16,
) -> Option<Vec<Rectangle>> {
    let mut xs = BTreeSet::new();
    let mut ys = BTreeSet::new();
    xs.insert(screen.x);
    xs.insert(screen.x + screen.width as i16);
    ys.insert(screen.y);
    ys.insert(screen.y + screen.height as i16);
    let screen_right = screen.x + screen.width as i16;
    let screen_bottom = screen.y + screen.height as i16;
    for w in windows {
        let frame = w.frame_geometry();
        let right = frame.x + frame.width as i16;
        let bottom = frame.y + frame.height as i16;
        if frame.x > screen.x {
            xs.insert(frame.x);
        }
        if right < screen_right {
            xs.insert(frame.x + frame.width as i16);
        }
        if frame.y > screen.y {
            ys.insert(frame.y);
        }
        if bottom < screen_bottom {
            ys.insert(frame.y + frame.height as i16);
        }
    }
    let map = RectangleMap::new(&xs, &ys, windows)?;
    let mut spaces = Vec::new();
    // doing 3 checks here is kinda ugly but since we reduce the number of
    // rectangles we check with the RectangleMap and provided a cut off value
    // based on the number of windows it shouldn't be a problem.
    for origin in map.iter_origins() {
        // try rectangle with new windows aspect ratio
        let mut builder =
            RectangleBuilder::new(&map, origin, aspect_ratio, target_width, target_height);
        while builder.try_grow() {
            builder.swap_direction();
        }
        builder.swap_direction();
        while builder.try_grow() {}
        let a = builder.into();
        spaces.push(a);

        // try expanding in one direction until we can't and then the other
        builder = RectangleBuilder::new(&map, origin, aspect_ratio, target_width, target_height);
        while builder.try_grow() {}
        builder.swap_direction_unchecked();
        while builder.try_grow() {}
        let b = builder.into();
        if b != a {
            spaces.push(b);
        }

        // as the previous but with swapped directions
        builder = RectangleBuilder::new(&map, origin, aspect_ratio, target_width, target_height);
        builder.swap_direction_unchecked();
        while builder.try_grow() {}
        builder.swap_direction_unchecked();
        while builder.try_grow() {}
        let c = builder.into();
        if c != a {
            spaces.push(c);
        }
    }
    Some(spaces)
}

/// Moves `inner` as clsoe to `point` as possible while staying inside `outer`.
fn move_towards(inner: Rectangle, outer: Rectangle, point: (i16, i16)) -> Rectangle {
    fn get(outer_pos: i16, outer_size: u16, inner_size: u16, point_pos: i16) -> i16 {
        if inner_size > outer_size {
            outer_pos + (outer_size as i16 - inner_size as i16) / 2
        } else {
            let lo = outer_pos;
            let hi = outer_pos + (outer_size as i16 - inner_size as i16);
            (point_pos - inner_size as i16 / 2).clamp(lo, hi)
        }
    }
    Rectangle::new(
        get(outer.x, outer.width, inner.width, point.0),
        get(outer.y, outer.height, inner.height, point.1),
        inner.width,
        inner.height,
    )
}

/// Opposite of `move_towards`
fn move_away(inner: Rectangle, outer: Rectangle, point: (i16, i16)) -> Rectangle {
    fn get(outer_pos: i16, outer_size: u16, inner_size: u16, point_pos: i16) -> i16 {
        if inner_size > outer_size {
            outer_pos + (outer_size as i16 - inner_size as i16) / 2
        } else {
            let lo = outer_pos;
            let hi = outer_pos + (outer_size as i16 - inner_size as i16);
            outer_pos + inner_size as i16 - (point_pos - inner_size as i16 / 2).clamp(lo, hi)
        }
    }
    Rectangle::new(
        get(outer.x, outer.width, inner.width, point.0),
        get(outer.y, outer.height, inner.height, point.1),
        inner.width,
        inner.height,
    )
}

/// Represents a space from `find_open_spaces` and the position the rectangle
/// would have inside it.
struct SpaceInfo {
    space: Rectangle,
    rect: Rectangle,
}

impl SpaceInfo {
    fn new(space: Rectangle, mut rect: Rectangle, screen: Rectangle) -> Self {
        rect = move_towards(rect, space, screen.center());
        rect.clamp_inside(&screen);
        Self { space, rect }
    }

    // Updates the space info so the rectangle is as far away from the center
    // as possible,
    pub fn move_away_from_center(&mut self, original_rect: Rectangle, screen: Rectangle) {
        self.rect = move_away(original_rect, self.space, screen.center());
        self.rect.clamp_inside(&screen);
    }

    // Returns the width of the space
    fn width(&self) -> u16 {
        self.space.width
    }

    // Returns the height of the space
    fn height(&self) -> u16 {
        self.space.height
    }

    // Returns the area of the space
    fn area(&self) -> usize {
        self.space.width as usize * self.space.height as usize
    }

    // Returns the center of the rectangle
    fn center(&self) -> (i16, i16) {
        self.rect.center()
    }

    // Returns the position of the rectangle
    fn position(&self) -> (i16, i16) {
        (self.rect.x, self.rect.y)
    }
}

fn center_distance(a_center: (i16, i16), b: Rectangle) -> f32 {
    let (ax, ay) = a_center;
    let (bx, by) = b.center();
    let dx = (ax - bx) as f32;
    let dy = (ay - by) as f32;
    (dx * dx + dy * dy).sqrt()
}

/// Finds the best position for a new window so it either doesn't overlap any
/// windows or has a minimal overlap.  If the entire window area is already
/// covered with windows `None` is returned.
fn find_position(
    rect: Rectangle,
    screen: Rectangle,
    windows: &[&Arc<Client>],
) -> Option<(i16, i16)> {
    let aspect_ratio = rect.width as f32 / rect.height as f32;
    let mut open_spaces: Vec<_> =
        find_open_spaces(screen, windows, aspect_ratio, rect.width, rect.height)?
            .into_iter()
            .map(|space| SpaceInfo::new(space, rect, screen))
            .collect();
    open_spaces.sort_by_key(|space| {
        // the sort function doesn't likes floats...
        (center_distance(space.center(), screen) * 100.0) as isize
    });
    let idx = open_spaces
        .iter()
        .position(|space| space.width() >= rect.width && space.height() >= rect.height)
        .or_else(|| {
            open_spaces
                .iter_mut()
                .for_each(|space| space.move_away_from_center(rect, screen));
            // if no space can fit the rectangle we use the largest space to
            // minimize overlap of the new window with other windows.
            // This is only determined by the size of the free space which is
            // kinda bad.  It's slightly better now because we stop growing
            // spaces once they are wide enough to fit the new window so we
            // won't get spaces with wildly different aspect ratios but it's
            // not actually try to minimize overlap.
            (0..open_spaces.len()).max_by_key(|&i| open_spaces[i].area())
        })?;
    Some(open_spaces[idx].position())
}

/// Returns the geometry a new client should have when spawning.
pub fn spawn_geometry(
    new_client: &Client,
    current_workspace: &Workspace,
    config: &Config,
) -> Rectangle {
    let window_area = *monitors().primary().window_area();
    let mut frame = new_client.frame_geometry();
    if !config.layout.smart_window_placement {
        frame.random_position_inside(&window_area);
        return frame;
    }
    let windows: Vec<_> = current_workspace
        .clients()
        .iter()
        .filter(|client| !client.is_minimized() && window_area.overlaps(client.frame_geometry()))
        .collect();
    let m = config.layout.smart_window_placement_max;
    if windows.len() == 0 || (m > 0 && windows.len() >= m) {
        frame.random_position_inside(&window_area);
    } else {
        frame.clamp_inside(&window_area);
        if let Some((x, y)) = find_position(frame, window_area, &windows) {
            frame.x = x;
            frame.y = y;
        } else {
            frame.random_position_inside(&window_area);
        }
    }
    frame
}
