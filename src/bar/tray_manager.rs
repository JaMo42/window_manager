use super::{tray_client::TrayClient, xembed};
use crate::{
    event::Signal,
    ewmh::{self, WindowType},
    set_compositor_opacity,
    window_manager::{WindowKind, WindowManager},
    x::{Display, PropertyValue, SetProperty, Window, XcbWindow},
    AnyResult,
};
use std::{
    collections::BTreeMap,
    sync::Arc,
    thread::{self, sleep, JoinHandle},
    time::Duration,
};
use xcb::{
    x::{
        AllocColor, ClientMessageData, ClientMessageEvent, Colormap, EventMask,
        GetWindowAttributes, MapNotifyEvent, PropertyNotifyEvent, UnmapNotifyEvent, CURRENT_TIME,
    },
    Xid, XidNew,
};

const ORIENTATION_HORIZONTAL: u32 = 0;
const OPCODE_REQUEST_DOCK: u32 = 0;

pub struct TrayManager {
    wm: Arc<WindowManager>,
    window: Window,
    clients: Vec<TrayClient>,
    // Handle of the thread for the delayed selection notification.
    notify_thread: Option<JoinHandle<()>>,
    current_mapped_count: usize,
    height: u16,
    monitor_width: u16,
    // When the tray is empty we unmap the window.
    is_mapped: bool,
    // Unless the tray icons use the same truecolor colormap as the manager
    // window we need to allocate our background color on them and use that.
    known_colormaps: BTreeMap<Colormap, u32>,
}

