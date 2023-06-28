use std::{cell::Cell, sync::Arc};

use xcb::{
    x::{ConfigWindow, ConfigureWindow, StackMode},
    Xid,
};

use crate::{
    client::Client,
    cursor::{self, Cursors},
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
        self.window.map(client.display());
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

    pub fn get_cursor_id(&self, x: i16, y: i16, corner_size: u16) -> u32 {
        if self.extents == 0 {
            return cursor::XC_left_ptr;
        }
        MouseResizeOptions::from_position(self.geometry.get(), x, y, corner_size).cursor_id()
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

impl PartialEq for ExtendedFrame {
    fn eq(&self, other: &Self) -> bool {
        self.window == other.window
    }
}

enum HoveredFrameInner {
    None,
    Frame(Arc<Client>),
    Extended(ExtendedFrame),
}

impl HoveredFrameInner {
    fn get_cursor_id(&self, x: i16, y: i16, corner_size: u16) -> u32 {
        match self {
            Self::Frame(client) => {
                let geometry = client.frame_geometry();
                let frame_offset = client.frame_offset();
                if y < geometry.y + frame_offset.y {
                    cursor::XC_left_ptr
                } else {
                    MouseResizeOptions::from_position(geometry, x, y, corner_size).cursor_id()
                }
            }
            Self::Extended(exframe) => {
                exframe.get_cursor_id(x, y, corner_size + exframe.extents as u16)
            }
            _ => cursor::XC_left_ptr,
        }
    }
}

/// Manages setting the cursor for a clients frame and extended frame.
pub struct HoveredFrame {
    inner: HoveredFrameInner,
    last_cursor: u32,
    corner_size: u16,
}

impl HoveredFrame {
    pub fn new() -> Self {
        Self {
            inner: HoveredFrameInner::None,
            last_cursor: u32::MAX,
            corner_size: 0,
        }
    }

    pub fn update_cursor(&mut self, display: &Display, cursors: &Cursors, x: i16, y: i16) {
        let id = self.inner.get_cursor_id(x, y, self.corner_size);
        if id == self.last_cursor {
            return;
        }
        let cursor = cursors.by_id(id);
        match &self.inner {
            HoveredFrameInner::Frame(client) => client.frame().set_cursor(cursor),
            HoveredFrameInner::Extended(exframe) => exframe.window.set_cursor(display, cursor),
            _ => {}
        }
        self.last_cursor = id;
    }

    fn leave(&self) {
        if let HoveredFrameInner::Frame(client) = &self.inner {
            client
                .frame()
                .set_cursor(client.get_window_manager().cursors.normal);
        }
    }

    pub fn clear(&mut self) {
        self.leave();
        self.inner = HoveredFrameInner::None;
    }

    fn set_border(&mut self, client: &Client) {
        self.corner_size = 2 * client.frame_offset().x as u16;
    }

    pub fn set_extended(&mut self, exframe: ExtendedFrame, client: &Client) {
        self.leave();
        self.set_border(client);
        self.inner = HoveredFrameInner::Extended(exframe);
        self.last_cursor = u32::MAX;
    }

    pub fn set_frame(&mut self, client: Arc<Client>) {
        self.leave();
        self.set_border(&client);
        self.inner = HoveredFrameInner::Frame(client);
        self.last_cursor = u32::MAX;
    }

    pub fn is_some(&self) -> bool {
        !matches!(self.inner, HoveredFrameInner::None)
    }

    pub fn is_extended(&self) -> bool {
        matches!(self.inner, HoveredFrameInner::Extended(_))
    }

    pub fn is_frame(&self) -> bool {
        matches!(self.inner, HoveredFrameInner::Frame(_))
    }

    pub fn is_client(&self, client: &Client) -> bool {
        match &self.inner {
            HoveredFrameInner::Frame(my_client) => my_client.frame() == client.frame(),
            HoveredFrameInner::Extended(exframe) => exframe == client.extended_frame(),
            _ => false,
        }
    }
}
