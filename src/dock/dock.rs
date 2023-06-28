use super::{item::Item, layout::DockLayout};
use crate::{
    class_hint::ClassHint,
    client::Client,
    color::Color,
    event::{EventSink, Signal},
    ewmh::{set_window_type, WindowType},
    monitors::monitors,
    mouse::{BUTTON_1, BUTTON_2, BUTTON_3},
    rectangle::Rectangle,
    timeout_thread::RepeatableTimeoutThread,
    window_manager::{WindowKind, WindowManager},
    x::{Window, XcbWindow},
};
use parking_lot::Mutex;
use std::{ptr::NonNull, sync::Arc};
use xcb::{
    x::{ButtonPressEvent, EnterNotifyEvent, EventMask, LeaveNotifyEvent},
    Event, Xid,
};

#[derive(Copy, Clone)]
pub struct ItemRef {
    // The dock is in an `Arc` so its address never changes and it's alive
    // as long as we are running so this pointer is always safe.
    dock: NonNull<Dock>,
    item_window: XcbWindow,
}

impl ItemRef {
    pub fn get_dock(&mut self) -> &mut Dock {
        unsafe { self.dock.as_mut() }
    }

    pub fn get(&mut self) -> &mut Item {
        let dock: &'static mut _ = unsafe { self.dock.as_mut() };
        let idx = dock.find_item_window(self.item_window);
        &mut dock.items[idx]
    }
}

pub struct Dock {
    pub(super) wm: Arc<WindowManager>,
    items: Vec<Item>,
    window: Window,
    show_window: Window,
    layout: DockLayout,
    hide_thread: Option<RepeatableTimeoutThread>,
    visible: bool,
    keep_open: bool,
    geometry: Rectangle,
}

unsafe impl Send for Dock {}

impl Dock {
    pub fn new(wm: &Arc<WindowManager>) -> Arc<Mutex<Self>> {
        let mut layout = DockLayout::default();
        layout.compute(&wm.config);
        let visual = wm.display.truecolor_visual();
        let class_hint = ClassHint::new("Window_manager_dock", "window_manager_dock");

        let geometry = layout.dock(wm.config.dock.pinned.len());
        let window = Window::builder(wm.display.clone())
            .geometry(geometry)
            .depth(visual.depth)
            .visual(visual.id)
            .attributes(|attributes| {
                attributes
                    .override_redirect()
                    .cursor(wm.cursors.normal)
                    .background_pixel(0)
                    .border_pixel(0)
                    .event_mask(
                        EventMask::ENTER_WINDOW | EventMask::LEAVE_WINDOW | EventMask::BUTTON_PRESS,
                    )
                    .colormap(visual.colormap);
            })
            .build();
        set_window_type(&window, WindowType::Dock);
        wm.set_window_kind(&window, WindowKind::Dock);
        class_hint.set(&window);

        let show_window = Window::builder(wm.display.clone())
            .geometry(layout.show_window())
            .depth(visual.depth)
            .visual(visual.id)
            .attributes(|attributes| {
                attributes
                    .override_redirect()
                    .cursor(wm.cursors.normal)
                    .background_pixel(0)
                    .border_pixel(0)
                    .event_mask(
                        EventMask::ENTER_WINDOW | EventMask::LEAVE_WINDOW | EventMask::BUTTON_PRESS,
                    )
                    .colormap(visual.colormap);
            })
            .build();
        set_window_type(&show_window, WindowType::Dock);
        wm.set_window_kind(&show_window, WindowKind::DockShow);
        class_hint.set(&show_window);

        log::trace!("dock: dock window: {}", window);
        log::trace!("dock: show window: {}", show_window);
        show_window.map();
        window.map();

        let mut items = Vec::with_capacity(wm.config.dock.pinned.len());
        let mut all_ok = true;
        for pinned in wm.config.dock.pinned.iter() {
            if let Some(item) = Item::new(
                &window,
                pinned,
                true,
                layout.item(items.len()),
                layout.icon(),
                wm,
            ) {
                items.push(item);
            } else {
                all_ok = false;
            }
        }
        window.raise();

        let this = Arc::new(Mutex::new(Self {
            wm: wm.clone(),
            items,
            window,
            show_window,
            layout,
            hide_thread: None,
            visible: true,
            keep_open: true,
            geometry,
        }));

        if !all_ok {
            this.lock().resize();
        }

        let hide_this = Arc::downgrade(&this);
        this.lock().hide_thread = Some(RepeatableTimeoutThread::new(Arc::new(move || {
            if let Some(this) = hide_this.upgrade() {
                this.lock().hide();
            }
        })));
        this.lock().update();

        this
    }

