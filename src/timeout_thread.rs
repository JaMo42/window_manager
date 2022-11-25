use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub struct Timeout_Thread {
  handle: JoinHandle<()>,
  sender: Sender<()>,
}

impl Timeout_Thread {
  pub fn new (delay: u64, function: fn ()) -> Self {
    let (tx, rx) = mpsc::channel ();
    let duration = Duration::from_millis (delay);
    let handle = thread::spawn (move || {
      if let Ok (_) = rx.recv_timeout (duration) {
        return;
      }
      function ();
    });
    Self { handle, sender: tx }
  }

  pub fn cancel (&mut self) {
    log_error! (self.sender.send (()));
  }

  pub fn join (self) {
    if let Err (e) = self.handle.join () {
      std::panic::resume_unwind (e);
    }
  }
}

pub struct Repeatable_Timeout_Thread {
  function: fn (),
  thread: Option<Timeout_Thread>,
}

impl Repeatable_Timeout_Thread {
  pub fn new (function: fn ()) -> Self {
    Self {
      function,
      thread: None,
    }
  }

  pub fn start (&mut self, delay: u64) {
    if let Some (old) = self.thread.take () {
      old.join ();
    }
    self.thread = Some (Timeout_Thread::new (delay, self.function))
  }

  pub fn cancel (&mut self) {
    if let Some (thread) = &mut self.thread {
      thread.cancel ();
    }
  }

  pub fn destroy (&mut self) {
    self.cancel ();
    if let Some (handle) = self.thread.take () {
      handle.join ();
    }
  }
}
