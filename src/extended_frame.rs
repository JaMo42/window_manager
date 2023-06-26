use std::{cell::Cell, sync::Arc};

use xcb::{
    x::{ConfigWindow, ConfigureWindow, StackMode},
    Xid,
};

use crate::{
    client::Client,
    cursor::Cursors,
    mouse::MouseResizeOptions,
    rectangle::Rectangle,
    window_manager::{WindowKind, WindowManager},
    x::{Display, InputOnlyWindow, XcbWindow},
};

#[derive(Clone)]
pub struct ExtendedFrame {
    extents: i16,
    window: InputOnlyWindow,
    geometry: Cell<Rectangle>,
}

impl ExtendedFrame {
    pub fn new(display: &Display, mut frame_geometry: Rectangle, size: i16) -> Self {
        if size == 0 {
            Self::none()
        } else {
            frame_geometry.resize(size);
            let window = InputOnlyWindow::builder()
                .with_geometry(frame_geometry)
                .with_crossing()
                // we don't use `with_mouse` here because we want the events
                // from this to have the handle as the child not as the window
                .build(display);
            window.map(display);
            Self {
                extents: size,
                window,
                geometry: Cell::new(frame_geometry),
            }
        }
    }

    pub fn associate(&self, wm: &WindowManager, client: &Arc<Client>) {
        if self.extents == 0 {
            return;
        }
        let handle = self.window.handle();
        wm.associate_client(&handle, client);
        wm.set_window_kind(&handle, WindowKind::ExtendedFrame);
    }

    pub fn destroy(&self, display: &Display) {
        if self.extents == 0 {
            return;
        }
        self.window.destroy(display);
    }

    pub fn handle(&self) -> Option<XcbWindow> {
        if self.extents == 0 {
            None
        } else {
            Some(self.window.handle())
        }
    }

    pub fn handle_eq(&self, handle: XcbWindow) -> bool {
        self.extents != 0 && handle == self.window.handle()
    }

    pub fn restack(&self, client: &Client) {
        if self.extents == 0 {
            return;
        }
        client.display().void_request(&ConfigureWindow {
            window: self.window.handle(),
            value_list: &[
                ConfigWindow::Sibling(client.frame().handle()),
                ConfigWindow::StackMode(StackMode::Below),
            ],
        });
    }

    pub fn map(&self, client: &Client) {
        if self.extents == 0 {
            return;
        }
        self.window.map(&client.display());
        self.restack(client);
    }

    pub fn unmap(&self, display: &Display) {
        if self.extents == 0 {
            return;
        }
        self.window.unmap(display);
    }

    pub fn resize(&self, display: &Display, frame_geometry: impl Into<Rectangle>) {
        if self.extents == 0 {
            return;
        }
        let mut frame_geometry = frame_geometry.into();
        frame_geometry.resize(self.extents);
        self.window.move_and_resize(display, frame_geometry);
        self.geometry.set(frame_geometry);
    }

    pub fn update_cursor(&self, display: &Display, cursors: &Cursors, x: i16, y: i16) {
        if self.extents == 0 {
            return;
        }
        let options =
            MouseResizeOptions::from_position(self.geometry.get(), x, y, 3 * self.extents as u16);
        self.window
            .set_cursor(display, cursors.by_id(options.cursor_id()));
    }
}

impl Xid for ExtendedFrame {
    fn none() -> Self {
        Self {
            extents: 0,
            window: InputOnlyWindow::new_none(),
            geometry: Cell::new(Rectangle::zeroed()),
        }
    }

    fn resource_id(&self) -> u32 {
        self.window.handle().resource_id()
    }
}
