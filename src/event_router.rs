use crate::event::{x_event_number, DisplayEventName, DisplaySignalName, Signal, SinkStorage};
use xcb::Event;

const MAX_EVENT_NUMBER: usize = 34;
const MASK_COUNT: usize = MAX_EVENT_NUMBER + 1;

/// Handles filtered event dispatching
pub struct EventRouter {
    sinks: Vec<SinkStorage>,
    /// Indices into `sinks` for each X event number
    masks: [Vec<usize>; MASK_COUNT],
    dirty: bool,
}

impl EventRouter {
    pub fn new() -> Self {
        let as_vec = vec![Vec::new(); MASK_COUNT];
        let as_boxed: Box<[Vec<usize>; MASK_COUNT]> = match as_vec.into_boxed_slice().try_into() {
            Ok(x) => x,
            _ => unreachable!(),
        };
        Self {
            sinks: Vec::new(),
            masks: *as_boxed,
            dirty: false,
        }
    }

    /// Adds a sink, this does not update the masks.
    pub fn add(&mut self, sink: SinkStorage) {
        // MainEventSink should always be the last one.
        self.sinks.insert(0, sink);
        self.dirty = true;
    }

    /// Removes a sink and updates mask
    pub fn remove(&mut self, id: usize) {
        for (i, sink) in self.sinks.iter().enumerate() {
            if sink.id() == id {
                self.sinks.remove(i);
                self.dirty = true;
                return;
            }
        }
    }

    /// Updates masks
    fn set_masks(&mut self) {
        for sink in self.sinks.iter() {
            for mask in sink.filter() {
                self.masks[*mask as usize].clear();
            }
        }
        for (i, sink) in self.sinks.iter().enumerate() {
            for mask in sink.filter() {
                self.masks[*mask as usize].push(i);
            }
        }
    }

    /// Updates masks if `add` or `remove` was called since the last call to update.
    pub fn update(&mut self) {
        // This needs to be a separate function since we need to lock mutex
        // sinks to get their filter but if a sink adds another sink this could
        // cause a deadlock.
        if self.dirty {
            self.set_masks();
            self.dirty = false;
        }
    }

    /// Dispatches an event.  If the event is an X event it is only dispatched
    /// to sinks that specify that event number in their filter.
    pub fn dispatch_event(&mut self, event: &Event) {
        let do_log =
            std::option_env!("WM_LOG_ALL_EVENTS").is_some() && !matches!(event, Event::Unknown(_));
        if do_log {
            log::trace!("\x1b[2m Event: \x1b[92m{}\x1b[0m", DisplayEventName(event));
        }
        if let Event::X(x_event) = event {
            let id = x_event_number(x_event);
            for &i in self.masks[id as usize].iter() {
                if self.sinks[i].accept(event) {
                    return;
                }
            }
        } else {
            for sink in self.sinks.iter_mut() {
                if sink.accept(event) {
                    return;
                }
            }
        }
        if do_log {
            log::trace!("\x1b[2m      : Unhandeled\x1b[0m");
        }
    }

    /// Dispatches a signal to all event sinks
    pub fn dispatch_signal(&mut self, signal: Signal) {
        if std::option_env!("WM_LOG_ALL_EVENTS").is_some() {
            log::trace!(
                "\x1b[2mSignal: \x1b[96m{}\x1b[0m",
                DisplaySignalName(&signal)
            );
        }
        for sink in self.sinks.iter_mut() {
            sink.signal(&signal);
        }
    }

    /// Clears the sink list.  This is for deinitialization with the ref counted
    /// sinks and does not affect the masks.
    pub fn clear(&mut self) {
        self.sinks.clear();
    }
}
