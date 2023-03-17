use super::{property::Atoms, GetProperty, Visual, XcbWindow};
use crate::error::{fatal_error, OrFatal};
use libc::c_void;
use parking_lot::Mutex;
use std::{ffi::CStr, mem::zeroed, sync::Arc};
use x11::xlib::{
    KeyCode, MappingKeyboard, MappingModifier, MappingPointer, XCreateFontCursor,
    XDefaultRootWindow, XDefaultScreen, XDisplayHeight, XDisplayString, XDisplayWidth, XFree,
    XFreeCursor, XGetModifierMapping, XGetWMNormalHints, XKeycodeToKeysym, XKeysymToKeycode,
    XMatchVisualInfo, XModifierKeymap, XRefreshKeyboardMapping, XSizeHints, XVisualInfo,
};
use xcb::{
    ffi::xcb_generic_event_t,
    x::{
        Atom, ButtonIndex, Colormap, ColormapAlloc, CreateColormap, CreatePixmap, Cursor, Drawable,
        EventMask, FreePixmap, GetAtomName, GetInputFocus, GetSelectionOwner, GrabButton, GrabKey,
        GrabKeyboard, GrabMode, GrabPointer, GrabStatus, InputFocus, Keycode, Mapping,
        MappingNotifyEvent, ModMask, Pixmap, QueryPointer, Screen, SetInputFocus,
        SetSelectionOwner, UngrabButton, UngrabKey, UngrabKeyboard, UngrabPointer, ATOM_WM_NAME,
        CURRENT_TIME,
    },
    Connection, CookieWithReplyChecked, Error, Event, ProtocolResult, Raw, RequestWithoutReply,
    UnknownEvent, Xid, XidNew,
};

/// Event contains a raw pointer so we need a wrapper to implement `Send` for it.
struct EventWrapper(Result<Event, Error>);
unsafe impl Send for EventWrapper {}

pub struct Display {
    pub(super) connection: Connection,
    screen_num: i32,
    pub(super) root: XcbWindow,
    peeked_event: Mutex<Option<EventWrapper>>,
    pub atoms: Atoms,
    truecolor_visual: Visual,
}

impl Display {
    pub fn connect() -> xcb::Result<Self> {
        let (connection, screen_num) = Connection::connect_with_xlib_display()?;
        let screen = connection
            .get_setup()
            .roots()
            .nth(screen_num as usize)
            .unwrap();
        let root = screen.root();
        let atoms = Atoms::intern_all(&connection)?;
        let mut this = Self {
            connection,
            screen_num,
            root,
            peeked_event: Mutex::new(None),
            atoms,
            truecolor_visual: Visual::uninit(),
        };
        this.truecolor_visual = Visual::get_truecolor(&this);
        Ok(this)
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    /// Returns the connection as the type `xcb_util` neebs (`xcb::base::Connection`).
    /// In our xcb version `xcb::base` is a pirvate module so this function takes
    /// any type and infers the correct one for `xcb_util` calls.
    /// Should never be used for anything else!
    pub fn connection_for_xcb_util<T>(&self) -> &T {
        unsafe { std::mem::transmute(&self.connection) }
    }

    pub fn xlib_display(&self) -> *mut x11::xlib::Display {
        self.connection.get_raw_dpy()
    }

    /// Get the handle of the root window.
    pub fn root(&self) -> XcbWindow {
        self.root
    }

    pub fn screen(&self) -> &Screen {
        self.connection
            .get_setup()
            .roots()
            .nth(self.screen_num as usize)
            .unwrap()
    }

    pub fn truecolor_visual(&self) -> &Visual {
        &self.truecolor_visual
    }

    pub fn void_request<'a, R>(&'a self, request: &'a R)
    where
        R: RequestWithoutReply,
    {
        self.connection.send_request(request);
        self.connection.flush().unwrap(); // TODO
    }

    pub fn try_void_request<'a, R>(&'a self, request: &'a R) -> ProtocolResult<()>
    where
        R: RequestWithoutReply,
    {
        let cookie = self.connection.send_request_checked(request);
        self.connection.check_request(cookie)
    }

    pub fn request_with_reply<'a, R>(
        &'a self,
        request: &'a R,
    ) -> xcb::Result<<<R as xcb::Request>::Cookie as CookieWithReplyChecked>::Reply>
    where
        R: xcb::RequestWithReply,
        <R as xcb::Request>::Cookie: CookieWithReplyChecked,
    {
        let cookie = self.connection.send_request(request);
        self.connection.wait_for_reply(cookie)
    }