    pub fn destroy(&mut self) {
        log::trace!("dock: destroying dock");
        self.items.clear();
        self.wm.remove_all_contexts(&self.window);
        self.window.destroy();
        self.wm.remove_all_contexts(&self.show_window);
        self.show_window.destroy();
        self.hide_thread().destroy();
    }

    /// Handles a change in monitor configuration.
    pub fn resize(&mut self) {
        self.layout.compute(&self.wm.config);
        self.geometry = self.layout.dock(self.items.len());
        self.window.move_and_resize(self.geometry);
        self.show_window.move_and_resize(self.layout.show_window());
        self.show_window.clear();
        self.layout_items();
        for i in self.items.iter_mut() {
            i.set_icon_rect(self.layout.icon());
        }

        self.show_window.raise();
        if self.visible {
            self.window.raise();
        }
        self.update();
    }

    /// Sets the correct geometry for all items.
    fn layout_items(&mut self) {
        for (idx, item) in self.items.iter_mut().enumerate() {
            item.set_geometry(self.layout.item(idx));
        }
    }

    /// Redraw.
    pub fn update(&mut self) {
        let dc = self.wm.drawing_context.lock();
        let transparent = Color::new(0.0, 0.0, 0.0, 0.0);
        if self.layout.is_offset() {
            let g = self.geometry.at(0, 0);
            dc.fill_rect(g, transparent);
            dc.rect(g)
                .color(self.wm.config.colors.dock_background)
                .corner_percent(0.2);
        } else {
            let corner = self.geometry.height as f64 * 0.2;
            {
                let corner = corner.ceil() as u16;
                let clear = Rectangle::new(0, 0, corner, corner);
                dc.rect(clear).color(transparent).draw();
                dc.rect(clear.with_x((self.geometry.width - corner) as i16))
                    .color(transparent)
                    .draw();
            }
            // We only want rounded corners on the top so we need to draw it ourselves.
            let ctx = dc.cairo();
            let w = self.geometry.width as f64;
            let h = self.geometry.height as f64;
            let r = corner;
            ctx.move_to(0.0, h);
            ctx.arc(r, r, r, 180.0f64.to_radians(), 270.0f64.to_radians());
            ctx.arc(w - r, r, r, -90.0f64.to_radians(), 0.0f64.to_radians());
            ctx.line_to(w, h);
            ctx.close_path();
            dc.set_color(self.wm.config.colors.dock_background);
            ctx.fill().unwrap();
        }

        for i in self.items.iter() {
            i.update(&self.window, &dc, true);
        }

        dc.render(&self.window, self.geometry.at(0, 0));
    }

    fn update_item(&self, idx: usize) {
        self.items[idx].update(&self.window, &self.wm.drawing_context.lock(), false);
    }

    /// Get the hide thread.
    fn hide_thread(&mut self) -> &mut RepeatableTimeoutThread {
        unsafe { self.hide_thread.as_mut().unwrap_unchecked() }
    }

    /// Shows the dock.
    pub fn show(&mut self) {
        if !self.visible {
            self.window.map();
            self.window.raise();
            self.visible = true;
            self.update();
        }
    }

    /// Hides the dock. The `keep_open` value is ignored.
    pub fn hide(&mut self) {
        if self.visible {
            self.window.unmap();
            self.visible = false;
        }
        self.keep_open = false;
    }

    /// Hides the dock after the given timeout.
    /// Request may be blocked if `keep_open` is true.
    pub fn hide_after(&mut self, ms: u64) {
        self.cancel_hide();
        if self.keep_open {
            return;
        }
        self.hide_thread().start(ms);
    }

