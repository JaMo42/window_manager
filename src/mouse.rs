use crate::{
    action,
    client::Client,
    cursor,
    geometry_preview::GeometryPreview,
    monitors::monitors,
    normal_hints::NormalHints,
    rectangle::{PointOffset, Rectangle},
    x::{Display, ScopedKeyboardGrab, ScopedPointerGrab},
};
use std::{cell::RefCell, rc::Rc, sync::Arc};
use x11::keysym::XK_Escape;
use xcb::{
    x::{
        ButtonIndex, ButtonPressEvent, Cursor, KeyButMask, KeyPressEvent, MotionNotifyEvent,
        Timestamp,
    },
    Event,
};

const MOUSE_MOVE_RESIZE_RATE: u32 = 1000 / 30;
const MOUSE_MOVE_ACTIVATION_THRESHHOLD: i16 = 10;

/// Left button
pub const BUTTON_1: u8 = ButtonIndex::N1 as u8;
/// Middle button
pub const BUTTON_2: u8 = ButtonIndex::N2 as u8;
/// Right button
pub const BUTTON_3: u8 = ButtonIndex::N3 as u8;
/// Scroll down
pub const BUTTON_4: u8 = ButtonIndex::N4 as u8;
/// Scroll up
pub const BUTTON_5: u8 = ButtonIndex::N5 as u8;

type MotionCallback<'a> = &'a mut dyn FnMut(&MotionNotifyEvent, i16, i16);
type ButtonCallback<'a> = &'a mut dyn FnMut(&ButtonPressEvent) -> bool;
type KeyCallback<'a> = &'a mut dyn FnMut(&KeyPressEvent) -> bool;
type FinishCallback<'a> = &'a mut dyn FnMut(FinishReason);
type ActicationCallback<'a> = &'a mut dyn FnMut();

#[derive(Copy, Clone)]
pub enum FinishReason {
    #[allow(unused)]
    Finish(i16, i16),
    Cancel,
    Failure,
}

pub struct TrackedMotion<'a> {
    display: Arc<Display>,
    on_motion: Option<MotionCallback<'a>>,
    on_button_press: Option<ButtonCallback<'a>>,
    on_key_press: Option<KeyCallback<'a>>,
    on_finish: Option<FinishCallback<'a>>,
    on_activation: Option<ActicationCallback<'a>>,
    activation_threshold: i16,
    rate: u32,
    my_on_key_press: bool,
}

impl<'a> TrackedMotion<'a> {
    pub fn new(display: Arc<Display>) -> Self {
        Self {
            display,
            on_motion: None,
            on_button_press: None,
            on_key_press: None,
            on_finish: None,
            on_activation: None,
            activation_threshold: 0,
            rate: 30,
            my_on_key_press: false,
        }
    }

    pub fn on_motion(&mut self, callback: MotionCallback<'a>) -> &mut Self {
        self.on_motion = Some(callback);
        self
    }

    /// If the callback returns `true` the operation is cancelled.
    pub fn on_button_press(&mut self, callback: ButtonCallback<'a>) -> &mut Self {
        self.on_button_press = Some(callback);
        self
    }

    /// If the callback returns `true` the operation is cancelled.
    pub fn on_key_press(&mut self, callback: KeyCallback<'a>) -> &mut Self {
        self.on_key_press = Some(callback);
        self
    }

    pub fn on_finish(&mut self, callback: FinishCallback<'a>) -> &mut Self {
        self.on_finish = Some(callback);
        self
    }

    pub fn activation_threshold(
        &mut self,
        threshold: i16,
        callback: ActicationCallback<'a>,
    ) -> &mut Self {
        self.on_activation = Some(callback);
        self.activation_threshold = threshold;
        self
    }

    pub fn rate(&mut self, rate: u32) -> &mut Self {
        self.rate = rate;
        self
    }

    // Installs a `on_key_press` handler that cancels the operation when the
    // escape key is pressed.
    pub fn cancel_on_escape(&mut self) -> &mut Self {
        let escape_code = self.display.keysym_to_keycode(XK_Escape);
        let callback = Box::new(move |event: &KeyPressEvent| event.detail() == escape_code);
        self.my_on_key_press = true;
        self.on_key_press(Box::leak(callback))
    }

