use crate::{
    action,
    client::{Client, SetClientGeometry},
    mouse::{mouse_move, mouse_resize, MouseResizeOptions, BUTTON_1},
    rectangle::Rectangle,
    snap::SnapState,
    window_manager::{WindowManager, NAME},
    x::{Display, GetProperty, PropertyValue, SetProperty, Window},
};
use xcb::{
    x::{Atom, ClientMessageData, ClientMessageEvent, ATOM_ATOM, ATOM_WINDOW},
    Xid, XidNew,
};

// https://specifications.freedesktop.org/wm-spec/wm-spec-1.3.html#idm45582155069600
const _NET_WM_MOVERESIZE_SIZE_TOPLEFT: u32 = 0;
const _NET_WM_MOVERESIZE_SIZE_TOP: u32 = 1;
const _NET_WM_MOVERESIZE_SIZE_TOPRIGHT: u32 = 2;
const _NET_WM_MOVERESIZE_SIZE_RIGHT: u32 = 3;
const _NET_WM_MOVERESIZE_SIZE_BOTTOMRIGHT: u32 = 4;
const _NET_WM_MOVERESIZE_SIZE_BOTTOM: u32 = 5;
const _NET_WM_MOVERESIZE_SIZE_BOTTOMLEFT: u32 = 6;
const _NET_WM_MOVERESIZE_SIZE_LEFT: u32 = 7;
const _NET_WM_MOVERESIZE_MOVE: u32 = 8;
const _NET_WM_MOVERESIZE_SIZE_KEYBOARD: u32 = 9;
const _NET_WM_MOVERESIZE_MOVE_KEYBOARD: u32 = 10;

// https://x.org/releases/X11R7.6/doc/xorg-docs/specs/ICCCM/icccm.html#wm_state_property
const WM_STATE_NORMAL: u32 = 1;

#[derive(Debug, Clone)]
pub struct Root(pub Window);

impl std::ops::Deref for Root {
    type Target = Window;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Root {
    /// Sets up EWMH properties on the root window.
    pub fn setup(&self) {
        let display = self.display();

        let wm_check_window = Window::builder(display.clone()).build();
        wm_check_window.set_property(
            display,
            display.atoms.net_supporting_wm_check,
            PropertyValue::Window(wm_check_window.handle()),
        );
        wm_check_window.set_property(
            display,
            display.atoms.net_wm_name,
            PropertyValue::String(NAME.to_owned()),
        );

        self.set_property(
            display,
            display.atoms.net_supporting_wm_check,
            PropertyValue::Window(wm_check_window.handle()),
        );
        self.set_property(
            display,
            display.atoms.net_supported,
            PropertyValue::AtomList(display.atoms.as_slice().to_vec()),
        );
        self.delete_property(display, display.atoms.net_active_window);
        self.delete_property(display, display.atoms.net_client_list);
        self.set_property(
            display,
            display.atoms.net_number_of_desktops,
            PropertyValue::Cardinal(4),
        );
        self.set_property(
            display,
            display.atoms.net_current_desktop,
            PropertyValue::Cardinal(0),
        );
    }

    /// Removes all our properties from the root window and destroys the
    /// supporing WM check window.
    pub fn clean(&self) {
        let display = self.display();
        if let Ok(prop) =
            self.get_property(display, display.atoms.net_supporting_wm_check, ATOM_WINDOW)
        {
            if prop.r#type() == ATOM_WINDOW {
                let win = Window::from_handle(display.clone(), prop.value()[0]);
                win.destroy();
            } else {
                log::warn!("Value of _NET_SUPPORTING_WM_CHECK is not a window?");
            }
        } else {
            log::warn!("Root window has no _NET_SUPPORTING_WM_CHECK?");
        }
        self.delete_property(display, display.atoms.net_supporting_wm_check);
        self.delete_property(display, display.atoms.net_active_window);
        self.delete_property(display, display.atoms.net_client_list);
        self.delete_property(display, display.atoms.net_number_of_desktops);
        self.delete_property(display, display.atoms.net_current_desktop);
    }

    /// Set `_NET_CURRENT_DESKTOP`.
    pub fn set_active_workspace(&self, workspace_index: usize) {
        let display = self.display();
        self.set_property(
            display,
            display.atoms.net_current_desktop,
            PropertyValue::Cardinal(workspace_index as u32),
        )
    }

