use std::ffi::{CStr, CString};
use x11::xlib::{KeySym, NoSymbol, XKeysymToString, XStringToKeysym};

mod display;
mod input_only_window;
mod modmap;
mod property;
pub mod randr;
mod visual;
mod window;
mod window_attributes;
mod window_builder;

pub use display::{Display, ScopedKeyboardGrab, ScopedPointerGrab};
pub use input_only_window::InputOnlyWindow;
pub use modmap::ModifierMapping;
pub use property::{Atoms, GetProperty, PropertyValue, SetProperty};
pub use randr::Monitor;
pub use visual::Visual;
pub use window::Window;
pub use window_attributes::WindowAttributes;
pub use window_builder::WindowBuilder;

pub type XcbWindow = xcb::x::Window;

pub fn string_to_keysym(s: &str) -> Option<u64> {
    unsafe {
        let c_str = CString::new(s).ok()?;
        let maybe_sym = XStringToKeysym(c_str.as_ptr());
        if maybe_sym == NoSymbol as u64 {
            None
        } else {
            Some(maybe_sym)
        }
    }
}

#[allow(dead_code)]
pub fn keysym_to_string(sym: KeySym) -> Option<String> {
    let ptr = unsafe { XKeysymToString(sym) };
    if ptr.is_null() {
        None
    } else {
        unsafe { CStr::from_ptr(ptr) }
            .to_str()
            .ok()
            .map(ToString::to_string)
    }
}

/// Destroys a windows using the `WM_DELETE_WINDOW` message if supported or
/// the `KillClient` request if not.
pub fn close_window(window: &Window) {
    use xcb::{
        x::{ClientMessageData, ClientMessageEvent, EventMask, CURRENT_TIME},
        Xid,
    };
    use xcb_util::icccm::get_wm_protocols;
    let mut is_supported = false;
    let display = window.display();
    let protocol = display.atoms.wm_delete_window;
    if let Ok(get_protocols_reply) = get_wm_protocols(
        display.connection_for_xcb_util(),
        window.handle().resource_id(),
        display.atoms.wm_protocols.resource_id(),
    )
    .get_reply()
    {
        let protocol_id = protocol.resource_id();
        for supported in get_protocols_reply.atoms().iter().cloned() {
            if supported == protocol_id {
                is_supported = true;
                break;
            }
        }
    }
    if is_supported {
        window.send_event(
            EventMask::NO_EVENT,
            &ClientMessageEvent::new(
                window.handle(),
                display.atoms.wm_protocols,
                ClientMessageData::Data32([protocol.resource_id(), CURRENT_TIME, 0, 0, 0]),
            ),
        );
    } else {
        window.kill_client();
        display.flush();
    }
}
