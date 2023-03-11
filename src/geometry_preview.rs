use crate::{
    action::snap_on_monitor,
    client::{Client, FrameKind, SetClientGeometry},
    color::Color,
    draw::ColorKind,
    ewmh::{set_window_type, WindowType},
    layout::ClientLayout,
    monitors::monitors,
    normal_hints::NormalHints,
    rectangle::{PointOffset, Rectangle},
    snap::SnapState,
    split_handles::Splits,
    window_manager::WindowManager,
    x::Window,
};
use std::{
    mem::discriminant,
    sync::{Arc, Mutex},
};

pub struct GeometryPreview {
    wm: Arc<WindowManager>,
    window: Window,
    original_geometry: Rectangle,
    geometry: Rectangle,
    snap_geometry: Rectangle,
    snap_state: SnapState,
    final_geometry: Rectangle,
    is_snapped: bool,
    did_finish: bool,
    client_layout: ClientLayout,
    frame_kind: FrameKind,
    splits: Vec<Splits>,
}

impl GeometryPreview {
    const MIN_WIDTH: u16 = 160;
    const MIN_HEIGHT: u16 = 90;
    const BORDER_WIDTH: u16 = 5;
    // Only apply resize increment if resize amount is larger than this value.
    // (if this is 0 it becomes very hard to not resize should the user change
    // their mind about resizing, if it's too large it may become impossible to
    // resize by a single increment)
    const RESIZE_INCREMENT_THRESHHOLD: i16 = 5;

    pub fn new(
        wm: Arc<WindowManager>,
        initial_geometry: Rectangle,
        workspace: usize,
        frame_kind: FrameKind,
    ) -> Self {
        let display = &wm.display;
        let visual = display.truecolor_visual();
        let window = Window::builder(display.clone())
            .geometry(initial_geometry)
            .depth(visual.depth)
            .visual(visual.id)
            .attributes(|attributes| {
                attributes
                    .override_redirect()
                    .colormap(visual.colormap)
                    .background_pixel(0)
                    .border_pixel(0);
            })
            .build();
        set_window_type(&window, WindowType::Desktop);
        let layout_class = wm.config.client_layout();
        let layout_class = layout_class.borrow();
        let client_layout = layout_class
            .get(monitors().at(initial_geometry.center()))
            .clone();
        let splits = wm
            .split_manager()
            .get_workspace(workspace)
            .iter()
            .map(|handles| handles.as_splits())
            .collect();
        Self {
            wm,
            window,
            original_geometry: initial_geometry,
            geometry: initial_geometry,
            snap_geometry: Rectangle::zeroed(),
            snap_state: SnapState::None,
            final_geometry: initial_geometry,
            is_snapped: false,
            did_finish: false,
            client_layout,
            frame_kind,
            splits,
        }
    }

    pub fn show(&self) {
        self.window.map();
        self.draw(Rectangle::zeroed());
        self.draw(self.original_geometry);
    }

    fn draw(&self, geometry: Rectangle) {
        static LAST_GEOMETRY: Mutex<Rectangle> = Mutex::new(Rectangle::zeroed());
        let geometry = geometry.at(0, 0);
        let mut last = LAST_GEOMETRY.lock().unwrap();
        if *last == geometry {
            return;
        }
        *last = geometry;
        if geometry == Rectangle::zeroed() {
            return;
        }
        let dc = self.wm.drawing_context.lock();
        dc.rect(geometry)
            .stroke(
                Self::BORDER_WIDTH,
                ColorKind::Solid(self.wm.config.colors.selected),
            )
            .color(Color::new(0.2, 0.2, 0.2, 0.2))
            .corner_radius(2 * Self::BORDER_WIDTH)
            .clear_below_corners()
            .draw();
        dc.render(&self.window, geometry);
    }

    pub fn ensure_unsnapped(&mut self, mouse_x: i16, mouse_y: i16, offset: &PointOffset) {
        if self.is_snapped {
            let (x_offset, y_offset) = offset.point_inside(&self.original_geometry);
            self.geometry.x = mouse_x - x_offset;
            self.geometry.y = mouse_y - y_offset;
            self.is_snapped = false;
        }
    }

    pub fn move_by(&mut self, x: i16, y: i16) {
        self.geometry.x += x;
        self.geometry.y += y;
        self.final_geometry = self.geometry;
    }