    /// Set or delete `_NET_ACTIVE_WINDOW`. If the given value is `None` the
    /// input focus is also set to the root window.
    pub fn set_focused_client(&self, maybe_client: Option<crate::x::XcbWindow>) {
        let display = self.display();
        if let Some(client) = maybe_client {
            self.set_property(
                display,
                display.atoms.net_active_window,
                PropertyValue::Window(client),
            );
        } else {
            self.delete_property(display, display.atoms.net_active_window);
            display.set_input_focus(display.root());
        }
    }
}

#[derive(Copy, Clone)]
pub enum WindowType {
    Desktop,
    Dock,
    Toolbar,
    Menu,
    Utility,
    Splash,
    Dialog,
    DropdownMenu,
    PopupMenu,
    Tooltip,
    Notification,
    Combo,
    Dnd,
    Normal,
}

impl WindowType {
    pub fn is_dialog(&self) -> bool {
        matches!(self, Self::Dialog)
    }

    /// Tries to get the window type. Returns `None` if the property is not set
    /// for the given window or has an invalid value.
    pub fn try_get(display: &Display, window: impl GetProperty) -> Option<Self> {
        if let Ok(reply) = window.get_property_full(
            display,
            false,
            display.atoms.net_wm_window_type,
            ATOM_ATOM,
            0,
            1,
        ) {
            if reply.length() == 0 {
                return None;
            }
            use WindowType::*;
            match reply.value::<Atom>()[0] {
                x if x == display.atoms.net_wm_window_type_desktop => Some(Desktop),
                x if x == display.atoms.net_wm_window_type_dock => Some(Dock),
                x if x == display.atoms.net_wm_window_type_toolbar => Some(Toolbar),
                x if x == display.atoms.net_wm_window_type_menu => Some(Menu),
                x if x == display.atoms.net_wm_window_type_utility => Some(Utility),
                x if x == display.atoms.net_wm_window_type_splash => Some(Splash),
                x if x == display.atoms.net_wm_window_type_dialog => Some(Dialog),
                x if x == display.atoms.net_wm_window_type_dropdown_menu => Some(DropdownMenu),
                x if x == display.atoms.net_wm_window_type_popup_menu => Some(PopupMenu),
                x if x == display.atoms.net_wm_window_type_tooltip => Some(Tooltip),
                x if x == display.atoms.net_wm_window_type_notification => Some(Notification),
                x if x == display.atoms.net_wm_window_type_combo => Some(Combo),
                x if x == display.atoms.net_wm_window_type_dnd => Some(Dnd),
                x if x == display.atoms.net_wm_window_type_normal => Some(Normal),
                _ => {
                    log::error!(
                        "Invalid _NET_WM_WINDOW_TYPE value on {}",
                        window.window().resource_id()
                    );
                    None
                }
            }
        } else {
            None
        }
    }

    /// Returns the type of the given window. If getting the type fails
    /// `WindowType::Normal` is returned.
    pub fn get(display: &Display, window: impl GetProperty) -> Self {
        Self::try_get(display, window).unwrap_or(Self::Normal)
    }

    fn into_atom(self, display: &Display) -> Atom {
        use WindowType::*;
        match self {
            Desktop => display.atoms.net_wm_window_type_desktop,
            Dock => display.atoms.net_wm_window_type_dock,
            Toolbar => display.atoms.net_wm_window_type_toolbar,
            Menu => display.atoms.net_wm_window_type_menu,
            Utility => display.atoms.net_wm_window_type_utility,
            Splash => display.atoms.net_wm_window_type_splash,
            Dialog => display.atoms.net_wm_window_type_dialog,
            DropdownMenu => display.atoms.net_wm_window_type_dropdown_menu,
            PopupMenu => display.atoms.net_wm_window_type_popup_menu,
            Tooltip => display.atoms.net_wm_window_type_tooltip,
            Notification => display.atoms.net_wm_window_type_notification,
            Combo => display.atoms.net_wm_window_type_combo,
            Dnd => display.atoms.net_wm_window_type_dnd,
            Normal => display.atoms.net_wm_window_type_normal,
        }
    }
}

pub fn set_window_type(window: &Window, window_type: WindowType) {
    let display = window.display();
    window.set_property(
        display,
        display.atoms.net_wm_window_type,
        PropertyValue::Atom(window_type.into_atom(display)),
    );
}

