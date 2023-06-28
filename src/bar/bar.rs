use super::{
    tray_manager::TrayManager,
    widget::{self, Widget},
};
use crate::{
    class_hint::ClassHint,
    config::Config,
    error::OrFatal,
    event::Signal,
    ewmh::{self, WindowType},
    monitors::monitors_mut,
    rectangle::Rectangle,
    window_manager::{WindowKind, WindowManager},
    x::{Window, XcbWindow},
};
use std::{cell::RefCell, rc::Rc, sync::Arc};
use xcb::{
    x::{ButtonPressEvent, EventMask, Timestamp},
    Xid,
};

pub struct Bar {
    pub(super) wm: Arc<WindowManager>,
    pub(super) window: Window,
    geometry: Rectangle,
    config: Arc<Config>,
    pub(super) last_scroll_time: Timestamp,
    pub(super) left_widgets: Vec<Rc<RefCell<dyn Widget>>>,
    pub(super) right_widgets: Vec<Rc<RefCell<dyn Widget>>>,
    pub(super) mouse_widget: Option<Rc<RefCell<dyn Widget>>>,
    tray: Option<TrayManager>,
}

unsafe impl Send for Bar {}

impl Bar {
    /// Gap between widgets.
    const GAP: u16 = 10;
    /// Despite its name the rate any mouse events are processed at.
    const SCROLL_RATE: Timestamp = (1000 / 10);

    pub fn create(wm: Arc<WindowManager>) -> Self {
        let display = wm.display.clone();
        let config = wm.config.clone();
        let font_height = wm
            .drawing_context
            .lock()
            .font_height(Some(&config.bar.font));
        let mut monitors = monitors_mut();
        let monitor = monitors.primary();
        let height = config.bar.height.resolve(
            Some(monitor.dpmm()),
            Some(monitor.geometry().height),
            Some(font_height),
        );
        let visual = *display.truecolor_visual();
        let mut geometry = *monitor.geometry();
        geometry.height = height;
        monitors.set_bar_height(height);
        let window = Window::builder(display)
            .geometry(geometry)
            .depth(visual.depth)
            .visual(visual.id)
            .attributes(|attributes| {
                attributes
                    .override_redirect()
                    .colormap(visual.colormap)
                    .background_pixel(config.colors.bar_background.pack())
                    .border_pixel(0)
                    .cursor(wm.cursors.normal)
                    .event_mask(EventMask::EXPOSURE);
            })
            .build();
        ewmh::set_window_type(&window, WindowType::Dock);
        wm.set_window_kind(&window, WindowKind::MetaOrUnmanaged);
        ClassHint::new("window_manager_bar", "window_manager_bar").set(&window);
        window.map();
        window.raise();
        Self {
            wm,
            window,
            geometry,
            config,
            last_scroll_time: 0,
            left_widgets: Vec::new(),
            right_widgets: Vec::new(),
            mouse_widget: None,
            tray: None,
        }
    }

    pub fn destroy(&mut self) {
        log::trace!("bar: destroying bar");
        for widget in self
            .left_widgets
            .drain(..)
            .chain(self.right_widgets.drain(..))
        {
            let widget = widget.borrow();
            let window = widget.window();
            self.wm.remove_all_contexts(window);
            window.destroy();
        }
        self.wm.remove_all_contexts(&self.window);
        self.window.destroy();
        if let Some(tray) = self.tray() {
            tray.destroy()
        }
    }

    pub fn width(&self) -> u16 {
        self.geometry.width
    }

    pub fn height(&self) -> u16 {
        self.geometry.height
    }

    pub fn add_widgets(&mut self) {
        macro_rules! push {
            ($target:expr, $widget:ident) => {
                if let Some(w) = widget::$widget::new(&self.window, &self.wm) {
                    $target.push(Rc::new(RefCell::new(w)));
                } else {
                    log::warn!("Could not create widget: {}", stringify!($widget));
                }
            };
        }
        push!(self.left_widgets, Workspaces);
        push!(self.right_widgets, Quit);
        push!(self.right_widgets, DateTime);
        push!(self.right_widgets, Volume);
        push!(self.right_widgets, Battery);
        self.window.map_subwindows();
        self.draw_background();
    }

    pub fn invalidate_widgets(&mut self) {
        for widget in self
            .left_widgets
            .iter_mut()
            .chain(self.right_widgets.iter_mut())
        {
            widget.borrow_mut().invalidate();
        }
    }

