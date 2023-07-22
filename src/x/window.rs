use super::Visual;
use super::{Display, WindowAttributes, WindowBuilder, XcbWindow};
use crate::error::OrFatal;
use std::sync::Arc;
use xcb::x::ClearArea;
use xcb::x::Cursor;
use xcb::x::Cw;
use xcb::x::GetWindowAttributes;
use xcb::x::GetWindowAttributesReply;
use xcb::x::SendEvent;
use xcb::x::SendEventDest;
use xcb::x::UnmapWindow;
use xcb::x::{ConfigWindow, ConfigureWindow, StackMode};
use xcb::BaseEvent;
use xcb::XidNew;
use xcb::{
    x::{
        ChangeWindowAttributes, DestroyWindow, Drawable, EventMask, GetGeometry, KillClient,
        MapSubwindows, MapWindow, ReparentWindow,
    },
    Xid,
};

#[derive(Clone)]
pub struct Window {
    handle: XcbWindow,
    display: Arc<Display>,
}

impl Window {
    pub fn from_handle(display: Arc<Display>, handle: XcbWindow) -> Self {
        Self { handle, display }
    }

    pub fn new_none(display: Arc<Display>) -> Self {
        Self::from_handle(display, Xid::none())
    }

    pub fn builder(display: Arc<Display>) -> WindowBuilder {
        WindowBuilder::new(display)
    }

    pub fn handle(&self) -> XcbWindow {
        self.handle
    }

    pub fn display(&self) -> &Arc<Display> {
        &self.display
    }

    pub fn destroy(&self) {
        self.display.void_request(&DestroyWindow {
            window: self.handle,
        })
    }

    pub fn kill_client(&self) {
        self.display.void_request(&KillClient {
            resource: self.handle.resource_id(),
        })
    }

    pub fn map(&self) {
        self.display.void_request(&MapWindow {
            window: self.handle,
        })
    }

    pub fn map_subwindows(&self) {
        self.display.void_request(&MapSubwindows {
            window: self.handle,
        })
    }

    pub fn unmap(&self) {
        self.display.void_request(&UnmapWindow {
            window: self.handle,
        })
    }

    pub fn get_geometry(&self) -> (i16, i16, u16, u16) {
        self.display
            .request_with_reply(&GetGeometry {
                drawable: Drawable::Window(self.handle),
            })
            .map(|reply| (reply.x(), reply.y(), reply.width(), reply.height()))
            .unwrap_or((0, 0, 160 * 3, 90 * 3))
    }

    pub fn reparent(&self, parent: &impl Xid, x: i16, y: i16) {
        self.display.void_request(&ReparentWindow {
            window: self.handle,
            parent: unsafe { XcbWindow::new(parent.resource_id()) },
            x,
            y,
        })
    }

    pub fn change_attributes<F>(&self, f: F)
    where
        F: FnOnce(&mut WindowAttributes),
    {
        let mut attributes = WindowAttributes::new();
        f(&mut attributes);
        self.display.void_request(&ChangeWindowAttributes {
            window: self.handle,
            value_list: attributes.value_list(),
        });
    }

    pub fn change_event_mask(&self, mask: EventMask) {
        self.change_attributes(|attributes| {
            attributes.event_mask(mask);
        });
    }

    pub fn get_depth(&self) -> u8 {
        self.display
            .request_with_reply(&GetGeometry {
                drawable: Drawable::Window(self.handle),
            })
            .unwrap_or_fatal(&self.display)
            .depth()
    }

    pub fn get_attributes(&self) -> GetWindowAttributesReply {
        self.display
            .request_with_reply(&GetWindowAttributes {
                window: self.handle,
            })
            .unwrap_or_fatal(&self.display)
    }

    pub fn get_visual(&self) -> Visual {
        let attributes = self.get_attributes();
        Visual {
            depth: self.get_depth(),
            id: attributes.visual(),
            colormap: attributes.colormap(),
        }
    }

    pub fn configure(&self, value_list: &mut [ConfigWindow]) {
        self.display.void_request(&ConfigureWindow {
            window: self.handle,
            value_list,
        });
    }

    pub fn r#move(&self, x: i16, y: i16) {
        self.configure(&mut [ConfigWindow::X(x as i32), ConfigWindow::Y(y as i32)]);
    }

    pub fn resize(&self, width: u16, height: u16) {
        self.configure(&mut [
            ConfigWindow::Width(width as u32),
            ConfigWindow::Height(height as u32),
        ]);
    }

    pub fn move_and_resize(&self, geometry: impl Into<(i16, i16, u16, u16)>) {
        let (x, y, width, height) = geometry.into();
        self.configure(&mut [
            ConfigWindow::X(x as i32),
            ConfigWindow::Y(y as i32),
            ConfigWindow::Width(width as u32),
            ConfigWindow::Height(height as u32),
        ]);
    }

    pub fn raise(&self) {
        self.configure(&mut [ConfigWindow::StackMode(StackMode::TopIf)]);
    }

    pub fn lower(&self) {
        self.configure(&mut [ConfigWindow::StackMode(StackMode::BottomIf)]);
    }

    pub fn stack_above(&self, sibling: XcbWindow) {
        self.configure(&mut [
            ConfigWindow::Sibling(sibling),
            ConfigWindow::StackMode(StackMode::Above),
        ]);
    }

    pub fn set_cursor(&self, cursor: Cursor) {
        self.display.void_request(&ChangeWindowAttributes {
            window: self.handle,
            value_list: &[Cw::Cursor(cursor)],
        })
    }

    pub fn send_event<E: BaseEvent>(&self, mask: EventMask, event: &'_ E) {
        self.display.void_request(&SendEvent {
            propagate: false,
            destination: SendEventDest::Window(self.handle),
            event_mask: mask,
            event,
        });
    }

    pub fn clear(&self) {
        self.display.void_request(&ClearArea {
            exposures: false,
            window: self.handle,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        });
    }
}

impl PartialEq for Window {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}

impl PartialEq<XcbWindow> for Window {
    fn eq(&self, other: &XcbWindow) -> bool {
        self.handle == *other
    }
}

impl Xid for Window {
    fn none() -> Self {
        unimplemented!()
    }

    fn is_none(&self) -> bool {
        self.handle.is_none()
    }

    fn resource_id(&self) -> u32 {
        self.handle.resource_id()
    }
}

impl Xid for &Window {
    fn none() -> Self {
        unimplemented!()
    }

    fn is_none(&self) -> bool {
        self.handle.is_none()
    }

    fn resource_id(&self) -> u32 {
        self.handle.resource_id()
    }
}

impl std::fmt::Display for Window {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.handle.resource_id())
    }
}

impl std::fmt::Debug for Window {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Window({})", self.handle.resource_id())
    }
}
