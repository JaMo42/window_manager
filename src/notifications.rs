use crate::{
    config::{Config, Size},
    draw::{Alignment, DrawingContext, Svg},
    event::{EventSink, Signal},
    ewmh::{self, WindowType},
    log_error,
    monitors::monitors,
    rectangle::Rectangle,
    timeout_thread::TimeoutThread,
    window_manager::WindowManager,
    x::{Window, XcbWindow},
};
use pango::{Layout, Weight, WrapMode};
use parking_lot::Mutex;
use std::{
    cmp::Ordering,
    collections::HashMap,
    sync::{Arc, Weak},
};
use xcb::{x::EventMask, Event};
use zbus::{block_on, dbus_interface, zvariant, Result, SignalContext};

const NAME: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";

/// The notification expired
const CLOSE_REASON_EXPIRED: u32 = 1;
/// The notification was dismissed by the user
const CLOSE_REASON_DISMISSED: u32 = 2;
/// The notification was closed by a call to `CloseNotification`
const CLOSE_REASON_CLOSED: u32 = 3;

#[derive(Debug)]
#[allow(dead_code)]
pub struct NotificationProps<'a> {
    pub app_name: &'a str,
    pub app_icon: &'a str,
    pub summary: &'a str,
    pub body: &'a str,
    pub actions: &'a [&'a str],
    pub hints: &'a HashMap<&'a str, zvariant::Value<'a>>,
}

struct Notification {
    id: u32,
    window: Window,
    geometry: Rectangle,
    summary: String,
    body: String,
    icon: Option<Svg>,
}

impl Notification {
    fn new(wm: &WindowManager, id: u32, props: &NotificationProps) -> Self {
        let visual = wm.display.truecolor_visual();
        let window = Window::builder(wm.display.clone())
            .depth(visual.depth)
            .visual(visual.id)
            .attributes(|attributes| {
                attributes
                    .border_pixel(0)
                    .background_pixel(0)
                    .event_mask(EventMask::BUTTON_PRESS)
                    .colormap(visual.colormap);
            })
            .build();
        ewmh::set_window_type(&window, WindowType::Notification);
        let mut this = Self {
            id,
            window,
            geometry: Rectangle::zeroed(),
            summary: String::new(),
            body: String::new(),
            icon: None,
        };
        this.window.map();
        this.replace(props, &wm.drawing_context.lock(), &wm.config);
        this
    }

    fn destroy(&self) {
        self.window.destroy();
    }

    fn replace(&mut self, props: &NotificationProps, dc: &DrawingContext, config: &Config) {
        self.summary = props.summary.to_owned();
        self.body = props.body.to_owned();
        if props.app_icon.is_empty() {
            self.icon = None;
        } else {
            let app_icon = if props.app_icon.starts_with("file:") {
                &props.app_icon[7..]
            } else {
                props.app_icon
            };
            if let Some(path) = config.icon_reg.lookup(app_icon) {
                self.icon = Svg::try_load(&path).ok();
            }
        }
        self.update(dc, config);
    }

    fn update(&mut self, dc: &DrawingContext, config: &Config) {
        let (mon_geometry, dpmm) = {
            let m = monitors();
            let m = m.primary();
            let g = *m.geometry();
            let d = m.dpmm();
            (g, d)
        };
        let max_width = (mon_geometry.width as i32 / 4) * pango::SCALE;

        let set_text = |layout: &Layout, font, text| {
            layout.set_font_description(Some(font));
            layout.set_text(text);
            layout.set_width(max_width);
            layout.set_wrap(WrapMode::WordChar);
            let (width, height) = layout.size();
            #[rustfmt::skip]
            return ((width / pango::SCALE) as u16, (height / pango::SCALE) as u16);
        };

        let summary_layout = dc.create_layout();
        let mut summary_font = config.bar.font.clone();
        summary_font.set_weight(Weight::Semibold);
        summary_font.set_size((summary_font.size() as f64 * 1.1).round() as i32);
        let (s_width, s_height) = set_text(&summary_layout, &summary_font, &self.summary);

        let body_layout = dc.create_layout();
        let (b_width, b_height) = set_text(&body_layout, &config.bar.font, &self.body);

        let padding = Size::Physical(2.0).resolve(Some(dpmm), None, None) as i16;
        let summary_body_space = padding;
        let space = (padding + 1) / 2;
        let content_height = s_height + summary_body_space as u16 + b_height;
        let text_width = u16::max(s_width, b_width);

        dc.fill_rect(
            (
                0,
                0,
                2 * padding as u16 + content_height + space as u16 + text_width,
                2 * padding as u16 + content_height,
            ),
            config.colors.bar_background,
        );

        let x;
        if let Some(icon) = &self.icon {
            let max_size = Size::Physical(20.0).resolve(Some(dpmm), None, None);
            let size = u16::min(content_height, max_size);
            let y = (content_height - size) as i16 / 2;
            dc.draw_svg(icon, (padding, padding + y, size, size));
            x = padding + size as i16 + space;
        } else {
            x = padding;
        }
        dc.set_color(config.colors.bar_text);
        dc.text_layout(&summary_layout, (x, padding, text_width, s_height))
            .vertical_alignment(Alignment::CENTER)
            .draw();
        dc.text_layout(
            &body_layout,
            (
                x,
                padding + s_height as i16 + summary_body_space,
                text_width,
                b_height,
            ),
        )
        .vertical_alignment(Alignment::CENTER)
        .draw();

        self.geometry.width = x as u16 + text_width + padding as u16;
        self.geometry.height = 2 * padding as u16 + content_height;
        self.window
            .resize(self.geometry.width, self.geometry.height);
        dc.render(&self.window, self.geometry.at(0, 0));
    }
}

