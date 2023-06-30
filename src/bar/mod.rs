use crate::window_manager::WindowManager;
use std::sync::Arc;

#[allow(clippy::module_inception)]
mod bar;
mod event_sink;
mod tray_client;
mod tray_manager;
mod volume_mixer;
mod widget;
mod xembed;

pub use bar::Bar;
pub use event_sink::EventSink;

use self::tray_manager::TrayManager;

pub const COLOR_KIND: u8 = 4;

pub fn create(wm: Arc<WindowManager>) -> EventSink {
    let update_interval = wm.config.bar.update_interval;
    let mut bar = Bar::create(wm.clone());
    bar.add_widgets();
    if let Ok(tray) = TrayManager::create(&wm, bar.height(), bar.width()) {
        bar.set_tray(tray);
    }
    bar.draw();
    EventSink::new(bar, update_interval)
}