    /// Cancels a `hide_after` request.
    pub fn cancel_hide(&mut self) {
        self.hide_thread().cancel();
    }

    /// Signal the dock to stay open. Only affects the `hide_after` function.
    /// If the given value is `true` the dock is shown, otherwise it is hidden
    /// unless the mouse is currently on it.
    pub fn keep_open(&mut self, yay_or_nay: bool) {
        self.keep_open = yay_or_nay;
        if yay_or_nay {
            self.show();
        } else if !self
            .geometry
            .contains(self.wm.display.query_pointer_position())
        {
            self.hide();
        }
    }

    fn find_item_name(&self, name: &str) -> Option<usize> {
        self.items.iter().position(|i| i.id() == name)
    }

    pub(super) fn find_item_window(&self, window: XcbWindow) -> usize {
        for (i, item) in self.items.iter().enumerate() {
            if item.window().handle() == window {
                return i;
            }
        }
        unreachable!()
    }

    /// Finds the item of an existing client.
    fn find_client_item(&self, client: &Client) -> Option<usize> {
        if let Some(app_id) = client.application_id() {
            self.find_item_name(app_id)
        } else {
            for (i, item) in self.items.iter().enumerate() {
                if item.contains(client.handle()) {
                    return Some(i);
                }
            }
            None
        }
    }

    fn item_ref(&mut self, idx: usize) -> ItemRef {
        let dock = NonNull::new(self).unwrap();
        let item_window = self.items[idx].window().handle();
        ItemRef { dock, item_window }
    }

    /// Checks if the dock is occluded by any client and sets the `keep_open`
    /// flag accordingly.
    pub fn check_occluded(&mut self, ignore: Option<XcbWindow>) {
        let mut is_occluded = false;
        let main_mon = monitors().primary().index() as isize;
        for client in self.wm.active_workspace().iter() {
            if Some(client.handle()) == ignore || client.is_minimized() {
                continue;
            }
            is_occluded = is_occluded
                || if client.is_fullscreen() {
                    client.monitor() == main_mon && client.is_focused()
                } else {
                    client.frame_geometry().overlaps(self.geometry)
                };
            if is_occluded {
                break;
            }
        }
        self.keep_open(!is_occluded);
    }
}

// Event and signal handlers
impl Dock {
    fn enter(&mut self, event: &EnterNotifyEvent) -> bool {
        match self.wm.get_window_kind(&event.event()) {
            WindowKind::Dock => {
                self.cancel_hide();
                self.window.raise();
                true
            }
            WindowKind::DockShow => {
                self.cancel_hide();
                self.show();
                true
            }
            WindowKind::DockItem => {
                self.cancel_hide();
                self.window.raise();
                let idx = self.find_item_window(event.event());
                self.items[idx].is_hovered(true);
                self.update_item(idx);
                self.items[idx].show_tooltip(self.geometry);
                true
            }
            _ => false,
        }
    }

    fn leave(&mut self, event: &LeaveNotifyEvent) -> bool {
        match self.wm.get_window_kind(&event.event()) {
            WindowKind::Dock => {
                self.hide_after(100);
                true
            }
            WindowKind::DockShow => {
                if !self.geometry.contains((event.root_x(), event.root_y())) {
                    self.hide_after(500);
                }
                true
            }
            WindowKind::DockItem => {
                let idx = self.find_item_window(event.event());
                self.items[idx].is_hovered(false);
                self.update_item(idx);
                self.items[idx].hide_tooltip();
                true
            }
            _ => false,
        }
    }

    fn click(&mut self, event: &ButtonPressEvent) -> bool {
        match self.wm.get_window_kind(&event.event()) {
            WindowKind::Dock => true,
            WindowKind::DockShow => {
                // Under some circumstances the show window loses its
                // transparency (not sure what caused this when I observed it)
                // so for now we do this as an easy way to restore it.
                // TODO: can this be detected with something like exposure events?
                self.show_window.clear();
                true
            }
            WindowKind::DockItem => {
                let idx = self.find_item_window(event.event());
                let item = &mut self.items[idx];
                match event.detail() {
                    BUTTON_1 => item.click(),
                    BUTTON_2 => item.launch_new_instance(),
                    BUTTON_3 => {
                        self.keep_open(true);
                        // Need to drop item before calling `keep_open` so we need
                        // to create a new binding here.
                        let item_ref = self.item_ref(idx);
                        let item = &self.items[idx];
                        item.hide_tooltip();
                        let menu_handle = item.context_menu(self.geometry, item_ref);
                        self.window.stack_above(menu_handle);
                    }
                    _ => {}
                }
                true
            }
            _ => false,
        }
    }

