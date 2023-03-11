use super::Display;
use x11::xlib::TrueColor;
use xcb::{x::Colormap, XidNew};

#[derive(Copy, Clone, Debug)]
pub struct Visual {
    pub depth: u8,
    pub id: u32,
    pub colormap: Colormap,
}

impl Visual {
    pub(crate) fn uninit() -> Self {
        Self {
            depth: 0,
            id: 0,
            colormap: unsafe { Colormap::new(0) },
        }
    }

    /// Get the 32-bit Truecolor visual of the given display and create a new
    /// colormap for it.
    pub(crate) fn get_truecolor(display: &Display) -> Self {
        let vi = display.match_visual_info(32, TrueColor);
        let colormap = display.create_colormap(&vi);
        Self {
            depth: vi.depth as u8,
            id: vi.visualid as u32,
            colormap,
        }
    }
}
