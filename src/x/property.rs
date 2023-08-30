use super::{Display, XcbWindow};
use crate::error::fatal_error;
use std::mem::size_of;
use xcb::{
    x::{
        Atom, ChangeProperty, DeleteProperty, GetPropertyReply, PropMode, ATOM_ANY, ATOM_ATOM,
        ATOM_CARDINAL, ATOM_NONE, ATOM_STRING, ATOM_WINDOW,
    },
    Xid, XidNew,
};

xcb::atoms_struct! {
    #[derive(Clone, Debug)]
    pub struct Atoms {
        pub net_active_window => b"_NET_ACTIVE_WINDOW" only_if_exists = false,
        pub net_client_list => b"_NET_CLIENT_LIST" only_if_exists = false,
        pub net_current_desktop => b"_NET_CURRENT_DESKTOP" only_if_exists = false,
        pub net_frame_extents => b"_NET_FRAME_EXTENTS" only_if_exists = false,
        pub net_moveresize_window => b"_NET_MOVERESIZE_WINDOW" only_if_exists = false,
        pub net_number_of_desktops => b"_NET_NUMBER_OF_DESKTOPS" only_if_exists = false,
        pub net_supported => b"_NET_SUPPORTED" only_if_exists = false,
        pub net_supporting_wm_check => b"_NET_SUPPORTING_WM_CHECK" only_if_exists = false,
        pub net_system_tray_opcode => b"_NET_SYSTEM_TRAY_OPCODE" only_if_exists = false,
        pub net_system_tray_orientation => b"_NET_SYSTEM_TRAY_ORIENTATION" only_if_exists = false,
        pub net_system_tray_s0 => b"_NET_SYSTEM_TRAY_S0" only_if_exists = false,
        pub net_wm_action_change_desktop => b"_NET_WM_ACTION_CHANGE_DESKTOP" only_if_exists = false,
        pub net_wm_action_close => b"_NET_WM_ACTION_CLOSE" only_if_exists = false,
        pub net_wm_action_fullscreen => b"_NET_WM_ACTION_FULLSCREEN" only_if_exists = false,
        pub net_wm_action_maximize_horz => b"_NET_WM_ACTION_MAXIMIZE_HORZ" only_if_exists = false,
        pub net_wm_action_maximize_vert => b"_NET_WM_ACTION_MAXIMIZE_VERT" only_if_exists = false,
        pub net_wm_action_move => b"_NET_WM_ACTION_MOVE" only_if_exists = false,
        pub net_wm_action_resize => b"_NET_WM_ACTION_RESIZE" only_if_exists = false,
        pub net_wm_allowed_actions => b"_NET_WM_ALLOWED_ACTIONS" only_if_exists = false,
        pub net_wm_moveresize => b"_NET_WM_MOVERESIZE" only_if_exists = false,
        pub net_wm_name => b"_NET_WM_NAME" only_if_exists = false,
        pub net_wm_state => b"_NET_WM_STATE" only_if_exists = false,
        pub net_wm_state_demands_attention => b"_NET_WM_STATE_DEMANDS_ATTENTION" only_if_exists = false,
        pub net_wm_state_fullscreen => b"_NET_WM_STATE_FULLSCREEN" only_if_exists = false,
        pub net_wm_state_hidden => b"_NET_WM_STATE_HIDDEN" only_if_exists = false,
        pub net_wm_state_maximized_horz => b"_NET_WM_STATE_MAXIMIZED_HORZ" only_if_exists = false,
        pub net_wm_state_maximized_vert => b"_NET_WM_STATE_MAXIMIZED_VERT" only_if_exists = false,
        pub net_wm_user_time => b"_NET_WM_USER_TIME" only_if_exists = false,
        pub net_wm_window_opacity => b"_NET_WM_WINDOW_OPACITY" only_if_exists = false,
        pub net_wm_window_type => b"_NET_WM_WINDOW_TYPE" only_if_exists = false,
        pub net_wm_window_type_combo => b"_NET_WM_WINDOW_TYPE_COMBO" only_if_exists = false,
        pub net_wm_window_type_desktop => b"_NET_WM_WINDOW_TYPE_DESKTOP" only_if_exists = false,
        pub net_wm_window_type_dialog => b"_NET_WM_WINDOW_TYPE_DIALOG" only_if_exists = false,
        pub net_wm_window_type_dnd => b"_NET_WM_WINDOW_TYPE_DND" only_if_exists = false,
        pub net_wm_window_type_dock => b"_NET_WM_WINDOW_TYPE_DOCK" only_if_exists = false,
        pub net_wm_window_type_dropdown_menu => b"_NET_WM_WINDOW_TYPE_DROPDOWN_MENU" only_if_exists = false,
        pub net_wm_window_type_menu => b"_NET_WM_WINDOW_TYPE_MENU" only_if_exists = false,
        pub net_wm_window_type_normal => b"_NET_WM_WINDOW_TYPE_NORMAL" only_if_exists = false,
        pub net_wm_window_type_notification => b"_NET_WM_WINDOW_TYPE_NOTIFICATION" only_if_exists = false,
        pub net_wm_window_type_popup_menu => b"_NET_WM_WINDOW_TYPE_POPUP_MENU" only_if_exists = false,
        pub net_wm_window_type_splash => b"_NET_WM_WINDOW_TYPE_SPLASH" only_if_exists = false,
        pub net_wm_window_type_toolbar => b"_NET_WM_WINDOW_TYPE_TOOLBAR" only_if_exists = false,
        pub net_wm_window_type_tooltip => b"_NET_WM_WINDOW_TYPE_TOOLTIP" only_if_exists = false,
        pub net_wm_window_type_utility => b"_NET_WM_WINDOW_TYPE_UTILITY" only_if_exists = false,

        pub wm_change_state => b"WM_CHANGE_STATE" only_if_exists = false,
        pub wm_class => b"WM_CLASS" only_if_exists = false,
        pub wm_delete_window => b"WM_DELETE_WINDOW" only_if_exists = false,
        pub wm_protocols => b"WM_PROTOCOLS" only_if_exists = false,
        pub wm_take_focus => b"WM_TAKE_FOCUS" only_if_exists = false,

        pub xembed => b"_XEMBED" only_if_exists = false,
        pub xembed_info => b"_XEMBED_INFO" only_if_exists = false,

        pub gtk_application_id => b"_GTK_APPLICATION_ID" only_if_exists = false,
        pub manager => b"MANAGER" only_if_exists = false,
        pub motif_wm_hints => b"_MOTIF_WM_HINTS" only_if_exists = false,
        pub utf8_string => b"UTF8_STRING" only_if_exists = false,
    }
}