pub struct NotificationManager {
    wm: Weak<WindowManager>,
    notifications: Vec<Notification>,
    next_id: u32,
    timeout_threads: Vec<TimeoutThread>,
    space: i16,
}

impl NotificationManager {
    pub fn new() -> Self {
        let dpmm = monitors().primary().dpmm();
        let space = Size::Physical(1.0).resolve(Some(dpmm), None, None) as i16;
        Self {
            wm: Weak::new(),
            notifications: Vec::new(),
            next_id: 1,
            timeout_threads: Vec::new(),
            space,
        }
    }

    pub fn set_window_manager(&mut self, wm: &Arc<WindowManager>) {
        self.wm = Arc::downgrade(wm);
    }

    pub fn register(manager: Arc<Mutex<Self>>) -> Result<()> {
        let dbus = manager.lock().wm.upgrade().unwrap().dbus.clone();
        let server = Server { manager };
        dbus.register_server(NAME, PATH, server)?;
        Ok(())
    }

    pub fn unregister(&self) {
        let wm = self.wm.upgrade().unwrap();
        log_error!(wm.dbus.remove_server::<Server>(NAME, PATH));
    }

    pub fn destroy(&mut self) {
        log::trace!("notification manager: destroying notification manager");
        for t in self.timeout_threads.iter_mut() {
            t.cancel();
        }
        self.join_finished_timeout_threads();
        for n in self.notifications.iter() {
            n.destroy();
        }
    }

    pub fn get_id(&mut self, replaces: u32) -> u32 {
        if replaces != 0 {
            replaces
        } else if self.next_id == u32::MAX {
            self.next_id = 1;
            u32::MAX
        } else {
            let id = self.next_id;
            self.next_id += 1;
            id
        }
    }

    fn find(&self, id: u32) -> Option<usize> {
        self.notifications.iter().position(|n| n.id == id)
    }

    pub fn new_notification(&mut self, id: u32, props: &NotificationProps) {
        let wm = match self.wm.upgrade() {
            Some(wm) => wm,
            None => return,
        };
        if let Some(idx) = self.find(id) {
            self.notifications[idx].replace(props, &wm.drawing_context.lock(), &wm.config);
        } else {
            self.notifications.push(Notification::new(&wm, id, props));
        }
        self.arrange();
    }

    pub fn close_after(&mut self, id: u32, timeout: Option<i32>, manager: Arc<Mutex<Self>>) {
        self.join_finished_timeout_threads();
        let timeout = match timeout {
            Some(t) => t as u64,
            None => {
                let wm = self.wm.upgrade().unwrap();
                let default = wm.config.general.default_notification_timeout;
                if default == 0 {
                    return;
                }
                default
            }
        };
        self.timeout_threads.push(TimeoutThread::new(
            timeout,
            Arc::new(move || {
                let mut manager = manager.lock();
                manager.close_notification(id);
                manager.signal_close(id, CLOSE_REASON_EXPIRED);
            }),
        ));
    }

