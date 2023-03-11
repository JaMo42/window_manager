use crate::mouse::{BUTTON_2, BUTTON_3};
use crate::x::XcbWindow;
use crate::{mouse::BUTTON_1, snap::SnapState};
use libc::c_void;
use parking_lot::Mutex;
use std::{cell::RefCell, rc::Rc, sync::Arc};
use xcb::Event;

/// Things modules need to react to that aren't covered by events.
#[derive(Copy, Clone, Debug)]
pub enum Signal {
    NewClient(XcbWindow),
    ClientRemoved(XcbWindow),
    FocusClient(XcbWindow),
    UrgencyChanged(XcbWindow),
    // `(client, from, to)`
    ClientWorkspaceChanged(XcbWindow, usize, usize),
    // `(client, from, to)`
    ClientMonitorChanged(XcbWindow, isize, isize),
    /// `(client, from, to)`
    SnapStateChanged(XcbWindow, SnapState, SnapState),
    /// `(from, to)`
    WorkspaceChanged(usize, usize),
    ActiveWorkspaceEmpty(bool),
    /// Monitors changed
    Resize,
    /// The contained bool specifies whether all widgets should be invalidated
    /// before drawing.
    UpdateBar(bool),
    /// The window manager is quitting.
    Quit,
}

// TODO: filters for event sinks so we don't need to dispatch every event to
//       every sink.

/// Wraps different ways of storing an event sink.
pub enum SinkStorage {
    Unique(Box<dyn EventSink>),
    Shared(Rc<RefCell<dyn EventSink>>),
    Mutex(Arc<Mutex<dyn EventSink>>),
}

impl SinkStorage {
    pub fn accept(&mut self, event: &Event) -> bool {
        match self {
            Self::Unique(ref mut boxed) => boxed.accept(event),
            Self::Shared(ref rc) => rc.borrow_mut().accept(event),
            Self::Mutex(ref arc) => arc.lock().accept(event),
        }
    }

    pub fn signal(&mut self, signal: &Signal) {
        match self {
            Self::Unique(ref mut boxed) => boxed.signal(signal),
            Self::Shared(ref rc) => rc.borrow_mut().signal(signal),
            Self::Mutex(ref arc) => {
                arc.lock().signal(signal);
            }
        }
    }

    pub fn id(&self) -> SinkId {
        let ptr: *const dyn EventSink = match *self {
            Self::Unique(ref boxed) => boxed.as_ref(),
            Self::Shared(ref rc) => rc.as_ptr(),
            Self::Mutex(ref arc) => &*arc.lock() as *const dyn EventSink,
        };
        ptr as *const c_void as SinkId
    }
}

pub type SinkId = usize;

pub trait EventSink {
    fn id(&self) -> SinkId
    where
        Self: Sized,
    {
        self as *const dyn EventSink as *const c_void as SinkId
    }

    fn accept(&mut self, event: &Event) -> bool;

    fn signal(&mut self, _signal: &Signal) {}
}

/// Prints only the variant name of the contained event.
pub struct DisplayEventName<'a>(pub &'a Event);

impl std::fmt::Display for DisplayEventName<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let dbg = format!("{:?}", self.0);
        let mut split = if let Some(first) = dbg.split('{').next() {
            first.split('(').peekable()
        } else {
            dbg.split('(').peekable()
        };
        write!(f, "{}", split.next().unwrap())?;
        while let Some(part) = split.next() {
            if split.peek().is_some() {
                write!(f, ".{}", part)?;
            } else {
                break;
            }
        }
        Ok(())
    }
}

/// Prints only the variant name of the contained signal.
pub struct DisplaySignalName<'a>(pub &'a Signal);

impl std::fmt::Display for DisplaySignalName<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let dbg = format!("{:?}", self.0);
        let mut split = dbg.split('(').peekable();
        write!(f, "{}", split.next().unwrap())?;
        while let Some(part) = split.next() {
            if split.peek().is_some() {
                write!(f, ".{}", part.trim())?;
            } else {
                break;
            }
        }
        Ok(())
    }
}

/// Returns the source window of an X event. Not all event types are implemented
/// so an optional value is returned.
pub fn x_event_source(ev: &Event) -> Option<XcbWindow> {
    use xcb::x::Event::*;
    Some(match ev {
        Event::X(ButtonPress(e)) => e.event(),
        Event::X(ButtonRelease(e)) => e.event(),
        Event::X(KeyPress(e)) => e.event(),
        Event::X(KeyRelease(e)) => e.event(),
        Event::X(EnterNotify(e)) => e.event(),
        Event::X(LeaveNotify(e)) => e.event(),
        Event::X(MotionNotify(e)) => e.event(),
        _ => None?,
    })
}

/// Checks if the given event is a button press event. The mouse wheel is not
/// considered a button press despite being the same event type.
pub fn is_button_press(ev: &Event) -> bool {
    use xcb::x::Event::*;
    if let Event::X(ButtonPress(e)) = ev {
        let button = e.detail();
        button == BUTTON_1 || button == BUTTON_2 || button == BUTTON_3
    } else {
        false
    }
}
