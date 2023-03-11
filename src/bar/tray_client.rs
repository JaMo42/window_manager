use super::xembed;
use crate::{
    window_manager::{WindowKind, WindowManager},
    x::{GetProperty, Window, XcbWindow},
};
use xcb::{
    x::{ConfigureNotifyEvent, EventMask},
    Xid,
};

#[derive(Clone, Debug)]
pub struct TrayClient {
    window: Window,
    xembed_info: xembed::Info,
    size: u16,
    is_mapped: bool,
}

impl TrayClient {
    pub fn new(wm: &WindowManager, window: XcbWindow, size: u16) -> Self {
        let window = Window::from_handle(wm.display.clone(), window);
        wm.set_window_kind(&window, WindowKind::TrayClient);
        Self {
            window,
            xembed_info: xembed::Info::new(),
            size,
            is_mapped: false,
        }
    }

    pub fn destroy(&self, wm: &WindowManager) {
        wm.remove_all_contexts(&self.window);
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn xembed_info(&self) -> &xembed::Info {
        &self.xembed_info
    }

    pub fn query_xembed_info(&mut self) -> bool {
        self.xembed_info
            .query(self.window.handle(), self.window.display())
    }

    pub fn set_position(&self, x: i16, y: i16) {
        self.window.r#move(x, y);
        self.configure(x, y);
    }

    pub fn set_size(&mut self, size: u16) {
        self.size = size;
    }

    pub fn configure(&self, x: i16, y: i16) {
        self.window.send_event(
            EventMask::STRUCTURE_NOTIFY,
            &ConfigureNotifyEvent::new(
                self.window().handle(),
                self.window().handle(),
                XcbWindow::none(),
                x,
                y,
                self.size,
                self.size,
                0,
                false,
            ),
        );
    }

    /// If the client currently mapped?
    pub fn is_mapped(&self) -> bool {
        self.is_mapped
    }

    /// Set the clients mapped state. This does not map/unmap the window.
    pub fn set_mapped(&mut self, state: bool) {
        self.is_mapped = state;
    }

    /// Ensures the clients mapped state matches the XEmbed info.
    /// This does not query the XEmbed info.
    pub fn update_mapped_state(&mut self) {
        if self.xembed_info.is_mapped() {
            self.window.map();
            self.window.raise();
            self.is_mapped = true;
        } else {
            self.window.unmap();
            self.is_mapped = false;
        }
    }

    /// Get the class name of the client. If not set the window id is used.
    pub fn class(&self) -> String {
        let display = self.window.display();
        self.window
            .get_string_property(display, display.atoms.wm_class)
            .unwrap_or_else(|| format!("{}", self.window.handle().resource_id()))
    }
}