    fn run_impl(&mut self, cursor: Cursor) -> Option<()> {
        use xcb::x::Event::*;
        let _pointer_grab = ScopedPointerGrab::begin(self.display.clone(), cursor);
        let _keyboard_grab = if self.on_key_press.is_some() {
            Some(ScopedKeyboardGrab::begin(self.display.clone()))
        } else {
            None
        };
        let (start_x, start_y) = self.display.query_pointer_position();
        let mut last_time: Timestamp = 0;
        let mut mouse_x = start_x;
        let mut mouse_y = start_y;
        let mut active = self.activation_threshold == 0;
        let finish_reason;
        loop {
            let event = if let Ok(Event::X(event)) = self.display.next_event() {
                event
            } else {
                continue;
            };
            match event {
                MotionNotify(motion) => {
                    if motion.time() - last_time < self.rate {
                        continue;
                    }
                    last_time = motion.time();
                    if !active {
                        if (start_x - motion.root_x()).abs() > self.activation_threshold
                            || (start_y - motion.root_y()).abs() > self.activation_threshold
                        {
                            active = true;
                            (self.on_activation.take().unwrap())();
                        } else {
                            continue;
                        }
                    }
                    (unsafe { self.on_motion.as_mut().unwrap_unchecked() })(
                        &motion, mouse_x, mouse_y,
                    );
                    mouse_x = motion.root_x();
                    mouse_y = motion.root_y();
                }
                ButtonPress(button) => {
                    if let Some(on_button_press) = &mut self.on_button_press {
                        if on_button_press(&button) {
                            finish_reason = FinishReason::Cancel;
                            break;
                        }
                    }
                }
                KeyPress(key) => {
                    if let Some(on_key_press) = &mut self.on_key_press {
                        if on_key_press(&key) {
                            finish_reason = FinishReason::Cancel;
                            break;
                        }
                    }
                }
                ButtonRelease(button) => {
                    // FIXME: this can be weird as draggin a window with the left
                    // mouse button can be cancelled by pressing the right mouse
                    // button but it's a low priority scenario.
                    finish_reason = FinishReason::Finish(button.root_x(), button.root_y());
                    break;
                }
                _ => {}
            }
        }
        if let Some(on_finish) = self.on_finish.take() {
            on_finish(finish_reason);
        }
        Some(())
    }

