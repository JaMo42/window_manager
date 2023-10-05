use crate::{
    color::Color,
    draw::{ColorKind, DrawingContext},
    ewmh::{self, WindowType},
    icccm::ClassHint,
    monitors::{monitors, Monitor},
    mouse::{FinishReason, TrackedMotion},
    rectangle::Rectangle,
    window_manager::{WindowKind, WindowManager},
    x::Window,
};
use std::cell::RefCell;
use xcb::x::{EventMask, KeyButMask};

#[derive(Copy, Clone, Debug)]
pub enum Role {
    Left,
    Vertical,
    Right,
}

#[derive(Copy, Clone, Debug)]
pub struct Splits {
    vertical: i16,
    left: i16,
    right: i16,
}

impl Splits {
    pub const fn new(vertical: i16, left: i16, right: i16) -> Self {
        Self {
            vertical,
            left,
            right,
        }
    }

    pub fn left(&self) -> i16 {
        self.left
    }

    pub fn vertical(&self) -> i16 {
        self.vertical
    }

    pub fn right(&self) -> i16 {
        self.right
    }
}

fn stick(p: i16, sticky_points: &[i16], threshold: i16) -> i16 {
    sticky_points
        .iter()
        .copied()
        .find(|sticky| {
            let lo = *sticky - threshold;
            let hi = *sticky + threshold;
            p >= lo && p <= hi
        })
        .unwrap_or(p)
}

pub struct SplitHandle {
    window: Window,
    geometry: Rectangle,
    role: Role,
    monitor: usize,
}