impl Atoms {
    /// Get a slice of all the atoms in the structure.
    pub fn as_slice(&self) -> &[Atom] {
        const COUNT: usize = size_of::<Atoms>() / size_of::<Atom>();
        let ptr = self as *const Atoms as *const Atom;
        unsafe { std::slice::from_raw_parts(ptr, COUNT) }
    }
}

/// Property types.
#[allow(dead_code)]
#[derive(Clone)]
pub enum PropertyValue {
    Atom(Atom),
    AtomList(Vec<Atom>),
    Window(XcbWindow),
    WindowList(Vec<XcbWindow>),
    Cardinal(u32),
    CardinalList(Vec<u32>),
    /// Properties of type `XA_STRING`. This still uses utf8 strings on our end.
    Latin1(String),
    /// Properties of type `UTF8_STRING`
    String(String),
}

impl PropertyValue {
    /// Get the type atom for the value.
    fn r#type(&self, display: &Display) -> Atom {
        match self {
            Self::Atom(_) => ATOM_ATOM,
            Self::AtomList(_) => ATOM_ATOM,
            Self::Window(_) => ATOM_WINDOW,
            Self::WindowList(_) => ATOM_WINDOW,
            Self::Cardinal(_) => ATOM_CARDINAL,
            Self::CardinalList(_) => ATOM_CARDINAL,
            Self::Latin1(_) => ATOM_STRING,
            Self::String(_) => display.atoms.utf8_string,
        }
    }

    // Apparently there's just no way of getting the data out of this in a
    // dynamic way to we'll need to implement changing the property here.
    /// Changes a windows property to this value, consuming the value.
    fn change(self, display: &Display, window: XcbWindow, property: Atom, mode: PropMode) {
        let r#type = self.r#type(display);
        macro_rules! make_request {
            ($data:expr) => {
                display.void_request(&ChangeProperty {
                    mode,
                    window,
                    property,
                    r#type,
                    data: $data,
                })
            };
        }
        match self {
            Self::Atom(atom) => make_request!(&[atom]),
            Self::AtomList(atoms) => make_request!(atoms.as_slice()),
            Self::Window(window) => make_request!(&[window]),
            Self::WindowList(windows) => make_request!(windows.as_slice()),
            Self::Cardinal(cardinal) => make_request!(&[cardinal]),
            Self::CardinalList(cardinals) => make_request!(cardinals.as_slice()),
            Self::Latin1(string) => make_request!(string.as_bytes()),
            Self::String(string) => make_request!(string.as_bytes()),
        }
    }
}

