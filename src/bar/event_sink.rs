use super::Bar;
use crate::{
    event::{self, Signal},
    update_thread::UpdateThread,
    window_manager::WindowKind,
};
use parking_lot::Mutex;
use std::sync::Arc;
use xcb::Event;

pub struct EventSink {
    bar: Arc<Mutex<Bar>>,
    update_thread: Option<UpdateThread>,
}

impl EventSink {
    pub fn new(bar: Bar, update_interval: u64) -> Self {
        let bar = Arc::new(Mutex::new(bar));
        let bar2 = bar.clone();
        let update_thread = if update_interval == 0 {
            None
        } else {
            Some(UpdateThread::new(update_interval, move || {
                bar2.lock().draw();
            }))
        };
        Self { bar, update_thread }
    }
}

impl event::EventSink for EventSink {
    fn accept(&mut self, event: &xcb::Event) -> bool {
        use xcb::x::Event::*;
        let mut bar = self.bar.lock();
        macro_rules! require_kind {
            ($window:expr, $kind:ident) => {
                if !matches!(bar.wm.get_window_kind(&$window), WindowKind::$kind) {
                    return false;
                }
            };
        }
        match event {
            //**** Normal widgets ****//
            Event::X(EnterNotify(event)) => {
                require_kind!(event.event(), StatusBar);
                bar.enter(event.event());
            }
            Event::X(LeaveNotify(event)) => {
                require_kind!(event.event(), StatusBar);
                bar.leave();
            }
            Event::X(ButtonPress(event)) => {
                require_kind!(event.event(), StatusBar);
                bar.click(event);
            }
            //**** Tray ****//
            Event::X(ClientMessage(event)) => {
                if event.window() != bar.tray_window() {
                    return false;
                }
                if let Some(tray) = bar.tray() {
                    tray.client_message(event);
                }
            }
            Event::X(PropertyNotify(event)) => {
                require_kind!(event.window(), TrayClient);
                if let Some(tray) = bar.tray() {
                    tray.property_notify(event);
                }
            }
            Event::X(MapNotify(event)) => {
                require_kind!(event.window(), TrayClient);
                if let Some(tray) = bar.tray() {
                    tray.map_notify(event);
                }
            }
            Event::X(UnmapNotify(event)) => {
                require_kind!(event.window(), TrayClient);
                if let Some(tray) = bar.tray() {
                    tray.unmap_notify(event);
                }
            }
            Event::X(DestroyNotify(event)) => {
                require_kind!(event.window(), TrayClient);
                if let Some(tray) = bar.tray() {
                    tray.maybe_remove_client(event.window());
                }
            }
            _ => return false,
        }
        bar.draw();
        true
    }

    fn signal(&mut self, signal: &Signal) {
        let mut bar = self.bar.lock();
        match signal {
            Signal::UpdateBar(invalidate) => {
                if *invalidate {
                    bar.invalidate_widgets()
                }
            }
            Signal::Resize => bar.resize(),
            Signal::Quit => {
                if let Some(update_thread) = self.update_thread.take() {
                    log::trace!("bar: stopping update thread");
                    update_thread.stop();
                }
                bar.destroy();
                return;
            }
            Signal::ClientMinimized(handle, is_minimized) => {
                if *is_minimized && bar.wm.win2client(handle).unwrap().is_fullscreen() {
                    bar.window.raise();
                }
            }
            _ => {
                for widget in &bar.left_widgets {
                    widget.borrow_mut().signal(signal);
                }
                for widget in &bar.right_widgets {
                    widget.borrow_mut().signal(signal);
                }
            }
        }
        bar.draw();
    }

    fn filter(&self) -> &'static [u32] {
        use xcb::{x::*, BaseEvent};
        &[
            EnterNotifyEvent::NUMBER,
            LeaveNotifyEvent::NUMBER,
            ButtonPressEvent::NUMBER,
            ClientMessageEvent::NUMBER,
            PropertyNotifyEvent::NUMBER,
            MapNotifyEvent::NUMBER,
            UnmapNotifyEvent::NUMBER,
            DestroyNotifyEvent::NUMBER,
        ]
    }
}