impl SplitHandle {
    fn new(wm: &WindowManager, geometry: Rectangle, role: Role, monitor: usize) -> Self {
        let visual = wm.display.truecolor_visual();
        let window = Window::builder(wm.display.clone())
            .geometry(geometry)
            .depth(visual.depth)
            .visual(visual.id)
            .attributes(|attributes| {
                attributes
                    .override_redirect()
                    .background_pixel(0)
                    .border_pixel(0)
                    .colormap(visual.colormap)
                    .event_mask(
                        EventMask::ENTER_WINDOW | EventMask::LEAVE_WINDOW | EventMask::BUTTON_PRESS,
                    );
            })
            .build();
        wm.set_window_kind(&window, WindowKind::SplitHandle);
        ewmh::set_window_type(&window, WindowType::Desktop);
        ClassHint::new("Window_manager_split_handle", "window_manager_split_handle").set(&window);
        Self {
            window,
            geometry,
            role,
            monitor,
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    fn is_horizontal(&self) -> bool {
        !matches!(self.role, Role::Vertical)
    }

    pub fn draw_clicked(&self, dc: &DrawingContext) {
        dc.rect(self.geometry.at(0, 0))
            .corner_percent(0.49)
            .clear_below_corners()
            .color(Color::new(0.05, 0.05, 0.05, 0.9))
            .stroke(1, ColorKind::Solid(Color::new_rgb(0.1, 0.1, 0.1)))
            .draw();
        let lines: [Rectangle; 2] = if self.is_horizontal() {
            let length = self.geometry.height * 2;
            let width = (self.geometry.height as f64 / 5.0) as u16;
            let x = (self.geometry.width - length) as i16 / 2;
            let y1 = width as i16;
            let y2 = (self.geometry.height - 2 * width) as i16;
            [
                Rectangle::new(x, y1, length, width),
                Rectangle::new(x, y2, length, width),
            ]
        } else {
            let length = self.geometry.width * 2;
            let width = (self.geometry.width as f64 / 5.0) as u16;
            let y = (self.geometry.height - length) as i16 / 2;
            let x1 = width as i16;
            let x2 = (self.geometry.width - 2 * width) as i16;
            [
                Rectangle::new(x1, y, width, length),
                Rectangle::new(x2, y, width, length),
            ]
        };
        for line in lines {
            dc.rect(line).color(Color::new_rgb(0.95, 0.95, 0.95)).draw()
        }
        dc.render(&self.window, self.geometry.at(0, 0));
    }

    pub fn raise_and_draw_hovered(&self, dc: &DrawingContext) {
        self.window.raise();
        dc.rect(self.geometry.at(0, 0))
            .color(Color::new(0.5, 0.5, 0.5, 0.7))
            .corner_percent(0.49)
            .clear_below_corners()
            .stroke(1, ColorKind::Solid(Color::new_rgb(0.5, 0.5, 0.5)))
            .draw();
        dc.render(&self.window, self.geometry.at(0, 0));
    }

    pub fn lower_and_clear(&self) {
        self.window.lower();
        self.window.clear();
    }

    /// Set the size of the window to the stored geometry.
    pub fn update_window_geometry(&self) {
        self.window.move_and_resize(self.geometry);
    }

    pub fn set_position(&mut self, position: i16) {
        if self.is_horizontal() {
            self.geometry.y = position;
        } else {
            self.geometry.x = position;
        }
        self.update_window_geometry();
    }

    /// Sets the width of the left/right split handles.
    pub fn resize(&mut self, to: u16) {
        // This is not used for updating the size when the monitor geometry
        // changes so the vertical handles size cannot be changed.
        assert!(self.is_horizontal());
        if let Role::Right = self.role {
            self.geometry.x += self.geometry.width as i16 - to as i16;
        }
        self.geometry.width = to;
        self.update_window_geometry();
    }

    pub fn mouse_move(&mut self, start_x: i16, start_y: i16, wm: &WindowManager) -> Option<i16> {
        self.draw_clicked(&wm.drawing_context.lock());
        let reset_geometry = self.geometry;
        let window_area = *monitors().get(self.monitor as isize).window_area();
        let cursor = if self.is_horizontal() {
            wm.cursors.resizing_vertical
        } else {
            wm.cursors.resizing_horizontal
        };
        let sticky_points: Vec<_> = {
            let (base, size) = if self.is_horizontal() {
                (window_area.y, window_area.height)
            } else {
                (window_area.x, window_area.width)
            };
            let percentages = &wm.config.split_handles.horizontal_sticky;
            percentages
                .iter()
                .map(|p| base + (size as u32 * *p as u32 / 100) as i16)
                .collect()
        };
        let sticky_threshold = if self.is_horizontal() {
            self.geometry.height as i16
        } else {
            self.geometry.width as i16
        };
        let (min, max) = if self.is_horizontal() {
            let min = window_area.height * wm.config.split_handles.min_split_size / 100;
            (min as i16, (window_area.height - min) as i16)
        } else {
            let min = window_area.width * wm.config.split_handles.min_split_size / 100;
            (min as i16, (window_area.width - min) as i16)
        };
        let mut pos = None;
        let this = RefCell::new(self);
        TrackedMotion::new(wm.display.clone())
            .on_motion(&mut |motion, _, _| {
                let mut this = this.borrow_mut();
                let is_shift = motion.state().contains(KeyButMask::SHIFT);
                if this.is_horizontal() {
                    let y = motion.event_y() - start_y;
                    if is_shift {
                        this.geometry.y = y;
                    } else {
                        this.geometry.y = stick(y, &sticky_points, sticky_threshold);
                    }
                    this.geometry.y = this.geometry.y.clamp(min, max);
                } else {
                    let x = motion.event_x() - start_x;
                    if is_shift {
                        this.geometry.x = x;
                    } else {
                        this.geometry.x = stick(x, &sticky_points, sticky_threshold);
                    }
                    this.geometry.x = this.geometry.x.clamp(min, max);
                }
                this.update_window_geometry();
            })
            .on_finish(&mut |finish_reason| {
                if let FinishReason::Finish(_, _) = finish_reason {
                    let this = this.borrow();
                    pos = Some(if this.is_horizontal() {
                        this.geometry.y
                    } else {
                        this.geometry.x
                    });
                    this.raise_and_draw_hovered(&wm.drawing_context.lock());
                } else {
                    let mut this = this.borrow_mut();
                    this.geometry = reset_geometry;
                    this.update_window_geometry();
                }
            })
            .cancel_on_escape()
            .run(cursor);
        pos
    }
}

/// The split handles on a single monitor, on a single workspace.
pub struct SplitHandles {
    vertical_handle: SplitHandle,
    left_handle: SplitHandle,
    right_handle: SplitHandle,
    window_area: Rectangle,
    size: u16,
    vertical: i16,
    left: i16,
    right: i16,
    monitor_name: String,
    pub vertical_clients: u16,
    pub left_clients: u16,
    pub right_clients: u16,
}

impl SplitHandles {
    pub fn with_percentages(
        wm: &WindowManager,
        mon: &Monitor,
        percentages: &(f64, f64, f64),
    ) -> Self {
        let size = wm
            .config
            .split_handles
            .size
            .resolve(Some(mon.dpmm()), None, None);
        let window_area = *mon.window_area();
        let vertical = (window_area.width as f64 * percentages.0) as i16;
        let left = (window_area.height as f64 * percentages.1) as i16;
        let right = (window_area.height as f64 * percentages.2) as i16;
        let size_offset = size as i16 / 2;
        let vertical_handle = SplitHandle::new(
            wm,
            Rectangle::new(
                window_area.x + vertical - size_offset,
                window_area.y,
                size,
                window_area.height,
            ),
            Role::Vertical,
            mon.index(),
        );
        let left_handle = SplitHandle::new(
            wm,
            Rectangle::new(
                window_area.x,
                window_area.y + left - size_offset,
                vertical as u16,
                size,
            ),
            Role::Left,
            mon.index(),
        );
        let right_handle = SplitHandle::new(
            wm,
            Rectangle::new(
                window_area.x + vertical,
                window_area.y + right - size_offset,
                window_area.width - vertical as u16,
                size,
            ),
            Role::Right,
            mon.index(),
        );
        Self {
            vertical_handle,
            left_handle,
            right_handle,
            window_area,
            size,
            vertical,
            left,
            right,
            monitor_name: mon.name().to_owned(),
            vertical_clients: 0,
            left_clients: 0,
            right_clients: 0,
        }
    }

    pub fn new(wm: &WindowManager, mon: &Monitor) -> Self {
        Self::with_percentages(wm, mon, &(0.5, 0.5, 0.5))
    }

    pub fn destroy(&self, wm: &WindowManager) {
        wm.remove_all_contexts(&self.vertical_handle.window);
        self.vertical_handle.window.destroy();
        wm.remove_all_contexts(&self.left_handle.window);
        self.left_handle.window.destroy();
        wm.remove_all_contexts(&self.right_handle.window);
        self.right_handle.window.destroy();
    }

    pub fn monitor_name(&self) -> &str {
        &self.monitor_name
    }

    pub fn as_splits(&self) -> Splits {
        Splits::new(self.vertical, self.left, self.right)
    }

    pub fn split_percentages(&self) -> (f64, f64, f64) {
        (
            self.vertical as f64 / self.window_area.width as f64,
            self.left as f64 / self.window_area.height as f64,
            self.right as f64 / self.window_area.height as f64,
        )
    }

    pub fn handle(&self, role: Role) -> &SplitHandle {
        match role {
            Role::Vertical => &self.vertical_handle,
            Role::Left => &self.left_handle,
            Role::Right => &self.right_handle,
        }
    }

    pub fn handle_mut(&mut self, role: Role) -> &mut SplitHandle {
        match role {
            Role::Vertical => &mut self.vertical_handle,
            Role::Left => &mut self.left_handle,
            Role::Right => &mut self.right_handle,
        }
    }

    pub fn visible(&self, visible: bool) {
        if visible {
            if self.vertical_clients > 0 {
                self.vertical_handle.window.map();
            } else {
                self.vertical_handle.window.unmap();
            }
            if self.left_clients > 0 {
                self.left_handle.window.map();
            } else {
                self.left_handle.window.unmap();
            }
            if self.right_clients > 0 {
                self.right_handle.window.map();
            } else {
                self.right_handle.window.unmap();
            }
        } else {
            self.vertical_handle.window.unmap();
            self.left_handle.window.unmap();
            self.right_handle.window.unmap();
        }
    }

    pub fn update(&mut self, role: Role, position: i16) {
        match role {
            Role::Vertical => {
                self.vertical = position + self.size as i16 / 2 - self.window_area.x;
                self.vertical_handle.set_position(position);
                self.left_handle.resize(self.vertical as u16);
                self.right_handle
                    .resize(self.window_area.width - self.vertical as u16);
            }
            Role::Left => {
                self.left = position + self.size as i16 / 2 - self.window_area.y;
            }
            Role::Right => {
                self.right = position + self.size as i16 / 2 - self.window_area.y;
                self.right_handle.set_position(position);
            }
        }
    }
}
