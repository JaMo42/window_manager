use super::{Display, Visual, Window, WindowAttributes, XcbWindow};
use crate::error::OrFatal;
use std::sync::Arc;
use xcb::x::{CreateWindow, Visualid, WindowClass, COPY_FROM_PARENT};

pub struct WindowBuilder {
    display: Arc<Display>,
    depth: u8,
    parent: XcbWindow,
    x: i16,
    y: i16,
    width: u16,
    height: u16,
    border_width: u16,
    class: WindowClass,
    visual: Visualid,
    attributes: WindowAttributes,
}

impl WindowBuilder {
    pub fn new(display: Arc<Display>) -> Self {
        let parent = display.root();
        Self {
            display,
            depth: COPY_FROM_PARENT as u8,
            parent,
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            border_width: 0,
            class: WindowClass::InputOutput,
            visual: COPY_FROM_PARENT,
            attributes: WindowAttributes::new(),
        }
    }

    pub fn parent(mut self, parent: XcbWindow) -> Self {
        self.parent = parent;
        self
    }

    pub fn position(mut self, x: i16, y: i16) -> Self {
        self.x = x;
        self.y = y;
        self
    }

    pub fn size(mut self, width: u16, height: u16) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn geometry(mut self, rect: impl Into<(i16, i16, u16, u16)>) -> Self {
        (self.x, self.y, self.width, self.height) = rect.into();
        self
    }

    pub fn border_width(mut self, width: u16) -> Self {
        self.border_width = width;
        self
    }

    pub fn visual_info(mut self, vi: &Visual) -> Self {
        self.depth = vi.depth;
        self.visual = vi.id;
        self.attributes.colormap(vi.colormap);
        self
    }

    pub fn depth(mut self, depth: u8) -> Self {
        self.depth = depth;
        self
    }

    pub fn visual(mut self, visual: Visualid) -> Self {
        self.visual = visual;
        self
    }

    pub fn attributes<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut WindowAttributes),
    {
        f(&mut self.attributes);
        self
    }

    pub fn class(mut self, class: WindowClass) -> Self {
        self.class = class;
        self
    }

    pub fn build(mut self) -> Window {
        let wid = self.display.connection.generate_id();
        let value_list = self.attributes.value_list();
        self.display
            .try_void_request(&CreateWindow {
                depth: self.depth,
                wid,
                parent: self.parent,
                x: self.x,
                y: self.y,
                width: self.width,
                height: self.height,
                border_width: self.border_width,
                class: self.class,
                visual: self.visual,
                value_list,
            })
            .or_fatal(&self.display);
        Window::from_handle(self.display, wid)
    }
}
