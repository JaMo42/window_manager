use super::{Display, WindowAttributes, XcbWindow};
use crate::{error::OrFatal, rectangle::Rectangle};
use xcb::{
    x::{
        ChangeWindowAttributes, ConfigWindow, ConfigureWindow, CreateWindow, Cursor, Cw,
        DestroyWindow, EventMask, MapWindow, UnmapWindow, WindowClass, COPY_FROM_PARENT,
    },
    Xid,
};

/// A more lightweight window specialization for `InputOnly` windows.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct InputOnlyWindow(XcbWindow);

impl InputOnlyWindow {
    pub fn new_none() -> Self {
        Self(Xid::none())
    }

    pub fn builder() -> InputOnlyWindowBuilder {
        InputOnlyWindowBuilder::new()
    }

    pub fn handle(&self) -> XcbWindow {
        self.0
    }

    pub fn is_none(&self) -> bool {
        self.0.is_none()
    }

    pub fn destroy(&self, display: &Display) {
        display.void_request(&DestroyWindow { window: self.0 });
    }

    pub fn map(&self, display: &Display) {
        display.void_request(&MapWindow { window: self.0 })
    }

    pub fn unmap(&self, display: &Display) {
        display.void_request(&UnmapWindow { window: self.0 })
    }

    pub fn move_and_resize(&self, display: &Display, geometry: impl Into<(i16, i16, u16, u16)>) {
        let (x, y, width, height) = geometry.into();
        display.void_request(&ConfigureWindow {
            window: self.0,
            value_list: &[
                ConfigWindow::X(x as i32),
                ConfigWindow::Y(y as i32),
                ConfigWindow::Width(width as u32),
                ConfigWindow::Height(height as u32),
            ],
        });
    }

    pub fn set_cursor(&self, display: &Display, cursor: Cursor) {
        display.void_request(&ChangeWindowAttributes {
            window: self.0,
            value_list: &[Cw::Cursor(cursor)],
        });
    }
}

impl Xid for InputOnlyWindow {
    fn resource_id(&self) -> u32 {
        self.0.resource_id()
    }

    fn is_none(&self) -> bool {
        self.is_none()
    }

    fn none() -> Self {
        Self::new_none()
    }
}

impl PartialEq<XcbWindow> for InputOnlyWindow {
    fn eq(&self, other: &XcbWindow) -> bool {
        self.0 == *other
    }
}

macro_rules! opt {
    ($b:expr, $m:ident) => {
        if $b {
            EventMask::$m
        } else {
            EventMask::NO_EVENT
        }
    };
}

pub struct InputOnlyWindowBuilder {
    event_mask: EventMask,
    geometry: Rectangle,
    parent: XcbWindow,
}

impl InputOnlyWindowBuilder {
    fn new() -> Self {
        Self {
            geometry: Rectangle::new(0, 0, 10, 10),
            event_mask: EventMask::NO_EVENT,
            parent: Xid::none(),
        }
    }

    pub fn with_mouse(mut self, press: bool, release: bool, motion: bool) -> Self {
        self.event_mask |= opt!(press, BUTTON_PRESS)
            | opt!(release, BUTTON_RELEASE)
            | opt!(motion, POINTER_MOTION);
        self
    }

    pub fn with_crossing(mut self) -> Self {
        self.event_mask |= EventMask::ENTER_WINDOW | EventMask::LEAVE_WINDOW;
        self
    }

    #[allow(dead_code)]
    pub fn with_key(mut self, press: bool, release: bool) -> Self {
        self.event_mask |= opt!(press, KEY_PRESS) | opt!(release, KEY_RELEASE);
        self
    }

    pub fn with_geometry(mut self, geometry: Rectangle) -> Self {
        self.geometry = geometry;
        self
    }

    pub fn with_parent(mut self, parent: XcbWindow) -> Self {
        self.parent = parent;
        self
    }

    pub fn build(self, display: &Display) -> InputOnlyWindow {
        let wid = display.connection.generate_id();
        let mut attrs = WindowAttributes::new();
        attrs.event_mask(self.event_mask);
        let value_list = attrs.value_list();
        let (x, y, width, height) = self.geometry.into_parts();
        let parent = if self.parent.is_none() {
            display.root
        } else {
            self.parent
        };
        display
            .try_void_request(&CreateWindow {
                depth: COPY_FROM_PARENT as u8,
                wid,
                parent,
                x,
                y,
                width,
                height,
                border_width: 0,
                class: WindowClass::InputOnly,
                visual: COPY_FROM_PARENT,
                value_list,
            })
            .or_fatal(display);
        InputOnlyWindow(wid)
    }
}
