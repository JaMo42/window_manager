use crate::{
    action::{maximize, resnap},
    appinfo::get_application_id,
    button::Button,
    color::BorderColor,
    draw::{load_app_icon, Alignment, GradientSpec, Svg},
    error::OrFatal,
    event::Signal,
    ewmh::{set_allowed_actions, set_frame_extents, WindowState, WindowType},
    extended_frame::ExtendedFrame,
    icccm::{get_wm_protocols, ClassHint},
    layout::{ClientLayout, LayoutClass},
    monitors::{monitors, Monitor},
    motif_hints::MotifHints,
    rectangle::Rectangle,
    snap::SnapState,
    split_handles::Splits,
    window_manager::{WindowKind, WindowManager},
    wm_hints::WmHints,
    x::{Display, GetProperty, Window, XcbWindow},
};
use pango::EllipsizeMode;
use parking_lot::{MappedRwLockReadGuard, RwLockReadGuard};
use std::{
    cell::{Cell, RefCell},
    mem::discriminant,
    rc::Rc,
    sync::{Arc, Weak},
};
use xcb::{
    x::{
        Atom, ClientMessageData, ClientMessageEvent, ConfigureNotifyEvent, Cursor, EventMask,
        Timestamp, CURRENT_TIME,
    },
    Xid,
};

#[derive(Debug)]
pub enum SetClientGeometry {
    /// Set the size of the frame (outer window)
    Frame(Rectangle),
    /// Set the size of the frame for snapping (applies gaps)
    Snap(Rectangle),
    /// Set the size of the client (inner window)
    Client(Rectangle),
}

#[derive(Copy, Clone, Debug)]
pub enum FrameKind {
    /// Frame with title bar and buttons
    Decorated,
    /// Frame with only a basic border (same size on all sides)
    Border,
    /// No visible frame
    None,
}

impl FrameKind {
    pub fn should_draw_decorations(&self) -> bool {
        matches!(self, Self::Decorated)
    }

    pub fn should_draw_border(&self) -> bool {
        !matches!(self, Self::None)
    }
}

fn create_frame(display: &Arc<Display>, geometry: Rectangle, cursor: Cursor) -> Window {
    let vi = display.truecolor_visual();
    Window::builder(display.clone())
        .geometry(geometry)
        .depth(vi.depth)
        .visual(vi.id)
        .attributes(|attributes| {
            attributes
                .colormap(vi.colormap)
                .background_pixel(0xFF1F1F1F) // AARRGGBB
                .border_pixel(0xFF000000)
                .override_redirect()
                .cursor(cursor)
                .event_mask(
                    EventMask::SUBSTRUCTURE_REDIRECT
                        | EventMask::ENTER_WINDOW
                        | EventMask::LEAVE_WINDOW
                        | EventMask::BUTTON_MOTION,
                );
        })
        .build()
}

fn fix_frame_position(geometry: &mut Rectangle, frame_geometry: &mut Rectangle) {
    let dx = geometry.x - frame_geometry.x;
    let dy = geometry.y - frame_geometry.y;
    frame_geometry.x = geometry.x;
    frame_geometry.y = geometry.y;
    geometry.x += dx;
    geometry.y += dy;
}

pub struct Client {
    wm: Weak<WindowManager>,
    window: Window,
    frame: Window,
    /// The current client geometry.
    geometry: Cell<Rectangle>,
    /// The frame geometry before snapping. Value is unspecified if the client
    /// is not snapped.
    prev_geometry: Cell<Rectangle>,
    frame_geometry: Cell<Rectangle>,
    frame_offset: Cell<Rectangle>,
    title: RefCell<Option<String>>,
    application_id: Option<String>,
    icon: Option<Rc<Svg>>,
    frame_kind: FrameKind,
    layout_class: Rc<RefCell<LayoutClass<ClientLayout>>>,
    window_state: Cell<WindowState>,
    window_type: Cell<WindowType>,
    snap_state: Cell<SnapState>,
    is_focused: Cell<bool>,
    is_urgent: Cell<bool>,
    border_color: RefCell<BorderColor>,
    left_buttons: RefCell<Vec<Button>>,
    right_buttons: RefCell<Vec<Button>>,
    last_click_time: Cell<Timestamp>,
    workspace: Cell<usize>,
    monitor: Cell<isize>,
    extended_frame: ExtendedFrame,
    // we consider unexpected unmapping of windows as that client being closed,
    // however whenever we reparent a client to fullscreen it we also get those
    // events.  This is flag is used to block handling of them.
    // XXX: For some reason we get two of those events when leaving fullscreen
    // mode so we store it as an integer that is decremented.
    block_unmap_deletion: Cell<u8>,
}

