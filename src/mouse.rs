use crate::core::*;
use crate::event::{configure_request, map_request};
use crate::x;
use x11::keysym::XK_Escape;
use x11::xlib::*;

const MASK: i64 = ButtonPressMask | ButtonReleaseMask | PointerMotionMask;

type MotionCallback<'a> = &'a mut dyn FnMut(&XMotionEvent, i32, i32);
type ButtonCallback<'a> = &'a mut dyn FnMut(&XButtonEvent) -> bool;
type KeyCallback<'a> = &'a mut dyn FnMut(&XKeyEvent) -> bool;
type FinishCallback<'a> = &'a mut dyn FnMut(FinishReason);
type ActicationCallback<'a> = &'a mut dyn FnMut();

#[derive(Copy, Clone)]
pub enum FinishReason {
  Finish(i32, i32),
  Cancel,
  Failure,
}

pub struct TrackedMotion<'a> {
  on_motion: Option<MotionCallback<'a>>,
  on_button_press: Option<ButtonCallback<'a>>,
  on_key_press: Option<KeyCallback<'a>>,
  on_finish: Option<FinishCallback<'a>>,
  on_activation: Option<ActicationCallback<'a>>,
  activation_threshold: i32,
  rate: u64,
}

impl<'a> TrackedMotion<'a> {
  pub fn new() -> Self {
    Self {
      on_motion: None,
      on_button_press: None,
      on_key_press: None,
      on_finish: None,
      on_activation: None,
      activation_threshold: 0,
      rate: 30,
    }
  }

  pub fn on_motion(&mut self, callback: &'a mut dyn FnMut(&XMotionEvent, i32, i32)) -> &mut Self {
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
    threshold: i32,
    callback: ActicationCallback<'a>,
  ) -> &mut Self {
    self.on_activation = Some(callback);
    self.activation_threshold = threshold;
    self
  }

  pub fn rate(&mut self, rate: u64) -> &mut Self {
    self.rate = rate;
    self
  }

  // Installs a `on_key_press` handler that cancels the operation when the
  // escape key is pressed.
  pub fn cancel_on_escape(&mut self) -> &mut Self {
    static mut callback: fn(&XKeyEvent) -> bool =
      |event| x::lookup_keysym(event) as u32 == XK_Escape;
    self.on_key_press(unsafe { &mut callback })
  }

  unsafe fn run_impl(&mut self, cursor: Cursor) -> Option<()> {
    let _pointer_grab = display.scoped_pointer_grab(MASK, cursor);
    let (start_x, start_y) = display.query_pointer_position()?;
    let mut event: XEvent = zeroed!();
    let mut last_time: Time = 0;
    let mut mouse_x = start_x;
    let mut mouse_y = start_y;
    let mut active = self.activation_threshold == 0;
    let finish_reason;
    if self.on_key_press.is_some() {
      display.grab_keyboard(root);
    }
    let event_mask =
      MASK | SubstructureRedirectMask | (KeyPressMask * self.on_key_press.is_some() as i64);
    loop {
      display.mask_event(event_mask, &mut event);
      match event.type_ {
        ConfigureRequest => configure_request(&event.configure_request),
        MapRequest => map_request(&event.map_request),
        MotionNotify => {
          let motion = event.motion;
          if (motion.time - last_time) < self.rate {
            continue;
          }
          last_time = motion.time;
          if !active {
            if (start_x - motion.x).abs() > self.activation_threshold
              || (start_y - motion.y).abs() > self.activation_threshold
            {
              active = true;
              (self.on_activation.take().unwrap())();
            } else {
              continue;
            }
          }
          (self.on_motion.as_mut().unwrap_unchecked())(&motion, mouse_x, mouse_y);
          mouse_x = motion.x;
          mouse_y = motion.y;
        }
        ButtonPress => {
          if let Some(on_button_press) = &mut self.on_button_press {
            if on_button_press(&event.button) {
              finish_reason = FinishReason::Cancel;
              break;
            }
          }
        }
        KeyPress => {
          if let Some(on_key_press) = &mut self.on_key_press {
            if on_key_press(&event.key) {
              finish_reason = FinishReason::Cancel;
              break;
            }
          }
        }
        ButtonRelease => {
          finish_reason = FinishReason::Finish(event.button.x, event.button.y);
          break;
        }
        _ => {}
      }
    }
    if self.on_key_press.is_some() {
      display.ungrab_keyboard();
    }
    if let Some(on_finish) = self.on_finish.take() {
      on_finish(finish_reason);
    }
    Some(())
  }

  pub fn run(&mut self, cursor: Cursor) {
    // The on_motion callback is required but uses the same optional api as the
    // other callbacks for aesthetics.
    assert!(self.on_motion.is_some());
    unsafe {
      if self.run_impl(cursor).is_none() {
        // Bailed out early, still need to call on_finish.
        if let Some(on_finish) = self.on_finish.take() {
          on_finish(FinishReason::Failure);
        }
      }
    }
  }
}
