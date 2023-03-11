use crate::x::Window;
use xcb::Xid;
use xcb_util::icccm::{get_wm_class, set_wm_class};

#[derive(Debug, Default)]
pub struct ClassHint {
    pub class: String,
    pub name: String,
}

impl ClassHint {
    pub fn get(window: &Window) -> Option<Self> {
        let cookie = get_wm_class(
            window.display().connection_for_xcb_util(),
            window.handle().resource_id(),
        );
        let reply = cookie.get_reply().ok()?;
        Some(Self {
            class: reply.class().to_owned(),
            name: reply.instance().to_owned(),
        })
    }

    pub fn new(class: &str, name: &str) -> Self {
        Self {
            class: class.to_string(),
            name: name.to_string(),
        }
    }

    /// Set the class hint on the given window.
    pub fn set(&self, window: &Window) {
        set_wm_class(
            window.display().connection_for_xcb_util(),
            window.handle().resource_id(),
            &self.class,
            &self.name,
        );
    }
}