impl TrayManager {
    fn acquire_selection(display: &Display, window: &Window) -> Result<(), &'static str> {
        log::trace!("tray: Acquiring selection");
        let selection = display.atoms.net_system_tray_s0;
        if !display.get_selection_owner(selection).is_none() {
            return Err("Selection already owned");
        }
        display.set_selection_owner(selection, window.handle());
        if display.get_selection_owner(selection) != window.handle() {
            return Err("Failed to set selection");
        }
        Ok(())
    }

    pub fn create(
        wm: &Arc<WindowManager>,
        height: u16,
        monitor_width: u16,
    ) -> Result<Self, &'static str> {
        log::trace!("tray: Creating manager");
        let display = wm.display.clone();
        let visual = display.truecolor_visual();
        let window = Window::builder(display.clone())
            .size(height, height)
            .depth(visual.depth)
            .visual(visual.id)
            .attributes(|attributes| {
                attributes
                    .override_redirect()
                    .colormap(visual.colormap)
                    .border_pixel(0)
                    .background_pixel(0)
                    .cursor(wm.cursors.normal)
                    .event_mask(
                        EventMask::SUBSTRUCTURE_REDIRECT
                            | EventMask::STRUCTURE_NOTIFY
                            | EventMask::EXPOSURE
                            | EventMask::PROPERTY_CHANGE,
                    );
            })
            .build();
        log::trace!("tray: Manager window: {}", window);
        ewmh::set_window_type(&window, WindowType::Dock);
        wm.set_window_kind(&window, WindowKind::MetaOrUnmanaged);
        window.set_property(
            &display,
            display.atoms.net_system_tray_orientation,
            PropertyValue::Cardinal(ORIENTATION_HORIZONTAL),
        );
        set_compositor_opacity(&window, wm.config.colors.bar_background.alpha);
        Self::acquire_selection(&display, &window)?;
        let mut this = Self {
            wm: wm.clone(),
            window,
            clients: Vec::with_capacity(4),
            notify_thread: None,
            current_mapped_count: 0,
            height,
            monitor_width,
            is_mapped: false,
            known_colormaps: BTreeMap::new(),
        };
        this.notify_clients_after(&wm.root, 1000);
        log::trace!("tray: Finished creating tray manager");
        Ok(this)
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    fn notify_clients(&self, root: &Window) {
        root.send_event(
            EventMask::NO_EVENT,
            &ClientMessageEvent::new(
                root.handle(),
                root.display().atoms.manager,
                ClientMessageData::Data32([
                    CURRENT_TIME,
                    root.display().atoms.net_system_tray_s0.resource_id(),
                    self.window.handle().resource_id(),
                    0,
                    0,
                ]),
            ),
        );
    }

    fn notify_clients_after(&mut self, root: &Window, milliseconds: u64) {
        let this = self as *const Self as usize;
        let root = root as *const Window as usize;
        self.notify_thread = Some(thread::spawn(move || {
            sleep(Duration::from_millis(milliseconds));
            let this = unsafe { &*(this as *const Self) };
            let root = unsafe { &*(root as *const Window) };
            this.notify_clients(root);
        }));
    }

    pub fn destroy(&mut self) {
        log::trace!("bar: destroying tray manager");
        let root = self.wm.display.root();
        for c in self.clients.iter() {
            self.wm.remove_all_contexts(c.window());
            c.window().reparent(&root, 0, 0);
        }
        self.wm.remove_all_contexts(&self.window);
        self.window.destroy();
        if let Some(notify_thread) = self.notify_thread.take() {
            notify_thread.join().ok();
        }
    }

    pub fn set_size_info(&mut self, height: u16, monitor_width: u16) {
        self.height = height;
        self.monitor_width = monitor_width;
        self.resize_window();
        for c in self.clients.iter_mut() {
            c.set_size(height);
        }
        self.rearrange();
        self.refresh();
    }

    /// Get the visible with of the tray.
    pub fn width(&self) -> u16 {
        self.current_mapped_count as u16 * self.height
    }

    /// Adjusts the windows size and position. If there are no visible
    /// icons it is unmapped.
    pub fn resize_window(&mut self) {
        let width = self.width();
        if width != 0 {
            if !self.is_mapped {
                self.window.map();
                self.is_mapped = true;
            }
            self.window.move_and_resize((
                (self.monitor_width - width) as i16,
                0,
                width,
                self.height,
            ));
        } else {
            self.window.unmap();
            self.is_mapped = false;
        }
    }

    /// Get the number of mapped clients
    fn mapped_count(&self) -> usize {
        self.clients
            .iter()
            .fold(0, |acc, icon| acc + icon.is_mapped() as usize)
    }

    /// If `window` is the window of a client, remove that client and return `true`.
    pub fn maybe_remove_client(&mut self, window: XcbWindow) -> bool {
        if let Some(idx) = self.client_position(window) {
            self.clients.remove(idx).destroy(&self.wm);
            self.rearrange();
            self.refresh();
            log::trace!("tray: Removed tray icon: {}", window.resource_id());
            true
        } else {
            false
        }
    }

    fn client_position(&self, window: XcbWindow) -> Option<usize> {
        self.clients
            .iter()
            .position(|client| client.window().handle() == window)
    }

    fn find_client(&mut self, window: XcbWindow) -> Option<&mut TrayClient> {
        self.client_position(window)
            .map(|idx| &mut self.clients[idx])
    }

    /// Updates the positions of mapped clients.
    fn rearrange(&mut self) {
        let mut x = 0;
        for icon in self.clients.iter().filter(|icon| icon.is_mapped()) {
            icon.set_position(x, 0);
            x += self.height as i16;
        }
    }

    pub fn refresh(&self) {
        if !self.is_mapped {
            return;
        }
        self.window.clear();
        let mut x = 0;
        for client in self.clients.iter() {
            if client.is_mapped() {
                // Some clients need this or they have the wrong size (?)
                client.configure(x, 0);
                x += self.height as i16;
            }
        }
    }

    fn reconfigure(&mut self) {
        self.current_mapped_count = self.mapped_count();
        self.resize_window();
        self.rearrange();
        self.refresh();
    }

    fn sort_clients(&mut self) {
        self.clients.sort_by_key(|client| client.class());
    }

    /// Get the pixel value of the background color for the given window.
    fn get_background_color(&mut self, window: XcbWindow) -> xcb::Result<u32> {
        let conn = self.wm.display.connection();
        let cookie = conn.send_request(&GetWindowAttributes { window });
        let reply = conn.wait_for_reply(cookie)?;
        let colormap = reply.colormap();
        if colormap == self.wm.display.truecolor_visual().colormap {
            Ok(self.wm.config.colors.bar_background.with_alpha(1.0).pack())
        } else if let Some(known) = self.known_colormaps.get(&colormap).cloned() {
            Ok(known)
        } else {
            let color = self.wm.config.colors.bar_background;
            let red = (color.red * 0xFFFF as f64).round() as u16;
            let green = (color.green * 0xFFFF as f64).round() as u16;
            let blue = (color.blue * 0xFFFF as f64).round() as u16;
            let cookie = conn.send_request(&AllocColor {
                cmap: colormap,
                red,
                green,
                blue,
            });
            let reply = conn.wait_for_reply(cookie)?;
            self.known_colormaps.insert(colormap, reply.pixel());
            Ok(reply.pixel())
        }
    }

    fn try_dock(&mut self, window: XcbWindow) -> AnyResult<()> {
        // Note: Some clients send a docking request and then immediately destroy
        // the window before sending the actual tray client, this will currently
        // cause a bunch of errors since the x functions send errors to the main
        // loop.
        log::trace!("tray: Docking {}", window.resource_id());
        self.clients
            .push(TrayClient::new(&self.wm, window, self.height));
        let client = self.clients.last_mut().unwrap();
        let window = client.window().clone();

        if !client.query_xembed_info() {
            Err("Could not query XEmbed information")?;
        }

        log::trace!("tray: Changing client attributes");
        let pixel = self.get_background_color(window.handle())?;
        window.change_attributes(|attributes| {
            attributes
                .background_pixel(pixel)
                .event_mask(EventMask::STRUCTURE_NOTIFY | EventMask::PROPERTY_CHANGE);
        });
        window.resize(self.height, self.height);

        log::trace!("tray: Reparenting client");
        let client = self.clients.last_mut().unwrap();
        window.reparent(&self.window.handle(), 0, 0);
        xembed::embed(
            window.handle(),
            &self.window,
            client.xembed_info().version(),
        );

        if client.xembed_info().is_mapped() {
            log::trace!("tray: Mapping client");
            client.set_mapped(true);
            window.map();
        }

        log::trace!("tray: Updating tray");
        self.sort_clients();
        self.update_mapped_count();
        Ok(())
    }

    fn dock(&mut self, window: XcbWindow) {
        if let Err(error) = self.try_dock(window) {
            log::trace!(
                "tray: Docking of {} cancelled: {}",
                window.resource_id(),
                error
            );
            self.maybe_remove_client(window);
        }
    }

    /// Checks the mapped count and reconfigures the tray if neccessary.
    fn update_mapped_count(&mut self) {
        let mapped_count = self.mapped_count();
        if mapped_count != self.current_mapped_count {
            self.current_mapped_count = mapped_count;
            self.reconfigure();
            self.wm.signal_sender.send(Signal::UpdateBar(true)).ok();
        }
    }

    //**** Event handlers ****//

    pub fn client_message(&mut self, event: &ClientMessageEvent) {
        let data = match event.data() {
            ClientMessageData::Data32(data) => data,
            _ => {
                log::warn!("Invalid data format in tray client message");
                return;
            }
        };
        let opcode = data[1];
        if opcode == OPCODE_REQUEST_DOCK {
            self.dock(unsafe { XcbWindow::new(data[2]) });
        }
    }

    pub fn property_notify(&mut self, event: &PropertyNotifyEvent) {
        if let Some(client) = self.find_client(event.window()) {
            client.query_xembed_info();
            client.update_mapped_state();
            self.update_mapped_count();
        }
    }

    pub fn map_notify(&mut self, event: &MapNotifyEvent) {
        if let Some(client) = self.find_client(event.window()) {
            client.set_mapped(true);
            self.update_mapped_count();
        }
    }

    pub fn unmap_notify(&mut self, event: &UnmapNotifyEvent) {
        if let Some(client) = self.find_client(event.window()) {
            client.set_mapped(false);
            self.update_mapped_count();
        }
    }
}
