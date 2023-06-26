use crate::{
    action::resnap,
    class_hint::ClassHint,
    client::{Client, SetClientGeometry},
    config::{Action, Config, WorkspaceAction},
    dialog::SpecialDialog,
    error::OrFatal,
    event::{EventSink, Signal},
    ewmh::{self, WindowType},
    extended_frame::ExtendedFrame,
    monitors::{monitors, monitors_mut},
    mouse::{mouse_move, mouse_resize, MouseResizeOptions, BUTTON_1, BUTTON_3},
    process::run_or_message_box,
    rectangle::Rectangle,
    session_manager::SESSION_MANAGER_EVENT,
    spawn_pos::spawn_geometry,
    window_manager::{WindowKind, WindowManager},
    x::{Display, PropertyValue, SetProperty, Window, XcbWindow},
};
use std::{
    ops::Deref,
    sync::{mpsc::Sender, Arc},
};
use xcb::{
    x::{
        ButtonPressEvent, ClientMessageEvent, ConfigWindowMask, ConfigureNotifyEvent,
        ConfigureRequestEvent, DestroyNotifyEvent, EnterNotifyEvent, KeyPressEvent,
        LeaveNotifyEvent, MapRequestEvent, Mapping, MappingNotifyEvent, ModMask, MotionNotifyEvent,
        PropertyNotifyEvent, UnmapNotifyEvent, ATOM_WM_HINTS, ATOM_WM_NAME,
    },
    Event, Xid,
};

pub struct MainEventSink {
    pressed_button: u8,
    display: Arc<Display>,
    config: Arc<Config>,
    signal_sender: Sender<Signal>,
    screen_size: (u16, u16),
    wm: Arc<WindowManager>,
    // we keep track of this so we don't need to check for window kind on every
    // motion event
    extended_frame_hovered: Option<ExtendedFrame>,
}

impl MainEventSink {
    pub fn new(wm: Arc<WindowManager>) -> Self {
        Self {
            pressed_button: 0,
            display: wm.display.clone(),
            config: wm.config.clone(),
            signal_sender: wm.signal_sender.clone(),
            screen_size: wm.display.get_total_size(),
            wm,
            extended_frame_hovered: None,
        }
    }

    fn map_request(&mut self, event: &MapRequestEvent) {
        use WindowType::*;
        if event.parent() != self.display.root() {
            log::debug!("Got map request for non-toplevel?");
            return;
        }
        if let Some(client) = self.wm.win2client(&event.window()) {
            if client.workspace() == self.wm.active_workspace_index() {
                client.unminimize();
            }
            return;
        }
        let window = Window::from_handle(self.display.clone(), event.window());
        if let Some(class_hint) = ClassHint::get(&window) {
            if self
                .config
                .general
                .meta_window_classes
                .contains(&class_hint.class)
            {
                self.map_meta(window);
                return;
            } else if class_hint.class == "Window_manager_quit" {
                log::trace!("created special dialog: {}", class_hint.name);
                SpecialDialog::create(window, &self.wm);
                return;
            }
        }
        // If the window type is not specified or invalid, assuming it's a normal
        // window makes the most sense.
        let window_type = WindowType::get(&self.display, event.window());
        match window_type {
            Toolbar | Menu | Utility | Dialog | Normal => self.map_new_client(window, window_type),
            Splash => self.map_splash(window),
            _ => self.map_meta(window),
        }
    }

    fn map_new_client(&mut self, window: Window, window_type: WindowType) {
        let handle = window.handle();
        let client = Client::new(&self.wm, window, window_type);
        let g = spawn_geometry(&client, &self.wm.active_workspace(), &self.wm.config);
        client.move_and_resize(SetClientGeometry::Frame(g));
        client.save_geometry();
        if client.workspace() == self.wm.active_workspace_index() {
            client.map();
            client.draw_border();
            self.display.set_input_focus(client.handle());
        }
        log::info!("Mapped new client: {}", client.id_info());
        self.signal_sender
            .send(Signal::NewClient(client.handle()))
            .or_fatal(&self.display);
        self.wm.root.append_property(
            &self.display,
            self.display.atoms.net_client_list,
            PropertyValue::Window(handle),
        );
        self.wm.active_workspace().push(client);
    }

    fn map_splash(&mut self, window: Window) {
        let mine = window.clone();
        self.map_meta(window);
        // Center it on the monitor it spawned on.
        let mut g = Rectangle::from_parts(mine.get_geometry());
        let mon = monitors().primary().clone();
        mine.move_and_resize(*g.center_inside(mon.window_area()));
    }