#[derive(Copy, Clone)]
pub enum WindowState {
    /// Not any of the other states.
    Normal,
    /// The window is snapped, note that the client always sets this when
    /// `set_snap_state` is called, even when the state is `SnapState::None`.
    Snapped,
    /// The window is in fullscreen mode
    Fullscreen,
    /// The window is minimized
    Minimized,
    /// The window is on a different workspace.
    /// The client never actually has this state but it is returned by the
    /// `state` function of clients.
    OtherWorkspace,
}

impl WindowState {
    pub fn is_minimized(&self) -> bool {
        matches!(self, Self::Minimized)
    }

    pub fn is_fullscreen(&self) -> bool {
        matches!(self, Self::Fullscreen)
    }

    /// Sets the correct _NET_WM_STATE property on the client window.
    pub fn set_net_wm_state(&self, client: &Client) {
        let window = client.window();
        let display = window.display();
        let state = match self {
            Self::Normal => vec![unsafe { Atom::new(WM_STATE_NORMAL) }],
            Self::Snapped => match client.snap_state() {
                SnapState::Left | SnapState::Right => {
                    vec![display.atoms.net_wm_action_maximize_vert]
                }
                SnapState::Maximized => vec![
                    display.atoms.net_wm_action_maximize_horz,
                    display.atoms.net_wm_action_maximize_vert,
                ],
                _ => Vec::new(),
            },
            Self::Fullscreen => vec![display.atoms.net_wm_state_fullscreen],
            Self::Minimized | Self::OtherWorkspace => vec![display.atoms.net_wm_state_hidden],
        };
        window.set_property(
            display,
            display.atoms.net_wm_state,
            PropertyValue::AtomList(state),
        );
    }
}

pub fn set_allowed_actions(window: &Window, may_resize: bool) {
    let display = window.display();
    let mut actions = vec![
        display.atoms.net_wm_action_move,
        display.atoms.net_wm_action_close,
        display.atoms.net_wm_action_change_desktop,
    ];
    if may_resize {
        actions.push(display.atoms.net_wm_action_resize);
        actions.push(display.atoms.net_wm_action_maximize_horz);
        actions.push(display.atoms.net_wm_action_maximize_vert);
        actions.push(display.atoms.net_wm_action_fullscreen);
    }
    window.set_property(
        display,
        display.atoms.net_wm_allowed_actions,
        PropertyValue::AtomList(actions),
    );
}

pub fn set_frame_extents(window: &Window, offset: &Rectangle) {
    let top = offset.y as u32;
    let left = offset.x as u32;
    let extents = vec![
        left,
        offset.width as u32 - left,
        top,
        offset.height as u32 - top,
    ];
    let display = window.display();
    window.set_property(
        display,
        display.atoms.net_frame_extents,
        PropertyValue::CardinalList(extents),
    );
}

fn focus_client_on_its_workspace(client: &Client) {
    let wm = client.get_window_manager();
    let mut workspace = wm.workspace(client.workspace());
    workspace.focus(client.window().handle());
}

fn net_wm_state(client: &Client, event: &ClientMessageEvent) {
    let display = client.display().clone();
    let data = match event.data() {
        ClientMessageData::Data32(data) => data,
        _ => return,
    };
    let first = unsafe { Atom::new(data[1]) };
    let second = unsafe { Atom::new(data[2]) };
    macro_rules! is {
        ($check:expr) => {{
            let check = $check;
            first == check || second == check
        }};
    }
    macro_rules! new_state {
        ($current:expr) => {
            data[0] == 1 || (data[0] == 2 && !$current)
        };
    }
    if is!(display.atoms.net_wm_state_fullscreen) {
        client.set_fullscreen(new_state!(client.state().is_fullscreen()));
    } else if is!(display.atoms.net_wm_state_demands_attention) {
        let new_state = new_state!(client.is_urgent());
        if new_state && client.is_focused_client() {
            return;
        }
        client.set_urgency(new_state);
    } else if is!(display.atoms.net_wm_state_maximized_horz)
        || is!(display.atoms.net_wm_state_maximized_vert)
    {
        // Maximizing on either axis is treated as a full maximize.
        if !client.snap_state().is_maximized() {
            action::snap(client, |state| *state = SnapState::Maximized);
        } else {
            client.unsnap();
            WindowState::Normal.set_net_wm_state(client);
        }
        focus_client_on_its_workspace(client);
    }
}