    fn new_client(&mut self, handle: XcbWindow) {
        let client = self.wm.win2client(&handle).unwrap();
        // The default value here is what ends up as the tooltip.
        let id = client.application_id().unwrap_or("(Unknown)");
        if id.starts_with("window_manager_") {
            // Would cause an infinite cycle with `window_manager_message_box`.
            return;
        }
        if let Some(idx) = self.find_item_name(id) {
            log::trace!("dock: new instance of '{id}'");
            self.items[idx].add_instance(client);
            self.update_item(idx);
        } else if let Some(mut item) = Item::new(
            &self.window,
            id,
            false,
            self.layout.item(self.items.len()),
            self.layout.icon(),
            &self.wm,
        ) {
            if id == "(Unknown)" {
                log::trace!("dock: new item without id: {}", handle.resource_id());
            } else {
                log::trace!("dock: new item: {id}");
            }
            item.add_instance(client);
            self.geometry = self.layout.dock(self.items.len() + 1);
            self.window.move_and_resize(self.geometry);
            self.items.push(item);
            self.update();
        }
    }

    fn remove_client(&mut self, handle: XcbWindow) {
        let client = self.wm.win2client(&handle).unwrap();
        if let Some(idx) = self.find_client_item(&client) {
            if self.items[idx].remove_instance(handle) {
                self.items.remove(idx);
                self.geometry = self.layout.dock(self.items.len());
                self.window.move_and_resize(self.geometry);
                self.layout_items();
                self.update();
            } else {
                self.update_item(idx);
            }
        }
    }

    fn update_focus(&mut self, handle: XcbWindow) {
        let client = self.wm.win2client(&handle).unwrap();
        if let Some(idx) = self.find_client_item(&client) {
            self.items[idx].update_focus(handle);
        }
    }

    fn update_urgency(&mut self, handle: XcbWindow) {
        let client = self.wm.win2client(&handle).unwrap();
        if let Some(idx) = self.find_client_item(&client) {
            if self.items[idx].update_urgency() {
                self.update_item(idx);
            }
        }
    }
}

impl EventSink for Dock {
    fn accept(&mut self, event: &Event) -> bool {
        use xcb::x::Event::*;
        match event {
            Event::X(x_event) => match x_event {
                EnterNotify(e) => self.enter(e),
                LeaveNotify(e) => self.leave(e),
                ButtonPress(e) => self.click(e),
                _ => false,
            },
            _ => false,
        }
    }

    fn signal(&mut self, signal: &Signal) {
        log::debug!("dock: signal: {signal:?}");
        match signal {
            Signal::ActiveWorkspaceEmpty(is_empty) => {
                if *is_empty {
                    self.keep_open(true)
                }
            }
            Signal::ClientGeometry(..) | Signal::ClientMinimized(..) => self.check_occluded(None),
            Signal::NewClient(handle) => self.new_client(*handle),
            Signal::ClientRemoved(handle) => {
                self.remove_client(*handle);
                self.check_occluded(Some(*handle));
            }
            Signal::FocusClient(handle) => self.update_focus(*handle),
            Signal::UrgencyChanged(handle) => self.update_urgency(*handle),
            Signal::Resize => {
                self.resize();
                self.check_occluded(None);
            }
            Signal::Quit => self.destroy(),
            _ => {}
        }
    }

    fn filter(&self) -> &'static [u32] {
        use xcb::{x::*, BaseEvent};
        &[
            EnterNotifyEvent::NUMBER,
            LeaveNotifyEvent::NUMBER,
            ButtonPressEvent::NUMBER,
        ]
    }
}