impl Client {
    pub fn new(wm: &Arc<WindowManager>, window: Window, window_type: WindowType) -> Arc<Self> {
        let layout_class = wm.config.client_layout();
        let layout_class_b = layout_class.borrow();
        let layout = layout_class_b.get(monitors().primary());
        let display = window.display();

        window.change_attributes(|attributes| {
            attributes.event_mask(
                EventMask::STRUCTURE_NOTIFY | EventMask::PROPERTY_CHANGE | EventMask::ENTER_WINDOW,
            );
            // TODO: do_not_propagate?
        });

        let mut geometry = Rectangle::from_parts(window.get_geometry())
            .clamp_size(layout.min_size(), monitors().primary().window_area().size());
        let frame_kind = if MotifHints::get(&window)
            .map(|motif_hints| motif_hints.has_own_decorations())
            .unwrap_or(false)
        {
            FrameKind::None
        } else if window_type.is_dialog() {
            // TODO: maybe also for other types if `transient for` is set
            FrameKind::Border
        } else {
            FrameKind::Decorated
        };
        let mut frame_size = layout.get_frame(frame_kind, &geometry);
        fix_frame_position(&mut geometry, &mut frame_size);
        let frame = create_frame(window.display(), frame_size, wm.cursors.normal);
        let (x, y) = layout.reparent_position(frame_kind);
        window.reparent(&frame, x, y);
        frame.map_subwindows();

        let title = display.window_title(&window);
        let class_hint = ClassHint::get(&window);
        let class_hint = class_hint.as_ref();

        let application_id = get_application_id(
            &[
                window
                    .get_string_property(display, display.atoms.gtk_application_id)
                    .as_deref(),
                class_hint.map(|h| h.name.as_str()),
                class_hint.map(|h| h.class.as_str()),
            ],
            class_hint.map(|h| h.name.as_str()).or(title.as_deref()),
        );

        let icon = application_id
            .as_ref()
            .and_then(|app_id| load_app_icon(app_id, &wm.config.icon_theme))
            .map(Rc::new);

        set_allowed_actions(&window, !window_type.is_dialog());
        let frame_offset = *layout.frame_offset(frame_kind);
        set_frame_extents(&window, &frame_offset);

        let extended_frame = ExtendedFrame::new(display, frame_size, layout.frame_extents());

        drop(layout_class_b);
        let window_handle = window.handle();
        let frame_handle = frame.handle();
        let this = Arc::new(Self {
            wm: Arc::downgrade(wm),
            window,
            frame,
            geometry: Cell::new(geometry),
            prev_geometry: Cell::new(geometry),
            frame_geometry: Cell::new(frame_size),
            frame_offset: Cell::new(frame_offset),
            title: RefCell::new(title),
            application_id,
            icon,
            frame_kind,
            layout_class,
            window_state: Cell::new(WindowState::Normal),
            window_type: Cell::new(window_type),
            snap_state: Cell::new(SnapState::None),
            is_focused: Cell::new(false),
            is_urgent: Cell::new(false),
            border_color: RefCell::new(wm.config.colors.normal_border()),
            left_buttons: RefCell::new(Vec::new()),
            right_buttons: RefCell::new(Vec::new()),
            last_click_time: Cell::new(0),
            workspace: Cell::new(wm.active_workspace_index()),
            monitor: Cell::new(monitors().primary().index() as isize),
            extended_frame: extended_frame.clone(),
            block_unmap_deletion: Cell::new(0),
        });
        wm.associate_client(&window_handle, &this);
        wm.associate_client(&frame_handle, &this);
        wm.set_window_kind(&window_handle, WindowKind::Client);
        wm.set_window_kind(&frame_handle, WindowKind::Frame);
        extended_frame.associate(wm, &this);
        extended_frame.restack(&this);
        if frame_kind.should_draw_decorations() {
            let layout_class = this.layout_class.borrow();
            let layout = layout_class.get(monitors().get(0));
            let button_layout = *layout.button_layout();
            let left_buttons: Vec<_> = wm
                .config
                .window
                .left_buttons
                .iter()
                .enumerate()
                .map(|(idx, name)| {
                    let mut b = Button::from_string(wm, &this, button_layout, name);
                    b.set_geometry(layout.left_button_rect(idx));
                    b
                })
                .collect();
            let width = this.frame_geometry().width;
            let right_buttons = wm
                .config
                .window
                .right_buttons
                .iter()
                .rev()
                .enumerate()
                .map(|(idx, name)| {
                    let mut b = Button::from_string(wm, &this, button_layout, name);
                    b.set_geometry(layout.right_button_rect(idx, width));
                    b
                })
                .collect();
            drop(layout_class);
            this.left_buttons.replace(left_buttons);
            this.right_buttons.replace(right_buttons);
            this.frame.map_subwindows();
        }
        wm.display.flush();
        this
    }