    fn map_meta(&mut self, window: Window) {
        self.wm
            .set_window_kind(&window, WindowKind::MetaOrUnmanaged);
        window.map();
        log::info!("Mapped new meta/unmanaged window: {}", window);
        self.wm.add_unmanaged(window);
    }

    fn key_press(&mut self, event: &KeyPressEvent) {
        if let Some(action) = self.wm.get_key_binding(event) {
            match action {
                Action::Client(f) => {
                    if let Some(client) = self.wm.focused_client() {
                        f(&client);
                    }
                }
                Action::Workspace(WorkspaceAction(f, idx, need_client)) => {
                    if need_client {
                        if let Some(focused) = self.wm.focused_client() {
                            f(self.wm.as_ref(), idx, Some(&*focused));
                        }
                    } else {
                        f(self.wm.as_ref(), idx, None);
                    }
                }
                Action::Launch(command) => {
                    run_or_message_box(&command.iter().map(Deref::deref).collect::<Vec<&str>>())
                }
                Action::Generic(f) => {
                    f(self.wm.as_ref());
                }
            }
        } else {
            log::trace!("Key press without action: {event:#?}");
        }
    }

    fn button_press(&mut self, event: &ButtonPressEvent) {
        let window = event.event();
        let window_kind = self.wm.get_window_kind(&window);
        let child = event.child();
        let child_kind = self.wm.get_window_kind(&child);
        if !child.is_none() && matches!(child_kind, WindowKind::MetaOrUnmanaged) {
            return;
        }
        let mut done = false;
        match window_kind {
            WindowKind::FrameButton => {
                if let Some(client) = self.wm.win2client(&window) {
                    client.click_button(window);
                } else {
                    log::warn!(
                        "Window button without associated client: {}",
                        window.resource_id()
                    );
                }
            }
            WindowKind::MetaOrUnmanaged | WindowKind::ExtendedFrame => {}
            _ => {
                log::warn!(
                    "Ignoring click on window with button press mask: {}",
                    window.resource_id()
                );
                done = true;
            }
        }
        if done {
            return;
        }
        if let Some(client) = self.wm.win2client(&child) {
            if event.detail() == BUTTON_1 && !client.may_move()
                || event.detail() == BUTTON_3 && !client.may_resize()
                || event.detail() == BUTTON_1 && client.click_frame(event.time())
            {
                return;
            }
            client.focus();
            self.pressed_button = event.detail();
            let mut workspace = self.wm.workspace(client.workspace());
            if let Some(prev) = workspace.focused() {
                client.ensure_stacked_above(prev);
            }
            workspace.focus(child);
        }
    }

    fn button_release(&mut self) {
        self.pressed_button = 0;
    }

    fn motion(&mut self, event: &MotionNotifyEvent) {
        if self.pressed_button != 0 {
            if let Some(client) = self.wm.win2client(&event.child()) {
                let options;
                let state = ModMask::from_bits_truncate(event.state().bits());
                if !state.contains(self.config.modifier()) {
                    let frame = client.frame_geometry();
                    options = MouseResizeOptions::from_position(
                        frame,
                        event.root_x(),
                        event.root_y(),
                        3 * client.frame_offset().x as u16,
                    );
                    if event.root_y() < frame.y + client.frame_offset().y
                        && matches!(self.wm.get_window_kind(&event.child()), WindowKind::Frame)
                    {
                        // on title bar
                        self.pressed_button = BUTTON_1;
                    } else {
                        self.pressed_button = BUTTON_3;
                    }
                } else {
                    options = MouseResizeOptions::default();
                }
                match self.pressed_button {
                    BUTTON_1 => mouse_move(&client, self.pressed_button),
                    BUTTON_3 => mouse_resize(&client, options),
                    _ => {}
                }
            }
            self.pressed_button = 0;
        } else if let Some(exframe) = &self.extended_frame_hovered {
            exframe.update_cursor(
                &self.display,
                &self.wm.cursors,
                event.root_x(),
                event.root_y(),
            );
        } else {
            // Ignore all immediately following motion events.
            use xcb::x::Event::*;
            let mut event;
            loop {
                event = self.display.next_event();
                if !matches!(event, Ok(Event::X(MotionNotify(_)))) {
                    break;
                }
            }
            self.display.put_back_event(event);
        }
    }

