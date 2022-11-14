use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Represents a thread that periodically calls a function and can be stopped
/// or asked to call the function right now at any time.
pub struct Update_Thread {
  handle: JoinHandle<()>,
  sender: Sender<u8>,
}

impl Update_Thread {
  const STOP_SIGNAL: u8 = 0;
  const UPDATE_SIGNAL: u8 = 1;

  pub fn new (interval: u64, update_fn: fn ()) -> Self {
    let (tx, rx) = mpsc::channel ();
    let duration = Duration::from_millis (interval);
    let handle = thread::spawn (move || loop {
      if let Ok (signal) = rx.recv_timeout (duration) {
        match signal {
          Self::STOP_SIGNAL => {
            return;
          }
          Self::UPDATE_SIGNAL => {}
          _ => unreachable! (),
        }
      }
      update_fn ();
    });
    Self { handle, sender: tx }
  }

  #[allow(dead_code)]
  /// Calls the update function immediately. The interval is reset after this.
  pub fn update (&mut self) {
    log_error! (self.sender.send (Self::UPDATE_SIGNAL));
  }

  /// Stops and joins the thread, consuming the object.
  pub fn stop (self) {
    log_error! (self.sender.send (Self::STOP_SIGNAL));
    if let Err (e) = self.handle.join () {
      std::panic::resume_unwind (e);
    }
  }
}