    pub fn flush(&self) {
        self.connection.flush().ok();
    }

    /// Make sure all previous requests are processed, and their replies received.
    pub fn sync(&self) {
        self.request_with_reply(&GetInputFocus {}).ok();
    }

    pub fn next_event(&self) -> Result<Event, Error> {
        if let Some(peeked) = self.peeked_event.lock().take() {
            peeked.0
        } else {
            self.connection.wait_for_event()
        }
    }

    pub fn put_back_event(&self, event: Result<Event, Error>) {
        let mut peeked = self.peeked_event.lock();
        if peeked.is_some() {
            log::warn!("Overwriting peeked event");
        }
        *peeked = Some(EventWrapper(event));
    }

    pub fn match_visual_info(&self, depth: i32, class: i32) -> XVisualInfo {
        unsafe {
            let mut vi: XVisualInfo = zeroed();
            let screen = XDefaultScreen(self.connection.get_raw_dpy());
            if XMatchVisualInfo(self.connection.get_raw_dpy(), screen, depth, class, &mut vi) != 0 {
                vi
            } else {
                Err::<(), String>(format!(
                    "Could not find visual for depth={} and class={}",
                    depth, class
                ))
                .or_fatal(self);
                unreachable!()
            }
        }
    }

    pub fn create_colormap(&self, vi: &XVisualInfo) -> Colormap {
        let screen = self
            .connection
            .get_setup()
            .roots()
            .nth(vi.screen as usize)
            .unwrap();
        let mid = self.connection.generate_id();
        self.try_void_request(&CreateColormap {
            alloc: ColormapAlloc::None,
            mid,
            window: screen.root(),
            visual: vi.visualid as u32,
        })
        .or_fatal(self);
        mid
    }

    pub fn keysym_to_keycode<T>(&self, sym: T) -> u8
    where
        T: Into<u64>,
    {
        unsafe { XKeysymToKeycode(self.connection.get_raw_dpy(), sym.into()) }
    }

    pub fn keycode_to_keysym<T>(&self, code: T) -> u64
    where
        T: Into<u8>,
    {
        unsafe { XKeycodeToKeysym(self.connection.get_raw_dpy(), code.into(), 0) }
    }

    pub fn get_modifier_mapping(&self) -> *mut XModifierKeymap {
        unsafe { XGetModifierMapping(self.connection.get_raw_dpy()) }
    }

    pub fn refresh_keyboard_mapping(&self, event: &MappingNotifyEvent) {
        let dpy = self.connection.get_raw_dpy();
        let root = unsafe { XDefaultRootWindow(dpy) };
        let mut xlib_event = x11::xlib::XMappingEvent {
            type_: event.response_type() as i32,
            serial: event.sequence() as u64,
            send_event: 0,
            display: dpy,
            event: root,
            request: match event.request() {
                Mapping::Modifier => MappingModifier,
                Mapping::Keyboard => MappingKeyboard,
                Mapping::Pointer => MappingPointer,
            },
            first_keycode: event.first_keycode() as i32,
            count: event.count() as i32,
        };
        unsafe { XRefreshKeyboardMapping(&mut xlib_event) };
    }