    fn draw_background(&self) {
        let dc = self.wm.drawing_context.lock();
        dc.fill_rect(self.geometry, self.config.colors.bar_background);
        dc.render(&self.window, self.geometry);
    }

    pub fn draw(&mut self) {
        self.draw_background();
        let dc = self.wm.drawing_context.lock();
        dc.set_font(&self.config.bar.font);
        let mut draw_x = 0;
        let mut place_x = 0;
        for widget in &self.left_widgets {
            let mut widget = widget.borrow_mut();
            let result = widget.update(&dc, self.geometry.height, draw_x);
            let width = result.width();
            let window = widget.window();
            if result.is_new() {
                window.move_and_resize((place_x, 0, width, self.geometry.height));
                dc.render_at(window, (draw_x, 0, width, self.geometry.height), (0, 0));
                draw_x += width as i16;
            } else {
                window.r#move(place_x, 0);
            }
            widget.set_geometry(Rectangle::new(place_x, 0, width, self.geometry.height));
            place_x += (width + Self::GAP) as i16;
        }
        place_x = self.geometry.width as i16;
        if let Some(tray) = &self.tray {
            let width = tray.width();
            place_x -= width as i16;
            tray.window().r#move(place_x, 0);
            self.window
                .resize(self.geometry.width - width, self.geometry.height);
            tray.refresh();
        }
        for widget in &self.right_widgets {
            let mut widget = widget.borrow_mut();
            let result = widget.update(&dc, self.geometry.height, draw_x);
            let width = result.width();
            place_x -= width as i16;
            let window = widget.window();
            if result.is_new() {
                window.move_and_resize((place_x, 0, width, self.geometry.height));
                dc.render_at(window, (draw_x, 0, width, self.geometry.height), (0, 0));
                draw_x += width as i16;
            } else {
                window.r#move(place_x, 0);
            }
            widget.set_geometry(Rectangle::new(place_x, 0, width, self.geometry.height));
            place_x -= Self::GAP as i16;
        }
    }

    pub fn enter(&mut self, window: XcbWindow) {
        for rc_widget in self.left_widgets.iter().chain(self.right_widgets.iter()) {
            let mut widget = rc_widget.borrow_mut();
            if widget.window().handle() == window {
                self.mouse_widget = Some(rc_widget.clone());
                widget.enter();
                return;
            }
        }
    }

    pub fn leave(&mut self) {
        // Same story as in `click`, we could get a leave event while still on
        // the window and then get a second when leaving it again.
        if let Some(widget) = self.mouse_widget.take() {
            widget.borrow_mut().leave();
        }
    }

    pub fn click(&mut self, event: &ButtonPressEvent) {
        if event.time() - self.last_scroll_time <= Self::SCROLL_RATE {
            return;
        }
        self.last_scroll_time = event.time();
        // If clicking a widget creates a window that covers the mouse widget
        // we will get the leave event which clears the mouse widget before
        // the button release event.
        if let Some(mouse_widget) = &mut self.mouse_widget {
            mouse_widget.borrow_mut().click(event);
        }
    }

    pub fn set_tray(&mut self, tray: TrayManager) {
        self.tray = Some(tray);
    }

    pub fn tray(&mut self) -> Option<&mut TrayManager> {
        self.tray.as_mut()
    }

    pub fn tray_window(&self) -> XcbWindow {
        if let Some(tray) = &self.tray {
            tray.window().handle()
        } else {
            XcbWindow::none()
        }
    }

    pub fn resize(&mut self) {
        let mut monitors = monitors_mut();
        let monitor = monitors.primary();
        let font_height = self
            .wm
            .drawing_context
            .lock()
            .font_height(Some(&self.config.bar.font));
        let height = self.config.bar.height.resolve(
            Some(monitor.dpmm()),
            Some(monitor.geometry().height),
            Some(font_height),
        );
        let mut width = 0;
        if let Some(tray) = self.tray() {
            tray.set_size_info(height, monitor.geometry().width);
            width = tray.width();
        }
        self.geometry = monitor.geometry().with_height(height);
        self.window.resize(self.geometry.width - width, height);
        monitors.set_bar_height(height);
        self.wm
            .signal_sender
            .send(Signal::UpdateBar(true))
            .or_fatal(&self.wm.display);
    }
}
