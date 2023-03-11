use crate::{
    color::Color,
    monitors::Monitor,
    window_manager::{WindowKind, WindowManager},
    x::{Window, XcbWindow},
};
use xcb::x::{EventMask, WindowClass};

pub struct MouseBlock {
    window: Window,
}

impl MouseBlock {
    pub fn new_invisible(wm: &WindowManager, monitor: &Monitor) -> Self {
        let geometry = *monitor.geometry();
        let window = Window::builder(wm.display.clone())
            .class(WindowClass::InputOnly)
            .geometry(geometry)
            .attributes(|attributes| {
                // In some cases we also want to get key event through the
                // mouse block window and in other cases having the key press
                // mask doesn't matter as something else will have the input
                // focus.
                attributes.event_mask(EventMask::BUTTON_PRESS | EventMask::KEY_PRESS);
            })
            .build();
        wm.set_window_kind(&window, WindowKind::MouseBlock);
        window.map();
        window.raise();
        Self { window }
    }

    pub fn new_colored(wm: &WindowManager, monitor: &Monitor, color: Color) -> Self {
        let geometry = *monitor.geometry();
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
                    .event_mask(EventMask::BUTTON_PRESS | EventMask::KEY_PRESS);
            })
            .build();
        wm.set_window_kind(&window, WindowKind::MouseBlock);
        let geometry = geometry.at(0, 0);
        let dc = wm.drawing_context.lock();
        dc.fill_rect(geometry, color);
        window.map();
        dc.render(&window, geometry);
        window.raise();
        Self { window }
    }

    pub fn destroy(&self, wm: &WindowManager) {
        wm.remove_all_contexts(&self.window);
        self.window.destroy();
    }

    pub fn handle(&self) -> XcbWindow {
        self.window.handle()
    }

    pub fn raise(&self) {
        self.window.raise();
    }
}

impl PartialEq<XcbWindow> for MouseBlock {
    fn eq(&self, other: &XcbWindow) -> bool {
        self.window.handle() == *other
    }
}