    fn mapping_notify(&mut self, event: &MappingNotifyEvent) {
        if !matches!(event.request(), Mapping::Pointer) {
            self.display.refresh_keyboard_mapping(event);
        }
        self.wm.mapping_changed(event.request());
    }

    fn destroy_window(&mut self, window: XcbWindow) {
        if let Some(client) = self.wm.win2client(&window) {
            if client
                .application_id()
                .map(|id| id.starts_with("window_manager_"))
                .unwrap_or(false)
            {
                self.wm.active_workspace().remove(&client);
                client.destroy();
                self.wm.update_client_list();
                return;
            }
            self.signal_sender
                .send(Signal::ClientRemoved(client.handle()))
                .or_fatal(&self.display);
            // We want to allow sinks to use `win2client` to get the client that
            // is being destroyed so we don't immediately remove it here but
            // instead use our own signal handler which is always executed
            // last.
        }
    }

    fn destroy_notify(&mut self, event: &DestroyNotifyEvent) {
        self.destroy_window(event.window());
    }

    fn property_notify(&mut self, event: &PropertyNotifyEvent) {
        let display = &self.display;
        let property = event.atom();
        if let Some(client) = self.wm.win2client(&event.window()) {
            if property == ATOM_WM_HINTS {
                client.update_wm_hints();
            } else if property == ATOM_WM_NAME || property == display.atoms.net_wm_name {
                client.update_title();
            } else if property == display.atoms.net_wm_user_time && !client.is_focused_client() {
                if client.is_on_active_workspace() {
                    let mut workspace = self.wm.workspace(client.workspace());
                    drop(client);
                    workspace.focus(event.window());
                } else {
                    client.set_urgency(true);
                }
            }
        }
    }

    fn configure_request(&mut self, event: &ConfigureRequestEvent) {
        if let Some(client) = self.wm.win2client(&event.window()) {
            let mut new_geometry = client.client_geometry();
            let mut geometry_changed = false;
            if event.value_mask().contains(ConfigWindowMask::X) {
                new_geometry.x = event.x();
                geometry_changed = true;
            }
            if event.value_mask().contains(ConfigWindowMask::Y) {
                new_geometry.y = event.y();
                geometry_changed = true;
            }
            if event.value_mask().contains(ConfigWindowMask::WIDTH) {
                new_geometry.width = event.width();
                geometry_changed = true;
            }
            if event.value_mask().contains(ConfigWindowMask::HEIGHT) {
                new_geometry.height = event.height();
                geometry_changed = true;
            }
            if geometry_changed {
                client.move_and_resize(SetClientGeometry::Client(new_geometry));
                if !client.is_snapped() {
                    client.save_geometry();
                }
            }
        }
    }

    fn configure_notify(&mut self, event: &ConfigureNotifyEvent) {
        if event.window() != self.display.root() {
            return;
        }
        let new_size = (event.width(), event.height());
        let size_changed = self.screen_size != new_size;
        self.screen_size = new_size;
        let mut monitors = monitors_mut();
        let old_primary_dpmm = monitors.primary().dpmm();
        if monitors.update(&self.display, &self.config) || size_changed {
            log::info!("Updating monitor configuration");
            let primary_dpmm = monitors.primary().dpmm();
            drop(monitors);
            #[rustfmt::skip]
            if (primary_dpmm - old_primary_dpmm).abs() > 0.01
                && self.config.general.scale_base_fonts
            {
                let factor = primary_dpmm / old_primary_dpmm;
                self.config.scale_fonts(factor);
            };
            {
                let mut dc = self.wm.drawing_context.lock();
                dc.resize(event.width(), event.height());
                self.config.recompute_layouts(&dc);
            }
            self.wm.update_fullscreen_windows();
            for client in self.wm.active_workspace().iter() {
                client.layout_changed();
                if client.is_snapped() {
                    resnap(client);
                } else {
                    let g = *client.get_monitor().window_area();
                    client.modify_saved_geometry(move |saved| {
                        saved.clamp_inside(&g);
                    });
                    client.move_and_resize(SetClientGeometry::Frame(client.saved_geometry()));
                }
            }
            self.signal_sender
                .send(Signal::Resize)
                .or_fatal(&self.display);
        }
    }

