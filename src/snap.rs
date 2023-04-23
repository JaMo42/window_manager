use crate::{monitors::Monitor, rectangle::Rectangle, split_handles::Splits};

// We have little enough values that we can use an enum instead of bit flags.
/// A clients snap state
#[derive(Copy, Clone, Debug)]
pub enum SnapState {
    None,
    Left,
    TopLeft,
    BottomLeft,
    Right,
    TopRight,
    BottomRight,
    Maximized,
}

impl SnapState {
    /// Returns the snap state for snap moving.
    pub fn move_snap_state(x: i16, y: i16, monitor: &Monitor) -> Self {
        let area = monitor.geometry();
        let mut state = if x < area.width as i16 / 2 {
            Self::Left
        } else {
            Self::Right
        };
        let v = area.height / 4;
        if y < v as i16 {
            state.snap_up();
        } else if y > (area.height - v) as i16 {
            state.snap_down();
        }
        state
    }

    /// Returns the snap state for dragging a client to the edge of a monitor.
    pub fn move_edge_state(x: i16, y: i16, monitor: &Monitor) -> Self {
        // Shrink it by 1 so we also trigger a snap when we are on the edge
        // of the window area, this allows move snapping to work when padding
        // on the monitor is set to 0.
        let area = *monitor.window_area().clone().resize(-1);
        let mut state = if x < area.x {
            Self::Left
        } else if x > (area.x + area.width as i16) {
            Self::Right
        } else {
            Self::None
        };
        if y < area.y {
            if state.is_none() {
                state = Self::Maximized
            } else {
                state.snap_up();
            }
        } else if y > (area.y + area.height as i16) {
            if state.is_none() {
                state = Self::Maximized
            } else {
                state.snap_down();
            }
        }
        state
    }

    /// Is it `None`?
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Is it not `None`?
    pub fn is_snapped(&self) -> bool {
        !self.is_none()
    }

    /// Is it snapped to the left?
    pub fn is_left(&self) -> bool {
        matches!(self, Self::Left | Self::TopLeft | Self::BottomLeft)
    }

    /// Is it snapped to the right?
    pub fn is_right(&self) -> bool {
        matches!(self, Self::Right | Self::TopRight | Self::BottomRight)
    }

    /// Is it snapped to the top?
    pub fn is_top(&self) -> bool {
        matches!(self, Self::TopLeft | Self::TopRight)
    }

    /// Is it snapped to the bottom?
    pub fn is_bottom(&self) -> bool {
        matches!(self, Self::BottomLeft | Self::BottomRight)
    }

    /// Is it `Maximized`?
    pub fn is_maximized(&self) -> bool {
        matches!(self, Self::Maximized)
    }

    /// Converts the state to its `Top` variant, if applicable.
    pub fn snap_up(&mut self) {
        if self.is_left() {
            *self = Self::TopLeft;
        } else if self.is_right() {
            *self = Self::TopRight;
        }
    }

    /// Converts the state to its `Bottom` variant, if applicable.
    pub fn snap_down(&mut self) {
        if self.is_left() {
            *self = Self::BottomLeft;
        } else if self.is_right() {
            *self = Self::BottomRight;
        }
    }

    pub fn snap_left(&mut self) {
        match self {
            Self::TopRight => *self = Self::TopLeft,
            Self::BottomRight => *self = Self::BottomLeft,
            _ => *self = Self::Left,
        }
    }

    pub fn snap_right(&mut self) {
        match self {
            Self::TopLeft => *self = Self::TopRight,
            Self::BottomLeft => *self = Self::BottomRight,
            _ => *self = Self::Right,
        }
    }

    /// Get the snap geometry for the given monitor.
    pub fn get_geometry(&self, splits: Splits, monitor: &Monitor, gaps: i16) -> Rectangle {
        let window_area = monitor.window_area();
        let mut target = *window_area;
        if self.is_maximized() {
            // We don't care about the gap for maximized windows so we add it here
            // since it gets removed inside `client.move_and_resize` again.
            target.resize(gaps);
        } else if self.is_left() {
            target.x = window_area.x;
            target.width = splits.vertical() as u16;
            if self.is_top() {
                target.height = splits.left() as u16;
            } else if self.is_bottom() {
                target.y += splits.left();
                target.height = window_area.height - splits.left() as u16;
            }
        } else if self.is_right() {
            target.x = window_area.x + splits.vertical();
            target.width = window_area.width - splits.vertical() as u16;
            if self.is_top() {
                target.height = splits.right() as u16;
            } else if self.is_bottom() {
                target.y += splits.right();
                target.height = window_area.height - splits.right() as u16;
            }
        } else if cfg!(debug_assertions) {
            log::warn!("Requested snap geometry for un-snapped state");
        }
        target
    }
}
