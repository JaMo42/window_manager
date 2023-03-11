use crate::x::{GetProperty, Window};

pub const MWM_HINTS_DECORATIONS: u32 = 1 << 1;

#[derive(Debug, Copy, Clone)]
pub struct MotifHints {
    pub flags: u32,
    pub functions: u32,
    pub decorations: u32,
    pub input_mode: u32,
    pub status: u32,
}

impl MotifHints {
    fn from_data(data: &[u32]) -> Option<Self> {
        if data.len() == 5 {
            Some(Self {
                flags: data[0],
                functions: data[1],
                decorations: data[2],
                input_mode: data[3],
                status: data[4],
            })
        } else {
            log::warn!("Incomplete motif hints ({}/5 values)", data.len());
            None
        }
    }

    /// Get the motif hints of the given window.
    pub fn get(window: &Window) -> Option<Self> {
        let display = window.display();
        let atom = display.atoms.motif_wm_hints;
        let reply = window.get_property(display, atom, atom).ok()?;
        if reply.length() == 0 {
            return None;
        }
        Self::from_data(reply.value())
    }

    /// Does the window owning the hints provide its own decorations (titlebar)?
    pub fn has_own_decorations(&self) -> bool {
        self.flags & MWM_HINTS_DECORATIONS == MWM_HINTS_DECORATIONS && self.decorations == 0
    }
}