    pub fn get_window_manager(&self) -> Arc<WindowManager> {
        self.wm
            .upgrade()
            .expect("client not to outlive the window manager")
    }

    pub fn get_monitor(&self) -> MappedRwLockReadGuard<Monitor> {
        RwLockReadGuard::map(monitors(), |m| m.get(self.monitor.get()))
    }

    /// Get the layout for the monitor the client is currently on.
    pub fn get_layout(&self) -> ClientLayout {
        self.layout_class.borrow().get(&self.get_monitor()).clone()
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn frame(&self) -> &Window {
        &self.frame
    }

    pub fn extended_frame(&self) -> &ExtendedFrame {
        &self.extended_frame
    }

    pub fn handle(&self) -> XcbWindow {
        self.window.handle()
    }

    /// Checks if the given handle is the handle of the clients window, frame
    /// window, or extended frame window.
    pub fn has_handle(&self, handle: XcbWindow) -> bool {
        self.window.handle() == handle
            || self.frame.handle() == handle
            || self.extended_frame.handle_eq(handle)
    }

    pub fn display(&self) -> &Arc<Display> {
        self.window.display()
    }

    pub fn workspace(&self) -> usize {
        self.workspace.get()
    }

    pub fn set_workspace(&self, idx: usize) -> usize {
        self.workspace.replace(idx)
    }

    pub fn monitor(&self) -> isize {
        self.monitor.get()
    }

    pub fn set_monitor(&self, idx: isize, frame_geometry: Rectangle) {
        let before = self.monitor.replace(idx);
        if before != idx {
            let mut layout_class = self.layout_class.borrow_mut();
            if let Some(layout) = layout_class.get_if_different(monitors().get(idx)) {
                self.frame_offset.set(*layout.frame_offset(self.frame_kind));
                self.layout_children(frame_geometry, layout);
                drop(layout_class);
                self.configure();
                self.draw_border();
            }
            self.get_window_manager()
                .signal_sender
                .send(Signal::ClientMonitorChanged(self.handle(), before, idx))
                .or_fatal(self.display());
        }
    }

    pub fn title(&self) -> Option<&str> {
        let static_ref: &'static _ = unsafe { &*(&*self.title.borrow() as *const Option<String>) };
        static_ref.as_deref()
    }

    pub fn application_id(&self) -> Option<&str> {
        self.application_id.as_deref()
    }

    pub fn icon(&self) -> Option<&Rc<Svg>> {
        self.icon.as_ref()
    }

    pub fn id_info(&self) -> ClientIdInfo {
        ClientIdInfo(self)
    }

    pub fn map(&self) {
        self.frame.map();
        self.extended_frame.map(self);
    }

    pub fn unmap(&self) {
        self.frame.unmap();
        self.extended_frame.unmap(self.display());
    }

    pub fn raise(&self) {
        self.frame.raise();
        self.extended_frame.restack(self);
    }

    pub fn is_on_active_workspace(&self) -> bool {
        self.workspace.get() == self.get_window_manager().active_workspace_index()
    }

    pub fn for_each_button(&self, mut f: impl FnMut(&Button) -> bool) {
        let left = self.left_buttons.borrow();
        let right = self.right_buttons.borrow();
        for button in left.iter() {
            if f(button) {
                return;
            }
        }
        for button in right.iter() {
            if f(button) {
                return;
            }
        }
    }

    ////////////////////////////////////////////////////////////////////////////////
    // Snap state
    ////////////////////////////////////////////////////////////////////////////////

    pub fn snap_state(&self) -> SnapState {
        self.snap_state.get()
    }

    pub fn is_snapped(&self) -> bool {
        self.snap_state.get().is_snapped()
    }

    /// Sets the snap state and updates the window state.
    /// If the new state is different from the current state a
    /// `SnapStateChanged` signal is sent.
    pub fn set_snap_state(&self, new_state: SnapState) {
        if discriminant(&self.snap_state.get()) == discriminant(&new_state) {
            return;
        }
        let before = self.snap_state.replace(new_state);
        if self.is_snapped() {
            self.extended_frame.unmap(self.display());
            self.set_state(WindowState::Snapped);
        } else {
            self.extended_frame.map(self);
            self.set_state(WindowState::Normal);
        }
        self.get_window_manager()
            .signal_sender
            .send(Signal::SnapStateChanged(self.handle(), before, new_state))
            .or_fatal(self.display());
    }

    pub fn unsnap(&self) {
        if self.is_snapped() {
            self.move_and_resize(SetClientGeometry::Frame(self.prev_geometry.get()));
            self.set_snap_state(SnapState::None);
        }
    }

    ////////////////////////////////////////////////////////////////////////////////
    // Window state
    ////////////////////////////////////////////////////////////////////////////////

    /// Returns the window state with check for `WindowState::OtherWorkspace`.
    /// If you don't care about that state use the cheaper `real_state` function.
    pub fn state(&self) -> WindowState {
        if self.is_on_active_workspace() {
            WindowState::OtherWorkspace
        } else {
            self.window_state.get()
        }
    }

    /// Get the window state
    pub fn real_state(&self) -> WindowState {
        self.window_state.get()
    }

    pub fn set_state(&self, new_state: WindowState) {
        new_state.set_net_wm_state(self);
        self.window_state.set(new_state);
    }

    /// Is the client allowed to move?
    pub fn may_move(&self) -> bool {
        !self.window_state.get().is_fullscreen()
    }

    /// Is the client allowed to be resized?
    pub fn may_resize(&self) -> bool {
        !(self.window_state.get().is_fullscreen() || self.window_type.get().is_dialog())
    }

    ////////////////////////////////////////////////////////////////////////////////
    // Frame kind
    ////////////////////////////////////////////////////////////////////////////////

    /// Get the clients frame kind.
    pub fn frame_kind(&self) -> FrameKind {
        self.frame_kind
    }

    /// Returns the geometry of the frame window (outer window)
    pub fn frame_geometry(&self) -> Rectangle {
        self.frame_geometry.get()
    }

    /// Get the frame offset.
    pub fn frame_offset(&self) -> Rectangle {
        self.frame_offset.get()
    }

    ////////////////////////////////////////////////////////////////////////////////
    // Geometry
    ////////////////////////////////////////////////////////////////////////////////

    /// Returns the geometry of the client window (inner window)
    pub fn client_geometry(&self) -> Rectangle {
        self.geometry.get()
    }

    /// Returns the saved frame  geometry. This is the frame geometry the
    /// client had before snapping and will have after being un-snapped.
    pub fn saved_geometry(&self) -> Rectangle {
        self.prev_geometry.get()
    }

    /// Saves the current unsnapped geometry.
    pub fn save_geometry(&self) {
        if self.is_snapped() {
            log::error!("Client::save_geometry called while client is snapped");
            return;
        }
        self.prev_geometry.set(self.frame_geometry());
    }

    /// Modifies the saved frame geometry.
    pub fn modify_saved_geometry<F>(&self, f: F)
    where
        F: FnOnce(&mut Rectangle),
    {
        let mut geometry = self.prev_geometry.get();
        f(&mut geometry);
        self.prev_geometry.set(geometry);
    }

    /// Sets the clients geometry.
    pub fn move_and_resize(&self, set: SetClientGeometry) {
        let layout = self.get_layout();
        let frame_offset = layout.frame_offset(self.frame_kind);
        let x;
        let y;
        let width;
        let height;
        let client_width;
        let client_height;
        match set {
            SetClientGeometry::Client(g) => {
                x = g.x - frame_offset.x;
                y = g.y - frame_offset.y;
                width = g.width + frame_offset.width;
                height = g.height + frame_offset.height;
                client_width = g.width;
                client_height = g.height;
            }
            SetClientGeometry::Frame(g) => {
                (x, y, width, height) = g.into_parts();
                client_width = width - frame_offset.width;
                client_height = height - frame_offset.height;
            }
            SetClientGeometry::Snap(mut g) => {
                (x, y, width, height) = g.resize(-layout.gap()).into_parts();
                client_width = width - frame_offset.width;
                client_height = height - frame_offset.height;
            }
        }
        let frame_rect = Rectangle::new(x, y, width, height);
        self.geometry
            .set(layout.get_client(self.frame_kind, &frame_rect));
        self.frame_geometry.set(frame_rect);
        self.frame.move_and_resize((x, y, width, height));
        self.extended_frame
            .resize(self.display(), (x, y, width, height));
        let mon_idx = monitors().at(frame_rect.center()).index() as isize;
        if mon_idx != self.monitor.get() {
            drop(layout);
            self.set_monitor(mon_idx, frame_rect);
        } else {
            for (idx, button) in self.left_buttons.borrow_mut().iter_mut().enumerate() {
                button.set_geometry(layout.left_button_rect(idx));
            }
            for (idx, button) in self.right_buttons.borrow_mut().iter_mut().enumerate() {
                button.set_geometry(layout.right_button_rect(idx, width));
            }
            self.window.resize(client_width, client_height);
            drop(layout);
            self.configure();
            self.draw_border();
        }
        self.get_window_manager()
            .signal_sender
            .send(Signal::ClientGeometry(self.handle(), frame_rect))
            .or_fatal(self.display());
    }

    /// Tell the client its geometry.
    pub fn configure(&self) {
        let geometry = self.client_geometry();
        self.window.send_event(
            EventMask::STRUCTURE_NOTIFY,
            &ConfigureNotifyEvent::new(
                self.window().handle(),
                self.window().handle(),
                XcbWindow::none(),
                geometry.x,
                geometry.y,
                geometry.width,
                geometry.height,
                0,
                false,
            ),
        );
    }

    ////////////////////////////////////////////////////////////////////////////////
    // Focus
    ////////////////////////////////////////////////////////////////////////////////

    pub fn is_focused(&self) -> bool {
        self.is_focused.get()
    }

    pub fn is_minimized(&self) -> bool {
        self.window_state.get().is_minimized()
    }

    /// Focus the client.
    /// Note: This only updates this client but has no effect on the the actual
    /// focus order, use `Workspace::focus` for that.
    /// Emits a `ClientMinimized` if the client is in fullscreen mode.
    pub fn focus(&self) {
        self.is_focused.set(true);
        self.display().set_input_focus(self.handle());
        if self.is_urgent.get() {
            self.set_urgency(false);
        }
        let state = self.real_state();
        if state.is_minimized() {
            self.unminimize();
        }
        if state.is_fullscreen() {
            self.window.raise();
            self.get_window_manager()
                .signal_sender
                .send(Signal::ClientMinimized(self.handle(), false))
                .or_fatal(self.display())
        } else {
            self.set_border(self.get_window_manager().config.colors.focused_border());
            self.frame.raise();
        }
    }

    /// Unfocus the client.
    /// Emits a `ClientMinimized` if the client is in fullscreen mode.
    pub fn unfocus(&self) {
        self.is_focused.set(false);
        self.set_border(self.get_window_manager().config.colors.normal_border());
        if self.is_fullscreen() {
            self.get_window_manager()
                .signal_sender
                .send(Signal::ClientMinimized(self.handle(), true))
                .or_fatal(self.display())
        }
    }

    /// Is this the globally focused client?
    pub fn is_focused_client(&self) -> bool {
        // TODO: this is called by property notify when the _NET_WM_USER_TIME
        // property changes which happend a lot, can this be more efficient?
        self.is_focused()
            && self.get_window_manager().active_workspace_index() == self.workspace.get()
    }

    pub fn unminimize(&self) {
        self.map();
        self.set_state(if self.is_snapped() {
            WindowState::Snapped
        } else {
            WindowState::Normal
        });
        self.draw_border();
        self.get_window_manager()
            .signal_sender
            .send(Signal::ClientMinimized(self.handle(), false))
            .or_fatal(self.display());
    }

    pub fn is_urgent(&self) -> bool {
        self.is_urgent.get()
    }

    /// Process a change in urgency.
    /// Emits a `UrgencyChanged` signal.
    fn update_urgency(&self) {
        let wm = self.get_window_manager();
        // TODO: old version only set urgent border if `is_urgent`, was that enough?
        self.set_border(if self.is_urgent.get() {
            wm.config.colors.urgent_border()
        } else if self.is_focused.get() {
            wm.config.colors.focused_border()
        } else {
            wm.config.colors.normal_border()
        });
        wm.signal_sender
            .send(Signal::UrgencyChanged(self.handle()))
            .or_fatal(self.display())
    }

    /// Change the urgency of the client.
    pub fn set_urgency(&self, urgency: bool) {
        if urgency == self.is_urgent.get() {
            return;
        }
        self.is_urgent.set(urgency);
        self.update_urgency();
        if let Some(mut wm_hints) = WmHints::get(&self.window) {
            wm_hints.set_urgent(urgency);
            wm_hints.set(&self.window);
        }
    }

    ////////////////////////////////////////////////////////////////////////////////
    // Visual
    ////////////////////////////////////////////////////////////////////////////////

    pub fn set_border(&self, color: BorderColor) {
        if self.frame_kind.should_draw_border() && *self.border_color.borrow() != color {
            *self.border_color.borrow_mut() = color;
            self.draw_border();
        }
    }

    pub fn draw_border(&self) {
        if self.frame_kind.should_draw_decorations() {
            let wm = self.get_window_manager();
            let dc = wm.drawing_context.lock();
            let layout_class = self.layout_class.borrow();
            let layout = layout_class.get(monitors().get(self.monitor.get()));
            let title_height = layout.title_bar_height();
            let full_rect = self.frame_geometry().at(0, 0);
            let mut rect = full_rect;
            let mut title_rect = full_rect;
            title_rect.height = title_height;
            rect.height -= title_height;
            rect.y += title_height as i16;
            let border_color = self.border_color.borrow();
            let color = border_color.border();
            dc.fill_rect(rect, color);
            dc.rect(title_rect)
                .gradient(GradientSpec::new_vertical(border_color.top(), color))
                .draw();
            if let Some(icon) = &self.icon {
                dc.draw_svg(icon, *layout.icon_rect());
            }
            if let Some(title) = &*self.title.borrow() {
                let title_rect = layout.title_rect(full_rect.width, self.icon.is_some());
                dc.set_font(layout.title_font());
                // FIXME: if alignment is centered it should be centered on the
                // window border, not the free space.
                dc.text(title, title_rect)
                    .color(border_color.text())
                    .vertical_alignment(Alignment::CENTER)
                    .horizontal_alignment(wm.config.window.title_alignment)
                    .ellipsize(EllipsizeMode::Middle)
                    .draw();
            }
            dc.render(&self.frame, full_rect);
            self.for_each_button(|button| {
                button.draw(&dc, &border_color, false);
                false
            });
        } else if self.frame_kind.should_draw_border() {
            let wm = self.get_window_manager();
            let dc = wm.drawing_context.lock();
            let rect = self.frame_geometry().at(0, 0);
            dc.fill_rect(rect, self.border_color.borrow().border());
            dc.render(&self.frame, rect);
        }
    }

    ////////////////////////////////////////////////////////////////////////////////
    // Mouse interaction
    ////////////////////////////////////////////////////////////////////////////////

    /// Process a click on one of the buttons of this window.
    pub fn click_button(&self, window: XcbWindow) {
        // The buttons vector cannot be borrowed since the click may trigger
        // a call to `draw_border` so we need to go through a pointer like this.
        let mut clicked = std::ptr::null();
        self.for_each_button(|button| {
            if button == window {
                clicked = button;
                true
            } else {
                false
            }
        });
        if !clicked.is_null() {
            unsafe { &*clicked }.click(self);
        }
    }

    /// Process a crossing event for one of the buttons of this window.
    pub fn cross_button(&self, window: XcbWindow, is_hovered: bool) {
        self.for_each_button(|button| {
            if button == window {
                let wm = self.get_window_manager();
                button.draw(
                    &wm.drawing_context.lock(),
                    &self.border_color.borrow(),
                    is_hovered,
                );
                true
            } else {
                false
            }
        });
    }

    /// Process a click on the frame.
    pub fn click_frame(&self, time: Timestamp) -> bool {
        let d = time - self.last_click_time.get();
        self.last_click_time.set(time);
        if d < self.get_window_manager().config.general.double_click_time && self.may_resize() {
            if self.is_snapped() {
                self.unsnap();
            } else {
                maximize(self);
            }
            true
        } else {
            false
        }
    }

    ////////////////////////////////////////////////////////////////////////////////
    // Client communication
    ////////////////////////////////////////////////////////////////////////////////

    /// Sends a client message to the client window.
    /// Messages are only sent if the client indicates that it supports them,
    /// if a message is sent `true` is returned.
    pub fn send_message(&self, protocol: Atom) -> bool {
        let mut is_supported = false;
        let display = self.window.display();
        if let Ok(reply) = get_wm_protocols(&self.window) {
            let protocol_id = protocol.resource_id();
            for supported in reply.value::<Atom>().iter() {
                if supported.resource_id() == protocol_id {
                    is_supported = true;
                    break;
                }
            }
        }
        if is_supported {
            self.window.send_event(
                EventMask::NO_EVENT,
                &ClientMessageEvent::new(
                    self.window.handle(),
                    display.atoms.wm_protocols,
                    ClientMessageData::Data32([protocol.resource_id(), CURRENT_TIME, 0, 0, 0]),
                ),
            );
        }
        is_supported
    }

    /// Fetch the window title.
    /// If the title cannot be fetches the old value is kept.
    pub fn update_title(&self) {
        if let Some(new_title) = self.display().window_title(&self.window()) {
            *self.title.borrow_mut() = Some(new_title);
            self.draw_border();
        }
    }

    /// Fetch the `WM_HINTS` property and update attributes according to it.
    pub fn update_wm_hints(&self) {
        if let Some(mut wm_hints) = WmHints::get(&self.window) {
            if self.is_focused_client() {
                wm_hints.set_urgent(false);
                wm_hints.set(&self.window);
            } else {
                self.is_urgent.set(wm_hints.is_urgent());
                self.update_urgency();
            }
        } else {
            log::error!("Failed to get WM_HINTS for {}", self);
        }
    }

    ////////////////////////////////////////////////////////////////////////////////
    // Misc.
    ////////////////////////////////////////////////////////////////////////////////

    pub fn get_acting_splits(&self) -> Splits {
        let workspace = self.workspace.get();
        let monitor_idx = self.monitor.get();
        let wm = self.get_window_manager();
        let split_manager = wm.split_manager();
        split_manager
            .get_handles(workspace, monitor_idx)
            .as_splits()
    }

    pub fn is_fullscreen(&self) -> bool {
        self.real_state().is_fullscreen()
    }

    /// Sets the fullscreen state.
    /// Emits a `ClientGeometry` signal.
    pub fn set_fullscreen(&self, state: bool) {
        if state == self.window_state.get().is_fullscreen() {
            return;
        }
        if state {
            let monitor_rect = *self.get_monitor().geometry();
            self.window
                .reparent(&self.display().root(), monitor_rect.x, monitor_rect.y);
            self.frame.unmap();
            self.window.resize(monitor_rect.width, monitor_rect.height);
            self.window.raise();
            self.display().set_input_focus(self.window.handle());
            if self.is_focused() {
                self.get_window_manager()
                    .signal_sender
                    .send(Signal::ClientGeometry(self.handle(), monitor_rect))
                    .or_fatal(self.display())
            }
            self.block_unmap_deletion.set(1);
        } else {
            let layout_class = self.layout_class.borrow();
            let (reparent_x, reparent_y) = layout_class
                .get(monitors().get(self.monitor.get()))
                .reparent_position(self.frame_kind);
            drop(layout_class);
            self.frame.map();
            self.window.reparent(&self.frame, reparent_x, reparent_y);
            if self.is_snapped() {
                resnap(self);
            } else {
                self.move_and_resize(SetClientGeometry::Frame(self.prev_geometry.get()));
            }
            self.focus();
            self.block_unmap_deletion.set(2);
        }
        self.set_state(if state {
            WindowState::Fullscreen
        } else if self.is_snapped() {
            WindowState::Snapped
        } else {
            WindowState::Normal
        });
    }

    /// Returns `true` if the client has indicated that it expects an unmap
    /// event for the client handle.  The flag is cleared afterwards.
    pub fn block_unmap_deletion(&self) -> bool {
        let value = self.block_unmap_deletion.get();
        self.block_unmap_deletion.replace(value.saturating_sub(1)) != 0
    }

    /// Positions and resizes all child windows according to the given layout,
    /// keeping the frame geometry. Does not reconfigure the client or redraw
    /// the border.
    fn layout_children(&self, frame_geometry: Rectangle, layout: &ClientLayout) {
        for (i, button) in self.left_buttons.borrow_mut().iter_mut().enumerate() {
            button.set_geometry(layout.left_button_rect(i));
            button.set_layout(*layout.button_layout());
        }
        for (i, button) in self.right_buttons.borrow_mut().iter_mut().enumerate() {
            button.set_geometry(layout.right_button_rect(i, frame_geometry.width));
            button.set_layout(*layout.button_layout());
        }
        let frame_offset = layout.frame_offset(self.frame_kind);
        self.window.move_and_resize((
            frame_offset.x,
            frame_offset.y,
            frame_geometry.width - frame_offset.width,
            frame_geometry.height - frame_offset.height,
        ));
        self.geometry
            .set(layout.get_client(self.frame_kind, &frame_geometry));
    }

    /// Handles a change in the clients layout.
    pub fn layout_changed(&self) {
        let mut layout_class = self.layout_class.borrow_mut();
        let monitor = self.get_monitor();
        if let Some(layout) = layout_class.get_if_different(&monitor) {
            drop(monitor);
            self.layout_children(
                layout.get_frame(self.frame_kind, &self.geometry.get()),
                layout,
            );
            drop(layout_class);
            self.configure();
            self.draw_border();
        }
    }

    /// Destroys all windows owned by the client and removes all its associated
    /// context from the context map.
    pub fn destroy(&self) {
        let wm = self.get_window_manager();
        wm.remove_all_contexts(&self.window);
        wm.remove_all_contexts(&self.frame);
        wm.remove_all_contexts(&self.extended_frame);
        self.for_each_button(|button| {
            wm.remove_all_contexts(button.window());
            button.window().destroy();
            false
        });
        self.frame.destroy();
        self.extended_frame.destroy(self.display());
    }
}

impl PartialEq for Client {
    fn eq(&self, other: &Self) -> bool {
        self.window == other.window
    }
}

impl std::fmt::Display for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(title) = self.title() {
            write!(f, "Client(\"{}\"; {})", title, self.window.resource_id())
        } else {
            write!(f, "Client({})", self.window.resource_id())
        }
    }
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Client({})", self.window.resource_id())
    }
}

pub struct ClientIdInfo<'a>(&'a Client);

impl<'a> std::fmt::Display for ClientIdInfo<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let client = self.0;
        writeln!(f, "Client(")?;
        #[rustfmt::skip]
        writeln!(f, "     |          window id: {}", client.window.resource_id())?;
        if let Some(title) = client.title() {
            writeln!(f, "     |              title: {}", title)?;
        }
        if let Some(class_hint) = ClassHint::get(client.window()) {
            writeln!(f, "     |               name: {}", class_hint.name)?;
            writeln!(f, "     |              class: {}", class_hint.class)?;
        }
        if let Some(application_id) = client.application_id() {
            writeln!(f, "     |     application_id: {}", application_id)?;
        }
        write!(f, "     | )")
    }
}