fn wm_change_state(client: &Client, state: u32) {
    const NORMAL_STATE: u32 = 1;
    const ICONIC_STATE: u32 = 3;
    if state == NORMAL_STATE {
        if client.is_on_active_workspace() {
            focus_client_on_its_workspace(client);
        }
        client.set_state(WindowState::Normal);
    } else if state == ICONIC_STATE {
        action::minimize(client);
    }
}

fn net_wm_moveresize(client: &Client, event: &ClientMessageEvent, grid_resize: bool) {
    let direction = match event.data() {
        ClientMessageData::Data32(data) => data[2],
        _ => return,
    };
    if client.is_on_active_workspace() {
        focus_client_on_its_workspace(client);
    }
    if direction == _NET_WM_MOVERESIZE_MOVE && client.may_move() {
        mouse_move(client, BUTTON_1, grid_resize);
    } else if !(direction == _NET_WM_MOVERESIZE_MOVE_KEYBOARD
        || direction == _NET_WM_MOVERESIZE_SIZE_KEYBOARD)
        && client.may_resize()
    {
        // assignment because rustfmt::skip doesn't work on expressions
        #[allow(clippy::let_unit_value)]
        #[rustfmt::skip]
        let _ = {
        let lock_width = [
            _NET_WM_MOVERESIZE_SIZE_TOP,
            _NET_WM_MOVERESIZE_SIZE_BOTTOM,
        ].contains(&direction);
        let lock_height = [
            _NET_WM_MOVERESIZE_SIZE_LEFT,
            _NET_WM_MOVERESIZE_SIZE_RIGHT,
        ].contains(&direction);
        let left = [
            _NET_WM_MOVERESIZE_SIZE_LEFT,
            _NET_WM_MOVERESIZE_SIZE_TOPLEFT,
            _NET_WM_MOVERESIZE_SIZE_BOTTOMLEFT,
        ].contains(&direction);
        let up = [
            _NET_WM_MOVERESIZE_SIZE_TOP,
            _NET_WM_MOVERESIZE_SIZE_TOPLEFT,
            _NET_WM_MOVERESIZE_SIZE_TOPRIGHT,
        ].contains(&direction);
        mouse_resize(client, MouseResizeOptions::new(lock_width, lock_height, up, left));
        };
    }
    // _NET_WM_MOVERESIZE_SIZE_KEYBOARD and _NET_WM_MOVERESIZE_MOVE_KEYBOARD are
    // not implemented.
}

fn net_moveresize_window(client: &Client, event: &ClientMessageEvent) {
    log::trace!("net_moveresize_window: {event:#?}");
    let data = match event.data() {
        ClientMessageData::Data32(data) => data,
        _ => return,
    };
    // TODO: unsnap?
    client.move_and_resize(SetClientGeometry::Frame(Rectangle::new(
        data[1] as i16,
        data[2] as i16,
        data[3] as u16,
        data[4] as u16,
    )));
    if !client.is_snapped() {
        client.save_geometry();
    }
}

/// Handles a client message event to a client.
pub fn client_message(client: &Client, event: &ClientMessageEvent, grid_resize: bool) {
    let display = client.display().clone();
    let message_type = event.r#type();
    if message_type == display.atoms.net_wm_state {
        net_wm_state(client, event);
    } else if message_type == display.atoms.net_active_window {
        let is_on_active_workspace = client.is_on_active_workspace();
        if client.is_focused() && is_on_active_workspace {
            return;
        }
        if is_on_active_workspace {
            focus_client_on_its_workspace(client);
        } else {
            client.set_urgency(true);
        }
    } else if message_type == display.atoms.wm_change_state {
        if let ClientMessageData::Data32(data) = event.data() {
            wm_change_state(client, data[0]);
        }
    } else if message_type == display.atoms.net_wm_moveresize {
        net_wm_moveresize(client, event, grid_resize);
    } else if message_type == display.atoms.net_moveresize_window {
        net_moveresize_window(client, event);
    } else {
        log::trace!(
            "Unhandeled client message: {:?} to {}",
            event.r#type(),
            event.window().resource_id()
        );
    }
}

/// Handles a client message event to the root window.
pub fn root_message(wm: &WindowManager, event: &ClientMessageEvent) {
    if event.r#type() == wm.display.atoms.net_current_desktop {
        if let ClientMessageData::Data32(data) = event.data() {
            wm.set_workspace(data[0] as usize);
        }
    }
}
