#![allow(non_upper_case_globals)]
use crate::x::Display;
use std::sync::Arc;
use xcb::x::Cursor;

// Font cursor shapes
pub const XC_left_ptr: u32 = 68;
pub const XC_fleur: u32 = 52;
pub const XC_sizing: u32 = 120;
pub const XC_sb_h_double_arrow: u32 = 108;
pub const XC_sb_v_double_arrow: u32 = 116;
// Custom ids for named cursors
pub const NESW_RESIZE: u32 = 1000;
pub const NWSE_RESIZE: u32 = 1001;

macro_rules! cursors {
    {
        font {
        $($name:ident => $shape:ident,)*
        }
        named {
        $($i_name:ident => $c_name:expr, $my_id:ident, $fallback:ident,)*
        }
    } => {
        pub struct Cursors {
            display: Arc<Display>,
            $(pub $name: Cursor,)*
            $(pub $i_name: Cursor,)*
        }

        impl Cursors {
            pub fn create(display: Arc<Display>) -> Self {
                let d = display.clone();
                Self {
                    display,
                    $(
                    $name: d.create_font_cursor($shape),
                    )*
                    $(
                    // fallbacks are loaded again instead of reusing the font cursors
                    // already loaded so we unconditionally free all cursors
                    $i_name: d
                        .load_cursor($c_name)
                        .unwrap_or_else(|| d.create_font_cursor($fallback)),
                    )*
                }
            }

            pub fn by_id(&self, id: u32) -> Cursor {
                match id {
                    $(
                    $shape => self.$name,
                    )*
                    $(
                    $my_id => self.$i_name,
                    )*
                    _ => panic!("invalid cursor id"),
                }
            }
        }

        impl Drop for Cursors {
            fn drop(&mut self) {
                $(
                self.display.free_cursor(self.$name);
                )*
                $(
                self.display.free_cursor(self.$i_name);
                )*
            }
        }
    }
}

cursors! {
    font {
        normal => XC_left_ptr,
        moving => XC_fleur,
        resizing => XC_sizing,
        resizing_horizontal => XC_sb_h_double_arrow,
        resizing_vertical => XC_sb_v_double_arrow,
    }
    named {
        nesw_resize => "nesw-resize", NESW_RESIZE, XC_sizing,
        nwse_resize => "nwse-resize", NWSE_RESIZE, XC_sizing,
    }
}
