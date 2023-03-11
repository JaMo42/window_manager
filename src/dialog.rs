use crate::{
    color::Color,
    event::{is_button_press, EventSink, Signal, SinkStorage},
    monitors::monitors,
    mouse_block::MouseBlock,
    rectangle::Rectangle,
    window_manager::WindowManager,
    x::{close_window, Window},
};
use std::sync::Arc;
use xcb::Event;

/// A dialog window that prevents interaction with anything else.
pub struct SpecialDialog {
    wm: Arc<WindowManager>,
    window: Window,
    mouse_block: MouseBlock,
}

impl SpecialDialog {
    const COLOR: Color = Color::new(0.0, 0.0, 0.0, 0.5);

    pub fn create(window: Window, wm: &Arc<WindowManager>) {
        let monitors = monitors();
        let monitor = monitors.primary();
        let mouse_block = MouseBlock::new_colored(wm, monitor, Self::COLOR);
        window.map();
        let mut geometry: Rectangle = window.get_geometry().into();
        window.move_and_resize(*geometry.center_inside(monitor.window_area()));
        window.stack_above(mouse_block.handle());
        // XXX: Specific to the quit dialog:
        // We take the input focus on the key window so we don't have to deal
        // with reading them from the gtk app, it seems it won't allow us to
        // select an option with the keyboard anyways so we don't lose anything
        // from taking the focus ourselves.
        wm.display.set_input_focus(mouse_block.handle());
        let this = Self {
            wm: wm.clone(),
            window,
            mouse_block,
        };
        wm.add_event_sink(SinkStorage::Unique(Box::new(this)));
    }
}

impl Drop for SpecialDialog {
    fn drop(&mut self) {
        close_window(&self.window);
        self.window.destroy();
        self.mouse_block.destroy(&self.wm);
    }
}

impl EventSink for SpecialDialog {
    fn accept(&mut self, event: &Event) -> bool {
        use x11::keysym::XK_Escape;
        use xcb::x::Event::{DestroyNotify, KeyPress};
        if is_button_press(event) {
            self.wm.remove_event_sink(self.id());
            return true;
        } else if let Event::X(DestroyNotify(e)) = event {
            if e.window() == self.window.handle() {
                self.wm.remove_event_sink(self.id());
                return true;
            }
        } else if let Event::X(KeyPress(e)) = event {
            if self.wm.display.keycode_to_keysym(e.detail()) == XK_Escape as u64 {
                self.wm.remove_event_sink(self.id());
            }
            return true;
        }
        self.wm.display.set_input_focus(self.mouse_block.handle());
        false
    }

    fn signal(&mut self, _: &Signal) {
        self.mouse_block.raise();
        self.window.stack_above(self.mouse_block.handle());
    }
}