pub trait GetProperty {
    /// Returns the window to use in the request.
    fn window(&self) -> XcbWindow;

    /// Makes a `GetProperty` request with all arguments.
    fn get_property_full(
        &self,
        display: &Display,
        delete: bool,
        property: Atom,
        r#type: Atom,
        long_offset: u32,
        long_length: u32,
    ) -> xcb::Result<GetPropertyReply> {
        display.request_with_reply(&xcb::x::GetProperty {
            delete,
            window: self.window(),
            property,
            r#type,
            long_offset,
            long_length,
        })
    }

    /// Makes a `GetProperty` request with some arguments set to a default value.
    fn get_property(
        &self,
        display: &Display,
        property: Atom,
        r#type: Atom,
    ) -> xcb::Result<GetPropertyReply> {
        // The length passed to the request is just an upper bound.
        const MAX_LENGTH: u32 = 0x1FFFFFFF;
        self.get_property_full(display, false, property, r#type, 0, MAX_LENGTH)
    }

    fn get_string_property(&self, display: &Display, property: Atom) -> Option<String> {
        let reply = self.get_property(display, property, ATOM_ANY).ok()?;
        let real_type = reply.r#type();
        if real_type == ATOM_NONE {
            return None;
        }
        if real_type != ATOM_STRING && real_type != display.atoms.utf8_string {
            fatal_error(
                display,
                format!("Not a string property: {property:?}\nReal type is: {real_type:?}"),
            );
        }
        Some(std::str::from_utf8(reply.value()).ok()?.to_string())
    }
}

impl<T> GetProperty for T
where
    T: Xid,
{
    fn window(&self) -> XcbWindow {
        unsafe { XcbWindow::new(self.resource_id()) }
    }
}

pub trait SetProperty {
    /// Returns the window to use in the request.
    fn window(&self) -> XcbWindow;

    fn set_property(&self, display: &Display, property: Atom, value: PropertyValue) {
        value.change(display, self.window(), property, PropMode::Replace);
    }

    fn append_property(&self, display: &Display, property: Atom, value: PropertyValue) {
        value.change(display, self.window(), property, PropMode::Append);
    }

    fn delete_property(&self, display: &Display, property: Atom) {
        display.void_request(&DeleteProperty {
            window: self.window(),
            property,
        });
    }
}

impl<T> SetProperty for T
where
    T: Xid,
{
    fn window(&self) -> XcbWindow {
        unsafe { XcbWindow::new(self.resource_id()) }
    }
}