    fn close_notification(&mut self, id: u32) {
        if let Some(idx) = self.find(id) {
            self.notifications.remove(idx).destroy();
            self.arrange();
        }
    }

    fn arrange(&mut self) {
        let window_area = *monitors().primary().window_area();
        let mut y = window_area.y;
        let right_x = window_area.right_edge();
        for n in self.notifications.iter_mut() {
            n.geometry.y = y;
            n.geometry.x = right_x - n.geometry.width as i16;
            n.window.r#move(n.geometry.x, n.geometry.y);
            y += n.geometry.height as i16 + self.space;
        }
    }

    fn join_finished_timeout_threads(&mut self) {
        for i in (0..self.timeout_threads.len()).rev() {
            if self.timeout_threads[i].is_finished() {
                self.timeout_threads.remove(i).join();
            }
        }
    }

    fn maybe_close(&mut self, window: XcbWindow) -> bool {
        if let Some(id) = self
            .notifications
            .iter()
            .find(|n| n.window.eq(&window))
            .map(|n| n.id)
        {
            self.close_notification(id);
            self.signal_close(id, CLOSE_REASON_DISMISSED);
            true
        } else {
            false
        }
    }

    fn signal_close(&self, id: u32, reason: u32) {
        block_on(async {
            if let Some(wm) = self.wm.upgrade() {
                let iface_ref = wm.dbus.get_interface::<Server>(PATH);
                let iface = iface_ref.get_mut().await;
                iface
                    .notification_closed(iface_ref.signal_context(), id, reason)
                    .await
                    .unwrap();
            }
        });
    }
}

impl EventSink for NotificationManager {
    fn accept(&mut self, event: &Event) -> bool {
        use xcb::x::Event::ButtonPress;
        if let Event::X(ButtonPress(e)) = event {
            // We shouldn't have a lot of open notifications at any time so just
            // iterating the list and looking for the window will be faster
            // than getting the window manager and checking the window kind.
            self.maybe_close(e.event())
        } else {
            false
        }
    }

    fn signal(&mut self, signal: &Signal) {
        match signal {
            Signal::Resize => {
                let dpmm = monitors().primary().dpmm();
                self.space = Size::Physical(1.0).resolve(Some(dpmm), None, None) as i16;
            }
            Signal::Quit => self.destroy(),
            _ => {}
        }
    }

    fn filter(&self) -> &'static [u32] {
        use xcb::{x::*, BaseEvent};
        &[ButtonPressEvent::NUMBER]
    }
}

struct Server {
    manager: Arc<Mutex<NotificationManager>>,
}

#[dbus_interface(name = "org.freedesktop.Notifications")]
impl Server {
    /// `org.freedesktop.Notifications.GetServerInformation`
    async fn get_server_information(&self) -> (&str, &str, &str, &str) {
        (
            "window_manager_notification_server", // name
            "window_manager",                     // vendor
            "1.0",                                // server version
            "1.2",                                // spec version
        )
    }

    /// `org.freedesktop.Notifications.GetCapabilities`
    async fn get_capabilities(&self) -> Vec<&str> {
        vec!["body", "persistence", "body-images"]
    }

    /// `org.freedesktop.Notifications.Notify`
    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &mut self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: Vec<&str>,
        hints: HashMap<&str, zvariant::Value<'_>>,
        expire_timeout: i32,
    ) -> u32 {
        let props = NotificationProps {
            app_name,
            app_icon,
            summary,
            body,
            actions: actions.as_slice(),
            hints: &hints,
        };
        let mut manager = self.manager.lock();
        let id = manager.get_id(replaces_id);
        manager.new_notification(id, &props);
        if let Some(timeout) = match expire_timeout.cmp(&0) {
            Ordering::Less => Some(None),
            Ordering::Greater => Some(Some(expire_timeout)),
            Ordering::Equal => None,
        } {
            manager.close_after(id, timeout, self.manager.clone());
        }
        id
    }

    /// `org.freedesktop.Notifications.CloseNotification`
    async fn close_notification(&mut self, id: u32) {
        let mut manager = self.manager.lock();
        manager.close_notification(id);
        manager.signal_close(id, CLOSE_REASON_CLOSED);
    }

    /// `org.freedesktop.Notifications.NotificationClosed`
    #[dbus_interface(signal)]
    async fn notification_closed(
        &self,
        ctxt: &SignalContext<'_>,
        id: u32,
        reason: u32,
    ) -> Result<()>;
}
