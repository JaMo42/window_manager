use std::{error::Error, sync::Arc};

type AnyResult<T> = Result<T, Box<dyn Error>>;

mod action;
mod appinfo;
mod bar;
mod button;
mod cfg;
mod class_hint;
mod client;
mod color;
mod color_scheme;
mod config;
mod config_types;
mod context;
mod context_menu;
mod cursor;
mod dbus;
mod desktop_entry;
mod dialog;
mod dock;
mod draw;
mod error;
mod event;
mod event_router;
mod ewmh;
mod extended_frame;
mod geometry_preview;
mod layout;
mod main_event_sink;
mod markup;
mod monitors;
mod motif_hints;
mod mouse;
mod mouse_block;
mod normal_hints;
mod notifications;
mod paths;
mod platform;
mod process;
mod rectangle;
mod session_manager;
mod snap;
mod spawn_pos;
mod split_handles;
mod split_manager;
mod timeout_thread;
mod tooltip;
mod update_thread;
mod window_manager;
mod wm_hints;
mod workspace;
mod x;

use error::display_fatal_error;
use window_manager::WindowManager;
use x::{Display, Window};

#[macro_export]
macro_rules! macro_arg_count {
    (@arg_count) => (0usize);
    (@arg_count $x:tt $($xs:tt)*) => (1usize + m_arg_count!(@arg_count $($xs)*));
}

#[macro_export]
macro_rules! log_error {
    ($result:expr) => {
        if let Err(error) = $result {
            log::error!("{}", error);
        }
    };

    ($result:expr, $what:expr) => {
        if let Err(error) = $result {
            log::error!("{}: {}", $what, error);
        }
    };
}

/// Set the `_NET_WM_WINDOW_OPACITY` property on the given window.
/// This has no effect on our rendering but a compositor may use it to make
/// the entire window transparent.
pub fn set_compositor_opacity(window: &Window, opacity: f64) {
    if opacity < 0.99 {
        let display = window.display();
        let value = (u32::MAX as f64 * opacity).round() as u32;
        // `window.set_property()` would crash if it fails but we don't really
        // care if this fails.
        display.connection().send_request(&xcb::x::ChangeProperty {
            mode: xcb::x::PropMode::Replace,
            window: window.handle(),
            property: display.atoms.net_wm_window_opacity,
            r#type: xcb::x::ATOM_CARDINAL,
            data: &[value],
        });
    } else {
        log::trace!("Ignoring set_compositor_opacity because opacity is too high: {opacity}");
    }
}

fn configure_logging() {
    use log::LevelFilter;
    use log4rs::{
        append::file::FileAppender,
        config::{Appender, Config, Logger, Root},
        encode::pattern::PatternEncoder,
    };
    let log_file = FileAppender::builder()
        .append(false)
        .encoder(Box::new(PatternEncoder::new("{l:<5}| {m}\n")))
        .build(paths::log_path())
        .unwrap();
    let log_config = Config::builder()
        .appender(Appender::builder().build("log_file", Box::new(log_file)))
        // Enable logging for this crate
        .logger(Logger::builder().appender("log_file").build(
            "window_manager",
            if cfg!(debug_assertions) {
                LevelFilter::Trace
            } else {
                LevelFilter::Info
            },
        ))
        // librsvg and zbus use the root logger so turn that off
        .build(Root::builder().build(LevelFilter::Off))
        .unwrap();
    log4rs::init_config(log_config).unwrap();
}

fn main() -> AnyResult<()> {
    configure_logging();
    let display = Arc::new(Display::connect()?);
    log::info!("Display: {}", display.get_name());
    monitors::monitors_mut().query(&display).log();
    if let Err(error) = WindowManager::main(display.clone()) {
        display_fatal_error(&display, format!("{}", error));
        Err(error)?;
    }
    log::info!("Finished without crashing :^)");
    Ok(())
}
