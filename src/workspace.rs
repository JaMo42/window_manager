use crate::{
    client::Client,
    config::Config,
    error::OrFatal,
    event::Signal,
    ewmh::Root,
    window_manager::WindowManager,
    x::{Display, Window, XcbWindow},
};
use itertools::Itertools;
use std::sync::{mpsc::Sender, Arc};
use x11::keysym::{XK_Alt_L, XK_Tab};
use xcb::{
    x::{ConfigWindow, ConfigureWindow, EventMask, KeyButMask, KeyPressEvent, StackMode},
    Event, Xid,
};

pub struct Workspace {
    index: usize,
    display: Arc<Display>,
    root: Root,
    config: Arc<Config>,
    clients: Vec<Arc<Client>>,
    signal_sender: Sender<Signal>,
    is_active: bool,
}

impl Workspace {
    pub fn new(index: usize, wm: &WindowManager) -> Self {
        Self {
            index,
            display: wm.display.clone(),
            root: wm.root.clone(),
            config: wm.config.clone(),
            clients: Vec::new(),
            signal_sender: wm.signal_sender.clone(),
            is_active: index == 0,
        }
    }

    pub fn set_active(&mut self, is_active: bool) {
        self.is_active = is_active;
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn is_empty(&self) -> bool {
        self.clients.is_empty()
    }

    pub fn clients(&self) -> &[Arc<Client>] {
        &self.clients
    }

    pub fn iter(&self) -> std::slice::Iter<Arc<Client>> {
        self.clients.iter()
    }

    /// Returns the focused client. None of the clients should be locked.
    pub fn focused(&self) -> Option<&Arc<Client>> {
        self.iter().find(|c| !c.real_state().is_minimized())
    }

    /// Find the index of the given client. Does not attempt to lock any of the clients.
    fn find_client(&self, client: &Client) -> Option<usize> {
        self.iter().position(|c| **c == *client)
    }

    fn find_client_by_id(&self, id: XcbWindow) -> Option<usize> {
        self.iter().position(|c| c.has_handle(id))
    }

    fn focus_client(&self, client: &Client) {
        client.focus();
        client.send_message(self.display.atoms.wm_take_focus);
        self.root.set_focused_client(Some(client.handle()));
        self.signal_sender
            .send(Signal::FocusClient(client.handle()))
            .or_fatal(&self.display);
    }

    pub fn push(&mut self, client: Arc<Client>) {
        if let Some(prev) = self.clients.first_mut() {
            prev.unfocus();
        }
        let client_workspace = client.workspace();
        self.clients.insert(0, client);
        if client_workspace == self.index {
            self.clients[0].focus();
        }
        if self.is_active && self.clients.len() == 1 {
            self.signal_sender
                .send(Signal::ActiveWorkspaceEmpty(false))
                .or_fatal(&self.display);
        }
    }

    pub fn remove(&mut self, client: &Client) -> Arc<Client> {
        if let Some(idx) = self.find_client(client) {
            let arc = self.clients.remove(idx);
            if let Some(new_focused) = self.focused().cloned() {
                self.focus_client(&new_focused);
            } else {
                self.root.set_focused_client(None);
            }
            if self.is_active && self.clients.is_empty() {
                self.signal_sender
                    .send(Signal::ActiveWorkspaceEmpty(true))
                    .or_fatal(&self.display);
            }
            return arc;
        }
        log::error!("Tried to remove client no on workspace");
        panic!("Tried to remove client no on workspace");
    }

    /// Restacks all clients in this workspace.
    /// For some reason this is neccessary to keep proper stacking order while
    /// also keeping the correct window order for extended frames because we
    /// we can't just raise the extended frame of the most recently focused
    /// window.
    fn restack(&self) {
        if self.clients.len() < 2 {
            return;
        }
        let stack = |upper, lower| {
            self.display
                .try_void_request(&ConfigureWindow {
                    window: lower,
                    value_list: &mut [
                        ConfigWindow::Sibling(upper),
                        ConfigWindow::StackMode(StackMode::Below),
                    ],
                })
                .unwrap();
        };
        self.clients[0].frame().raise();
        if let Some(exframe) = self.clients[0].extended_frame().handle() {
            stack(self.clients[0].frame().handle(), exframe);
        }
        for (upper, lower) in self.clients.iter().tuple_windows() {
            let upper = upper
                .extended_frame()
                .handle()
                .unwrap_or_else(|| upper.frame().handle());
            let lower_frame = lower.frame().handle();
            stack(upper, lower_frame);
            if let Some(exframe) = lower.extended_frame().handle() {
                stack(lower_frame, exframe);
            }
        }
    }

    /// Focus the client at the given index.
    /// Emits a `FocusClient` signal.
    pub fn focus_at(&mut self, idx: usize) {
        let window = self.clients[idx].handle();
        if let Some(prev) = self.focused() {
            if prev.window().handle() == window {
                prev.focus();
                return;
            }
            prev.unfocus();
        }
        if idx != 0 {
            let c = self.clients.remove(idx);
            self.clients.insert(0, c);
        }
        self.focus_client(&self.clients[0]);
        self.restack();
    }

    /// Focus the client with the given window.
    pub fn focus(&mut self, window: XcbWindow) {
        if cfg!(debug_assertions) && (window.is_none() || self.root.0 == window) {
            log::warn!(
                "Tried to focus {}",
                if window.is_none() { "None" } else { "Root" }
            );
        } else if let Some(idx) = self.find_client_by_id(window) {
            self.focus_at(idx);
        } else {
            log::error!("Tried to focus window not on workspace");
        }
    }

    /// Runs the window switcher.
    pub fn switch_window(&mut self, wm: &WindowManager) {
        use xcb::x::Event::*;
        const RATE: u32 = 1000 / 10;
        if self.clients.len() <= 1 {
            if let Some(only) = self.clients.first() {
                if only.real_state().is_minimized() {
                    only.focus();
                }
            }
            return;
        }
        let window = Window::builder(self.display.clone())
            .attributes(|attributes| {
                attributes.event_mask(EventMask::KEY_PRESS | EventMask::KEY_RELEASE);
            })
            .build();
        window.map();
        window.lower();
        self.display.set_input_focus(window.handle());
        // Put the first tab back
        let tab = self.display.keysym_to_keycode(XK_Tab);
        let event = KeyPressEvent::new(
            tab,
            RATE + 1,
            self.display.root(),
            window.handle(),
            window.handle(),
            0,
            0,
            0,
            0,
            KeyButMask::empty(),
            true,
        );
        self.display.put_back_event(Ok(Event::X(KeyPress(event))));
        let shift = KeyButMask::from_bits_truncate(wm.modmap.borrow().shift().bits());
        let alt = self.display.keysym_to_keycode(XK_Alt_L);
        let mut switch_idx = 0;
        let mut last_time = 0;
        loop {
            let event = match self.display.next_event() {
                Ok(event) => event,
                Err(_) => continue,
            };
            match event {
                Event::X(KeyPress(event)) => {
                    if event.time() - last_time < RATE {
                        continue;
                    }
                    last_time = event.time();
                    if event.detail() == tab {
                        {
                            let c = &self.clients[switch_idx];
                            c.set_border(self.config.colors.normal_border());
                            if c.real_state().is_minimized() {
                                c.unmap();
                            }
                        }
                        if event.state().contains(shift) {
                            if switch_idx == 0 {
                                switch_idx = self.clients.len() - 1;
                            } else {
                                switch_idx -= 1;
                            }
                        } else {
                            switch_idx = (switch_idx + 1) % self.clients.len();
                        }
                        {
                            let c = &self.clients[switch_idx];
                            if c.real_state().is_minimized() {
                                c.map();
                            }
                            c.set_border(self.config.colors.selected_border());
                            c.raise();
                        }
                    }
                }
                Event::X(KeyRelease(event)) => {
                    if event.detail() == alt {
                        break;
                    }
                }
                _ => {}
            }
        }
        self.focus_at(switch_idx);
    }
}
