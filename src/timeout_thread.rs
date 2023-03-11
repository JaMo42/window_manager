use crate::log_error;
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub struct TimeoutThread {
    handle: JoinHandle<()>,
    sender: Sender<()>,
}

impl TimeoutThread {
    pub fn new(delay: u64, function: Arc<dyn Fn() + Send + Sync>) -> Self {
        let (tx, rx) = mpsc::channel();
        let duration = Duration::from_millis(delay);
        let handle = thread::spawn(move || {
            if rx.recv_timeout(duration).is_ok() {
                return;
            }
            function();
        });
        Self { handle, sender: tx }
    }

    pub fn cancel(&mut self) {
        if !self.handle.is_finished() {
            log_error!(self.sender.send(()));
        }
    }

    pub fn join(self) {
        if let Err(e) = self.handle.join() {
            std::panic::resume_unwind(e);
        }
    }

    pub fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }
}

pub struct RepeatableTimeoutThread {
    function: Arc<dyn Fn() + Send + Sync>,
    thread: Option<TimeoutThread>,
}

impl RepeatableTimeoutThread {
    pub fn new(function: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self {
            function,
            thread: None,
        }
    }

    pub fn start(&mut self, delay: u64) {
        if let Some(mut old) = self.thread.take() {
            old.cancel();
            old.join();
        }
        self.thread = Some(TimeoutThread::new(delay, self.function.clone()))
    }

    pub fn cancel(&mut self) {
        if let Some(thread) = &mut self.thread {
            thread.cancel();
        }
        self.thread = None;
    }

    pub fn destroy(&mut self) {
        if let Some(mut thread) = self.thread.take() {
            thread.cancel();
            thread.join();
        }
    }
}
