use crate::{
    ewmh::{self, WindowType},
    rectangle::{Rectangle, ShowAt},
    window_manager::{WindowKind, WindowManager},
    x::Window,
};
use std::sync::Arc;

pub struct Tooltip {
    wm: Arc<WindowManager>,
    window: Window,
}

impl Tooltip {
    const BORDER: u16 = 10;

    pub fn new(wm: &Arc<WindowManager>) -> Self {
        let display = wm.display.clone();
        let visual = *display.truecolor_visual();
        let window = Window::builder(display)
            .depth(visual.depth)
            .visual(visual.id)
            .attributes(|attributes| {
                attributes
                    .override_redirect()
                    .colormap(visual.colormap)
                    .border_pixel(0)
                    .background_pixel(0);
            })
            .build();
        wm.set_window_kind(&window, WindowKind::MetaOrUnmanaged);
        ewmh::set_window_type(&window, WindowType::Tooltip);
        Self {
            wm: wm.clone(),
            window,
        }
    }

    pub fn show(&self, text: &str, at: ShowAt) {
        let dc = self.wm.drawing_context.lock();
        dc.set_font(&self.wm.config.bar.font);
        let mut text = dc.text(text, (Self::BORDER as i16, Self::BORDER as i16, 0, 0));
        let width = text.width() + 2 * Self::BORDER;
        let height = text.height() + 2 * Self::BORDER + 1;
        let local_rect = Rectangle::new(0, 0, width, height);
        self.window.move_and_resize(at.translate(local_rect));
        dc.fill_rect(local_rect, self.wm.config.colors.notification_background);
        text.color(self.wm.config.colors.notification_text).draw();
        self.window.map();
        self.window.raise();
        dc.render(&self.window, local_rect);
    }

    pub fn close(&self) {
        self.window.unmap();
    }
}

impl Drop for Tooltip {
    fn drop(&mut self) {
        self.wm.remove_all_contexts(&self.window);
        self.window.destroy();
    }
}