    pub fn get_name(&self) -> String {
        unsafe {
            let c_str = XDisplayString(self.connection.get_raw_dpy());
            let string = CStr::from_ptr(c_str).to_str().unwrap().to_string();
            XFree(c_str as *mut c_void);
            string
        }
    }

    /// Return the total display size, i.e. the size of a bounding box around
    /// all monitors.
    pub fn get_total_size(&self) -> (u16, u16) {
        unsafe {
            let dpy = self.connection.get_raw_dpy();
            let screen = XDefaultScreen(dpy);
            (
                XDisplayWidth(dpy, screen) as u16,
                XDisplayHeight(dpy, screen) as u16,
            )
        }
    }

    pub fn create_unknown_event(response_type: u8) -> UnknownEvent {
        unsafe {
            let raw = libc::malloc(std::mem::size_of::<xcb_generic_event_t>())
                as *mut xcb_generic_event_t;
            *raw = xcb_generic_event_t {
                response_type,
                pad0: 0,
                sequence: 0,
                pad: [0; 7],
                full_sequence: 0,
            };
            UnknownEvent::from_raw(raw)
        }
    }

    pub fn grab_key(&self, key: Keycode, modifiers: ModMask) {
        self.void_request(&GrabKey {
            owner_events: true,
            grab_window: self.root,
            modifiers,
            key,
            pointer_mode: GrabMode::Async,
            keyboard_mode: GrabMode::Async,
        });
    }

    pub fn ungrab_key(&self, key: KeyCode, modifiers: ModMask) {
        self.void_request(&UngrabKey {
            key,
            grab_window: self.root,
            modifiers,
        });
    }

    pub fn grab_button(&self, button: ButtonIndex, modifiers: ModMask) {
        const EVENT_MASK: EventMask = EventMask::from_bits_truncate(
            EventMask::BUTTON_PRESS.bits()
                | EventMask::BUTTON_RELEASE.bits()
                | EventMask::POINTER_MOTION.bits(),
        );
        self.void_request(&GrabButton {
            owner_events: true,
            grab_window: self.root,
            event_mask: EVENT_MASK,
            pointer_mode: GrabMode::Async,
            keyboard_mode: GrabMode::Async,
            confine_to: XcbWindow::none(),
            cursor: Cursor::none(),
            button,
            modifiers,
        });
    }

    pub fn ungrab_button(&self, button: ButtonIndex, modifiers: ModMask) {
        self.void_request(&UngrabButton {
            button,
            grab_window: self.root,
            modifiers,
        });
    }

    pub fn get_atom_name(&self, atom: Atom) -> Option<String> {
        self.request_with_reply(&GetAtomName { atom })
            .ok()
            .map(|reply| reply.name().to_utf8().to_string())
    }

    pub fn create_pixmap(&self, pid: Option<Pixmap>, depth: u8, width: u16, height: u16) -> Pixmap {
        let pid = match pid {
            Some(pid) => pid,
            None => self.connection.generate_id(),
        };
        self.try_void_request(&CreatePixmap {
            depth,
            pid,
            drawable: Drawable::Window(self.root),
            width,
            height,
        })
        .or_fatal(self);
        pid
    }

    pub fn free_pixmap(&self, pixmap: Pixmap) {
        self.void_request(&FreePixmap { pixmap });
    }

    pub fn create_font_cursor(&self, shape: u32) -> Cursor {
        unsafe {
            let xlib_cursor = XCreateFontCursor(self.connection.get_raw_dpy(), shape);
            Cursor::new(xlib_cursor as u32)
        }
    }

    pub fn free_cursor(&self, cursor: Cursor) {
        let xlib_cursor = cursor.resource_id() as x11::xlib::Cursor;
        unsafe {
            XFreeCursor(self.connection.get_raw_dpy(), xlib_cursor);
        }
    }

    pub fn window_title(&self, window: &impl Xid) -> Option<String> {
        window
            .get_string_property(self, self.atoms.net_wm_name)
            .or_else(|| window.get_string_property(self, ATOM_WM_NAME))
    }