    fn crossing(&mut self, window: XcbWindow, is_enter: bool) {
        // TODO
        match self.wm.get_window_kind(&window) {
            WindowKind::FrameButton => {
                if let Some(client) = self.wm.win2client(&window) {
                    client.cross_button(window, is_enter);
                }
            }
            WindowKind::ExtendedFrame => {
                self.extended_frame_hovered = None;
                if is_enter {
                    if let Some(client) = self.wm.win2client(&window) {
                        self.extended_frame_hovered = Some(client.extended_frame().clone())
                    }
                }
            }
            _ => {}
        }
    }

    fn enter_notify(&mut self, event: &EnterNotifyEvent) {
        self.crossing(event.event(), true);
    }

    fn leave_notify(&mut self, event: &LeaveNotifyEvent) {
        self.crossing(event.event(), false);
    }

    fn client_message(&mut self, event: &ClientMessageEvent) {
        if event.window() == self.display.root() {
            ewmh::root_message(&self.wm, event);
        } else if let Some(client) = self.wm.win2client(&event.window()) {
            ewmh::client_message(&client, event);
        } else {
            log::trace!(
                "Unhandeled client message: {:?} to {}",
                event.r#type(),
                event.window().resource_id()
            );
        }
    }

    fn unmap_notify(&mut self, event: &UnmapNotifyEvent) {
        if let Some(client) = self.wm.win2client(&event.window()) {
            if !client.is_minimized() && client.is_on_active_workspace() {
                // The client got unmapped but we didn't cause it.  This may
                // be a valid thing where we don't want to destroy the client
                // but some program seem to do this instead of sending a
                // DestroyNotify when closing their window (like solaar).
                // I guess because they don't actually destroy their window but
                // we need the window gone so we treat it the same a destruction.
                self.destroy_window(event.window());

                // This seems to work fine and fixes the problem of some windows
                // leaving their frame behind when being closed but will need to
                // keep an eye on it if it causes any problems.
            }
        }
    }
}

impl EventSink for MainEventSink {
    fn accept(&mut self, event: &Event) -> bool {
        use xcb::x::Event::*;
        match event {
            Event::X(x_event) => match x_event {
                ButtonPress(e) => self.button_press(e),
                ButtonRelease(_) => self.button_release(),
                ClientMessage(e) => self.client_message(e),
                ConfigureNotify(e) => self.configure_notify(e),
                ConfigureRequest(e) => self.configure_request(e),
                DestroyNotify(e) => self.destroy_notify(e),
                EnterNotify(e) => self.enter_notify(e),
                KeyPress(e) => self.key_press(e),
                LeaveNotify(e) => self.leave_notify(e),
                MappingNotify(e) => self.mapping_notify(e),
                MapRequest(e) => self.map_request(e),
                MotionNotify(e) => self.motion(e),
                PropertyNotify(e) => self.property_notify(e),
                UnmapNotify(e) => self.unmap_notify(e),
                _ => return false,
            },
            Event::Unknown(unknown_event) => match unknown_event.response_type() {
                SESSION_MANAGER_EVENT => {
                    self.wm.session_manager.lock().process();
                }
                _ => return false,
            },
            _ => return false,
        }
        true
    }

    fn signal(&mut self, signal: &Signal) {
        // See [`destroy_notify`] for why we remove the client here.
        if let Signal::ClientRemoved(handle) = signal {
            // We already checked `win2client` returns a valid value in `destroy_notify`
            let client = self.wm.win2client(handle).unwrap();
            self.wm.active_workspace().remove(&client);
            client.destroy();
            self.wm.update_client_list();
            if self.extended_frame_hovered.as_ref() == Some(client.extended_frame()) {
                self.extended_frame_hovered = None;
            }
            log::trace!("Removed client: {client}");
        }
    }

    fn filter(&self) -> &'static [u32] {
        use xcb::{x::*, BaseEvent};
        &[
            ButtonPressEvent::NUMBER,
            ButtonReleaseEvent::NUMBER,
            ClientMessageEvent::NUMBER,
            ConfigureNotifyEvent::NUMBER,
            ConfigureRequestEvent::NUMBER,
            DestroyNotifyEvent::NUMBER,
            EnterNotifyEvent::NUMBER,
            KeyPressEvent::NUMBER,
            LeaveNotifyEvent::NUMBER,
            MappingNotifyEvent::NUMBER,
            MapRequestEvent::NUMBER,
            MotionNotifyEvent::NUMBER,
            PropertyNotifyEvent::NUMBER,
            UnmapNotifyEvent::NUMBER,
        ]
    }
}