    pub fn move_edge(&mut self, x: i16, y: i16) {
        let monitors = monitors();
        let monitor = monitors.at((x, y));
        self.snap_state = SnapState::move_edge_state(x, y, monitor);
        let splits = self.splits[monitor.index()];
        self.snap_geometry =
            self.snap_state
                .get_geometry(splits, monitor, self.client_layout.gap());
        self.is_snapped = true;
    }

    pub fn resize_by(&mut self, w: i16, h: i16) {
        if w < 0 {
            let ww = -w as u16;
            if self.geometry.width > ww && self.geometry.width - ww >= Self::MIN_WIDTH {
                self.geometry.width -= ww as u16;
            }
        } else {
            self.geometry.width += w as u16;
        }
        if h < 0 {
            let hh = -h as u16;
            if self.geometry.height > hh && self.geometry.height - hh >= Self::MIN_HEIGHT {
                self.geometry.height -= hh;
            }
        } else {
            self.geometry.height += h as u16;
        }
        self.final_geometry = self.geometry;
    }

    pub fn snap(&mut self, x: i16, y: i16) {
        let monitors = monitors();
        let monitor = monitors.at(self.geometry.center());
        self.snap_state = SnapState::move_snap_state(x, y, monitor);
        self.is_snapped = true;
        let splits = self.splits[monitor.index()];
        self.snap_geometry =
            self.snap_state
                .get_geometry(splits, monitor, self.client_layout.gap());
    }

    fn get_client_rect(&self, rect: Rectangle) -> Rectangle {
        self.client_layout.get_client(self.frame_kind, &rect)
    }

    fn get_frame_rect(&self, rect: Rectangle) -> Rectangle {
        self.client_layout.get_frame(self.frame_kind, &rect)
    }

    pub fn apply_normal_hints(&mut self, hints: &NormalHints, keep_height: bool) {
        let g;
        // Apply resize increment
        if let Some((winc, hinc)) = hints.resize_inc() {
            let mut dw = self.geometry.width as i16 - self.original_geometry.width as i16;
            let mut dh = self.geometry.height as i16 - self.original_geometry.height as i16;
            if dw < -Self::RESIZE_INCREMENT_THRESHHOLD {
                dw = (dw - winc + 1) / winc * winc;
            } else if dw > Self::RESIZE_INCREMENT_THRESHHOLD {
                dw = (dw + winc - 1) / winc * winc;
            } else {
                dw = 0;
            }
            if dh < -Self::RESIZE_INCREMENT_THRESHHOLD {
                dh = (dh - hinc + 1) / hinc * hinc;
            } else if dh > Self::RESIZE_INCREMENT_THRESHHOLD {
                dh = (dh + hinc - 1) / hinc * hinc;
            } else {
                dh = 0;
            }
            g = Rectangle::new(
                self.geometry.x,
                self.geometry.y,
                (self.original_geometry.width as i16 + dw) as u16,
                (self.original_geometry.height as i16 + dh) as u16,
            );
        } else {
            g = self.geometry;
        }
        // Apply size constraints
        self.final_geometry =
            self.get_frame_rect(hints.constrain(&self.get_client_rect(g), keep_height));
    }

    pub fn update(&self) {
        let g = if self.is_snapped {
            self.snap_geometry
        } else {
            self.final_geometry
        };
        self.window.move_and_resize(g);
        self.draw(g);
    }

    pub fn finish(&mut self, client: &Client) {
        if self.did_finish {
            return;
        }
        self.did_finish = true;
        self.window.destroy();
        if self.is_snapped {
            if discriminant(&self.snap_state) == discriminant(&client.snap_state()) {
                return;
            }
            if client.snap_state().is_none() {
                client.save_geometry();
            }
            let monitors = monitors();
            let monitor = monitors.at(self.snap_geometry.center());
            snap_on_monitor(client, monitor, self.snap_state);
        } else {
            if self.final_geometry == self.original_geometry {
                return;
            }
            client.set_snap_state(SnapState::None);
            client.move_and_resize(SetClientGeometry::Frame(self.final_geometry));
            client.save_geometry();
        }
    }

    pub fn cancel(&mut self) {
        if self.did_finish {
            return;
        }
        self.did_finish = true;
        self.window.destroy();
    }
}
