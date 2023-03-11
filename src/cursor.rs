use crate::x::Display;
use std::sync::Arc;
use xcb::x::Cursor;

macro_rules! cursors {
    {
        $($name:ident => $shape:expr,)*
    } => {
        pub struct Cursors {
            display: Arc<Display>,
            $(pub $name: Cursor,)*
        }
        impl Cursors {
            pub fn create(display: Arc<Display>) -> Self {
                let d = display.clone();
                Self {
                    display,
                    $(
                        $name: d.create_font_cursor($shape),
                    )*
                }
            }
        }
        impl Drop for Cursors {
            fn drop(&mut self) {
                $(
                    self.display.free_cursor(self.$name);
                )*
            }
        }
    }
}

cursors! {
    normal => 68, // XC_left_ptr
    moving => 52, // XC_fleur
    resizing => 120, // XC_sizing
    resizing_horizontal => 108, // XC_sb_h_double_arrow
    resizing_vertical => 116, // XC_sb_v_double_arrow
}