    pub fn set_input_focus(&self, window: XcbWindow) {
        self.void_request(&SetInputFocus {
            revert_to: InputFocus::Parent,
            focus: window,
            time: CURRENT_TIME,
        });
    }

    pub fn query_pointer_position(&self) -> (i16, i16) {
        let reply = self
            .request_with_reply(&QueryPointer {
                window: self.root(),
            })
            .unwrap_or_fatal(self);
        (reply.root_x(), reply.root_y())
    }

    pub fn get_wm_normal_hints(&self, window: &impl Xid) -> Option<XSizeHints> {
        let window = window.resource_id() as x11::xlib::Window;
        let mut size_hints: XSizeHints = unsafe { zeroed() };
        let mut _ignored: i64 = 0;
        if unsafe {
            XGetWMNormalHints(
                self.connection.get_raw_dpy(),
                window,
                &mut size_hints,
                &mut _ignored,
            )
        } == 0
        {
            return None;
        }
        Some(size_hints)
    }

    pub fn get_selection_owner(&self, selection: Atom) -> XcbWindow {
        self.request_with_reply(&GetSelectionOwner { selection })
            .unwrap_or_fatal(self)
            .owner()
    }

    pub fn set_selection_owner(&self, selection: Atom, owner: XcbWindow) {
        self.void_request(&SetSelectionOwner {
            owner,
            selection,
            time: CURRENT_TIME,
        });
    }

    pub fn grab_pointer(&self, cursor: Cursor, grab_window: XcbWindow) {
        let reply = self
            .request_with_reply(&GrabPointer {
                owner_events: false,
                grab_window,
                event_mask: EventMask::BUTTON_PRESS
                    | EventMask::BUTTON_RELEASE
                    | EventMask::POINTER_MOTION,
                pointer_mode: GrabMode::Async,
                keyboard_mode: GrabMode::Async,
                confine_to: XcbWindow::none(),
                cursor,
                time: CURRENT_TIME,
            })
            .unwrap_or_fatal(self);
        if !matches!(reply.status(), GrabStatus::Success) {
            fatal_error(self, "Failed to grab pointer".to_string());
        }
    }

    pub fn ungrab_pointer(&self) {
        self.try_void_request(&UngrabPointer { time: CURRENT_TIME })
            .or_fatal(self);
    }

    pub fn grab_keyboard(&self, grab_window: XcbWindow) {
        let reply = self
            .request_with_reply(&GrabKeyboard {
                owner_events: false,
                grab_window,
                pointer_mode: GrabMode::Async,
                keyboard_mode: GrabMode::Async,
                time: CURRENT_TIME,
            })
            .unwrap_or_fatal(self);
        if !matches!(reply.status(), GrabStatus::Success) {
            fatal_error(self, "Failed to grab keyboard".to_string());
        }
    }

    pub fn ungrab_keyboard(&self) {
        self.try_void_request(&UngrabKeyboard { time: CURRENT_TIME })
            .or_fatal(self);
    }
}

impl std::fmt::Debug for Display {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Display({})", self.get_name())
    }
}

pub struct ScopedPointerGrab(Arc<Display>);

impl ScopedPointerGrab {
    pub fn begin(display: Arc<Display>, cursor: Cursor) -> Self {
        display.grab_pointer(cursor, display.root());
        Self(display)
    }
}

impl Drop for ScopedPointerGrab {
    fn drop(&mut self) {
        self.0.ungrab_pointer();
    }
}

pub struct ScopedKeyboardGrab(Arc<Display>);

impl ScopedKeyboardGrab {
    pub fn begin(display: Arc<Display>) -> Self {
        display.grab_keyboard(display.root());
        Self(display)
    }
}

impl Drop for ScopedKeyboardGrab {
    fn drop(&mut self) {
        self.0.ungrab_keyboard();
    }
}