    pub fn run(&mut self, cursor: Cursor) {
        assert!(self.on_motion.is_some());
        if self.run_impl(cursor).is_none() {
            // Bailed out early, still need to call on_finish.
            if let Some(on_finish) = self.on_finish.take() {
                on_finish(FinishReason::Failure);
            }
        }
        // If we have our own callback it was created as a boxed value.
        if self.my_on_key_press {
            drop(unsafe {
                Box::from_raw(
                    self.on_key_press.take().unwrap() as *mut dyn FnMut(&KeyPressEvent) -> bool
                )
            });
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct MouseResizeOptions {
    pub lock_width: bool,
    pub lock_height: bool,
    pub up: bool,
    pub left: bool,
}

impl MouseResizeOptions {
    pub const fn new(lock_width: bool, lock_height: bool, up: bool, left: bool) -> Self {
        Self {
            lock_width,
            lock_height,
            up,
            left,
        }
    }

    pub fn from_position(frame: Rectangle, mut x: i16, mut y: i16, corner_size: u16) -> Self {
        x -= frame.x;
        y -= frame.y;
        let corner = corner_size as i16;
        let corner_x = (frame.width - corner_size) as i16;
        let corner_y = (frame.height - corner_size) as i16;
        let lock_width = y < corner || y >= corner_y;
        let lock_height = x < corner || x >= corner_x;
        let not_both = !(lock_width && lock_height);
        Self {
            lock_width: lock_width && not_both,
            lock_height: lock_height && not_both,
            up: y < corner,
            left: x < corner,
        }
    }

    pub fn cursor_id(&self) -> u32 {
        #[rustfmt::skip]
        const TABLE: [(MouseResizeOptions, u32); 16] = [
            (MouseResizeOptions::new(true, false, false, false), cursor::XC_sb_v_double_arrow),
            (MouseResizeOptions::new(true, false, true, false), cursor::XC_sb_v_double_arrow),
            (MouseResizeOptions::new(false, true, false, false), cursor::XC_sb_h_double_arrow),
            (MouseResizeOptions::new(false, true, false, true), cursor::XC_sb_h_double_arrow),
            (MouseResizeOptions::new(false, false, false, false), cursor::NWSE_RESIZE),
            (MouseResizeOptions::new(false, false, false, true), cursor::NESW_RESIZE),
            (MouseResizeOptions::new(false, false, true, false), cursor::NESW_RESIZE),
            (MouseResizeOptions::new(false, false, true, true), cursor::NWSE_RESIZE),
            // These don't make sense and can never be returned from `from_position`
            // but we add them for safety
            (MouseResizeOptions::new(true, false, false, true), cursor::XC_sizing),
            (MouseResizeOptions::new(true, false, true, true), cursor::XC_sizing),
            (MouseResizeOptions::new(false, true, true, true), cursor::XC_sizing),
            (MouseResizeOptions::new(false, true, true, false), cursor::XC_sizing),
            (MouseResizeOptions::new(true, true, false, false), cursor::XC_sizing),
            (MouseResizeOptions::new(true, true, false, true), cursor::XC_sizing),
            (MouseResizeOptions::new(true, true, true, false), cursor::XC_sizing),
            (MouseResizeOptions::new(true, true, true, true), cursor::XC_sizing),
        ];
        for (opts, cursor_id) in TABLE {
            if opts == *self {
                return cursor_id;
            }
        }
        unreachable!()
    }
}

pub fn mouse_move(client: &Client, pressed_key: u8, grid_resize: bool) {
    let wm = client.get_window_manager();
    let display = wm.display.clone();
    let (x, y) = display.query_pointer_position();
    let offset = {
        let frame_offset = client.frame_offset();
        PointOffset::offset_inside(
            (x, y),
            &client.frame_geometry(),
            frame_offset.x,
            frame_offset.y,
        )
    };
    let initial_geometry = if client.is_snapped() {
        let mut g = client.saved_geometry();
        let (x_offset, y_offset) = offset.point_inside(&g);
        g.x = x - x_offset;
        g.y = y - y_offset;
        g
    } else {
        client.frame_geometry()
    };
    let preview = Rc::new(RefCell::new(GeometryPreview::new(
        wm.clone(),
        initial_geometry,
        client.workspace(),
        client.frame_kind(),
    )));
    let shared_client = RefCell::new(client);
    let cursor = wm.cursors.moving;
    TrackedMotion::new(display)
        .rate(MOUSE_MOVE_RESIZE_RATE)
        .activation_threshold(MOUSE_MOVE_ACTIVATION_THRESHHOLD, &mut || {
            preview.borrow_mut().show();
        })
        .on_motion(&mut |motion, last_x, last_y| {
            let mut preview = preview.borrow_mut();
            if motion.state().contains(KeyButMask::SHIFT) {
                preview.snap(motion.root_x(), motion.root_y());
            } else if !monitors()
                .at((motion.root_x(), motion.root_y()))
                .window_area()
                .contains((motion.root_x(), motion.root_y()))
            {
                preview.move_edge(motion.root_x(), motion.root_y());
            } else {
                preview.ensure_unsnapped(last_x, last_y, &offset);
                preview.move_by(motion.root_x() - last_x, motion.root_y() - last_y);
            }
            preview.update();
        })
        .on_button_press(&mut |event| {
            if pressed_key | event.detail() == BUTTON_1 | BUTTON_3 && grid_resize {
                preview.borrow_mut().cancel();
                action::grid_resize(&shared_client.borrow());
                true
            } else {
                false
            }
        })
        .cancel_on_escape()
        .on_finish(&mut |reason| {
            if matches!(reason, FinishReason::Finish(_, _)) {
                preview.borrow_mut().finish(&shared_client.borrow());
            } else {
                preview.borrow_mut().cancel();
            }
        })
        .run(cursor);
}

pub fn mouse_resize(client: &Client, opts: MouseResizeOptions) {
    let wm = client.get_window_manager();
    let mut dx = 0;
    let mut dy = 0;
    let width_mul = !opts.lock_width as i16;
    let height_mul = !opts.lock_height as i16;
    let normal_hints = NormalHints::get(client.window());
    let initial_geometry = if client.is_snapped() {
        client.saved_geometry()
    } else {
        client.frame_geometry()
    };
    let preview = Rc::new(RefCell::new(
        GeometryPreview::new(
            wm.clone(),
            initial_geometry,
            client.workspace(),
            client.frame_kind(),
        )
        .with_sizing_direction(opts.left, opts.up),
    ));
    let cursor = wm.cursors.by_id(opts.cursor_id());
    let display = wm.display.clone();
    TrackedMotion::new(display)
        .rate(MOUSE_MOVE_RESIZE_RATE)
        .activation_threshold(MOUSE_MOVE_ACTIVATION_THRESHHOLD, &mut || {
            preview.borrow_mut().show();
        })
        .on_motion(&mut |motion, last_x, last_y| {
            let mut preview = preview.borrow_mut();
            let mx = (motion.root_x() - last_x) * width_mul;
            let my = (motion.root_y() - last_y) * height_mul;
            dx += mx;
            dy += my;
            preview.resize_by(mx, my);
            if let Some(h) = normal_hints.as_ref() {
                // If resizing freely prefer the direction the mouse has moved more in
                let keep_height = opts.lock_width || (!opts.lock_height && dx > dy);
                preview.apply_normal_hints(h, keep_height);
            }
            preview.update();
        })
        .on_finish(&mut |reason| {
            if matches!(reason, FinishReason::Finish(_, _)) {
                preview.borrow_mut().finish(client);
            } else {
                preview.borrow_mut().cancel();
            }
        })
        .cancel_on_escape()
        .run(cursor);
}
