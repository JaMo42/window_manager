use xcb::{
    x::{ClientMessageData, ClientMessageEvent, EventMask, SendEvent, SendEventDest, CURRENT_TIME},
    Xid,
};

use crate::x::{Display, GetProperty, Window, XcbWindow};

const VERSION: u32 = 5;
const EMBEDDED_NOTIFY: u32 = 0;
const FLAG_MAPPED: u32 = 1 << 0;

#[derive(Copy, Clone, Debug)]
pub struct Info {
    version: u32,
    flags: u32,
}

impl Info {
    pub const fn new() -> Self {
        Self {
            version: 0,
            flags: 0,
        }
    }

    pub fn query(&mut self, window: XcbWindow, display: &Display) -> bool {
        if let Ok(reply) = window.get_property_full(
            display,
            false,
            display.atoms.xembed_info,
            display.atoms.xembed_info,
            0,
            2,
        ) {
            if reply.length() == 2 {
                let data = reply.value::<u32>();
                self.version = data[0];
                self.flags = data[1];
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn is_mapped(&self) -> bool {
        self.flags & FLAG_MAPPED == FLAG_MAPPED
    }
}

/// Sends a xembed client message.
fn send_message(display: &Display, window: XcbWindow, msg: u32, d1: u32, d2: u32, d3: u32) {
    display.connection().send_request(&SendEvent {
        propagate: false,
        destination: SendEventDest::Window(window),
        event_mask: EventMask::NO_EVENT,
        event: &ClientMessageEvent::new(
            window,
            display.atoms.xembed,
            ClientMessageData::Data32([CURRENT_TIME, msg, d1, d2, d3]),
        ),
    });
    display.flush();
}

/// Tells `window` that it was embedded into `parent`.
pub fn embed(window: XcbWindow, parent: &Window, version: u32) {
    send_message(
        parent.display(),
        window,
        EMBEDDED_NOTIFY,
        0,
        parent.handle().resource_id(),
        u32::min(version, VERSION),
    );
}
